//! Versioned Oracle-text effect descriptors.
//!
//! These descriptors deliberately capture only behavior the bounded simulator
//! or policy evaluator can consume. Unmodeled clauses remain attached to the
//! card and lower confidence instead of being silently treated as irrelevant.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::ability_program::{ExecutableAbilityProgramV1, normalize_oracle_clause_for_receipt};
use crate::alternative_cast_runtime::{
    AlternativeCastCardInput, CompiledAlternativeCast, compile_alternative_cast_runtime,
};
use crate::bounded_oracle_runtime::{
    BoundedOracleCardContext, BoundedOracleClause, ClauseAddress, OracleClauseInput,
    OracleFaceInput, Timing as BoundedOracleTiming, compile_bounded_oracle_clause_with_context,
    compile_bounded_oracle_face, normalize_oracle_clause,
};
use crate::characteristic_oracle_runtime::{
    AttractionLightsProcedure, CharacteristicOracleInput, CharacteristicOwnershipRequest,
    CombatKeyword as OracleCombatKeyword, CompiledCharacteristicOracle, CompiledCharacteristicView,
    CompiledDynamicCharacteristic as OracleDynamicCharacteristic, DefenseInitializationProcedure,
    DevotionColor as OracleDevotionColor, EvaluatedPrintedStat, ExactColorSetProcedure,
    ExactManaValueProcedure, ExactRational, ExactTypeLineProcedure, LoyaltyInitializationInputs,
    LoyaltyInitializationProcedure, PrintedStatInputs, PrintedStatProcedure,
    SourceCardType as OracleSourceCardType, StandardCardType, StandardSupertype,
    VanguardModifierProcedure, compile_characteristic_oracle_ownership,
    compile_exact_attraction_lights_procedure, compile_exact_color_set_procedure,
    compile_exact_defense_initialization_procedure, compile_exact_loyalty_initialization_procedure,
    compile_exact_mana_value_procedure, compile_exact_printed_stat_procedure,
    compile_exact_type_line_procedure, compile_exact_vanguard_modifier_procedure,
};
use crate::continuous_trigger_runtime::{
    CompiledContinuousTrigger, ContinuousTriggerCardInput, ContinuousTriggerFaceInput,
    compile_continuous_trigger_runtime,
};
use crate::domain::CardDefinition;
use crate::dynamic_characteristic_runtime::{
    DynamicCharacteristicProcedure, DynamicCharacteristicState, DynamicCharacteristicSubject,
    DynamicRuntimeValue, compile_dynamic_loyalty_procedure, compile_dynamic_printed_stat_procedure,
};
use crate::graveyard_transform_keyword_runtime::{
    CardLayout as GraveyardTransformCardLayout, FaceId as GraveyardTransformFaceId,
    FaceSemanticContext as GraveyardTransformFaceSemanticContext,
    ManaColor as GraveyardTransformManaColor,
    SourceSemanticContext as GraveyardTransformSourceSemanticContext,
    TransformSemanticContext as GraveyardTransformSemanticContext,
    normalize_face_oracle_text_for_semantics,
};
use crate::level_progression_runtime::{
    LevelProgressionFaceInput, LevelProgressionProgram, compile_level_progression_face,
};
use crate::mana::{ManaColorMask, parse_mana_cost};
use crate::mana_network_runtime::{ExactManaNetworkProgram, classify_exact_mana_network_program};
use crate::mechanic_runtime::{
    MechanicClauseInput, MechanicOccurrenceInput, MechanicProgram, PrintedMechanic,
    compile_mechanic_program,
};
use crate::object_lifecycle_runtime::{
    CompiledObjectLifecycle, ObjectLifecycleCardInput, compile_object_lifecycle_runtime,
};
use crate::oracle_clause_backend::{
    CompiledOracleClause, DelegatedKeywordClause, OracleClauseBackendInput,
    OracleClauseCardContext, OracleClauseSemanticContext,
    compile_oracle_clause_backend_with_semantic_context,
};
use crate::oracle_clause_syntax::{
    OracleClauseSyntaxError, OracleClauseSyntaxInput, OracleSyntaxProvenance,
    OracleSyntaxSemanticContext, RecognizedOracleClauseSyntax, recognize_oracle_clause_syntax,
    validate_oracle_clause_line,
};
use crate::utility_modal_runtime::{
    CompiledUtilityModal, UtilityModalCardInput, compile_utility_modal_runtime,
};

pub(crate) const EFFECT_DESCRIPTOR_VERSION: &str = "oracle-effects-0.39";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EffectMagnitude {
    #[default]
    None,
    Fixed(u8),
    Dynamic,
}

impl EffectMagnitude {
    pub fn conservative_value(self, dynamic_fallback: u8) -> u8 {
        match self {
            Self::None => 0,
            Self::Fixed(value) => value,
            Self::Dynamic => dynamic_fallback,
        }
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Dynamic, _) | (_, Self::Dynamic) => Self::Dynamic,
            (Self::Fixed(left), Self::Fixed(right)) => {
                Self::Fixed(left.saturating_add(right).min(12))
            }
            (Self::None, value) | (value, Self::None) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorScope {
    #[default]
    None,
    Restricted,
    AnyCard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorSourceZone {
    #[default]
    None,
    Library,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorTarget {
    #[default]
    None,
    AnyCard,
    BasicLand,
    Land,
    Creature,
    Artifact,
    Enchantment,
    Instant,
    Sorcery,
    InstantOrSorcery,
    ArtifactOrEnchantment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorDestination {
    #[default]
    None,
    Hand,
    LibraryTop,
    BattlefieldUntapped,
    BattlefieldTapped,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorTiming {
    #[default]
    None,
    /// The search is a direct instruction of an instant or sorcery after its
    /// printed mana cost has been paid.
    SpellResolution,
    /// Search text exists, but its trigger, activation, additional cost, or
    /// condition is outside the bounded executor.
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TutorInstruction {
    pub source: TutorSourceZone,
    pub target: TutorTarget,
    pub destination: TutorDestination,
    pub quantity: u8,
    pub reveal: bool,
    /// Randomize the remaining searched library after this instruction has
    /// finished moving cards. This is attached only to the final instruction
    /// produced by a single search clause, and only when that clause explicitly
    /// instructs its controller to shuffle.
    pub shuffle_after: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TutorDescriptor {
    pub timing: TutorTiming,
    pub instructions: Vec<TutorInstruction>,
    /// Fixed life loss that follows the complete search instruction during
    /// this spell's resolution. Zero means the reviewed Oracle text has no
    /// such clause; variable or differently ordered clauses remain
    /// unsupported instead of being approximated.
    pub life_loss_after_resolution: u8,
}

impl TutorDescriptor {
    pub fn is_executable_on_spell_resolution(&self) -> bool {
        self.timing == TutorTiming::SpellResolution && !self.instructions.is_empty()
    }
}

/// Structural card traits used by typed library-search matching. These are
/// compiled from the printed type line and cannot be supplied by semantic
/// role overrides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardTypeProfile {
    pub is_land: bool,
    pub is_basic_land: bool,
    pub is_creature: bool,
    pub is_artifact: bool,
    pub is_battle: bool,
    pub is_enchantment: bool,
    pub is_instant: bool,
    pub is_kindred: bool,
    pub is_planeswalker: bool,
    pub is_sorcery: bool,
}

/// Exact characteristics of the physical card while it is in its owner's
/// hand.
///
/// Double-faced cards use only their front face outside the stack and
/// battlefield (CR 712.8a). Adventure, Omen, and preparation cards likewise
/// use their normal characteristics in hand (CR 715.4, 720.4, and 722.4).
/// Keeping this separate from the root/combined Scryfall fields prevents a
/// modal double-faced card with a nonland front and a land back from being
/// treated as a land card in hand. The same distinction matters for effects
/// that inspect a hand card's colors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandZoneCharacteristics {
    /// False means the source record did not contain enough face data to bind
    /// the hand characteristics exactly. Consumers must fail closed rather
    /// than infer eligibility from empty fields.
    pub exact: bool,
    pub type_line: String,
    pub card_types: CardTypeProfile,
    pub colors: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StructuralCharacteristicProfile {
    pub layout: String,
    pub type_line: Option<ExactTypeLineProcedure>,
    pub mana_value: Option<ExactManaValueProcedure>,
    pub colors: Option<ExactColorSetProcedure>,
    pub color_indicator: Option<ExactColorSetProcedure>,
    pub power: Option<PrintedStatProcedure>,
    pub toughness: Option<PrintedStatProcedure>,
    pub loyalty: Option<LoyaltyInitializationProcedure>,
    pub dynamic_power: Option<DynamicCharacteristicProcedure>,
    pub dynamic_toughness: Option<DynamicCharacteristicProcedure>,
    pub dynamic_loyalty: Option<DynamicCharacteristicProcedure>,
    pub defense: Option<DefenseInitializationProcedure>,
    pub hand_modifier: Option<VanguardModifierProcedure>,
    pub life_modifier: Option<VanguardModifierProcedure>,
    pub attraction_lights: Option<AttractionLightsProcedure>,
}

impl StructuralCharacteristicProfile {
    pub(crate) fn card_types(&self) -> Option<CardTypeProfile> {
        let procedure = self.type_line.as_ref()?;
        Some(CardTypeProfile {
            is_land: procedure.has_card_type(StandardCardType::Land),
            is_basic_land: procedure.has_supertype(StandardSupertype::Basic)
                && procedure.has_card_type(StandardCardType::Land),
            is_creature: procedure.has_card_type(StandardCardType::Creature),
            is_artifact: procedure.has_card_type(StandardCardType::Artifact),
            is_battle: procedure.has_card_type(StandardCardType::Battle),
            is_enchantment: procedure.has_card_type(StandardCardType::Enchantment),
            is_instant: procedure.has_card_type(StandardCardType::Instant),
            is_kindred: procedure.has_card_type(StandardCardType::Kindred),
            is_planeswalker: procedure.has_card_type(StandardCardType::Planeswalker),
            is_sorcery: procedure.has_card_type(StandardCardType::Sorcery),
        })
    }

    pub(crate) fn fixed_power(&self) -> Option<ExactRational> {
        finite_printed_stat(self.resolved_power()?)
    }

    pub(crate) fn fixed_toughness(&self) -> Option<ExactRational> {
        finite_printed_stat(self.resolved_toughness()?)
    }

    pub(crate) fn resolved_power(&self) -> Option<EvaluatedPrintedStat> {
        self.power?.evaluate(PrintedStatInputs::default())
    }

    pub(crate) fn dynamic_power_with<R: DynamicCharacteristicState>(
        &self,
        state: &R,
    ) -> Option<DynamicRuntimeValue> {
        self.dynamic_power.as_ref()?.evaluate(state)
    }

    pub(crate) fn resolved_toughness(&self) -> Option<EvaluatedPrintedStat> {
        self.toughness?.evaluate(PrintedStatInputs::default())
    }

    pub(crate) fn dynamic_toughness_with<R: DynamicCharacteristicState>(
        &self,
        state: &R,
    ) -> Option<DynamicRuntimeValue> {
        self.dynamic_toughness.as_ref()?.evaluate(state)
    }

    pub(crate) fn initial_loyalty(&self) -> Option<u16> {
        self.loyalty?
            .initial_counters(LoyaltyInitializationInputs::default())
    }

    pub(crate) fn dynamic_initial_loyalty_with<R: DynamicCharacteristicState>(
        &self,
        state: &R,
    ) -> Option<DynamicRuntimeValue> {
        self.dynamic_loyalty.as_ref()?.evaluate(state)
    }

    pub(crate) fn initial_defense(&self) -> Option<u16> {
        Some(self.defense?.counters)
    }
}

fn finite_printed_stat(value: EvaluatedPrintedStat) -> Option<ExactRational> {
    match value {
        EvaluatedPrintedStat::Finite(value) => Some(value),
        EvaluatedPrintedStat::Infinite => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StructuralCharacteristics {
    pub root: StructuralCharacteristicProfile,
    pub faces: Vec<StructuralCharacteristicProfile>,
}

impl StructuralCharacteristics {
    pub(crate) fn battlefield_profile(
        &self,
        face_index: Option<usize>,
    ) -> &StructuralCharacteristicProfile {
        face_index
            .and_then(|face_index| self.faces.get(face_index))
            .or_else(|| self.faces.first())
            .unwrap_or(&self.root)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DevotionColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl DevotionColor {
    pub(crate) const fn pip_index(self) -> usize {
        match self {
            Self::White => 0,
            Self::Blue => 1,
            Self::Black => 2,
            Self::Red => 3,
            Self::Green => 4,
        }
    }

    pub(crate) const fn as_name(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Blue => "blue",
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
        }
    }

    const fn pip_symbol(self) -> &'static str {
        match self {
            Self::White => "{w}",
            Self::Blue => "{u}",
            Self::Black => "{b}",
            Self::Red => "{r}",
            Self::Green => "{g}",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DynamicCreatureCharacteristic {
    ToughnessEqualsDevotion(DevotionColor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PrintedKeyword {
    Deathtouch,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Shroud,
    Trample,
    Vigilance,
    Defender,
    Partner,
    FriendsForever,
    Bargain,
    Imprint,
    Metalcraft,
    Threshold,
    Storm,
    Flashback,
    Flash,
    Ward,
    Protection,
    Equip,
    CumulativeUpkeep,
    Affinity,
    Kicker,
    Prowess,
    Boast,
    Escape,
    Convoke,
    Delve,
    Devoid,
    Changeling,
}

impl PrintedKeyword {
    pub(crate) fn from_name(keyword: &str) -> Option<Self> {
        match keyword.trim().to_ascii_lowercase().as_str() {
            "deathtouch" => Some(Self::Deathtouch),
            "double strike" => Some(Self::DoubleStrike),
            "first strike" => Some(Self::FirstStrike),
            "flying" => Some(Self::Flying),
            "haste" => Some(Self::Haste),
            "hexproof" => Some(Self::Hexproof),
            "indestructible" => Some(Self::Indestructible),
            "lifelink" => Some(Self::Lifelink),
            "menace" => Some(Self::Menace),
            "reach" => Some(Self::Reach),
            "shroud" => Some(Self::Shroud),
            "trample" => Some(Self::Trample),
            "vigilance" => Some(Self::Vigilance),
            "defender" => Some(Self::Defender),
            "partner" => Some(Self::Partner),
            "friends forever" => Some(Self::FriendsForever),
            "bargain" => Some(Self::Bargain),
            "imprint" => Some(Self::Imprint),
            "metalcraft" => Some(Self::Metalcraft),
            "threshold" => Some(Self::Threshold),
            "storm" => Some(Self::Storm),
            "flashback" => Some(Self::Flashback),
            "flash" => Some(Self::Flash),
            "ward" => Some(Self::Ward),
            "protection" => Some(Self::Protection),
            "equip" => Some(Self::Equip),
            "cumulative upkeep" => Some(Self::CumulativeUpkeep),
            "affinity" => Some(Self::Affinity),
            "kicker" => Some(Self::Kicker),
            "prowess" => Some(Self::Prowess),
            "boast" => Some(Self::Boast),
            "escape" => Some(Self::Escape),
            "convoke" => Some(Self::Convoke),
            "delve" => Some(Self::Delve),
            "devoid" => Some(Self::Devoid),
            "changeling" => Some(Self::Changeling),
            _ => None,
        }
    }

    const fn mask(self) -> u64 {
        1u64 << self as u8
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PrintedKeywordProfile {
    bits: u64,
}

impl PrintedKeywordProfile {
    pub(crate) fn contains(self, keyword: PrintedKeyword) -> bool {
        self.bits & keyword.mask() != 0
    }
}

pub(crate) fn compile_printed_keyword_profile(keywords: &[String]) -> PrintedKeywordProfile {
    let bits = keywords
        .iter()
        .filter_map(|keyword| PrintedKeyword::from_name(keyword))
        .fold(0u64, |bits, keyword| bits | keyword.mask());
    PrintedKeywordProfile { bits }
}

impl TutorTarget {
    pub fn matches(self, card_types: CardTypeProfile) -> bool {
        match self {
            Self::None => false,
            Self::AnyCard => true,
            Self::BasicLand => card_types.is_basic_land,
            Self::Land => card_types.is_land,
            Self::Creature => card_types.is_creature,
            Self::Artifact => card_types.is_artifact,
            Self::Enchantment => card_types.is_enchantment,
            Self::Instant => card_types.is_instant,
            Self::Sorcery => card_types.is_sorcery,
            Self::InstantOrSorcery => card_types.is_instant || card_types.is_sorcery,
            Self::ArtifactOrEnchantment => card_types.is_artifact || card_types.is_enchantment,
        }
    }
}

/// The only mana-production lifecycles the bounded trajectory model can
/// execute without inventing a payment, trigger, or reusable permanent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ManaProductionKind {
    #[default]
    None,
    /// A clean instant/sorcery Add-mana instruction after its mana cost has
    /// been paid (for example, Dark Ritual).
    SpellResolution,
    /// A permanent with a tap-only Add-mana activation. It remains available
    /// on later turns (for example, Arcane Signet or Llanowar Elves).
    ReusableActivated,
    /// A permanent whose entire modeled activation cost is sacrificing itself,
    /// optionally with a tap cost that is immediately legal for a noncreature
    /// permanent (for example, Lotus Petal).
    OneShotActivated,
    /// A reviewed permanent with a tap-only mana activation whose complete
    /// Oracle lifecycle prevents normal untapping. The trajectory model may
    /// use its first activation while retaining the permanent, but must not
    /// refresh that mana source on later turns.
    NonRefreshingActivated,
    /// Mana text exists, but at least one cost, restriction, trigger, or timing
    /// prerequisite is outside the bounded executor.
    Unsupported,
}

#[derive(Debug, Clone, Default)]
pub struct EffectDescriptor {
    pub card_types: CardTypeProfile,
    pub hand_zone: HandZoneCharacteristics,
    pub(crate) structural_characteristics: StructuralCharacteristics,
    pub(crate) printed_keywords: PrintedKeywordProfile,
    pub(crate) printed_devotion_pips: [u16; 5],
    pub(crate) dynamic_creature_characteristic: Option<DynamicCreatureCharacteristic>,
    pub(crate) alternative_cast: Option<CompiledAlternativeCast>,
    pub(crate) characteristic_oracle: Vec<CompiledCharacteristicOracle>,
    pub(crate) continuous_triggers: Vec<CompiledContinuousTrigger>,
    pub(crate) mana_network: Option<ExactManaNetworkProgram>,
    pub(crate) object_lifecycle: Option<CompiledObjectLifecycle>,
    pub(crate) utility_modal: Option<CompiledUtilityModal>,
    /// Complete occurrence-addressed Oracle clauses accepted by the generic
    /// bounded executor. Unsupported clauses are absent and therefore remain
    /// strict coverage blockers.
    pub(crate) bounded_oracle: Vec<BoundedOracleClause>,
    /// Exact delegated keyword clauses retained separately from the native
    /// bounded executor. Recognition alone does not make these clauses live.
    pub(crate) delegated_oracle: Vec<DelegatedKeywordClause>,
    /// Lossless syntax retained for every nonempty Oracle line. Syntax
    /// recognition, including a successful result, never authorizes execution
    /// and never replaces either executable program collection above.
    pub(crate) oracle_syntax: Vec<RetainedOracleClauseSyntax>,
    /// Every normalized source clause for each exact face, retained even when
    /// a specialized compiler represents the complete root as one program.
    /// Bounded receipts use this source partition to preserve the original
    /// occurrence address and never infer sibling coverage from a collapsed
    /// specialized program.
    pub(crate) bounded_oracle_source_roots: Vec<BoundedOracleSourceRoot>,
    /// Exact printed-mechanic procedures compiled from occurrence-addressed
    /// Oracle clauses. A bounded clause cannot advertise a mechanic capability
    /// unless the corresponding program is retained here.
    pub(crate) mechanic_programs: Vec<MechanicProgram>,
    pub draw_cards: EffectMagnitude,
    pub impulse_access: EffectMagnitude,
    pub tutor_scope: TutorScope,
    pub tutor: TutorDescriptor,
    pub lands_to_battlefield: EffectMagnitude,
    pub mana_produced: EffectMagnitude,
    pub mana_production_kind: ManaProductionKind,
    pub creature_tokens: EffectMagnitude,
    pub treasure_tokens: EffectMagnitude,
    pub targeted_removal: bool,
    pub board_wipe: bool,
    pub mass_land_denial: bool,
    pub protection: bool,
    pub recursion: bool,
    pub extra_turns: EffectMagnitude,
    pub repeatable: bool,
    pub conditional: bool,
    pub requires_tap: bool,
    pub requires_sacrifice: bool,
    pub requires_discard: bool,
    pub life_cost: bool,
    pub unsupported_clauses: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundedOracleSourceRoot {
    pub face_index: u16,
    pub type_line: String,
    pub normalized_clauses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedOracleClauseSyntax {
    /// Exact only when the retained source supplied real face records or one
    /// unambiguous single-face Oracle root. Legacy `//` combined text has no
    /// exact occurrence address.
    pub(crate) address: Option<ClauseAddress>,
    /// The exact trimmed Oracle line used by the lossless syntax recognizer.
    /// Its syntax digest therefore depends on text, not card metadata.
    pub(crate) syntax_line: String,
    /// The existing compiler-normalized form retained for later binding to
    /// executable programs whose self references are name independent.
    pub(crate) normalized_line: String,
    pub(crate) recognition: Result<RecognizedOracleClauseSyntax, OracleClauseSyntaxError>,
}

impl EffectDescriptor {
    pub fn has_printed_keyword(&self, keyword: &str) -> bool {
        PrintedKeyword::from_name(keyword)
            .is_some_and(|keyword| self.printed_keywords.contains(keyword))
    }

    pub(crate) fn retain_exact_mana_network_program(
        &mut self,
        type_line: &str,
        ability_program: &ExecutableAbilityProgramV1,
    ) {
        self.mana_network = classify_exact_mana_network_program(type_line, ability_program);
    }
}

fn retain_oracle_clause_syntax(card: &CardDefinition) -> Vec<RetainedOracleClauseSyntax> {
    let mut retained = Vec::new();
    if card.faces.is_empty() {
        append_oracle_face_syntax(
            &mut retained,
            0,
            &card.name,
            &card.type_line,
            &card.oracle_text,
            !has_legacy_combined_oracle_root(card),
        );
    } else {
        for (face_index, face) in card.faces.iter().enumerate() {
            let Some(face_index) = u16::try_from(face_index).ok() else {
                continue;
            };
            append_oracle_face_syntax(
                &mut retained,
                face_index,
                &face.name,
                &face.type_line,
                &face.oracle_text,
                true,
            );
        }
    }
    retained
}

fn append_oracle_face_syntax(
    retained: &mut Vec<RetainedOracleClauseSyntax>,
    face_index: u16,
    source_name: &str,
    source_type_line: &str,
    oracle_text: &str,
    address_is_exact: bool,
) {
    for (clause_index, oracle_clause) in oracle_text
        .lines()
        .map(str::trim)
        .filter(|clause| !clause.is_empty() && *clause != "//")
        .enumerate()
    {
        let Ok(clause_index) = u16::try_from(clause_index) else {
            continue;
        };
        let address = ClauseAddress {
            face_index,
            clause_index,
        };
        let normalized_line = normalize_oracle_clause(oracle_clause, source_name, source_type_line);
        let recognition = recognize_effect_oracle_clause_syntax(
            oracle_clause,
            source_name,
            address_is_exact.then_some(address),
        );
        retained.push(RetainedOracleClauseSyntax {
            address: address_is_exact.then_some(address),
            syntax_line: oracle_clause.to_owned(),
            normalized_line,
            recognition,
        });
    }
}

fn recognize_effect_oracle_clause_syntax(
    oracle_clause: &str,
    source_name: &str,
    address: Option<ClauseAddress>,
) -> Result<RecognizedOracleClauseSyntax, OracleClauseSyntaxError> {
    recognize_oracle_clause_syntax(OracleClauseSyntaxInput {
        normalized_line: oracle_clause,
        semantic_context: OracleSyntaxSemanticContext::CardFace,
        provenance: OracleSyntaxProvenance {
            source_name: Some(source_name),
            face_index: address.map(|address| address.face_index),
            clause_index: address.map(|address| address.clause_index),
            ..OracleSyntaxProvenance::default()
        },
    })
}

fn has_legacy_combined_oracle_root(card: &CardDefinition) -> bool {
    card.faces.is_empty()
        && (layout_requires_exact_oracle_faces(&card.layout)
            || card.oracle_text.lines().any(|line| line.trim() == "//"))
}

fn layout_requires_exact_oracle_faces(layout: &str) -> bool {
    matches!(
        layout.trim().to_ascii_lowercase().as_str(),
        "split"
            | "transform"
            | "adventure"
            | "modal_dfc"
            | "flip"
            | "reversible_card"
            | "double_faced_token"
            | "prepare"
    )
}

fn compile_bounded_oracle_clauses(card: &CardDefinition) -> Vec<BoundedOracleClause> {
    if has_legacy_combined_oracle_root(card) || card.layout.eq_ignore_ascii_case("leveler") {
        return Vec::new();
    }
    let mut compiled = Vec::new();
    let card_context = BoundedOracleCardContext {
        layout: &card.layout,
        face_count: card.faces.len().max(1),
    };
    if card.faces.is_empty() {
        compiled.extend(compile_bounded_oracle_face_clauses(
            0,
            &card.name,
            &card.type_line,
            &card.oracle_text,
            card_context,
        ));
    } else {
        for (face_index, face) in card.faces.iter().enumerate() {
            let Some(face_index) = u16::try_from(face_index).ok() else {
                continue;
            };
            compiled.extend(compile_bounded_oracle_face_clauses(
                face_index,
                &face.name,
                &face.type_line,
                &face.oracle_text,
                card_context,
            ));
        }
    }
    compiled.sort_by_key(|clause| clause.address());
    compiled
}

fn exact_integral_graveyard_transform_mana_value(value: Option<f32>) -> Option<u32> {
    let value = f64::from(value?);
    (value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= f64::from(u32::MAX))
        .then_some(value as u32)
}

fn graveyard_transform_colors(values: &[String]) -> Option<BTreeSet<GraveyardTransformManaColor>> {
    values
        .iter()
        .map(|value| match value.trim().to_ascii_uppercase().as_str() {
            "W" => Some(GraveyardTransformManaColor::White),
            "U" => Some(GraveyardTransformManaColor::Blue),
            "B" => Some(GraveyardTransformManaColor::Black),
            "R" => Some(GraveyardTransformManaColor::Red),
            "G" => Some(GraveyardTransformManaColor::Green),
            "C" => Some(GraveyardTransformManaColor::Colorless),
            _ => None,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn graveyard_transform_face_semantic_context(
    name: &str,
    type_line: &str,
    oracle_text: &str,
    mana_cost: Option<&str>,
    colors: &[String],
    color_indicator: &[String],
    root_mana_value: u32,
    power: Option<&str>,
    toughness: Option<&str>,
    loyalty: Option<&str>,
    defense: Option<&str>,
) -> Option<GraveyardTransformFaceSemanticContext> {
    Some(GraveyardTransformFaceSemanticContext {
        normalized_oracle_text: normalize_face_oracle_text_for_semantics(oracle_text, name),
        type_line: type_line.to_owned(),
        mana_cost: mana_cost.unwrap_or_default().to_owned(),
        colors: graveyard_transform_colors(colors)?,
        color_indicator: graveyard_transform_colors(color_indicator)?,
        root_mana_value,
        power: power.map(str::to_owned),
        toughness: toughness.map(str::to_owned),
        loyalty: loyalty.map(str::to_owned),
        defense: defense.map(str::to_owned),
    })
}

/// Build one optional content-only context per addressed Oracle face.
///
/// No snapshot identity, row position, update timestamp, Oracle ID, or display
/// metadata enters these values. A context is absent unless the physical-card
/// relationship and every rules-relevant characteristic are complete.
fn graveyard_transform_semantic_contexts(
    card: &CardDefinition,
) -> Vec<Option<GraveyardTransformSourceSemanticContext>> {
    let face_count = card.faces.len().max(1);
    let mut contexts = vec![None; face_count];
    match card.layout.trim().to_ascii_lowercase().as_str() {
        "normal" if card.faces.is_empty() => {
            contexts[0] = Some(GraveyardTransformSourceSemanticContext::SingleFace {
                layout: GraveyardTransformCardLayout::Normal,
                type_line: card.type_line.clone(),
                normalized_oracle_text: normalize_face_oracle_text_for_semantics(
                    &card.oracle_text,
                    &card.name,
                ),
            });
        }
        "normal" if card.faces.len() == 1 => {
            let face = &card.faces[0];
            contexts[0] = Some(GraveyardTransformSourceSemanticContext::SingleFace {
                layout: GraveyardTransformCardLayout::Normal,
                type_line: face.type_line.clone(),
                normalized_oracle_text: normalize_face_oracle_text_for_semantics(
                    &face.oracle_text,
                    &face.name,
                ),
            });
        }
        "transform" if card.faces.len() == 2 => {
            let root_mana_value = match card.root_mana_value {
                Some(value) => exact_integral_graveyard_transform_mana_value(Some(value)),
                None => exact_integral_graveyard_transform_mana_value(card.faces[0].mana_value),
            };
            let Some(root_mana_value) = root_mana_value else {
                return contexts;
            };
            let front = &card.faces[0];
            let back = &card.faces[1];
            let Some(front) = graveyard_transform_face_semantic_context(
                &front.name,
                &front.type_line,
                &front.oracle_text,
                front.mana_cost.as_deref(),
                &front.colors,
                &front.color_indicator,
                root_mana_value,
                front.power.as_deref(),
                front.toughness.as_deref(),
                front.loyalty.as_deref(),
                front.defense.as_deref(),
            ) else {
                return contexts;
            };
            let Some(back) = graveyard_transform_face_semantic_context(
                &back.name,
                &back.type_line,
                &back.oracle_text,
                back.mana_cost.as_deref(),
                &back.colors,
                &back.color_indicator,
                root_mana_value,
                back.power.as_deref(),
                back.toughness.as_deref(),
                back.loyalty.as_deref(),
                back.defense.as_deref(),
            ) else {
                return contexts;
            };
            contexts[0] = Some(GraveyardTransformSourceSemanticContext::Transform(
                GraveyardTransformSemanticContext {
                    layout: GraveyardTransformCardLayout::Transform,
                    keyword_face: GraveyardTransformFaceId::Front,
                    front,
                    back,
                },
            ));
        }
        _ => {}
    }
    contexts
}

fn level_progression_semantic_contexts(
    card: &CardDefinition,
) -> Vec<Option<LevelProgressionProgram>> {
    let face_count = card.faces.len().max(1);
    let mut contexts = vec![None; face_count];
    if card.layout != "leveler" || face_count != 1 {
        return contexts;
    }

    let input = if card.faces.is_empty() {
        LevelProgressionFaceInput {
            exact_oracle_text: card.oracle_text.clone(),
            exact_layout: card.layout.clone(),
            exact_type_line: card.type_line.clone(),
            printed_power: card.power.as_deref().and_then(|value| value.parse().ok()),
            printed_toughness: card
                .toughness
                .as_deref()
                .and_then(|value| value.parse().ok()),
        }
    } else {
        let face = &card.faces[0];
        LevelProgressionFaceInput {
            exact_oracle_text: face.oracle_text.clone(),
            exact_layout: card.layout.clone(),
            exact_type_line: face.type_line.clone(),
            printed_power: face.power.as_deref().and_then(|value| value.parse().ok()),
            printed_toughness: face
                .toughness
                .as_deref()
                .and_then(|value| value.parse().ok()),
        }
    };
    contexts[0] = compile_level_progression_face(input).ok();
    contexts
}

fn compile_additional_oracle_clauses(
    card: &CardDefinition,
    bounded_oracle: &[BoundedOracleClause],
) -> (Vec<BoundedOracleClause>, Vec<DelegatedKeywordClause>) {
    if has_legacy_combined_oracle_root(card) {
        return (Vec::new(), Vec::new());
    }
    let bounded_addresses = bounded_oracle
        .iter()
        .map(BoundedOracleClause::address)
        .collect::<BTreeSet<_>>();
    let mut bounded = Vec::new();
    let mut delegated = Vec::new();
    let card_context = OracleClauseCardContext {
        layout: &card.layout,
        face_count: card.faces.len().max(1),
    };
    let graveyard_transform_contexts = graveyard_transform_semantic_contexts(card);
    let level_progression_contexts = level_progression_semantic_contexts(card);

    if card.faces.is_empty() {
        let keywords = card.keywords.iter().map(String::as_str).collect::<Vec<_>>();
        append_additional_oracle_clauses(
            &mut bounded,
            &mut delegated,
            &bounded_addresses,
            0,
            &card.name,
            &card.type_line,
            exact_nonnegative_u32(card.root_mana_value.unwrap_or(card.mana_value)),
            &card.oracle_text,
            &keywords,
            card_context,
            graveyard_transform_contexts[0].as_ref(),
            level_progression_contexts[0].as_ref(),
        );
    } else {
        for (face_index, face) in card.faces.iter().enumerate() {
            let Some(face_index) = u16::try_from(face_index).ok() else {
                continue;
            };
            let mut keywords = card.keywords.iter().map(String::as_str).collect::<Vec<_>>();
            keywords.extend(face.keywords.iter().map(String::as_str));
            append_additional_oracle_clauses(
                &mut bounded,
                &mut delegated,
                &bounded_addresses,
                face_index,
                &face.name,
                &face.type_line,
                face.mana_value.and_then(exact_nonnegative_u32),
                &face.oracle_text,
                &keywords,
                card_context,
                graveyard_transform_contexts
                    .get(usize::from(face_index))
                    .and_then(Option::as_ref),
                level_progression_contexts
                    .get(usize::from(face_index))
                    .and_then(Option::as_ref),
            );
        }
    }

    bounded.sort_by_key(BoundedOracleClause::address);
    delegated.sort_by_key(DelegatedKeywordClause::address);
    (bounded, delegated)
}

// Kept explicit because this compiler boundary carries independent source evidence.
#[allow(clippy::too_many_arguments)]
fn append_additional_oracle_clauses(
    bounded: &mut Vec<BoundedOracleClause>,
    delegated: &mut Vec<DelegatedKeywordClause>,
    bounded_addresses: &BTreeSet<crate::bounded_oracle_runtime::ClauseAddress>,
    face_index: u16,
    source_name: &str,
    source_type_line: &str,
    source_mana_value: Option<u32>,
    oracle_text: &str,
    printed_keywords: &[&str],
    card_context: OracleClauseCardContext<'_>,
    graveyard_transform_context: Option<&GraveyardTransformSourceSemanticContext>,
    level_progression_context: Option<&LevelProgressionProgram>,
) {
    for (clause_index, oracle_clause) in oracle_text
        .lines()
        .map(str::trim)
        .filter(|clause| !clause.is_empty() && *clause != "//")
        .enumerate()
    {
        let Ok(clause_index) = u16::try_from(clause_index) else {
            continue;
        };
        let address = crate::bounded_oracle_runtime::ClauseAddress {
            face_index,
            clause_index,
        };
        if bounded_addresses.contains(&address) {
            continue;
        }
        let input = OracleClauseBackendInput {
            face_index,
            clause_index,
            source_name,
            source_type_line,
            oracle_clause,
            printed_keywords,
        };
        match compile_oracle_clause_backend_with_semantic_context(
            input,
            OracleClauseSemanticContext {
                card: card_context,
                graveyard_transform: graveyard_transform_context,
                level_progression: level_progression_context,
                source_mana_value,
                complete_face_oracle_text: Some(oracle_text),
            },
        ) {
            Ok(CompiledOracleClause::Bounded(clause)) => bounded.push(clause),
            Ok(CompiledOracleClause::Delegated(clause)) => delegated.push(clause),
            Err(_) => {}
        }
    }
}

fn exact_nonnegative_u32(value: f32) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return None;
    }
    let integer = value as u32;
    (integer as f32 == value).then_some(integer)
}

fn compile_bounded_oracle_source_roots(card: &CardDefinition) -> Vec<BoundedOracleSourceRoot> {
    if has_legacy_combined_oracle_root(card) {
        return Vec::new();
    }
    let mut roots = Vec::new();
    if card.faces.is_empty() {
        roots.push(bounded_oracle_source_root(
            0,
            &card.name,
            &card.type_line,
            &card.oracle_text,
        ));
    } else {
        for (face_index, face) in card.faces.iter().enumerate() {
            let Some(face_index) = u16::try_from(face_index).ok() else {
                continue;
            };
            roots.push(bounded_oracle_source_root(
                face_index,
                &face.name,
                &face.type_line,
                &face.oracle_text,
            ));
        }
    }
    roots
}

fn bounded_oracle_source_root(
    face_index: u16,
    source_name: &str,
    type_line: &str,
    oracle_text: &str,
) -> BoundedOracleSourceRoot {
    BoundedOracleSourceRoot {
        face_index,
        type_line: type_line.to_owned(),
        normalized_clauses: oracle_text
            .lines()
            .map(str::trim)
            .filter(|clause| !clause.is_empty() && *clause != "//")
            .map(|clause| normalize_oracle_clause_for_receipt(clause, source_name, type_line))
            .collect(),
    }
}

fn compile_bounded_oracle_face_clauses(
    face_index: u16,
    source_name: &str,
    source_type_line: &str,
    oracle_text: &str,
    card_context: BoundedOracleCardContext<'_>,
) -> Vec<BoundedOracleClause> {
    let oracle_clauses = oracle_text
        .lines()
        .map(str::trim)
        .filter(|clause| !clause.is_empty() && *clause != "//")
        .collect::<Vec<_>>();
    if oracle_clauses.is_empty() {
        return Vec::new();
    }

    if let Ok(face) = compile_bounded_oracle_face(OracleFaceInput {
        face_index,
        source_name,
        source_type_line,
        oracle_clauses: &oracle_clauses,
    }) {
        return face.clauses;
    }

    oracle_clauses
        .iter()
        .enumerate()
        .filter_map(|(clause_index, oracle_clause)| {
            let clause = compile_bounded_oracle_clause_with_context(
                OracleClauseInput {
                    face_index,
                    clause_index: u16::try_from(clause_index).ok()?,
                    source_name,
                    source_type_line,
                    oracle_clause,
                },
                card_context,
            )
            .ok()?;
            (!matches!(
                clause.timing(),
                BoundedOracleTiming::ModalHeader { .. }
                    | BoundedOracleTiming::TriggeredModalHeader { .. }
                    | BoundedOracleTiming::ModalBranch { .. }
            ))
            .then_some(clause)
        })
        .collect()
}

const BOUNDED_PRINTED_MECHANICS: [PrintedMechanic; 23] = [
    PrintedMechanic::Cycling,
    PrintedMechanic::Typecycling,
    PrintedMechanic::Enchant,
    PrintedMechanic::Food,
    PrintedMechanic::Prowess,
    PrintedMechanic::Channel,
    PrintedMechanic::Treasure,
    PrintedMechanic::Scry,
    PrintedMechanic::Landfall,
    PrintedMechanic::Double,
    PrintedMechanic::Paradigm,
    PrintedMechanic::Transform,
    PrintedMechanic::Surveil,
    PrintedMechanic::Crew,
    PrintedMechanic::Ward,
    PrintedMechanic::SplitSecond,
    PrintedMechanic::Evoke,
    PrintedMechanic::Manifest,
    PrintedMechanic::Partner,
    PrintedMechanic::Ferocious,
    PrintedMechanic::Dash,
    PrintedMechanic::Gift,
    PrintedMechanic::Mobilize,
];

#[derive(Clone, Copy)]
struct MechanicSourceClause<'a> {
    face_index: u16,
    clause_index: u16,
    source_name: &'a str,
    source_type_line: &'a str,
    oracle_clause: &'a str,
}

impl<'a> MechanicSourceClause<'a> {
    fn input(self) -> MechanicClauseInput<'a> {
        MechanicClauseInput {
            face_index: self.face_index,
            clause_index: self.clause_index,
            source_name: self.source_name,
            source_type_line: self.source_type_line,
            oracle_clause: self.oracle_clause,
        }
    }
}

fn compile_mechanic_programs(card: &CardDefinition) -> Vec<MechanicProgram> {
    if has_legacy_combined_oracle_root(card) {
        return Vec::new();
    }
    let mut clauses = Vec::new();
    if card.faces.is_empty() {
        for (clause_index, oracle_clause) in card
            .oracle_text
            .lines()
            .map(str::trim)
            .filter(|clause| !clause.is_empty() && *clause != "//")
            .enumerate()
        {
            let Ok(clause_index) = u16::try_from(clause_index) else {
                continue;
            };
            if validate_oracle_clause_line(oracle_clause).is_err() {
                continue;
            }
            clauses.push(MechanicSourceClause {
                face_index: 0,
                clause_index,
                source_name: &card.name,
                source_type_line: &card.type_line,
                oracle_clause,
            });
        }
    } else {
        for (face_index, face) in card.faces.iter().enumerate() {
            let Ok(face_index) = u16::try_from(face_index) else {
                continue;
            };
            for (clause_index, oracle_clause) in face
                .oracle_text
                .lines()
                .map(str::trim)
                .filter(|clause| !clause.is_empty() && *clause != "//")
                .enumerate()
            {
                let Ok(clause_index) = u16::try_from(clause_index) else {
                    continue;
                };
                if validate_oracle_clause_line(oracle_clause).is_err() {
                    continue;
                }
                clauses.push(MechanicSourceClause {
                    face_index,
                    clause_index,
                    source_name: &face.name,
                    source_type_line: &face.type_line,
                    oracle_clause,
                });
            }
        }
    }

    let mut printed_keywords = card.keywords.clone();
    for face in &card.faces {
        printed_keywords.extend(face.keywords.iter().cloned());
    }
    printed_keywords.sort_by_key(|keyword| keyword.trim().to_ascii_lowercase());
    printed_keywords.dedup_by(|left, right| left.trim().eq_ignore_ascii_case(right.trim()));

    let mut programs = Vec::new();
    for primary in &clauses {
        for mechanic in BOUNDED_PRINTED_MECHANICS {
            if !printed_keywords.iter().any(|keyword| {
                keyword
                    .trim()
                    .eq_ignore_ascii_case(mechanic.printed_label())
            }) {
                continue;
            }
            let base = MechanicOccurrenceInput {
                mechanic,
                marker_label: None,
                layout: &card.layout,
                printed_keywords: &printed_keywords,
                primary: primary.input(),
                companion: None,
            };
            if mechanic == PrintedMechanic::Transform {
                for companion in clauses.iter().filter(|candidate| {
                    candidate.face_index != primary.face_index
                        || candidate.clause_index != primary.clause_index
                }) {
                    let input = MechanicOccurrenceInput {
                        companion: Some(companion.input()),
                        ..base
                    };
                    if let Ok(program) = compile_mechanic_program(input) {
                        programs.push(program);
                        break;
                    }
                }
            } else if let Ok(program) = compile_mechanic_program(base) {
                programs.push(program);
            }
        }
        for marker_label in &printed_keywords {
            let input = MechanicOccurrenceInput {
                mechanic: PrintedMechanic::AbilityWord,
                marker_label: Some(marker_label),
                layout: &card.layout,
                printed_keywords: &printed_keywords,
                primary: primary.input(),
                companion: None,
            };
            if let Ok(program) = compile_mechanic_program(input) {
                programs.push(program);
            }
        }
    }
    programs.sort_by_key(|program| (program.primary_address(), program.mechanic()));
    programs.dedup_by(|left, right| {
        left.primary_address() == right.primary_address() && left.mechanic() == right.mechanic()
    });
    programs
}

// Kept explicit because each printed field is independently audited into the profile.
#[allow(clippy::too_many_arguments)]
fn compile_structural_characteristic_profile(
    layout: &str,
    oracle_text: &str,
    type_line: &str,
    mana_value: Option<f32>,
    colors: &[String],
    color_indicator: &[String],
    power: Option<&str>,
    toughness: Option<&str>,
    loyalty: Option<&str>,
    defense: Option<&str>,
    hand_modifier: Option<&str>,
    life_modifier: Option<&str>,
    attraction_lights: &[u8],
) -> StructuralCharacteristicProfile {
    let power = power.and_then(|value| compile_exact_printed_stat_procedure(layout, value));
    let toughness = toughness.and_then(|value| compile_exact_printed_stat_procedure(layout, value));
    let loyalty = loyalty.and_then(compile_exact_loyalty_initialization_procedure);
    StructuralCharacteristicProfile {
        layout: layout.trim().to_ascii_lowercase(),
        type_line: compile_exact_type_line_procedure(type_line),
        mana_value: mana_value.and_then(compile_exact_mana_value_procedure),
        colors: compile_exact_color_set_procedure(colors),
        color_indicator: compile_exact_color_set_procedure(color_indicator),
        power,
        toughness,
        loyalty,
        dynamic_power: power.and_then(|procedure| {
            compile_dynamic_printed_stat_procedure(
                layout,
                oracle_text,
                procedure,
                DynamicCharacteristicSubject::Power,
            )
        }),
        dynamic_toughness: toughness.and_then(|procedure| {
            compile_dynamic_printed_stat_procedure(
                layout,
                oracle_text,
                procedure,
                DynamicCharacteristicSubject::Toughness,
            )
        }),
        dynamic_loyalty: loyalty
            .and_then(|procedure| compile_dynamic_loyalty_procedure(oracle_text, procedure)),
        defense: defense.and_then(compile_exact_defense_initialization_procedure),
        hand_modifier: hand_modifier.and_then(compile_exact_vanguard_modifier_procedure),
        life_modifier: life_modifier.and_then(compile_exact_vanguard_modifier_procedure),
        attraction_lights: compile_exact_attraction_lights_procedure(attraction_lights),
    }
}

fn compile_structural_characteristics(card: &CardDefinition) -> StructuralCharacteristics {
    let root = compile_structural_characteristic_profile(
        &card.layout,
        &card.oracle_text,
        &card.type_line,
        card.root_mana_value,
        &card.colors,
        &card.color_indicator,
        card.power.as_deref(),
        card.toughness.as_deref(),
        card.loyalty.as_deref(),
        card.defense.as_deref(),
        card.hand_modifier.as_deref(),
        card.life_modifier.as_deref(),
        &card.attraction_lights,
    );
    let faces = card
        .faces
        .iter()
        .map(|face| {
            let layout = if face.layout.trim().is_empty() {
                card.layout.as_str()
            } else {
                face.layout.as_str()
            };
            compile_structural_characteristic_profile(
                layout,
                &face.oracle_text,
                &face.type_line,
                face.mana_value,
                &face.colors,
                &face.color_indicator,
                face.power.as_deref(),
                face.toughness.as_deref(),
                face.loyalty.as_deref(),
                face.defense.as_deref(),
                face.hand_modifier.as_deref(),
                face.life_modifier.as_deref(),
                &face.attraction_lights,
            )
        })
        .collect();
    StructuralCharacteristics { root, faces }
}

pub fn compile_effect_descriptor(card: &CardDefinition) -> EffectDescriptor {
    let oracle = normalize_oracle(&card.oracle_text);
    let structural_characteristics = compile_structural_characteristics(card);
    let card_types = structural_characteristics
        .root
        .card_types()
        .unwrap_or_else(|| compile_card_types(&card.type_line));
    let hand_zone = compile_hand_zone_characteristics(card);
    let printed_keywords = compile_printed_keyword_profile(&card.keywords);
    let printed_devotion_pips = exact_primary_face_devotion_pips(card);
    let dynamic_creature_characteristic = compile_dynamic_creature_characteristic(
        &card.name,
        &card.type_line,
        &card.oracle_text,
        card.toughness.as_deref(),
    );
    let characteristic_oracle = compile_characteristic_oracle_programs(
        card,
        card_types,
        printed_keywords,
        dynamic_creature_characteristic,
    );
    let alternative_cast = card.mana_cost.as_deref().and_then(|mana_cost| {
        compile_alternative_cast_runtime(AlternativeCastCardInput {
            layout: &card.layout,
            mana_cost,
            type_line: &card.type_line,
            oracle_text: &card.oracle_text,
        })
    });
    let continuous_faces = card
        .faces
        .iter()
        .map(|face| ContinuousTriggerFaceInput {
            type_line: &face.type_line,
            oracle_text: &face.oracle_text,
        })
        .collect::<Vec<_>>();
    let continuous_triggers = compile_continuous_trigger_runtime(ContinuousTriggerCardInput {
        layout: &card.layout,
        type_line: &card.type_line,
        oracle_text: (!card.oracle_text.trim().is_empty()).then_some(card.oracle_text.as_str()),
        faces: &continuous_faces,
    })
    .unwrap_or_default();
    let object_lifecycle = (card.layout == "normal" && card.faces.is_empty())
        .then(|| {
            compile_object_lifecycle_runtime(ObjectLifecycleCardInput {
                type_line: &card.type_line,
                oracle_text: &card.oracle_text,
            })
        })
        .flatten();
    let utility_modal = compile_utility_modal_runtime(UtilityModalCardInput {
        layout: &card.layout,
        type_line: &card.type_line,
        oracle_text: &card.oracle_text,
    });
    let oracle_syntax = retain_oracle_clause_syntax(card);
    let mut bounded_oracle = compile_bounded_oracle_clauses(card);
    let (additional_bounded, delegated_oracle) =
        compile_additional_oracle_clauses(card, &bounded_oracle);
    bounded_oracle.extend(additional_bounded);
    bounded_oracle.sort_by_key(BoundedOracleClause::address);
    let bounded_oracle_source_roots = compile_bounded_oracle_source_roots(card);
    let mechanic_programs = compile_mechanic_programs(card);
    if oracle.is_empty() {
        return EffectDescriptor {
            card_types,
            hand_zone,
            structural_characteristics,
            printed_keywords,
            printed_devotion_pips,
            dynamic_creature_characteristic,
            alternative_cast,
            characteristic_oracle,
            continuous_triggers,
            object_lifecycle,
            utility_modal,
            bounded_oracle,
            delegated_oracle,
            oracle_syntax,
            bounded_oracle_source_roots,
            mechanic_programs,
            confidence: if card.type_line.to_ascii_lowercase().contains("land") {
                0.96
            } else {
                0.42
            },
            ..Default::default()
        };
    }

    let mut descriptor = EffectDescriptor {
        card_types,
        hand_zone,
        structural_characteristics,
        printed_keywords,
        printed_devotion_pips,
        dynamic_creature_characteristic,
        alternative_cast,
        characteristic_oracle,
        continuous_triggers,
        object_lifecycle,
        utility_modal,
        bounded_oracle,
        delegated_oracle,
        oracle_syntax,
        bounded_oracle_source_roots,
        mechanic_programs,
        repeatable: oracle.contains("whenever ")
            || oracle.contains("at the beginning of ")
            || oracle.contains("{t}:")
            || oracle.contains(": draw ")
            || oracle.contains(": create "),
        conditional: contains_conditional_language(&oracle),
        requires_tap: oracle.contains("{t}"),
        requires_sacrifice: oracle.contains("sacrifice ")
            && (oracle.contains(':') || oracle.contains("as an additional cost")),
        requires_discard: oracle.contains("discard ")
            && (oracle.contains(':') || oracle.contains("as an additional cost")),
        life_cost: LIFE_COST.is_match(&oracle),
        ..Default::default()
    };

    for captures in DRAW_CARDS.captures_iter(&oracle) {
        descriptor.draw_cards = descriptor
            .draw_cards
            .merge(parse_magnitude(captures.get(1).map(|value| value.as_str())));
    }
    if oracle.contains("draw cards equal to")
        || oracle.contains("draw that many cards")
        || oracle.contains("draw x cards")
    {
        descriptor.draw_cards = EffectMagnitude::Dynamic;
    }

    for captures in LOOK_AT_TOP.captures_iter(&oracle) {
        descriptor.impulse_access = descriptor
            .impulse_access
            .merge(parse_magnitude(captures.get(1).map(|value| value.as_str())));
    }
    if oracle.contains("exile the top")
        && (oracle.contains("you may play") || oracle.contains("you may cast"))
    {
        descriptor.impulse_access = descriptor.impulse_access.merge(EffectMagnitude::Fixed(1));
    }

    if oracle.contains("search your library") {
        let tutor_compilation = compile_tutor_descriptor(card, &oracle);
        descriptor.tutor_scope = tutor_compilation.scope;
        descriptor.tutor = tutor_compilation.descriptor;
        descriptor.lands_to_battlefield = tutor_compilation.lands_to_battlefield;
        descriptor
            .unsupported_clauses
            .extend(tutor_compilation.unsupported_clauses);
    }

    descriptor.mana_produced = parse_add_mana(&oracle);
    descriptor.mana_production_kind =
        classify_mana_production(card, &oracle, descriptor.mana_produced);
    if descriptor.mana_production_kind == ManaProductionKind::ReusableActivated {
        descriptor.repeatable = true;
    } else if descriptor.mana_production_kind == ManaProductionKind::NonRefreshingActivated {
        descriptor.repeatable = false;
    }
    for captures in CREATE_TOKENS.captures_iter(&oracle) {
        let magnitude = parse_magnitude(captures.get(1).map(|value| value.as_str()));
        let token_text = captures
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default();
        if token_text.contains("treasure") {
            descriptor.treasure_tokens = descriptor.treasure_tokens.merge(magnitude);
        }
        if token_text.contains("creature") {
            descriptor.creature_tokens = descriptor.creature_tokens.merge(magnitude);
        }
    }
    if oracle.contains("create x ") || oracle.contains("create that many ") {
        if oracle.contains("treasure") {
            descriptor.treasure_tokens = EffectMagnitude::Dynamic;
        }
        if oracle.contains("creature token") {
            descriptor.creature_tokens = EffectMagnitude::Dynamic;
        }
    }

    descriptor.targeted_removal = (oracle.contains("destroy target")
        || oracle.contains("exile target")
        || oracle.contains("return target") && oracle.contains("owner's hand")
        || oracle.contains("target creature gets -"))
        && !oracle.contains("you control");
    descriptor.board_wipe = oracle.contains("destroy all")
        || contains_mass_battlefield_exile(&oracle)
        || oracle.contains("all creatures get -")
        || oracle.contains("each player sacrifices all");
    descriptor.mass_land_denial = is_mass_land_denial(&oracle);
    descriptor.protection = oracle.contains("hexproof")
        || oracle.contains("indestructible")
        || oracle.contains("phase out")
        || oracle.contains("protection from")
        || oracle.contains("counter target spell or ability that targets");
    descriptor.recursion = oracle.contains("graveyard")
        && (oracle.contains("return target")
            || oracle.contains("return up to")
            || oracle.contains("you may cast")
            || oracle.contains("cast target"));
    descriptor.extra_turns = if oracle.contains("take two extra turns") {
        EffectMagnitude::Fixed(2)
    } else if oracle.contains("take an extra turn") || oracle.contains("take one extra turn") {
        EffectMagnitude::Fixed(1)
    } else {
        EffectMagnitude::None
    };

    descriptor
        .unsupported_clauses
        .extend(unsupported_clauses(&oracle));
    descriptor.unsupported_clauses.sort();
    descriptor.unsupported_clauses.dedup();
    if descriptor.mana_production_kind == ManaProductionKind::Unsupported {
        descriptor
            .unsupported_clauses
            .push("Mana activation cost or timing".into());
    }
    let modeled_signal_count = [
        descriptor.draw_cards != EffectMagnitude::None,
        descriptor.impulse_access != EffectMagnitude::None,
        descriptor.tutor_scope != TutorScope::None,
        descriptor.lands_to_battlefield != EffectMagnitude::None,
        descriptor.mana_produced != EffectMagnitude::None,
        descriptor.creature_tokens != EffectMagnitude::None,
        descriptor.treasure_tokens != EffectMagnitude::None,
        descriptor.targeted_removal,
        descriptor.board_wipe,
        descriptor.mass_land_denial,
        descriptor.protection,
        descriptor.recursion,
        descriptor.extra_turns != EffectMagnitude::None,
    ]
    .into_iter()
    .filter(|modeled| *modeled)
    .count();
    let dynamic_count = [
        descriptor.draw_cards,
        descriptor.impulse_access,
        descriptor.lands_to_battlefield,
        descriptor.mana_produced,
        descriptor.creature_tokens,
        descriptor.treasure_tokens,
        descriptor.extra_turns,
    ]
    .iter()
    .filter(|magnitude| matches!(magnitude, EffectMagnitude::Dynamic))
    .count();
    let activation_cost_count = [
        descriptor.requires_tap,
        descriptor.requires_sacrifice,
        descriptor.requires_discard,
        descriptor.life_cost,
    ]
    .into_iter()
    .filter(|required| *required)
    .count();
    let base_confidence = if modeled_signal_count == 0 {
        // A non-empty Oracle box without a supported effect must not be
        // reported as highly modeled merely because it parsed successfully.
        0.54
    } else {
        0.90 + modeled_signal_count.min(2) as f32 * 0.02
    };
    descriptor.confidence = (base_confidence
        - dynamic_count as f32 * 0.055
        - usize::from(descriptor.conditional) as f32 * 0.025
        - usize::from(descriptor.repeatable) as f32 * 0.025
        - activation_cost_count as f32 * 0.0125
        - descriptor.unsupported_clauses.len().min(4) as f32 * 0.075)
        .clamp(0.38, 0.97);
    descriptor
}

fn compile_characteristic_oracle_programs(
    card: &CardDefinition,
    card_types: CardTypeProfile,
    printed_keywords: PrintedKeywordProfile,
    dynamic_characteristic: Option<DynamicCreatureCharacteristic>,
) -> Vec<CompiledCharacteristicOracle> {
    if card.layout != "normal" || !card.faces.is_empty() {
        return Vec::new();
    }
    let mut source_card_types = Vec::new();
    for (present, card_type) in [
        (card_types.is_artifact, OracleSourceCardType::Artifact),
        (card_types.is_battle, OracleSourceCardType::Battle),
        (card_types.is_creature, OracleSourceCardType::Creature),
        (card_types.is_enchantment, OracleSourceCardType::Enchantment),
        (card_types.is_instant, OracleSourceCardType::Instant),
        (card_types.is_kindred, OracleSourceCardType::Kindred),
        (card_types.is_land, OracleSourceCardType::Land),
        (
            card_types.is_planeswalker,
            OracleSourceCardType::Planeswalker,
        ),
        (card_types.is_sorcery, OracleSourceCardType::Sorcery),
    ] {
        if present {
            source_card_types.push(card_type);
        }
    }
    let keyword_pairs = [
        (PrintedKeyword::Deathtouch, OracleCombatKeyword::Deathtouch),
        (
            PrintedKeyword::DoubleStrike,
            OracleCombatKeyword::DoubleStrike,
        ),
        (
            PrintedKeyword::FirstStrike,
            OracleCombatKeyword::FirstStrike,
        ),
        (PrintedKeyword::Flying, OracleCombatKeyword::Flying),
        (PrintedKeyword::Haste, OracleCombatKeyword::Haste),
        (PrintedKeyword::Hexproof, OracleCombatKeyword::Hexproof),
        (
            PrintedKeyword::Indestructible,
            OracleCombatKeyword::Indestructible,
        ),
        (PrintedKeyword::Lifelink, OracleCombatKeyword::Lifelink),
        (PrintedKeyword::Menace, OracleCombatKeyword::Menace),
        (PrintedKeyword::Reach, OracleCombatKeyword::Reach),
        (PrintedKeyword::Shroud, OracleCombatKeyword::Shroud),
        (PrintedKeyword::Trample, OracleCombatKeyword::Trample),
        (PrintedKeyword::Vigilance, OracleCombatKeyword::Vigilance),
        (PrintedKeyword::Defender, OracleCombatKeyword::Defender),
    ];
    let printed_combat_keywords = keyword_pairs
        .iter()
        .filter_map(|(printed, oracle)| printed_keywords.contains(*printed).then_some(*oracle))
        .collect::<Vec<_>>();
    let dynamic_characteristic = dynamic_characteristic.map(|characteristic| {
        let DynamicCreatureCharacteristic::ToughnessEqualsDevotion(color) = characteristic;
        OracleDynamicCharacteristic::ToughnessEqualsDevotion(match color {
            DevotionColor::White => OracleDevotionColor::White,
            DevotionColor::Blue => OracleDevotionColor::Blue,
            DevotionColor::Black => OracleDevotionColor::Black,
            DevotionColor::Red => OracleDevotionColor::Red,
            DevotionColor::Green => OracleDevotionColor::Green,
        })
    });
    let compiled = CompiledCharacteristicView {
        source_card_types,
        printed_combat_keywords: printed_combat_keywords.clone(),
        dynamic_characteristic,
    };
    let mut programs = printed_combat_keywords
        .into_iter()
        .flat_map(|keyword| {
            compile_characteristic_oracle_ownership(CharacteristicOracleInput {
                face_index: 0,
                type_line: &card.type_line,
                oracle_text: &card.oracle_text,
                printed_toughness: card.toughness.as_deref(),
                request: CharacteristicOwnershipRequest::PrintedCombatKeyword(keyword),
                compiled: &compiled,
            })
            .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    if let Some(OracleDynamicCharacteristic::ToughnessEqualsDevotion(color)) =
        compiled.dynamic_characteristic
    {
        programs.extend(
            compile_characteristic_oracle_ownership(CharacteristicOracleInput {
                face_index: 0,
                type_line: &card.type_line,
                oracle_text: &card.oracle_text,
                printed_toughness: card.toughness.as_deref(),
                request: CharacteristicOwnershipRequest::ToughnessEqualsDevotion(color),
                compiled: &compiled,
            })
            .unwrap_or_default(),
        );
    }
    programs.sort_by_key(|program| (program.ownership.face_index, program.ownership.clause_index));
    let mut seen = BTreeSet::new();
    programs.retain(|program| {
        seen.insert((program.ownership.face_index, program.ownership.clause_index))
    });
    programs
}

fn exact_primary_face_devotion_pips(card: &CardDefinition) -> [u16; 5] {
    let layout = card.layout.trim().to_ascii_lowercase();
    let mana_cost = if matches!(
        layout.as_str(),
        "transform"
            | "modal_dfc"
            | "double_faced_token"
            | "reversible_card"
            | "flip"
            | "adventure"
            | "prepare"
    ) {
        card.faces
            .first()
            .and_then(|face| face.mana_cost.as_deref())
    } else {
        card.mana_cost.as_deref()
    };
    let parsed = parse_mana_cost(mana_cost);
    let [face] = parsed.faces.as_slice() else {
        return [0; 5];
    };
    if parsed.confidence < 0.999
        || face.confidence < 0.999
        || face
            .pips
            .iter()
            .any(|pip| pip.is_unknown || pip.is_variable)
    {
        return [0; 5];
    }
    let mut pips = [0u16; 5];
    pips.copy_from_slice(&face.pip_appearances[..5]);
    pips
}

pub(crate) fn compile_dynamic_creature_characteristic(
    card_name: &str,
    type_line: &str,
    oracle_text: &str,
    toughness: Option<&str>,
) -> Option<DynamicCreatureCharacteristic> {
    if !compile_card_types(type_line).is_creature || toughness.map(str::trim) != Some("*") {
        return None;
    }

    let normalized_name = normalize_oracle(card_name);
    let short_name = normalized_name
        .split_once(',')
        .map_or(normalized_name.as_str(), |(short, _)| short)
        .trim();
    let full_possessive = format!("{normalized_name}'s toughness is equal to your devotion to ");
    let short_possessive = format!("{short_name}'s toughness is equal to your devotion to ");
    let prefixes = [
        full_possessive.as_str(),
        short_possessive.as_str(),
        "this creature's toughness is equal to your devotion to ",
        "this permanent's toughness is equal to your devotion to ",
    ];

    let mut matches = oracle_text
        .lines()
        .map(normalize_oracle)
        .filter_map(|clause| {
            let remainder = prefixes
                .iter()
                .find_map(|prefix| clause.strip_prefix(prefix))?;
            let color = [
                DevotionColor::White,
                DevotionColor::Blue,
                DevotionColor::Black,
                DevotionColor::Red,
                DevotionColor::Green,
            ]
            .into_iter()
            .find(|color| {
                remainder == format!("{}.", color.as_name())
                    || remainder
                        == format!(
                            "{}. (each {} in the mana costs of permanents you control counts toward your devotion to {}.)",
                            color.as_name(),
                            color.pip_symbol(),
                            color.as_name()
                        )
            })?;
            Some(DynamicCreatureCharacteristic::ToughnessEqualsDevotion(
                color,
            ))
        });
    let characteristic = matches.next()?;
    matches.next().is_none().then_some(characteristic)
}

#[derive(Debug, Default)]
struct TutorCompilation {
    scope: TutorScope,
    descriptor: TutorDescriptor,
    lands_to_battlefield: EffectMagnitude,
    unsupported_clauses: Vec<String>,
}

pub(crate) fn compile_card_types(type_line: &str) -> CardTypeProfile {
    let normalized = type_line
        .replace(['\u{2014}', '-'], " ")
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let has = |word: &str| normalized.iter().any(|part| part == word);
    CardTypeProfile {
        is_land: has("land"),
        is_basic_land: has("basic") && has("land"),
        is_creature: has("creature"),
        is_artifact: has("artifact"),
        is_battle: has("battle"),
        is_enchantment: has("enchantment"),
        is_instant: has("instant"),
        is_kindred: has("kindred"),
        is_planeswalker: has("planeswalker"),
        is_sorcery: has("sorcery"),
    }
}

fn compile_hand_zone_characteristics(card: &CardDefinition) -> HandZoneCharacteristics {
    let layout = card.layout.trim().to_ascii_lowercase();
    let face = match layout.as_str() {
        // These layouts have one rules-defined front/main face that supplies
        // the physical card's characteristics in hand. Scryfall preserves that
        // face at index zero and supplies face-local colors for these layouts.
        "transform" | "modal_dfc" | "double_faced_token" | "reversible_card" => {
            let Some(front) = card.faces.first() else {
                return HandZoneCharacteristics::default();
            };
            Some((
                front.name.as_str(),
                &front.type_line,
                &front.colors,
                &front.color_indicator,
                front.mana_cost.as_deref(),
                front.oracle_text.as_str(),
                front.keywords.as_slice(),
            ))
        }
        // Scryfall represents the normal/front characteristics of these
        // single-card, multi-frame layouts at face index zero, but currently
        // reports their actual card color at the root rather than per face.
        // `adventure` also covers Omen cards; `prepare` is the newer
        // preparation-card layout.
        "flip" | "adventure" | "prepare" => {
            let Some(normal) = card.faces.first() else {
                return HandZoneCharacteristics::default();
            };
            Some((
                normal.name.as_str(),
                &normal.type_line,
                &card.colors,
                &card.color_indicator,
                normal.mana_cost.as_deref().or(card.mana_cost.as_deref()),
                normal.oracle_text.as_str(),
                normal.keywords.as_slice(),
            ))
        }
        // Split-card characteristics are combined in hand (CR 709.4), which
        // is exactly what Scryfall's root fields contain.
        "split" => None,
        // A new multiface layout must be reviewed before combined root fields
        // can be called exact hand characteristics. This makes future card
        // frames fail closed instead of repeating the MDFC land-back bug.
        _ if !card.faces.is_empty() => return HandZoneCharacteristics::default(),
        _ => None,
    };

    let (card_name, type_line, colors, color_indicator, mana_cost, oracle_text, keywords) = face
        .unwrap_or((
            card.name.as_str(),
            &card.type_line,
            &card.colors,
            &card.color_indicator,
            card.mana_cost.as_deref(),
            card.oracle_text.as_str(),
            card.keywords.as_slice(),
        ));
    let type_line = type_line.trim();
    if type_line.is_empty() {
        return HandZoneCharacteristics::default();
    }

    HandZoneCharacteristics {
        exact: true,
        type_line: type_line.to_string(),
        card_types: compile_card_types(type_line),
        colors: exact_hand_colors(
            card_name,
            colors,
            color_indicator,
            mana_cost,
            oracle_text,
            keywords,
        ),
    }
}

fn oracle_has_self_color_characteristic(
    normalized_oracle: &str,
    card_name: &str,
    characteristic: &str,
) -> bool {
    let normalized_name = normalize_oracle(card_name);
    if normalized_name.is_empty() {
        return false;
    }
    let named_clause = format!("{normalized_name} {characteristic}");
    let this_card_clause = format!("this card {characteristic}");
    normalized_oracle
        .split('.')
        .map(str::trim)
        .any(|clause| clause == named_clause || clause == this_card_clause)
}

/// Recover the rules-defined color characteristic when a retained local
/// record has an exact hand face and mana cost but an empty Scryfall `colors`
/// array. Card color normally follows colored mana symbols and/or a color
/// indicator (CR 105.2); explicit color-setting characteristic abilities such
/// as Devoid or “is colorless” take precedence. This is card color, not color
/// identity, so off-color rules/reminder text is never consulted.
fn exact_hand_colors(
    card_name: &str,
    reported_colors: &[String],
    color_indicator: &[String],
    mana_cost: Option<&str>,
    oracle_text: &str,
    keywords: &[String],
) -> Vec<String> {
    let normalized_oracle = normalize_oracle(oracle_text);
    let explicitly_colorless = keywords
        .iter()
        .any(|keyword| keyword.eq_ignore_ascii_case("Devoid"))
        || oracle_text.lines().any(|line| {
            matches!(
                normalize_oracle(line).as_str(),
                "devoid" | "devoid (this card has no color.)"
            )
        })
        || oracle_has_self_color_characteristic(&normalized_oracle, card_name, "is colorless");
    if explicitly_colorless {
        return Vec::new();
    }

    if oracle_has_self_color_characteristic(&normalized_oracle, card_name, "is all colors") {
        return ["W", "U", "B", "R", "G"]
            .into_iter()
            .map(str::to_string)
            .collect();
    }

    let parsed_cost = parse_mana_cost(mana_cost);
    [
        ("W", ManaColorMask::WHITE),
        ("U", ManaColorMask::BLUE),
        ("B", ManaColorMask::BLACK),
        ("R", ManaColorMask::RED),
        ("G", ManaColorMask::GREEN),
    ]
    .into_iter()
    .filter(|(symbol, color)| {
        reported_colors
            .iter()
            .chain(color_indicator)
            .any(|reported| reported.eq_ignore_ascii_case(symbol))
            || reported_colors.is_empty()
                && color_indicator.is_empty()
                && parsed_cost.colors.intersects(*color)
    })
    .map(|(symbol, _)| symbol.to_string())
    .collect()
}

fn compile_tutor_descriptor(card: &CardDefinition, oracle: &str) -> TutorCompilation {
    let mut result = TutorCompilation {
        descriptor: TutorDescriptor {
            timing: TutorTiming::Unsupported,
            ..Default::default()
        },
        ..Default::default()
    };

    if oracle.matches("search your library").count() != 1 {
        result
            .unsupported_clauses
            .push("Multiple or linked library searches".into());
        return result;
    }

    let reveal_clause_is_simple = [
        "reveal it",
        "reveal that card",
        "reveal the card",
        "reveal those cards",
        "reveal them",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase));
    if oracle.contains("reveal ") && !reveal_clause_is_simple {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }
    if oracle.contains("instead") {
        result
            .unsupported_clauses
            .push("Search replacement effect".into());
    }
    if [
        "at random",
        "an opponent chooses",
        "target opponent chooses",
        "separate those cards",
        "separate them",
        "face down",
        "under an opponent's control",
        "under target opponent's control",
        "choose one or more",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let Some(after_search) = oracle
        .split_once("search your library for ")
        .map(|(_, rest)| rest)
    else {
        result
            .unsupported_clauses
            .push("Search source or target scope".into());
        return result;
    };
    let target_phrase = after_search
        .split([',', '.'])
        .next()
        .unwrap_or_default()
        .trim();
    let Some((target, quantity)) = parse_tutor_target(target_phrase) else {
        result
            .unsupported_clauses
            .push("Search target predicate".into());
        result.unsupported_clauses.sort();
        result.unsupported_clauses.dedup();
        return result;
    };
    result.scope = if target == TutorTarget::AnyCard {
        TutorScope::AnyCard
    } else {
        TutorScope::Restricted
    };

    let reveal = oracle.contains("reveal ");
    let shuffle_after = search_clause_explicitly_shuffles(oracle);
    let life_loss_after_resolution = parse_tutor_resolution_life_loss(oracle);
    if oracle.contains("you lose ") && life_loss_after_resolution.is_none() {
        result
            .unsupported_clauses
            .push("Search-linked life loss".into());
    }
    if reveal
        && ![
            "reveal it",
            "reveal that card",
            "reveal the card",
            "reveal those cards",
            "reveal them",
        ]
        .iter()
        .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let type_line = card.type_line.to_ascii_lowercase();
    let is_spell = type_line.contains("instant") || type_line.contains("sorcery");
    let has_variable_cost = card
        .mana_cost
        .as_deref()
        .is_some_and(|cost| cost.to_ascii_lowercase().contains("{x}"));
    let lacks_payable_printed_cost = card
        .mana_cost
        .as_deref()
        .is_none_or(|cost| cost.trim().is_empty());
    let unsupported_timing_or_cost = !is_spell
        || has_variable_cost
        || lacks_payable_printed_cost
        || contains_conditional_language(oracle)
        || oracle.contains("as an additional cost")
        || oracle.contains("when ")
        || oracle.contains("whenever ")
        || oracle.contains("at the beginning of ")
        || oracle
            .split_once("search your library")
            .is_some_and(|(before, _)| before.contains(':'));
    if unsupported_timing_or_cost {
        result
            .unsupported_clauses
            .push("Search timing or cost".into());
    }

    if oracle.contains("instead") {
        result
            .unsupported_clauses
            .push("Search replacement effect".into());
    }
    if [
        "at random",
        "an opponent chooses",
        "target opponent chooses",
        "separate those cards",
        "separate them",
        "face down",
        "under an opponent's control",
        "under target opponent's control",
        "choose one or more",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let mut instructions = Vec::new();
    if target == TutorTarget::BasicLand
        && quantity == 2
        && oracle.contains("put one onto the battlefield tapped")
        && (oracle.contains("the other into your hand")
            || oracle.contains("another into your hand"))
    {
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination: TutorDestination::BattlefieldTapped,
            quantity: 1,
            reveal,
            shuffle_after: false,
        });
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination: TutorDestination::Hand,
            quantity: 1,
            reveal,
            shuffle_after,
        });
    } else if let Some(destination) = parse_tutor_destination(oracle) {
        if matches!(
            destination,
            TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
        ) && !matches!(target, TutorTarget::BasicLand | TutorTarget::Land)
        {
            result
                .unsupported_clauses
                .push("Search battlefield target execution".into());
        }
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination,
            quantity,
            reveal,
            shuffle_after,
        });
    } else {
        result.unsupported_clauses.push("Search destination".into());
    }

    result.unsupported_clauses.sort();
    result.unsupported_clauses.dedup();
    if result.unsupported_clauses.is_empty() && !instructions.is_empty() {
        result.descriptor = TutorDescriptor {
            timing: TutorTiming::SpellResolution,
            instructions,
            life_loss_after_resolution: life_loss_after_resolution.unwrap_or_default(),
        };
        let land_count = result
            .descriptor
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.target,
                    TutorTarget::BasicLand | TutorTarget::Land
                ) && matches!(
                    instruction.destination,
                    TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
                )
            })
            .fold(0u8, |total, instruction| {
                total.saturating_add(instruction.quantity)
            });
        result.lands_to_battlefield = match land_count {
            0 => EffectMagnitude::None,
            count => EffectMagnitude::Fixed(count),
        };
    }
    result
}

fn parse_tutor_resolution_life_loss(oracle: &str) -> Option<u8> {
    let captures = TUTOR_RESOLUTION_LIFE_LOSS.captures(oracle)?;
    captures
        .get(1)?
        .as_str()
        .parse::<u16>()
        .ok()
        .and_then(|value| u8::try_from(value).ok())
}

fn search_clause_explicitly_shuffles(oracle: &str) -> bool {
    let Some((_, after_search)) = oracle.split_once("search your library") else {
        return false;
    };

    let mut sentences = after_search.split('.');
    let search_sentence = sentences.next().unwrap_or_default();
    if search_sentence.contains("then shuffle") || search_sentence.contains("shuffle your library")
    {
        return true;
    }

    // Some printed wordings put the shuffle in a standalone sentence directly
    // after the search instructions. Do not scan farther: a later shuffle may
    // belong to an unrelated effect or ability.
    let immediately_following_sentence = sentences.next().unwrap_or_default().trim();
    immediately_following_sentence.starts_with("then shuffle")
        || immediately_following_sentence.starts_with("shuffle your library")
}

fn parse_tutor_target(phrase: &str) -> Option<(TutorTarget, u8)> {
    match phrase {
        "a card" | "one card" | "up to one card" => Some((TutorTarget::AnyCard, 1)),
        "a basic land card" | "one basic land card" | "up to one basic land card" => {
            Some((TutorTarget::BasicLand, 1))
        }
        "up to two basic land cards" | "two basic land cards" => Some((TutorTarget::BasicLand, 2)),
        "a land card" | "one land card" | "up to one land card" => Some((TutorTarget::Land, 1)),
        "a creature card" | "one creature card" | "up to one creature card" => {
            Some((TutorTarget::Creature, 1))
        }
        "an artifact card" | "one artifact card" | "up to one artifact card" => {
            Some((TutorTarget::Artifact, 1))
        }
        "an enchantment card" | "one enchantment card" | "up to one enchantment card" => {
            Some((TutorTarget::Enchantment, 1))
        }
        "an instant card" | "one instant card" | "up to one instant card" => {
            Some((TutorTarget::Instant, 1))
        }
        "a sorcery card" | "one sorcery card" | "up to one sorcery card" => {
            Some((TutorTarget::Sorcery, 1))
        }
        "an instant or sorcery card" => Some((TutorTarget::InstantOrSorcery, 1)),
        "an artifact or enchantment card" => Some((TutorTarget::ArtifactOrEnchantment, 1)),
        _ => None,
    }
}

fn parse_tutor_destination(oracle: &str) -> Option<TutorDestination> {
    if [
        "put that card into your hand",
        "put the card into your hand",
        "put it into your hand",
        "put those cards into your hand",
        "put them into your hand",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::Hand)
    } else if [
        "put that card on top of your library",
        "put the card on top of your library",
        "put it on top of your library",
        "put that card on top",
        "put the card on top",
        "put it on top",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::LibraryTop)
    } else if [
        "put that card onto the battlefield tapped",
        "put the card onto the battlefield tapped",
        "put it onto the battlefield tapped",
        "put those cards onto the battlefield tapped",
        "put them onto the battlefield tapped",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::BattlefieldTapped)
    } else if [
        "put that card onto the battlefield",
        "put the card onto the battlefield",
        "put it onto the battlefield",
        "put those cards onto the battlefield",
        "put them onto the battlefield",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::BattlefieldUntapped)
    } else {
        None
    }
}

fn classify_mana_production(
    card: &CardDefinition,
    oracle: &str,
    magnitude: EffectMagnitude,
) -> ManaProductionKind {
    if magnitude == EffectMagnitude::None {
        return ManaProductionKind::None;
    }

    let type_line = card.type_line.to_ascii_lowercase();
    let is_spell = type_line.contains("instant") || type_line.contains("sorcery");
    if is_spell {
        return if spell_mana_resolution_is_supported(oracle, magnitude) {
            ManaProductionKind::SpellResolution
        } else {
            ManaProductionKind::Unsupported
        };
    }

    if reviewed_nonrefreshing_mana_activation(card, oracle, magnitude) {
        return ManaProductionKind::NonRefreshingActivated;
    }

    if oracle.contains("activate only")
        || oracle.contains("spend this mana only")
        || oracle.contains("spend that mana only")
        || oracle.contains("suspend ")
        || has_nonrefreshing_mana_lifecycle(oracle)
        || oracle.contains("would enter the battlefield") && oracle.contains("instead")
    {
        return ManaProductionKind::Unsupported;
    }

    let activation_costs = ADD_MANA_ABILITY
        .captures_iter(oracle)
        .filter_map(|captures| captures.get(1).map(|cost| cost.as_str().trim()))
        .collect::<Vec<_>>();
    if activation_costs.is_empty() {
        // Triggered, replacement, and static mana text needs an event/state
        // model. Do not turn it into an ETB burst or a reusable source.
        return ManaProductionKind::Unsupported;
    }

    let mut kind = None;
    for cost in activation_costs {
        let Some(candidate) = classify_mana_activation_cost(card, cost) else {
            return ManaProductionKind::Unsupported;
        };
        if kind.is_some_and(|current| current != candidate) {
            return ManaProductionKind::Unsupported;
        }
        kind = Some(candidate);
    }
    kind.unwrap_or(ManaProductionKind::Unsupported)
}

fn reviewed_nonrefreshing_mana_activation(
    card: &CardDefinition,
    oracle: &str,
    magnitude: EffectMagnitude,
) -> bool {
    if magnitude != EffectMagnitude::Fixed(3)
        || !card.type_line.trim().eq_ignore_ascii_case("artifact")
    {
        return false;
    }

    let card_name = normalize_rules_name(&card.name);
    let canonical = if card_name.is_empty() || card_name == "this artifact" {
        oracle.to_string()
    } else {
        oracle.replace(&card_name, "this artifact")
    };
    let mut clauses = canonical
        .split('.')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect::<Vec<_>>();
    clauses.sort_unstable();
    let mut expected = vec![
        "this artifact doesn't untap during your untap step",
        "at the beginning of your upkeep, you may pay {4}",
        "if you do, untap this artifact",
        "at the beginning of your draw step, if this artifact is tapped, it deals 1 damage to you",
        "{t}: add {c}{c}{c}",
    ];
    expected.sort_unstable();
    clauses == expected
}

fn has_nonrefreshing_mana_lifecycle(oracle: &str) -> bool {
    oracle.contains("doesn't untap during your untap step")
        || oracle.contains("doesn't untap during its controller's untap step")
        || oracle.contains("doesn't untap during their controller's untap step")
        || oracle.contains("at the beginning of your upkeep") && oracle.contains("untap")
        || oracle.contains("at the beginning of your draw step")
            && oracle.contains("tapped")
            && oracle.contains("damage")
}

fn spell_mana_resolution_is_supported(oracle: &str, magnitude: EffectMagnitude) -> bool {
    if magnitude == EffectMagnitude::Dynamic
        || contains_conditional_language(oracle)
        || oracle.contains("instead")
        || oracle.contains("as an additional cost to cast")
        || oracle.contains("spend this mana only")
        || oracle.contains("spend that mana only")
    {
        return false;
    }

    let before_add = oracle
        .split_once("add ")
        .map(|(before, _)| before)
        .unwrap_or(oracle);
    ![
        "sacrifice ",
        "discard ",
        "exile ",
        "remove ",
        "pay ",
        "reveal ",
    ]
    .iter()
    .any(|cost| before_add.contains(cost))
}

fn classify_mana_activation_cost(
    card: &CardDefinition,
    activation_cost: &str,
) -> Option<ManaProductionKind> {
    let card_name = normalize_rules_name(&card.name);
    let mut requires_tap = false;
    let mut sacrifices_self = false;

    for raw_term in activation_cost.split(',') {
        let term = raw_term.trim();
        if term == "{t}" {
            requires_tap = true;
            continue;
        }
        if term == format!("sacrifice {card_name}")
            || matches!(
                term,
                "sacrifice this artifact"
                    | "sacrifice this creature"
                    | "sacrifice this permanent"
                    | "sacrifice this token"
            )
        {
            sacrifices_self = true;
            continue;
        }
        return None;
    }

    if sacrifices_self {
        let is_creature = card.type_line.to_ascii_lowercase().contains("creature");
        if requires_tap && is_creature {
            // The source would normally have summoning sickness when the
            // simulator casts it. A future activation needs staged object
            // state, so it receives no trajectory credit yet.
            None
        } else {
            Some(ManaProductionKind::OneShotActivated)
        }
    } else if requires_tap {
        Some(ManaProductionKind::ReusableActivated)
    } else {
        None
    }
}

fn normalize_rules_name(name: &str) -> String {
    name.replace('’', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn parse_add_mana(oracle: &str) -> EffectMagnitude {
    if oracle.contains("add an amount of")
        || oracle.contains("add x mana")
        || oracle.contains("for each")
            && oracle
                .split('.')
                .any(|clause| clause.contains("add ") && clause.contains("for each"))
    {
        return EffectMagnitude::Dynamic;
    }
    let mut total = EffectMagnitude::None;
    for captures in ADD_MANA.captures_iter(oracle) {
        let output = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let symbol_count = MANA_SYMBOL.find_iter(output).count() as u8;
        let magnitude = if symbol_count > 0 {
            EffectMagnitude::Fixed(symbol_count.min(12))
        } else if output.starts_with("one ") || output.starts_with("a ") {
            EffectMagnitude::Fixed(1)
        } else if output.starts_with("two ") {
            EffectMagnitude::Fixed(2)
        } else if output.starts_with("three ") {
            EffectMagnitude::Fixed(3)
        } else {
            EffectMagnitude::Dynamic
        };
        total = total.merge(magnitude);
    }
    total
}

fn parse_magnitude(raw: Option<&str>) -> EffectMagnitude {
    match raw.unwrap_or_default().trim() {
        "a" | "an" | "one" => EffectMagnitude::Fixed(1),
        "two" => EffectMagnitude::Fixed(2),
        "three" => EffectMagnitude::Fixed(3),
        "four" => EffectMagnitude::Fixed(4),
        "five" => EffectMagnitude::Fixed(5),
        "six" => EffectMagnitude::Fixed(6),
        "x" | "that many" | "a number of" | "any number of" => EffectMagnitude::Dynamic,
        value => value
            .parse::<u8>()
            .ok()
            .map(|value| EffectMagnitude::Fixed(value.min(12)))
            .unwrap_or(EffectMagnitude::Dynamic),
    }
}

fn unsupported_clauses(oracle: &str) -> Vec<String> {
    const COMPLEX_SIGNALS: [(&str, &str); 21] = [
        ("copy target", "Spell or ability copying"),
        ("copy that spell", "Spell copying"),
        ("instead", "Replacement effect"),
        ("choose one or more", "Multi-mode choice"),
        ("vote", "Voting"),
        ("venture into the dungeon", "Dungeon progression"),
        ("take the initiative", "Initiative progression"),
        ("daybound", "Day/night state"),
        ("mutate", "Mutate pile"),
        ("suspend", "Suspend timing"),
        ("cascade", "Cascade resolution"),
        ("discover ", "Discover resolution"),
        ("you win the game", "Alternate-win condition"),
        ("loses the game", "Player-loss condition"),
        ("opponent loses ", "Opponent life-total pressure"),
        ("opponents lose ", "Opponent life-total pressure"),
        ("player loses ", "Player life-total pressure"),
        ("players lose ", "Player life-total pressure"),
        ("damage to each opponent", "Opponent life-total pressure"),
        ("damage to target opponent", "Opponent life-total pressure"),
        (" mill", "Library depletion"),
    ];
    let mut clauses = Vec::new();
    for (signal, label) in COMPLEX_SIGNALS {
        if oracle.contains(signal) && !clauses.iter().any(|existing| existing == label) {
            clauses.push(label.to_string());
        }
    }
    clauses
}

fn normalize_oracle(text: &str) -> String {
    text.replace('’', "'")
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn contains_mass_battlefield_exile(oracle: &str) -> bool {
    const BATTLEFIELD_OBJECT_WORDS: [&str; 16] = [
        "artifact",
        "artifacts",
        "battle",
        "battles",
        "creature",
        "creatures",
        "enchantment",
        "enchantments",
        "land",
        "lands",
        "permanent",
        "permanents",
        "planeswalker",
        "planeswalkers",
        "token",
        "tokens",
    ];

    oracle.match_indices("exile all").any(|(offset, _)| {
        let clause = oracle[offset..]
            .split(['.', ',', ';', '\n'])
            .next()
            .unwrap_or_default();
        let words = clause
            .split(|character: char| !character.is_ascii_alphabetic())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        !words.contains(&"card")
            && !words.contains(&"cards")
            && BATTLEFIELD_OBJECT_WORDS
                .iter()
                .any(|object| words.contains(object))
    })
}

fn contains_conditional_language(oracle: &str) -> bool {
    oracle.contains(" if ")
        || oracle.contains(" unless ")
        || oracle.contains(" only if ")
        || oracle.contains(" for each ")
        || oracle.contains(" equal to ")
        || oracle.contains(" where x ")
        || oracle.contains(" whenever ")
}

fn is_mass_land_denial(oracle: &str) -> bool {
    MASS_LAND_RESET.is_match(oracle)
        || MASS_LAND_SACRIFICE.is_match(oracle)
        || oracle.contains("each player chooses a land") && oracle.contains("sacrifices the rest")
        || oracle.contains("lands don't untap during their controllers' untap steps")
        || oracle.contains("players can't untap more than one land during their untap steps")
        || oracle.contains("players can't untap more than two permanents during their untap steps")
        || NONBASIC_LAND_REWRITE.is_match(oracle)
        || oracle.contains("if a land is tapped for mana, it produces")
            && oracle.contains("instead of any other type")
}

static DRAW_CARDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bdraw (a|an|one|two|three|four|five|six|[0-9]+|x|that many) cards?\b")
        .expect("draw regex")
});
static LOOK_AT_TOP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\blook at the top (one|two|three|four|five|six|[0-9]+|x) cards?\b")
        .expect("selection regex")
});
static CREATE_TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bcreate (a|an|one|two|three|four|five|six|[0-9]+|x|that many) ([^.]{0,100}?tokens?)\b",
    )
    .expect("token regex")
});
static ADD_MANA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\badd ([^.;\n]{1,80})").expect("mana regex"));
static ADD_MANA_ABILITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[.;])\s*([^.;]{1,180}?):\s*add\b").expect("mana activation regex")
});
static MANA_SYMBOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?:w|u|b|r|g|c|[0-9]+)\}").expect("mana symbol regex"));
static LIFE_COST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pay|lose) (?:[0-9]+|x|that much) life").expect("life regex"));
static TUTOR_RESOLUTION_LIFE_LOSS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\. )you lose ([0-9]+) life\.$").expect("tutor life-loss regex")
});
static MASS_LAND_RESET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:destroy|exile|return) all [^.]{0,100}\blands?\b|\b(?:destroy|exile|return) [^.]{0,100}\ball lands?\b",
    )
    .expect("mass land reset regex")
});
static MASS_LAND_SACRIFICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\beach player sacrifices (?:all|x|four|five|six|seven|eight|nine|ten|[4-9]|[1-9][0-9]+) lands?\b",
    )
    .expect("mass land sacrifice regex")
});
static NONBASIC_LAND_REWRITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bnonbasic lands (?:are|become) (?:plains|islands|swamps|mountains|forests)(?:\.| |$)",
    )
    .expect("nonbasic land rewrite regex")
});
