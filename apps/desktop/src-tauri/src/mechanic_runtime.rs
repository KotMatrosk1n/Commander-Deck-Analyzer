//! Typed procedures for the printed mechanics used by the strict runtime.
//!
//! Card names are identity context for Oracle self references only. Mechanic
//! selection is based on the requested occurrence, layout, type line, printed
//! keyword evidence, and a completely compiled Oracle clause.

use std::collections::BTreeSet;
use std::fmt;

use crate::bounded_oracle_runtime::{
    ActivationRestriction, AlternativeCastPermission, AlternativeCost, Amount, BoundedOracleClause,
    CardType, CastCopyEffect, ClauseAddress, Color, Comparison, CompileError, Condition, Cost,
    CountExpression, Duration, Effect, Keyword, ManaCost, ObjectFilter, ObjectRef,
    OracleClauseInput, PlayerRef, PowerToughnessChange, PowerToughnessOperation, ReminderSemantics,
    RepeatSchedule, Restriction, SearchDestination, SearchLibrary, SearchOrdinal,
    SpecialActionTiming, Step, Target, Timing, TokenCreation, TokenDefinition, TokenSpecification,
    Trigger, TurnPlayer, WardCost, Zone, ZoneMove, compile_bounded_oracle_clause,
};

pub const MECHANIC_RUNTIME_VERSION: &str = "mechanic-runtime-0.6";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrintedMechanic {
    AbilityWord,
    Cycling,
    Typecycling,
    Enchant,
    Food,
    Prowess,
    Channel,
    Treasure,
    Scry,
    Landfall,
    Double,
    Paradigm,
    Transform,
    Surveil,
    Crew,
    Ward,
    SplitSecond,
    Evoke,
    Manifest,
    Partner,
    Ferocious,
    Dash,
    Gift,
    Mobilize,
}

impl PrintedMechanic {
    pub fn printed_label(self) -> &'static str {
        match self {
            Self::AbilityWord => "Ability word",
            Self::Cycling => "Cycling",
            Self::Typecycling => "Typecycling",
            Self::Enchant => "Enchant",
            Self::Food => "Food",
            Self::Prowess => "Prowess",
            Self::Channel => "Channel",
            Self::Treasure => "Treasure",
            Self::Scry => "Scry",
            Self::Landfall => "Landfall",
            Self::Double => "Double",
            Self::Paradigm => "Paradigm",
            Self::Transform => "Transform",
            Self::Surveil => "Surveil",
            Self::Crew => "Crew",
            Self::Ward => "Ward",
            Self::SplitSecond => "Split second",
            Self::Evoke => "Evoke",
            Self::Manifest => "Manifest",
            Self::Partner => "Partner",
            Self::Ferocious => "Ferocious",
            Self::Dash => "Dash",
            Self::Gift => "Gift",
            Self::Mobilize => "Mobilize",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MechanicClauseInput<'a> {
    pub face_index: u16,
    pub clause_index: u16,
    pub source_name: &'a str,
    pub source_type_line: &'a str,
    pub oracle_clause: &'a str,
}

impl MechanicClauseInput<'_> {
    pub fn address(self) -> ClauseAddress {
        ClauseAddress {
            face_index: self.face_index,
            clause_index: self.clause_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MechanicOccurrenceInput<'a> {
    pub mechanic: PrintedMechanic,
    pub marker_label: Option<&'a str>,
    pub layout: &'a str,
    pub printed_keywords: &'a [String],
    pub primary: MechanicClauseInput<'a>,
    pub companion: Option<MechanicClauseInput<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkerDisposition {
    Executable,
    StructurallyNonoperative {
        owned_executable_clause: ClauseAddress,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanicProgram {
    runtime_version: &'static str,
    mechanic: PrintedMechanic,
    primary_address: ClauseAddress,
    marker_disposition: MarkerDisposition,
    executable_clauses: Vec<BoundedOracleClause>,
    procedure: MechanicProcedure,
}

impl MechanicProgram {
    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn mechanic(&self) -> PrintedMechanic {
        self.mechanic
    }

    pub fn primary_address(&self) -> ClauseAddress {
        self.primary_address
    }

    pub fn marker_disposition(&self) -> &MarkerDisposition {
        &self.marker_disposition
    }

    pub fn executable_clauses(&self) -> &[BoundedOracleClause] {
        &self.executable_clauses
    }

    pub fn procedure(&self) -> &MechanicProcedure {
        &self.procedure
    }

    pub fn has_exact_contract(&self) -> bool {
        compiled_program_has_exact_contract(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MechanicProcedure {
    AbilityWord(AbilityWordProcedure),
    Cycling(CyclingProcedure),
    Typecycling(TypecyclingProcedure),
    Enchant(EnchantProcedure),
    Food(FoodProcedure),
    Prowess(ProwessProcedure),
    Channel(ChannelProcedure),
    Treasure(TreasureProcedure),
    Scry(ScryProcedure),
    Landfall(AbilityWordProcedure),
    Double(DoubleProcedure),
    Paradigm(Box<ParadigmProcedure>),
    Transform(TransformProcedure),
    Surveil(SurveilProcedure),
    Crew(CrewProcedure),
    Ward(WardProcedure),
    SplitSecond(SplitSecondProcedure),
    Evoke(Box<EvokeProcedure>),
    Manifest(ManifestProcedure),
    Partner(PartnerProcedure),
    Ferocious(AbilityWordProcedure),
    Dash(DashProcedure),
    Gift(GiftProcedure),
    Mobilize(MobilizeProcedure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantProcedure {
    pub aura: ObjectRef,
    pub legal_attachment: ObjectFilter,
    pub attachment_zone: Zone,
    pub targets_while_cast: bool,
    pub attach_on_resolution: bool,
    pub move_to_graveyard_if_unattached_or_illegal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelProcedure {
    pub activation_zone: Zone,
    pub costs: Vec<Cost>,
    pub targets: Vec<Target>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreasureProcedure {
    pub token_definition: TokenDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScryProcedure {
    pub player: PlayerRef,
    pub amount: Amount,
    pub may_put_on_library_bottom: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurveilProcedure {
    pub player: PlayerRef,
    pub amount: Amount,
    pub may_put_in_graveyard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityWordProcedure {
    pub marker: PrintedMechanic,
    pub printed_label: String,
    pub owned_executable_clause: ClauseAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclingProcedure {
    pub activation_zone: Zone,
    pub costs: Vec<Cost>,
    pub player: PlayerRef,
    pub cards_drawn: Amount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecyclingProcedure {
    pub activation_zone: Zone,
    pub costs: Vec<Cost>,
    pub type_name: String,
    pub search: SearchLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoodProcedure {
    pub token_definition: TokenDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProwessProcedure {
    pub trigger: Trigger,
    pub change: PowerToughnessChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleProcedure {
    pub change: PowerToughnessChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParadigmProcedure {
    pub first_resolution_gate: Condition,
    pub exile_after_resolution: Effect,
    pub cast_copy: CastCopyEffect,
    pub cast_is_optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformProcedure {
    pub front_clause: ClauseAddress,
    pub back_annotation_clause: ClauseAddress,
    pub front_face_index: u16,
    pub back_face_index: u16,
    pub preserve_physical_object_identity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrewProcedure {
    pub controller: PlayerRef,
    pub eligible_creatures: ObjectFilter,
    pub tap_any_number: bool,
    pub minimum_total_power: Amount,
    pub becomes_artifact_creature: bool,
    pub keeps_printed_power_and_toughness: bool,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardProcedure {
    pub protected_object: ObjectRef,
    pub triggering_source_controller: PlayerRef,
    pub payer: PlayerRef,
    pub triggering_spell_or_ability: ObjectRef,
    pub applies_to_spells_and_abilities: bool,
    pub payment: Cost,
    pub counter_triggering_spell_or_ability_unless_paid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitSecondProcedure {
    pub source_zone: Zone,
    pub affected_players: PlayerRef,
    pub blocks_spell_casting_except_mana_actions: bool,
    pub blocks_nonmana_ability_activation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvokeProcedure {
    pub alternative_cast: AlternativeCastPermission,
    pub sacrifice_condition: Condition,
    pub sacrifice_trigger: Trigger,
    pub sacrifice_move: ZoneMove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestFaceUpCost {
    ManifestedCardManaCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestFaceUpTiming {
    SpecialActionAnyTimeControllerHasPriority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestProcedure {
    pub player: PlayerRef,
    pub card: ObjectRef,
    pub enters_zone: Zone,
    pub face_down: bool,
    pub base_power: Amount,
    pub base_toughness: Amount,
    pub card_type: CardType,
    pub face_up_condition: Condition,
    pub face_up_cost: ManifestFaceUpCost,
    pub face_up_timing: ManifestFaceUpTiming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerProcedure {
    pub permits_two_commanders: bool,
    pub both_commanders_must_have_partner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashProcedure {
    pub alternative_cast: AlternativeCastPermission,
    pub alternative_cost_condition: Condition,
    pub haste: Effect,
    pub return_to_hand: ZoneMove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftProcedure {
    pub promise_condition: Condition,
    pub recipient: PlayerRef,
    pub benefit: GiftBenefit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiftBenefit {
    Token(TokenCreation),
    DrawCard { player: PlayerRef, amount: Amount },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobilizeProcedure {
    pub attack_trigger: Trigger,
    pub token_creation: TokenCreation,
    pub delayed_destination: Zone,
    pub delayed_trigger: Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MechanicCompileError {
    MissingPrintedKeyword {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
    UnsupportedLayout {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
        layout: String,
    },
    UnsupportedTypeLine {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
        type_line: String,
    },
    MissingCompanion {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
    UnexpectedCompanion {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
    CompanionAddressCollision {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
    TransformOriginMismatch {
        address: ClauseAddress,
    },
    InvalidProcedureShape {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
        detail: &'static str,
    },
    BoundedClause {
        mechanic: PrintedMechanic,
        error: CompileError,
    },
    DuplicateOccurrence {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
}

impl fmt::Display for MechanicCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrintedKeyword { mechanic, address } => write!(
                formatter,
                "{} is missing from printed keyword evidence at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::UnsupportedLayout {
                mechanic,
                address,
                layout,
            } => write!(
                formatter,
                "{} has unsupported layout `{layout}` at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::UnsupportedTypeLine {
                mechanic,
                address,
                type_line,
            } => write!(
                formatter,
                "{} has unsupported type line `{type_line}` at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::MissingCompanion { mechanic, address } => write!(
                formatter,
                "{} needs a companion clause at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::UnexpectedCompanion { mechanic, address } => write!(
                formatter,
                "{} received an unused companion at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::CompanionAddressCollision { mechanic, address } => write!(
                formatter,
                "{} uses the same address for its primary and companion at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::TransformOriginMismatch { address } => write!(
                formatter,
                "transform origin does not match its front face at face {} clause {}",
                address.face_index, address.clause_index
            ),
            Self::InvalidProcedureShape {
                mechanic,
                address,
                detail,
            } => write!(
                formatter,
                "{} has an incomplete procedure at face {} clause {}: {detail}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::BoundedClause { mechanic, error } => {
                write!(
                    formatter,
                    "{} clause failed: {error}",
                    mechanic.printed_label()
                )
            }
            Self::DuplicateOccurrence { mechanic, address } => write!(
                formatter,
                "{} is duplicated at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
        }
    }
}

impl std::error::Error for MechanicCompileError {}

pub fn compile_mechanic_program(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    compile_mechanic_occurrence(input)
}

pub fn mechanic_clause_has_exact_contract(input: MechanicOccurrenceInput<'_>) -> bool {
    compile_mechanic_program(input).is_ok()
}

pub fn compile_mechanic_occurrences(
    inputs: &[MechanicOccurrenceInput<'_>],
) -> Result<Vec<MechanicProgram>, MechanicCompileError> {
    let mut seen = BTreeSet::new();
    let mut programs = Vec::with_capacity(inputs.len());
    for input in inputs {
        let key = (input.primary.address(), input.mechanic);
        if !seen.insert(key) {
            return Err(MechanicCompileError::DuplicateOccurrence {
                mechanic: input.mechanic,
                address: input.primary.address(),
            });
        }
        programs.push(compile_mechanic_occurrence(*input)?);
    }
    programs.sort_by_key(|program| (program.primary_address, program.mechanic));
    Ok(programs)
}

pub fn compile_mechanic_occurrence(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    match (input.mechanic, input.marker_label) {
        (PrintedMechanic::AbilityWord, Some(label)) if !label.trim().is_empty() => {}
        (PrintedMechanic::AbilityWord, _) | (_, Some(_)) => {
            return Err(shape_error(&input, "exact marker label"));
        }
        _ => {}
    }
    require_printed_keyword(&input)?;
    if input.mechanic != PrintedMechanic::Transform && input.companion.is_some() {
        return Err(MechanicCompileError::UnexpectedCompanion {
            mechanic: input.mechanic,
            address: input.primary.address(),
        });
    }

    match input.mechanic {
        PrintedMechanic::AbilityWord => compile_ability_word(input),
        PrintedMechanic::Cycling => compile_cycling(input),
        PrintedMechanic::Typecycling => compile_typecycling(input),
        PrintedMechanic::Enchant => compile_enchant(input),
        PrintedMechanic::Food => compile_food(input),
        PrintedMechanic::Prowess => compile_prowess(input),
        PrintedMechanic::Channel => compile_channel(input),
        PrintedMechanic::Treasure => compile_treasure(input),
        PrintedMechanic::Scry => compile_scry(input),
        PrintedMechanic::Landfall => compile_landfall(input),
        PrintedMechanic::Double => compile_double(input),
        PrintedMechanic::Paradigm => compile_paradigm(input),
        PrintedMechanic::Transform => compile_transform(input),
        PrintedMechanic::Surveil => compile_surveil(input),
        PrintedMechanic::Crew => compile_crew(input),
        PrintedMechanic::Ward => compile_ward(input),
        PrintedMechanic::SplitSecond => compile_split_second(input),
        PrintedMechanic::Evoke => compile_evoke(input),
        PrintedMechanic::Manifest => compile_manifest(input),
        PrintedMechanic::Partner => compile_partner(input),
        PrintedMechanic::Ferocious => compile_ferocious(input),
        PrintedMechanic::Dash => compile_dash(input),
        PrintedMechanic::Gift => compile_gift(input),
        PrintedMechanic::Mobilize => compile_mobilize(input),
    }
}

pub trait MechanicExecutionAdapter {
    type Error;

    fn begin_mechanic_procedure(
        &mut self,
        mechanic: PrintedMechanic,
        procedure: &MechanicProcedure,
    ) -> Result<(), Self::Error>;

    fn execute_bounded_clause(
        &mut self,
        mechanic: PrintedMechanic,
        clause: &BoundedOracleClause,
    ) -> Result<(), Self::Error>;

    fn finish_mechanic_procedure(
        &mut self,
        mechanic: PrintedMechanic,
        procedure: &MechanicProcedure,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanicExecutionReceipt {
    runtime_version: &'static str,
    mechanic: PrintedMechanic,
    primary_address: ClauseAddress,
    executed_clause_addresses: Vec<ClauseAddress>,
    procedure_applied: bool,
}

impl MechanicExecutionReceipt {
    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn mechanic(&self) -> PrintedMechanic {
        self.mechanic
    }

    pub fn primary_address(&self) -> ClauseAddress {
        self.primary_address
    }

    pub fn executed_clause_addresses(&self) -> &[ClauseAddress] {
        &self.executed_clause_addresses
    }

    pub fn procedure_applied(&self) -> bool {
        self.procedure_applied
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MechanicExecutionError<E> {
    InvalidProgram {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
    },
    BeginProcedure {
        mechanic: PrintedMechanic,
        error: E,
    },
    ExecuteClause {
        mechanic: PrintedMechanic,
        address: ClauseAddress,
        error: E,
    },
    FinishProcedure {
        mechanic: PrintedMechanic,
        error: E,
    },
}

impl<E: fmt::Display> fmt::Display for MechanicExecutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram { mechanic, address } => write!(
                formatter,
                "{} has an invalid compiled program at face {} clause {}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::BeginProcedure { mechanic, error } => write!(
                formatter,
                "{} procedure setup failed: {error}",
                mechanic.printed_label()
            ),
            Self::ExecuteClause {
                mechanic,
                address,
                error,
            } => write!(
                formatter,
                "{} execution failed at face {} clause {}: {error}",
                mechanic.printed_label(),
                address.face_index,
                address.clause_index
            ),
            Self::FinishProcedure { mechanic, error } => write!(
                formatter,
                "{} procedure completion failed: {error}",
                mechanic.printed_label()
            ),
        }
    }
}

impl<E> std::error::Error for MechanicExecutionError<E> where E: std::error::Error + 'static {}

pub fn execute_mechanic_program<A: MechanicExecutionAdapter>(
    adapter: &mut A,
    program: &MechanicProgram,
) -> Result<MechanicExecutionReceipt, MechanicExecutionError<A::Error>> {
    if !compiled_program_has_exact_contract(program) {
        return Err(MechanicExecutionError::InvalidProgram {
            mechanic: program.mechanic,
            address: program.primary_address,
        });
    }
    adapter
        .begin_mechanic_procedure(program.mechanic, &program.procedure)
        .map_err(|error| MechanicExecutionError::BeginProcedure {
            mechanic: program.mechanic,
            error,
        })?;
    let mut executed_clause_addresses = Vec::with_capacity(program.executable_clauses.len());
    for clause in &program.executable_clauses {
        adapter
            .execute_bounded_clause(program.mechanic, clause)
            .map_err(|error| MechanicExecutionError::ExecuteClause {
                mechanic: program.mechanic,
                address: clause.address(),
                error,
            })?;
        executed_clause_addresses.push(clause.address());
    }
    adapter
        .finish_mechanic_procedure(program.mechanic, &program.procedure)
        .map_err(|error| MechanicExecutionError::FinishProcedure {
            mechanic: program.mechanic,
            error,
        })?;
    Ok(MechanicExecutionReceipt {
        runtime_version: MECHANIC_RUNTIME_VERSION,
        mechanic: program.mechanic,
        primary_address: program.primary_address,
        executed_clause_addresses,
        procedure_applied: true,
    })
}

fn compiled_program_has_exact_contract(program: &MechanicProgram) -> bool {
    if program.runtime_version != MECHANIC_RUNTIME_VERSION
        || program.executable_clauses.is_empty()
        || program.executable_clauses[0].address() != program.primary_address
        || !procedure_matches_mechanic(program.mechanic, &program.procedure)
    {
        return false;
    }
    match &program.marker_disposition {
        MarkerDisposition::Executable => !matches!(
            program.mechanic,
            PrintedMechanic::AbilityWord | PrintedMechanic::Landfall | PrintedMechanic::Ferocious
        ),
        MarkerDisposition::StructurallyNonoperative {
            owned_executable_clause,
        } => {
            matches!(
                program.mechanic,
                PrintedMechanic::AbilityWord
                    | PrintedMechanic::Landfall
                    | PrintedMechanic::Ferocious
            ) && program
                .executable_clauses
                .iter()
                .any(|clause| clause.address() == *owned_executable_clause)
                && ability_word_program_has_exact_contract(program)
        }
    }
}

fn ability_word_program_has_exact_contract(program: &MechanicProgram) -> bool {
    let procedure = match (&program.mechanic, &program.procedure) {
        (PrintedMechanic::AbilityWord, MechanicProcedure::AbilityWord(procedure))
        | (PrintedMechanic::Landfall, MechanicProcedure::Landfall(procedure))
        | (PrintedMechanic::Ferocious, MechanicProcedure::Ferocious(procedure)) => procedure,
        _ => return false,
    };
    procedure.marker == program.mechanic
        && procedure.owned_executable_clause == program.primary_address
        && !procedure.printed_label.trim().is_empty()
        && (program.mechanic == PrintedMechanic::AbilityWord
            || procedure
                .printed_label
                .eq_ignore_ascii_case(program.mechanic.printed_label()))
        && program.executable_clauses.iter().any(|clause| {
            clause.address() == procedure.owned_executable_clause
                && clause
                    .ability_word()
                    .is_some_and(|word| word.eq_ignore_ascii_case(procedure.printed_label.trim()))
        })
}

fn procedure_matches_mechanic(mechanic: PrintedMechanic, procedure: &MechanicProcedure) -> bool {
    matches!(
        (mechanic, procedure),
        (
            PrintedMechanic::AbilityWord,
            MechanicProcedure::AbilityWord(_)
        ) | (PrintedMechanic::Cycling, MechanicProcedure::Cycling(_))
            | (
                PrintedMechanic::Typecycling,
                MechanicProcedure::Typecycling(_)
            )
            | (PrintedMechanic::Enchant, MechanicProcedure::Enchant(_))
            | (PrintedMechanic::Food, MechanicProcedure::Food(_))
            | (PrintedMechanic::Prowess, MechanicProcedure::Prowess(_))
            | (PrintedMechanic::Channel, MechanicProcedure::Channel(_))
            | (PrintedMechanic::Treasure, MechanicProcedure::Treasure(_))
            | (PrintedMechanic::Scry, MechanicProcedure::Scry(_))
            | (PrintedMechanic::Landfall, MechanicProcedure::Landfall(_))
            | (PrintedMechanic::Double, MechanicProcedure::Double(_))
            | (PrintedMechanic::Paradigm, MechanicProcedure::Paradigm(_))
            | (PrintedMechanic::Transform, MechanicProcedure::Transform(_))
            | (PrintedMechanic::Surveil, MechanicProcedure::Surveil(_))
            | (PrintedMechanic::Crew, MechanicProcedure::Crew(_))
            | (PrintedMechanic::Ward, MechanicProcedure::Ward(_))
            | (
                PrintedMechanic::SplitSecond,
                MechanicProcedure::SplitSecond(_)
            )
            | (PrintedMechanic::Evoke, MechanicProcedure::Evoke(_))
            | (PrintedMechanic::Manifest, MechanicProcedure::Manifest(_))
            | (PrintedMechanic::Partner, MechanicProcedure::Partner(_))
            | (PrintedMechanic::Ferocious, MechanicProcedure::Ferocious(_))
            | (PrintedMechanic::Dash, MechanicProcedure::Dash(_))
            | (PrintedMechanic::Gift, MechanicProcedure::Gift(_))
            | (PrintedMechanic::Mobilize, MechanicProcedure::Mobilize(_))
    )
}

fn compile_ability_word(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let label = input
        .marker_label
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .ok_or_else(|| shape_error(&input, "exact marker label"))?;
    let clause = compile_primary(&input)?;
    require_ability_word(&input, &clause, label)?;
    let address = clause.address();
    Ok(program(
        &input,
        MarkerDisposition::StructurallyNonoperative {
            owned_executable_clause: address,
        },
        vec![clause],
        MechanicProcedure::AbilityWord(AbilityWordProcedure {
            marker: PrintedMechanic::AbilityWord,
            printed_label: label.to_string(),
            owned_executable_clause: address,
        }),
    ))
}

fn compile_cycling(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Activated),
        "activated timing",
    )?;
    let [
        Cost::Mana(activation_cost),
        Cost::Discard(ObjectRef::Source),
    ] = clause.costs()
    else {
        return Err(shape_error(
            &input,
            "matching mana and discard-source costs",
        ));
    };
    if clause.activation_restriction() != Some(&ActivationRestriction::SourceZone(Zone::Hand))
        || clause.reminder().is_some_and(|reminder| {
            !matches!(
                reminder,
                ReminderSemantics::CyclingProcedure { cost }
                    if cost == activation_cost
            )
        })
    {
        return Err(shape_error(
            &input,
            "hand activation with matching mana and discard-source costs",
        ));
    }
    let [
        Effect::Draw {
            player,
            amount: cards_drawn,
            optional: false,
            delayed_until: None,
        },
    ] = clause.effects()
    else {
        return Err(shape_error(&input, "one exact draw effect"));
    };
    if player != &PlayerRef::You || cards_drawn != &Amount::Constant(1) {
        return Err(shape_error(&input, "draw one card for the controller"));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause.clone()],
        MechanicProcedure::Cycling(CyclingProcedure {
            activation_zone: Zone::Hand,
            costs: clause.costs().to_vec(),
            player: player.clone(),
            cards_drawn: cards_drawn.clone(),
        }),
    ))
}

fn compile_typecycling(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Activated),
        "activated timing",
    )?;
    let (type_name, reminder_cost, reminder_filter) = match clause.reminder() {
        Some(ReminderSemantics::TypecyclingProcedure {
            type_name,
            cost,
            filter,
        }) => (type_name, cost, filter.as_ref()),
        _ => {
            return Err(shape_error(&input, "exact typecycling reminder procedure"));
        }
    };
    if type_name.trim().is_empty()
        || clause.activation_restriction() != Some(&ActivationRestriction::SourceZone(Zone::Hand))
        || clause.costs()
            != [
                Cost::Mana(reminder_cost.clone()),
                Cost::Discard(ObjectRef::Source),
            ]
    {
        return Err(shape_error(
            &input,
            "hand activation with matching mana and discard-source costs",
        ));
    }
    let [Effect::SearchLibrary(search)] = clause.effects() else {
        return Err(shape_error(&input, "one exact type search"));
    };
    let exact_destination = [SearchDestination {
        selected_ordinal: SearchOrdinal::Each,
        zone: Zone::Hand,
        tapped: false,
    }];
    if search.player != PlayerRef::You
        || search.chooser != PlayerRef::You
        || search.optional
        || !search.allow_fail_to_find
        || search.amount != Amount::Constant(1)
        || search.predicate != *reminder_filter
        || !search.reveal
        || search.destinations != exact_destination
        || !search.shuffle_after
    {
        return Err(shape_error(
            &input,
            "reveal one matching library card into hand and shuffle",
        ));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause.clone()],
        MechanicProcedure::Typecycling(TypecyclingProcedure {
            activation_zone: Zone::Hand,
            costs: clause.costs().to_vec(),
            type_name: type_name.clone(),
            search: search.clone(),
        }),
    ))
}

fn compile_food(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    let created_definition = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::CreateToken(creation) => match &creation.specification {
                TokenSpecification::Defined(definition) if is_complete_food(definition) => {
                    Some(definition.as_ref().clone())
                }
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "exact executable Food token definition"))?;
    let reminder_definition = match clause.reminder() {
        Some(ReminderSemantics::FoodDefinition(definition)) if is_complete_food(definition) => {
            Some(definition.as_ref())
        }
        Some(_) => return Err(shape_error(&input, "matching Food reminder definition")),
        None => None,
    };
    if reminder_definition.is_some_and(|reminder| reminder != &created_definition) {
        return Err(shape_error(&input, "matching Food reminder definition"));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Food(FoodProcedure {
            token_definition: created_definition,
        }),
    ))
}

fn compile_prowess(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    if clause
        .reminder()
        .is_some_and(|reminder| !matches!(reminder, ReminderSemantics::ProwessProcedure))
    {
        return Err(shape_error(&input, "matching Prowess reminder procedure"));
    }
    let Timing::Triggered(trigger) = clause.timing() else {
        return Err(shape_error(&input, "cast trigger"));
    };
    let Trigger::Cast { player, spell } = trigger.as_ref() else {
        return Err(shape_error(&input, "cast trigger"));
    };
    if player != &PlayerRef::You
        || !spell.card_types.contains(&CardType::Spell)
        || !spell.excluded_card_types.contains(&CardType::Creature)
    {
        return Err(shape_error(&input, "controller noncreature-spell trigger"));
    }
    let [Effect::ModifyPowerToughness(change)] = clause.effects() else {
        return Err(shape_error(&input, "one exact power and toughness change"));
    };
    if change.objects != ObjectRef::Source
        || change.operation != PowerToughnessOperation::Add
        || change.power != Amount::Constant(1)
        || change.toughness != Amount::Constant(1)
        || change.duration != Duration::UntilEndOfTurn
    {
        return Err(shape_error(&input, "source gets +1/+1 until end of turn"));
    }
    let trigger = trigger.as_ref().clone();
    let change = change.clone();
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Prowess(ProwessProcedure { trigger, change }),
    ))
}

fn compile_enchant(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    if !matches!(
        input.layout.trim().to_ascii_lowercase().as_str(),
        "normal" | "transform" | "modal_dfc"
    ) {
        return Err(MechanicCompileError::UnsupportedLayout {
            mechanic: input.mechanic,
            address: input.primary.address(),
            layout: input.layout.to_string(),
        });
    }
    require_type_and_subtype(&input, CardType::Enchantment, Some("Aura"))?;
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Static),
        "static timing",
    )?;
    let legal_attachment = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Restriction(Restriction::EnchantRestriction { filter }) => Some(filter.clone()),
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "exact attachment restriction"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Enchant(EnchantProcedure {
            aura: ObjectRef::Source,
            legal_attachment,
            attachment_zone: Zone::Battlefield,
            targets_while_cast: true,
            attach_on_resolution: true,
            move_to_graveyard_if_unattached_or_illegal: true,
        }),
    ))
}

fn compile_channel(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Land])?;
    let clause = compile_primary(&input)?;
    require_ability_word(&input, &clause, "Channel")?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Activated),
        "activated timing",
    )?;
    if !clause
        .costs()
        .iter()
        .any(|cost| matches!(cost, Cost::Mana(_)))
        || !clause
            .costs()
            .iter()
            .any(|cost| matches!(cost, Cost::Discard(ObjectRef::Source)))
    {
        return Err(shape_error(&input, "mana and discard-this-object costs"));
    }
    if clause.targets().is_empty() {
        return Err(shape_error(&input, "a completely typed target"));
    }
    let has_resolution_action = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Destroy { .. } | Effect::MoveZone(_) | Effect::Counter { .. }
        )
    });
    let has_legendary_reduction = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::ReduceActivationCost {
                mana: ManaCost(value),
                per: CountExpression::MatchingObjects { filter, .. },
                ..
            } if value == "{1}"
                && filter.card_types.contains(&CardType::Creature)
                && filter.supertypes.iter().any(|value| {
                    matches!(value, crate::bounded_oracle_runtime::Supertype::Legendary)
                })
        )
    });
    if !has_resolution_action || !has_legendary_reduction {
        return Err(shape_error(
            &input,
            "resolution action and legendary-creature activation reduction",
        ));
    }
    let procedure = ChannelProcedure {
        activation_zone: Zone::Hand,
        costs: clause.costs().to_vec(),
        targets: clause.targets().to_vec(),
        effects: clause.effects().to_vec(),
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Channel(procedure),
    ))
}

fn compile_treasure(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    let created_definition = clause.effects().iter().find_map(|effect| match effect {
        Effect::CreateToken(creation) => match &creation.specification {
            TokenSpecification::Defined(definition) if is_complete_treasure(definition) => {
                Some(definition.as_ref().clone())
            }
            _ => None,
        },
        _ => None,
    });
    let reminder_definition = match clause.reminder() {
        Some(ReminderSemantics::TreasureDefinition(definition))
            if is_complete_treasure(definition) =>
        {
            Some(definition.as_ref().clone())
        }
        _ => None,
    };
    let token_definition = created_definition
        .filter(|definition| {
            reminder_definition
                .as_ref()
                .is_none_or(|reminder| reminder == definition)
        })
        .ok_or_else(|| shape_error(&input, "exact executable Treasure token definition"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Treasure(TreasureProcedure { token_definition }),
    ))
}

fn compile_scry(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    let (player, amount) = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Scry { player, amount } => Some((player.clone(), amount.clone())),
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "exact scry effect"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Scry(ScryProcedure {
            player,
            amount,
            may_put_on_library_bottom: true,
        }),
    ))
}

fn compile_landfall(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Creature])?;
    let clause = compile_primary(&input)?;
    require_ability_word(&input, &clause, "Landfall")?;
    let valid_trigger = matches!(
        clause.timing(),
        Timing::Triggered(trigger)
            if matches!(
                trigger.as_ref(),
                Trigger::ObjectEnters(filter)
                    if filter.card_types.contains(&CardType::Land)
                        && filter.controller == Some(PlayerRef::You)
            )
    );
    if !valid_trigger || clause.effects().is_empty() {
        return Err(shape_error(
            &input,
            "land entry trigger with a completely compiled effect",
        ));
    }
    let address = clause.address();
    Ok(program(
        &input,
        MarkerDisposition::StructurallyNonoperative {
            owned_executable_clause: address,
        },
        vec![clause],
        MechanicProcedure::Landfall(AbilityWordProcedure {
            marker: PrintedMechanic::Landfall,
            printed_label: "Landfall".into(),
            owned_executable_clause: address,
        }),
    ))
}

fn compile_double(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Enchantment, CardType::Creature])?;
    let clause = compile_primary(&input)?;
    let valid_timing = matches!(
        clause.timing(),
        Timing::Triggered(trigger)
            if matches!(
                trigger.as_ref(),
                Trigger::BeginningOf {
                    step: Step::Combat,
                    player: TurnPlayer::EachPlayer,
                }
            )
    );
    let change = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::ModifyPowerToughness(change)
                if change.operation == PowerToughnessOperation::Double
                    && change.duration == Duration::UntilEndOfTurn =>
            {
                Some(change.clone())
            }
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "temporary power and toughness doubling"))?;
    if !valid_timing {
        return Err(shape_error(&input, "beginning of each combat trigger"));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Double(DoubleProcedure { change }),
    ))
}

fn compile_paradigm(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Sorcery])?;
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Static),
        "static timing",
    )?;
    if !matches!(
        clause.reminder(),
        Some(ReminderSemantics::ParadigmProcedure)
    ) {
        return Err(shape_error(&input, "complete Paradigm reminder procedure"));
    }
    let exile_after_resolution = clause
        .effects()
        .iter()
        .find(|effect| {
            matches!(
                effect,
                Effect::ExileSpellAfterResolution {
                    object: ObjectRef::Source
                }
            )
        })
        .cloned()
        .ok_or_else(|| shape_error(&input, "exile after resolution action"))?;
    let cast_copy = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::CastCopy(copy)
                if copy.source == ObjectRef::Source
                    && copy.from == Zone::Exile
                    && copy.without_paying_mana_cost
                    && copy.repeat == RepeatSchedule::EachFirstMainPhase =>
            {
                Some(copy.clone())
            }
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "scheduled optional copy cast from exile"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Paradigm(Box::new(ParadigmProcedure {
            first_resolution_gate: Condition::FirstResolutionOfNamedSpell,
            exile_after_resolution,
            cast_copy,
            cast_is_optional: true,
        })),
    ))
}

fn compile_transform(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_layout(&input, "transform")?;
    let companion = input
        .companion
        .ok_or(MechanicCompileError::MissingCompanion {
            mechanic: input.mechanic,
            address: input.primary.address(),
        })?;
    if companion.address() == input.primary.address() {
        return Err(MechanicCompileError::CompanionAddressCollision {
            mechanic: input.mechanic,
            address: input.primary.address(),
        });
    }
    let front = compile_primary(&input)?;
    let back = compile_clause(input.mechanic, companion)?;
    if front.address().face_index != 0
        || back.address().face_index != 1
        || !front.effects().iter().any(|effect| {
            matches!(
                effect,
                Effect::Transform {
                    object: ObjectRef::Source
                }
            )
        })
    {
        return Err(shape_error(&input, "exact front-face transform effect"));
    }
    let origin = match (back.timing(), back.reminder()) {
        (
            Timing::SpecialAction(SpecialActionTiming::TransformBackFaceAnnotation),
            Some(ReminderSemantics::TransformOrigin { front_face_name }),
        ) => front_face_name,
        _ => return Err(shape_error(&input, "back face transform origin annotation")),
    };
    if !same_dynamic_face_name(input.primary.source_name, origin) {
        return Err(MechanicCompileError::TransformOriginMismatch {
            address: back.address(),
        });
    }
    let procedure = TransformProcedure {
        front_clause: front.address(),
        back_annotation_clause: back.address(),
        front_face_index: front.address().face_index,
        back_face_index: back.address().face_index,
        preserve_physical_object_identity: true,
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![front, back],
        MechanicProcedure::Transform(procedure),
    ))
}

fn compile_surveil(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    let (player, amount) = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Surveil { player, amount } => Some((player.clone(), amount.clone())),
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "exact surveil effect"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Surveil(SurveilProcedure {
            player,
            amount,
            may_put_in_graveyard: true,
        }),
    ))
}

fn compile_crew(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_type_and_subtype(&input, CardType::Artifact, Some("Vehicle"))?;
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Activated),
        "activated timing",
    )?;
    let minimum_total_power = clause
        .costs()
        .iter()
        .find_map(|cost| match cost {
            Cost::TapCreaturesWithTotalPower {
                player: PlayerRef::You,
                minimum,
            } => Some(minimum.clone()),
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "controlled-creature tapping cost"))?;
    if let Some(ReminderSemantics::CrewProcedure { required_power }) = clause.reminder()
        && required_power != &minimum_total_power
    {
        return Err(shape_error(&input, "matching Crew power requirement"));
    }
    let eligible_creatures = ObjectFilter {
        zones: vec![Zone::Battlefield],
        controller: Some(PlayerRef::You),
        card_types: vec![CardType::Creature],
        ..ObjectFilter::default()
    };
    if !clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Animate(animation)
                if animation.retain_printed_power_toughness
                    && animation.colors.is_empty()
                    && animation.subtypes.is_empty()
                    && animation.keywords.is_empty()
                    && !animation.retain_land
                    && animation.duration == Duration::UntilEndOfTurn
        )
    }) {
        return Err(shape_error(
            &input,
            "Vehicle animation that retains printed power and toughness",
        ));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Crew(CrewProcedure {
            controller: PlayerRef::You,
            eligible_creatures,
            tap_any_number: true,
            minimum_total_power,
            becomes_artifact_creature: true,
            keeps_printed_power_and_toughness: true,
            duration: Duration::UntilEndOfTurn,
        }),
    ))
}

fn compile_ward(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    let cost = match clause.timing() {
        Timing::Triggered(trigger) => {
            if !matches!(
                trigger.as_ref(),
                Trigger::BecomesTarget {
                    object: ObjectRef::Source,
                    controller: PlayerRef::Opponent,
                    source_kinds,
                } if source_kinds.is_empty()
            ) {
                return Err(shape_error(
                    &input,
                    "opponent spell-or-ability target trigger",
                ));
            }
            let [
                Effect::ResolveWard {
                    payer: PlayerRef::ThatPlayer,
                    source: ObjectRef::TriggeringObject,
                    cost,
                },
            ] = clause.effects()
            else {
                return Err(shape_error(&input, "exact Ward payment resolution"));
            };
            if clause.reminder().is_some_and(|reminder| {
                !matches!(
                    reminder,
                    ReminderSemantics::KeywordExplanation(Keyword::Ward(reminder_cost))
                        if reminder_cost.as_ref() == cost.as_ref()
                )
            }) {
                return Err(shape_error(&input, "matching Ward reminder procedure"));
            }
            cost.as_ref().clone()
        }
        Timing::Static => {
            let [
                Effect::GrantKeyword {
                    objects: ObjectRef::Source,
                    keywords,
                    duration: Duration::Permanent,
                },
            ] = clause.effects()
            else {
                return Err(shape_error(&input, "exact printed Ward keyword grant"));
            };
            let mut ward_costs = keywords.iter().filter_map(|keyword| match keyword {
                Keyword::Ward(cost) => Some(cost.as_ref()),
                _ => None,
            });
            let Some(cost) = ward_costs.next() else {
                return Err(shape_error(&input, "one printed Ward cost"));
            };
            if ward_costs.next().is_some() || clause.reminder().is_some() {
                return Err(shape_error(&input, "one exact printed Ward occurrence"));
            }
            cost.clone()
        }
        _ => {
            return Err(shape_error(
                &input,
                "becomes-target trigger or printed Ward",
            ));
        }
    };
    let payment = match &cost {
        WardCost::Mana(mana) => Cost::Mana(mana.clone()),
        WardCost::PayLife(amount) => Cost::PayLife(amount.clone()),
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Ward(WardProcedure {
            protected_object: ObjectRef::Source,
            triggering_source_controller: PlayerRef::Opponent,
            payer: PlayerRef::ThatPlayer,
            triggering_spell_or_ability: ObjectRef::TriggeringObject,
            applies_to_spells_and_abilities: true,
            payment,
            counter_triggering_spell_or_ability_unless_paid: true,
        }),
    ))
}

fn compile_split_second(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Instant])?;
    let clause = compile_primary(&input)?;
    let blocks_casting = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Restriction(Restriction::CannotCastNonManaSpellsWhileOnStack {
                affected: PlayerRef::Any
            })
        )
    });
    let blocks_activation = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Restriction(Restriction::CannotActivateNonManaAbilitiesWhileOnStack {
                affected: PlayerRef::Any
            })
        )
    });
    if !blocks_casting
        || !blocks_activation
        || !matches!(
            clause.reminder(),
            Some(ReminderSemantics::SplitSecondProcedure)
        )
    {
        return Err(shape_error(&input, "complete stack restriction procedure"));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::SplitSecond(SplitSecondProcedure {
            source_zone: Zone::Stack,
            affected_players: PlayerRef::Any,
            blocks_spell_casting_except_mana_actions: true,
            blocks_nonmana_ability_activation: true,
        }),
    ))
}

fn compile_evoke(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Creature])?;
    let clause = compile_primary(&input)?;
    let reminder_cost = match clause.reminder() {
        Some(ReminderSemantics::EvokeProcedure { cost }) => cost.clone(),
        _ => return Err(shape_error(&input, "complete Evoke reminder procedure")),
    };
    let alternative_cast = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Restriction(Restriction::AlternativeCastPermission(permission))
                if permission.from == Zone::Hand
                    && permission.object == ObjectRef::Source
                    && permission.cost == AlternativeCost::Mana(reminder_cost.clone()) =>
            {
                Some(permission.as_ref().clone())
            }
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "Evoke alternative cast permission from hand"))?;
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Evoke(Box::new(EvokeProcedure {
            alternative_cast,
            sacrifice_condition: Condition::CardWasCastWithAlternativeCost,
            sacrifice_trigger: Trigger::SourceEnters,
            sacrifice_move: ZoneMove {
                object: ObjectRef::Source,
                from: Some(Zone::Battlefield),
                to: Zone::Graveyard,
                tapped: false,
                face_down: false,
                delayed_until: None,
            },
        })),
    ))
}

fn compile_manifest(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Instant])?;
    let clause = compile_primary(&input)?;
    let has_exile_target = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::MoveZone(ZoneMove {
                object: ObjectRef::Target(_),
                from: Some(Zone::Battlefield),
                to: Zone::Exile,
                ..
            })
        )
    });
    let (player, card) = clause
        .effects()
        .iter()
        .find_map(|effect| match effect {
            Effect::Manifest { player, card } => Some((player.clone(), card.clone())),
            _ => None,
        })
        .ok_or_else(|| shape_error(&input, "manifest action for the target controller"))?;
    if !has_exile_target
        || !matches!(
            clause.reminder(),
            Some(ReminderSemantics::ManifestProcedure)
        )
    {
        return Err(shape_error(
            &input,
            "target exile and complete manifest procedure",
        ));
    }
    let manifested_object = ObjectRef::TopCard {
        player: Box::new(player.clone()),
    };
    if card != manifested_object {
        return Err(shape_error(
            &input,
            "top card of the affected player's library",
        ));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Manifest(ManifestProcedure {
            player,
            card: manifested_object.clone(),
            enters_zone: Zone::Battlefield,
            face_down: true,
            base_power: Amount::Constant(2),
            base_toughness: Amount::Constant(2),
            card_type: CardType::Creature,
            face_up_condition: Condition::ObjectIsCardType {
                object: manifested_object,
                card_type: CardType::Creature,
            },
            face_up_cost: ManifestFaceUpCost::ManifestedCardManaCost,
            face_up_timing: ManifestFaceUpTiming::SpecialActionAnyTimeControllerHasPriority,
        }),
    ))
}

fn compile_partner(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Creature])?;
    let clause = compile_primary(&input)?;
    let complete = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Restriction(Restriction::PartnerCommanderPairing)
        )
    }) && matches!(clause.reminder(), Some(ReminderSemantics::PartnerProcedure));
    if !complete {
        return Err(shape_error(&input, "complete commander pairing procedure"));
    }
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Partner(PartnerProcedure {
            permits_two_commanders: true,
            both_commanders_must_have_partner: true,
        }),
    ))
}

fn compile_ferocious(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_normal_layout(&input)?;
    require_any_type(&input, &[CardType::Sorcery])?;
    let clause = compile_primary(&input)?;
    require_ability_word(&input, &clause, "Ferocious")?;
    let has_complete_life_effect = clause.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::GainLife {
                player: PlayerRef::You,
                amount: Amount::Product { factor: 4, value },
            } if matches!(
                value.as_ref(),
                Amount::Count(expression)
                    if matches!(
                        expression.as_ref(),
                        CountExpression::MatchingObjects { filter, .. }
                            if filter.card_types.contains(&CardType::Creature)
                                && filter.power
                                    == Some((
                                        Comparison::AtLeast,
                                        Box::new(Amount::Constant(4)),
                                    ))
                    )
            )
        )
    });
    if !matches!(clause.timing(), Timing::SpellResolution) || !has_complete_life_effect {
        return Err(shape_error(
            &input,
            "four life for each controlled creature with power four or greater",
        ));
    }
    let address = clause.address();
    Ok(program(
        &input,
        MarkerDisposition::StructurallyNonoperative {
            owned_executable_clause: address,
        },
        vec![clause],
        MechanicProcedure::Ferocious(AbilityWordProcedure {
            marker: PrintedMechanic::Ferocious,
            printed_label: "Ferocious".into(),
            owned_executable_clause: address,
        }),
    ))
}

fn compile_dash(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_any_type(&input, &[CardType::Creature])?;
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Static),
        "static alternative cast procedure",
    )?;
    let reminder_cost = match clause.reminder() {
        Some(ReminderSemantics::DashProcedure { cost }) => cost,
        _ => return Err(shape_error(&input, "exact Dash reminder procedure")),
    };
    let [
        Effect::Restriction(Restriction::AlternativeCastPermission(alternative_cast)),
        Effect::Conditional {
            condition: Condition::CardWasCastWithAlternativeCost,
            if_true,
            if_false,
        },
    ] = clause.effects()
    else {
        return Err(shape_error(
            &input,
            "Dash cast permission followed by its cast-condition procedure",
        ));
    };
    let [
        haste @ Effect::GrantKeyword {
            objects: ObjectRef::Source,
            keywords,
            duration: Duration::Permanent,
        },
        Effect::MoveZone(return_to_hand),
    ] = if_true.as_slice()
    else {
        return Err(shape_error(
            &input,
            "haste and next-end-step return for the dashed source",
        ));
    };
    if !if_false.is_empty()
        || alternative_cast.object != ObjectRef::Source
        || alternative_cast.from != Zone::Hand
        || alternative_cast.cost != AlternativeCost::Mana(reminder_cost.clone())
        || alternative_cast.timing != Trigger::SourceCast
        || alternative_cast.condition.is_some()
        || keywords != &[Keyword::Haste]
        || return_to_hand.object != ObjectRef::Source
        || return_to_hand.from != Some(Zone::Battlefield)
        || return_to_hand.to != Zone::Hand
        || return_to_hand.tapped
        || return_to_hand.face_down
        || return_to_hand.delayed_until != Some(Trigger::BeginningOfNextEndStep)
    {
        return Err(shape_error(
            &input,
            "exact Dash alternative cost, haste, and delayed return",
        ));
    }
    let procedure = DashProcedure {
        alternative_cast: alternative_cast.as_ref().clone(),
        alternative_cost_condition: Condition::CardWasCastWithAlternativeCost,
        haste: haste.clone(),
        return_to_hand: return_to_hand.clone(),
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Dash(procedure),
    ))
}

fn compile_gift(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Triggered(trigger) if trigger.as_ref() == &Trigger::SourceCast),
        "source-cast gift procedure",
    )?;
    let [
        Effect::Conditional {
            condition: Condition::GiftPromised,
            if_true,
            if_false,
        },
    ] = clause.effects()
    else {
        return Err(shape_error(
            &input,
            "gift-promised conditional token procedure",
        ));
    };
    if !if_false.is_empty() {
        return Err(shape_error(&input, "gift-declined no-op branch"));
    }
    let benefit = match (clause.reminder(), if_true.as_slice()) {
        (
            Some(ReminderSemantics::GiftProcedure {
                token: reminder_token,
                tapped: reminder_tapped,
            }),
            [Effect::CreateToken(token_creation)],
        ) => {
            let TokenSpecification::Defined(definition) = &token_creation.specification else {
                return Err(shape_error(&input, "complete Gift token definition"));
            };
            if token_creation.player != PlayerRef::ThatPlayer
                || token_creation.amount != Amount::Constant(1)
                || definition.as_ref() != reminder_token.as_ref()
                || token_creation.tapped != *reminder_tapped
                || token_creation.attacking
            {
                return Err(shape_error(
                    &input,
                    "matching gift recipient, amount, token, and tapped state",
                ));
            }
            GiftBenefit::Token(token_creation.clone())
        }
        (
            Some(ReminderSemantics::GiftCardProcedure),
            [
                Effect::Draw {
                    player: PlayerRef::ThatPlayer,
                    amount: Amount::Constant(1),
                    optional: false,
                    delayed_until: None,
                },
            ],
        ) => GiftBenefit::DrawCard {
            player: PlayerRef::ThatPlayer,
            amount: Amount::Constant(1),
        },
        _ => return Err(shape_error(&input, "exact Gift reminder procedure")),
    };
    let procedure = GiftProcedure {
        promise_condition: Condition::GiftPromised,
        recipient: PlayerRef::ThatPlayer,
        benefit,
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Gift(procedure),
    ))
}

fn compile_mobilize(
    input: MechanicOccurrenceInput<'_>,
) -> Result<MechanicProgram, MechanicCompileError> {
    require_any_type(&input, &[CardType::Creature])?;
    let clause = compile_primary(&input)?;
    require_timing(
        &input,
        &clause,
        |timing| matches!(timing, Timing::Triggered(trigger) if trigger.as_ref() == &Trigger::SourceAttacks),
        "source-attacks trigger",
    )?;
    let (reminder_amount, reminder_token) = match clause.reminder() {
        Some(ReminderSemantics::MobilizeProcedure { amount, token }) => (amount, token.as_ref()),
        _ => return Err(shape_error(&input, "exact Mobilize reminder procedure")),
    };
    let [
        Effect::CreateTokenWithDelayedMove {
            creation,
            destination,
            trigger,
        },
    ] = clause.effects()
    else {
        return Err(shape_error(
            &input,
            "attacking token creation with one delayed move",
        ));
    };
    let TokenSpecification::Defined(definition) = &creation.specification else {
        return Err(shape_error(&input, "complete Mobilize token definition"));
    };
    if creation.player != PlayerRef::You
        || &creation.amount != reminder_amount
        || definition.as_ref() != reminder_token
        || !creation.tapped
        || !creation.attacking
        || destination != &Zone::Graveyard
        || trigger != &Trigger::BeginningOfNextEndStep
    {
        return Err(shape_error(
            &input,
            "matching amount, tapped attacking tokens, and delayed sacrifice",
        ));
    }
    let procedure = MobilizeProcedure {
        attack_trigger: Trigger::SourceAttacks,
        token_creation: creation.clone(),
        delayed_destination: *destination,
        delayed_trigger: trigger.clone(),
    };
    Ok(program(
        &input,
        MarkerDisposition::Executable,
        vec![clause],
        MechanicProcedure::Mobilize(procedure),
    ))
}

fn program(
    input: &MechanicOccurrenceInput<'_>,
    marker_disposition: MarkerDisposition,
    executable_clauses: Vec<BoundedOracleClause>,
    procedure: MechanicProcedure,
) -> MechanicProgram {
    MechanicProgram {
        runtime_version: MECHANIC_RUNTIME_VERSION,
        mechanic: input.mechanic,
        primary_address: input.primary.address(),
        marker_disposition,
        executable_clauses,
        procedure,
    }
}

fn compile_primary(
    input: &MechanicOccurrenceInput<'_>,
) -> Result<BoundedOracleClause, MechanicCompileError> {
    compile_clause(input.mechanic, input.primary)
}

fn compile_clause(
    mechanic: PrintedMechanic,
    input: MechanicClauseInput<'_>,
) -> Result<BoundedOracleClause, MechanicCompileError> {
    compile_bounded_oracle_clause(OracleClauseInput {
        face_index: input.face_index,
        clause_index: input.clause_index,
        source_name: input.source_name,
        source_type_line: input.source_type_line,
        oracle_clause: input.oracle_clause,
    })
    .map_err(|error| MechanicCompileError::BoundedClause { mechanic, error })
}

fn require_printed_keyword(
    input: &MechanicOccurrenceInput<'_>,
) -> Result<(), MechanicCompileError> {
    let label = input
        .marker_label
        .unwrap_or_else(|| input.mechanic.printed_label());
    if input
        .printed_keywords
        .iter()
        .any(|keyword| keyword.trim().eq_ignore_ascii_case(label))
    {
        Ok(())
    } else {
        Err(MechanicCompileError::MissingPrintedKeyword {
            mechanic: input.mechanic,
            address: input.primary.address(),
        })
    }
}

fn require_normal_layout(input: &MechanicOccurrenceInput<'_>) -> Result<(), MechanicCompileError> {
    require_layout(input, "normal")
}

fn require_layout(
    input: &MechanicOccurrenceInput<'_>,
    expected: &str,
) -> Result<(), MechanicCompileError> {
    if input.layout.trim().eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(MechanicCompileError::UnsupportedLayout {
            mechanic: input.mechanic,
            address: input.primary.address(),
            layout: input.layout.to_string(),
        })
    }
}

fn require_any_type(
    input: &MechanicOccurrenceInput<'_>,
    expected: &[CardType],
) -> Result<(), MechanicCompileError> {
    if expected
        .iter()
        .any(|card_type| type_has_card_type(input.primary.source_type_line, *card_type))
    {
        Ok(())
    } else {
        Err(MechanicCompileError::UnsupportedTypeLine {
            mechanic: input.mechanic,
            address: input.primary.address(),
            type_line: input.primary.source_type_line.to_string(),
        })
    }
}

fn require_type_and_subtype(
    input: &MechanicOccurrenceInput<'_>,
    card_type: CardType,
    subtype: Option<&str>,
) -> Result<(), MechanicCompileError> {
    let has_type = type_has_card_type(input.primary.source_type_line, card_type);
    let has_subtype =
        subtype.is_none_or(|subtype| type_has_subtype(input.primary.source_type_line, subtype));
    if has_type && has_subtype {
        Ok(())
    } else {
        Err(MechanicCompileError::UnsupportedTypeLine {
            mechanic: input.mechanic,
            address: input.primary.address(),
            type_line: input.primary.source_type_line.to_string(),
        })
    }
}

fn require_ability_word(
    input: &MechanicOccurrenceInput<'_>,
    clause: &BoundedOracleClause,
    expected: &str,
) -> Result<(), MechanicCompileError> {
    if clause
        .ability_word()
        .is_some_and(|word| word.eq_ignore_ascii_case(expected))
    {
        Ok(())
    } else {
        Err(shape_error(input, "matching printed ability word"))
    }
}

fn require_timing(
    input: &MechanicOccurrenceInput<'_>,
    clause: &BoundedOracleClause,
    predicate: impl FnOnce(&Timing) -> bool,
    detail: &'static str,
) -> Result<(), MechanicCompileError> {
    if predicate(clause.timing()) {
        Ok(())
    } else {
        Err(shape_error(input, detail))
    }
}

fn shape_error(input: &MechanicOccurrenceInput<'_>, detail: &'static str) -> MechanicCompileError {
    MechanicCompileError::InvalidProcedureShape {
        mechanic: input.mechanic,
        address: input.primary.address(),
        detail,
    }
}

fn type_has_card_type(type_line: &str, card_type: CardType) -> bool {
    let expected = match card_type {
        CardType::Artifact => "artifact",
        CardType::Battle => "battle",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Land => "land",
        CardType::Planeswalker => "planeswalker",
        CardType::Sorcery => "sorcery",
        CardType::Spell => "spell",
        CardType::Permanent => "permanent",
    };
    let type_section = type_line
        .split_once('\u{2014}')
        .map_or(type_line, |(types, _)| types);
    words(type_section).iter().any(|word| word == expected)
}

fn type_has_subtype(type_line: &str, expected: &str) -> bool {
    let Some((_, subtype_section)) = type_line.split_once('\u{2014}') else {
        return false;
    };
    words(subtype_section)
        .iter()
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn same_dynamic_face_name(source_name: &str, origin: &str) -> bool {
    let source_front = source_name
        .split_once(" // ")
        .map_or(source_name, |(front, _)| front);
    collapse_whitespace(source_front).eq_ignore_ascii_case(&collapse_whitespace(origin))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_complete_treasure(definition: &TokenDefinition) -> bool {
    let has_identity = definition
        .name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("Treasure"))
        && definition.card_types == vec![CardType::Artifact]
        && definition
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Treasure"));
    let Some(ability) = definition.abilities.first() else {
        return false;
    };
    let has_costs = ability.costs
        == vec![
            Cost::Tap(ObjectRef::Source),
            Cost::SacrificeObject(ObjectRef::Source),
        ];
    let produces_each_color = matches!(
        ability.effects.as_slice(),
        [Effect::AddMana(production)]
            if production.player == PlayerRef::You
                && production.amount == Amount::Constant(1)
                && !production.commander_identity_only
                && production.scales_with.is_none()
                && production.choices.len() == 5
                && [
                    Color::White,
                    Color::Blue,
                    Color::Black,
                    Color::Red,
                    Color::Green,
                ]
                .iter()
                .all(|color| production
                    .choices
                    .iter()
                    .any(|choice| choice.symbols == vec![*color]))
    );
    has_identity && definition.abilities.len() == 1 && has_costs && produces_each_color
}

fn is_complete_food(definition: &TokenDefinition) -> bool {
    let has_identity = definition
        .name
        .as_deref()
        .is_some_and(|name| name.eq_ignore_ascii_case("Food"))
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.colors.is_empty()
        && definition.card_types == vec![CardType::Artifact]
        && definition
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Food"))
        && definition.keywords.is_empty();
    let [ability] = definition.abilities.as_slice() else {
        return false;
    };
    has_identity
        && ability.costs
            == [
                Cost::Mana(ManaCost("{2}".into())),
                Cost::Tap(ObjectRef::Source),
                Cost::SacrificeObject(ObjectRef::Source),
            ]
        && ability.effects
            == [Effect::GainLife {
                player: PlayerRef::You,
                amount: Amount::Constant(3),
            }]
}
