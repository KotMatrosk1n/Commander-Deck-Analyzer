//! Fail-closed execution-coverage contracts.
//!
//! This module does not promote the heuristic semantic model to an executable
//! rules engine. It provides a lossless, versioned preflight manifest and can
//! promote an Oracle or atomicity leaf only when the live runtime emits an
//! exact, versioned capability receipt for the complete retained card
//! revision. Current combined card records are split only where their stored
//! separators make that safe; any missing face relationship, cost attribution,
//! Oracle clause, keyword, component, or executor remains explicitly blocking
//! for functional metrics.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ability_program::{
    EXECUTABLE_ABILITY_PROGRAM_VERSION, OracleCardFaceInput, OracleCardInput,
    compile_executable_ability_program, compile_face_bound_ability_program,
    normalize_oracle_clause_for_receipt,
};
use crate::mana_network_runtime::{
    COMMANDER_IDENTITY_MANA_EXECUTOR_ID, CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID,
    CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID, GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID,
    SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID,
};
use crate::runtime_receipts::{
    ALTERNATIVE_CAST_EXECUTOR_VERSION, ATOMIC_TRANSACTION_EXECUTOR_VERSION, AtomicRuntimeReceipt,
    CHARACTERISTIC_EXECUTOR_VERSION, CHARACTERISTIC_ORACLE_EXECUTOR_VERSION,
    CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION, CONTINUOUS_TRIGGER_EXECUTOR_VERSION,
    CharacteristicFaceBinding, CharacteristicFaceInput, CharacteristicRootAlignment,
    CharacteristicRuntimeReceipt, CharacteristicSubject, ConditionalManaSourceRuntimeReceipt,
    GRAVEYARD_RECLAMATION_EXECUTOR_VERSION, GraveyardReclamationRuntimeReceipt,
    INTERACTION_RUNTIME_EXECUTOR_VERSION, InteractionRuntimeReceipt, LAND_RUNTIME_EXECUTOR_VERSION,
    LIVE_ABILITY_EXECUTOR_VERSION, LandRuntimeReceipt, LiveAbilityRuntimeReceipt, LiveAbilityShape,
    MANA_NETWORK_RUNTIME_EXECUTOR_VERSION, OBJECT_LIFECYCLE_EXECUTOR_VERSION,
    RESTRICTION_PROTECTION_EXECUTOR_VERSION, RUNTIME_RECEIPT_SCHEMA_VERSION,
    RestrictionProtectionRuntimeReceipt, ReviewedRuntimeReceipt, RuntimeCapability,
    RuntimeExecutorBinding, RuntimeOracleClauseEvidence, RuntimeSourceEvidence,
    SACRIFICE_SELF_MANA_EXECUTOR_VERSION, SPELL_RESOLUTION_MANA_EXECUTOR_VERSION,
    SacrificeSelfManaRuntimeReceipt, SpellResolutionManaRuntimeReceipt,
    TUTOR_RUNTIME_EXECUTOR_VERSION, TutorRuntimeReceipt, TypedAtomicTransaction,
    UTILITY_MODAL_EXECUTOR_VERSION, compile_atomic_runtime_receipt,
    compile_characteristic_runtime_receipts, compile_conditional_mana_source_runtime_receipt,
    compile_graveyard_reclamation_runtime_receipt, compile_interaction_runtime_receipt,
    compile_land_runtime_receipts, compile_live_ability_runtime_receipts,
    compile_restriction_protection_runtime_receipt, compile_reviewed_runtime_receipts,
    compile_sacrifice_self_mana_runtime_receipt, compile_spell_resolution_mana_runtime_receipt,
    compile_tutor_runtime_receipt,
};
use crate::semantics::{CompiledCard, role};

pub const EXECUTION_COVERAGE_SCHEMA_VERSION: &str = "commander-execution-coverage-manifest/v7";
pub const EXECUTION_COVERAGE_COMPILER_VERSION: &str = "execution-coverage-0.9";
pub const COMPACT_BLOCKER_SAMPLE_LIMIT: usize = 20;

const METRICS: [ExecutionMetric; 7] = [
    ExecutionMetric::RawOpeningComposition,
    ExecutionMetric::FunctionalMulligan,
    ExecutionMetric::ManaConsistency,
    ExecutionMetric::GoldfishTiming,
    ExecutionMetric::InterferenceTiming,
    ExecutionMetric::SynergyDescription,
    ExecutionMetric::BracketRating,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionMetric {
    /// Physical sampling only. No card function or pilot evaluation executes.
    RawOpeningComposition,
    /// A keep/bottom decision that uses what cards can actually do.
    FunctionalMulligan,
    /// Castability and source reliability, including all costs and restrictions.
    ManaConsistency,
    /// Unopposed game-state execution and timing.
    GoldfishTiming,
    /// Game-state execution with opponent actions or state.
    InterferenceTiming,
    /// Descriptive, non-executable relationship reporting.
    SynergyDescription,
    /// Any numeric bracket recommendation that consumes functional evidence.
    BracketRating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutorBinding {
    pub receipt_schema_version: String,
    pub executor_id: String,
    pub executor_version: String,
    pub ability_program_version: String,
    pub runtime_source_evidence_sha256: String,
    pub normalized_oracle_sha256: String,
    pub normalized_oracle_clause_sha256s: Vec<String>,
    pub covered_oracle_clauses: Vec<OracleClauseBinding>,
    pub type_line_sha256: String,
    pub relevant_type_role_mask: u32,
    pub card_revision_sha256: String,
    pub leaf_evidence_sha256: String,
    pub rule_dependencies: Vec<String>,
    pub evidence_tests: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleClauseBinding {
    pub face_index: u16,
    pub clause_index: u16,
    pub normalized_clause_sha256: String,
}

impl From<&RuntimeOracleClauseEvidence> for OracleClauseBinding {
    fn from(value: &RuntimeOracleClauseEvidence) -> Self {
        Self {
            face_index: value.face_index,
            clause_index: value.clause_index,
            normalized_clause_sha256: value.normalized_clause_sha256.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IrrelevanceProof {
    pub proof_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageBlocker {
    pub blocker_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail", rename_all = "camelCase")]
pub enum CoverageDisposition {
    FullyExecutable { binding: Box<ExecutorBinding> },
    SafelyIrrelevant { proof: IrrelevanceProof },
    ReportOnly { reason: String },
    BlockingUnsupported { blocker: CoverageBlocker },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDisposition {
    pub metric: ExecutionMetric,
    pub disposition: CoverageDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverageLeafKind {
    OracleRulesText,
    OracleFormatting,
    CombinedFaceDelimiter,
    FaceStructure,
    PrintedCharacteristics,
    PrintedManaCost,
    KeywordAbility,
    RelatedComponent,
    AtomicityGuard,
    SourceSchemaCompleteness,
    UnreviewedUpstreamField,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum CoverageLeafSubject {
    SourceSchemaCompleteness,
    UnreviewedField(String),
    OracleRulesText,
    OracleFormatting,
    CombinedFaceDelimiter,
    FaceRelationship,
    ManaValue,
    TypeLine,
    Colors,
    ColorIndicator,
    ColorIdentity,
    ProducedMana,
    Power,
    Toughness,
    Loyalty,
    Defense,
    HandModifier,
    LifeModifier,
    AttractionLights,
    ManaCost,
    Keyword(String),
    RelatedComponent(String),
    AtomicityGuard,
    CommanderLegality,
    LegacyCommanderLegality,
    GameChanger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OracleSourceSpanKind {
    RulesText,
    Formatting,
    CombinedFaceDelimiter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OracleSourceReference {
    TopLevelCard,
    ExactFace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleSourceSpan {
    pub span_index: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub text: String,
    pub kind: OracleSourceSpanKind,
    pub source_reference: OracleSourceReference,
    pub face_index: Option<u16>,
    pub leaf_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionCoverageLeaf {
    pub leaf_id: String,
    pub kind: CoverageLeafKind,
    pub subject: CoverageLeafSubject,
    pub face_index: Option<u16>,
    pub source_span_indices: Vec<u32>,
    pub evidence_sha256: String,
    pub metric_dispositions: Vec<MetricDisposition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FaceRelationship {
    SingleFace,
    SplitHalf,
    TransformFront,
    TransformBack,
    AdventurePermanent,
    AdventureSpell,
    ModalDoubleFacedFront,
    ModalDoubleFacedBack,
    FlipFront,
    FlipBack,
    ReversibleFront,
    ReversibleBack,
    DoubleFacedTokenFront,
    DoubleFacedTokenBack,
    MeldCard,
    LegacySingleFaceLayoutUnknown,
    LegacyCombinedMultifaceRelationshipUnknown,
    UnsupportedLayoutFace,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceCoverageManifest {
    pub face_index: u16,
    pub oracle_id: Option<String>,
    pub layout: String,
    pub name: Option<String>,
    pub mana_value: Option<f32>,
    pub type_line: Option<String>,
    pub mana_cost: Option<String>,
    pub oracle_source: String,
    pub oracle_source_sha256: String,
    pub colors: Vec<String>,
    pub color_indicator: Vec<String>,
    pub keywords: Vec<String>,
    pub produced_mana: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub hand_modifier: Option<String>,
    pub life_modifier: Option<String>,
    pub attraction_lights: Vec<u8>,
    pub image_uri: Option<String>,
    pub relationship: FaceRelationship,
    pub oracle_span_indices: Vec<u32>,
    pub leaf_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardCoverageManifest {
    pub card_id: String,
    pub name: String,
    pub normalized_name: String,
    pub combined_type_line: String,
    pub combined_mana_cost: Option<String>,
    pub keywords: Vec<String>,
    pub layout: String,
    pub oracle_source: String,
    pub oracle_revision_sha256: String,
    pub source_record: CombinedCardRecord,
    pub multiface: bool,
    pub face_alignment_complete: bool,
    pub faces: Vec<FaceCoverageManifest>,
    pub oracle_spans: Vec<OracleSourceSpan>,
    pub leaves: Vec<ExecutionCoverageLeaf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageLeafReference {
    pub card_id: String,
    pub card_name: String,
    pub face_index: Option<u16>,
    pub leaf_id: String,
    pub blocker: CoverageBlocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricGateState {
    Executable,
    ReportOnly,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricCoverageGate {
    pub metric: ExecutionMetric,
    pub state: MetricGateState,
    pub fully_executable_leaf_count: u32,
    pub safely_irrelevant_leaf_count: u32,
    pub report_only_leaf_count: u32,
    pub blocking_leaf_count: u32,
    pub blockers: Vec<CoverageLeafReference>,
}

impl MetricCoverageGate {
    pub fn can_execute(&self) -> bool {
        self.state == MetricGateState::Executable
    }

    pub fn can_report(&self) -> bool {
        self.state != MetricGateState::Blocked
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageSnapshotProvenance {
    pub card_snapshot_sha256: Option<String>,
    pub comprehensive_rules_snapshot_sha256: Option<String>,
    pub comprehensive_rules_effective_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageManifestSummary {
    pub card_count: u32,
    pub face_count: u32,
    pub oracle_span_count: u32,
    pub leaf_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionCoverageManifest {
    pub schema_version: String,
    pub compiler_version: String,
    pub provenance: CoverageSnapshotProvenance,
    pub cards: Vec<CardCoverageManifest>,
    pub gates: Vec<MetricCoverageGate>,
    pub summary: CoverageManifestSummary,
    pub fingerprint_sha256: String,
}

/// Bounded report/cache projection. The complete card/leaf manifest is used
/// internally for preflight and is integrity-addressed by `fingerprint_sha256`;
/// it is deliberately not serialized into every analysis report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactExecutionCoverageManifest {
    pub schema_version: String,
    pub compiler_version: String,
    pub provenance: CoverageSnapshotProvenance,
    pub gates: Vec<CompactMetricCoverageGate>,
    pub summary: CoverageManifestSummary,
    /// SHA-256 of the complete lossless manifest used for preflight.
    pub fingerprint_sha256: String,
    /// SHA-256 of every field in this projection except this digest.
    pub projection_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactMetricCoverageGate {
    pub metric: ExecutionMetric,
    pub state: MetricGateState,
    pub fully_executable_leaf_count: u32,
    pub safely_irrelevant_leaf_count: u32,
    pub report_only_leaf_count: u32,
    pub blocking_leaf_count: u32,
    /// Deterministic prefix of the complete blocker list.
    pub blockers: Vec<CoverageLeafReference>,
    pub blocker_sample_truncated: bool,
    pub blocker_sample_limit: u16,
}

impl CompactMetricCoverageGate {
    pub fn can_execute(&self) -> bool {
        self.state == MetricGateState::Executable
    }

    pub fn can_report(&self) -> bool {
        self.state != MetricGateState::Blocked
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageFaceRecord {
    pub oracle_id: Option<String>,
    pub layout: String,
    pub name: String,
    pub mana_value: Option<f32>,
    pub mana_cost: Option<String>,
    pub type_line: String,
    pub oracle_text: String,
    pub colors: Vec<String>,
    pub color_indicator: Vec<String>,
    pub keywords: Vec<String>,
    pub produced_mana: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub hand_modifier: Option<String>,
    pub life_modifier: Option<String>,
    pub attraction_lights: Vec<u8>,
    pub image_uri: Option<String>,
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRelatedComponentRecord {
    pub id: String,
    pub component: String,
    pub name: String,
    pub type_line: String,
    pub uri: Option<String>,
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedCardRecord {
    pub oracle_id: Option<String>,
    pub name: String,
    pub normalized_name: String,
    pub layout: String,
    pub root_mana_value: Option<f32>,
    pub mana_value: f32,
    pub mana_cost: Option<String>,
    pub type_line: String,
    pub oracle_text: String,
    pub colors: Vec<String>,
    pub color_indicator: Vec<String>,
    pub color_identity: Vec<String>,
    pub keywords: Vec<String>,
    pub produced_mana: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub hand_modifier: Option<String>,
    pub life_modifier: Option<String>,
    pub attraction_lights: Vec<u8>,
    pub faces: Vec<CoverageFaceRecord>,
    pub related_components: Vec<CoverageRelatedComponentRecord>,
    pub game_changer: Option<bool>,
    pub commander_legality: Option<String>,
    pub legal_commander: bool,
    pub unreviewed_fields: BTreeMap<String, serde_json::Value>,
    pub source_schema_version: String,
}

impl From<&crate::domain::CardDefinition> for CombinedCardRecord {
    fn from(card: &crate::domain::CardDefinition) -> Self {
        Self {
            oracle_id: card.oracle_id.clone(),
            name: card.name.clone(),
            normalized_name: card.normalized_name.clone(),
            layout: card.layout.clone(),
            root_mana_value: card.root_mana_value,
            mana_value: card.mana_value,
            mana_cost: card.mana_cost.clone(),
            type_line: card.type_line.clone(),
            oracle_text: card.oracle_text.clone(),
            colors: card.colors.clone(),
            color_indicator: card.color_indicator.clone(),
            color_identity: card.color_identity.clone(),
            keywords: card.keywords.clone(),
            produced_mana: card.produced_mana.clone(),
            power: card.power.clone(),
            toughness: card.toughness.clone(),
            loyalty: card.loyalty.clone(),
            defense: card.defense.clone(),
            hand_modifier: card.hand_modifier.clone(),
            life_modifier: card.life_modifier.clone(),
            attraction_lights: card.attraction_lights.clone(),
            faces: card
                .faces
                .iter()
                .map(|face| CoverageFaceRecord {
                    oracle_id: face.oracle_id.clone(),
                    layout: face.layout.clone(),
                    name: face.name.clone(),
                    mana_value: face.mana_value,
                    mana_cost: face.mana_cost.clone(),
                    type_line: face.type_line.clone(),
                    oracle_text: face.oracle_text.clone(),
                    colors: face.colors.clone(),
                    color_indicator: face.color_indicator.clone(),
                    keywords: face.keywords.clone(),
                    produced_mana: face.produced_mana.clone(),
                    power: face.power.clone(),
                    toughness: face.toughness.clone(),
                    loyalty: face.loyalty.clone(),
                    defense: face.defense.clone(),
                    hand_modifier: face.hand_modifier.clone(),
                    life_modifier: face.life_modifier.clone(),
                    attraction_lights: face.attraction_lights.clone(),
                    image_uri: face.image_uri.clone(),
                    unreviewed_fields: face.unreviewed_fields.clone(),
                })
                .collect(),
            related_components: card
                .related_components
                .iter()
                .map(|component| CoverageRelatedComponentRecord {
                    id: component.id.clone(),
                    component: component.component.clone(),
                    name: component.name.clone(),
                    type_line: component.type_line.clone(),
                    uri: component.uri.clone(),
                    unreviewed_fields: component.unreviewed_fields.clone(),
                })
                .collect(),
            game_changer: card.game_changer,
            commander_legality: card.commander_legality.clone(),
            legal_commander: card.legal_commander,
            unreviewed_fields: card.unreviewed_fields.clone(),
            source_schema_version: card.source_schema_version.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CoverageManifestError {
    #[error("unsupported execution coverage schema `{0}`")]
    UnsupportedSchema(String),
    #[error("unsupported execution coverage compiler `{0}`")]
    UnsupportedCompiler(String),
    #[error("execution coverage contains duplicate card id `{0}`")]
    DuplicateCardId(String),
    #[error("execution coverage for `{card}` has an invalid Oracle partition: {detail}")]
    InvalidOraclePartition { card: String, detail: String },
    #[error("execution coverage for `{card}` has an invalid internal reference: {detail}")]
    InvalidReference { card: String, detail: String },
    #[error("execution coverage metric gates do not match their leaves")]
    GateMismatch,
    #[error("execution coverage summary does not match its contents")]
    SummaryMismatch,
    #[error("execution coverage fingerprint does not match its contents")]
    FingerprintMismatch,
    #[error("compact execution coverage projection is invalid: {0}")]
    InvalidCompactProjection(String),
    #[error("compact execution coverage projection fingerprint does not match its contents")]
    ProjectionFingerprintMismatch,
    #[error("execution coverage could not be serialized deterministically: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardRevisionMaterial<'a> {
    card_id: &'a str,
    source_record: &'a CombinedCardRecord,
    canonical_keywords: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFingerprintMaterial<'a> {
    schema_version: &'a str,
    compiler_version: &'a str,
    provenance: &'a CoverageSnapshotProvenance,
    cards: &'a [CardCoverageManifest],
    gates: &'a [MetricCoverageGate],
    summary: &'a CoverageManifestSummary,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompactProjectionFingerprintMaterial<'a> {
    schema_version: &'a str,
    compiler_version: &'a str,
    provenance: &'a CoverageSnapshotProvenance,
    gates: &'a [CompactMetricCoverageGate],
    summary: &'a CoverageManifestSummary,
    fingerprint_sha256: &'a str,
}

pub fn build_execution_coverage_manifest(
    provenance: CoverageSnapshotProvenance,
    records: &[CombinedCardRecord],
) -> Result<ExecutionCoverageManifest, CoverageManifestError> {
    let mut sorted_records = records.to_vec();
    sorted_records.sort_by(|left, right| {
        card_id(left)
            .cmp(&card_id(right))
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut seen_ids = BTreeSet::new();
    let mut cards = Vec::with_capacity(sorted_records.len());
    for record in &sorted_records {
        let id = card_id(record);
        if !seen_ids.insert(id.clone()) {
            return Err(CoverageManifestError::DuplicateCardId(id));
        }
        cards.push(compile_card(record)?);
    }

    let gates = build_metric_gates(&cards);
    let summary = manifest_summary(&cards);
    let fingerprint_sha256 = fingerprint_for_parts(
        EXECUTION_COVERAGE_SCHEMA_VERSION,
        EXECUTION_COVERAGE_COMPILER_VERSION,
        &provenance,
        &cards,
        &gates,
        &summary,
    )?;
    let manifest = ExecutionCoverageManifest {
        schema_version: EXECUTION_COVERAGE_SCHEMA_VERSION.into(),
        compiler_version: EXECUTION_COVERAGE_COMPILER_VERSION.into(),
        provenance,
        cards,
        gates,
        summary,
        fingerprint_sha256,
    };
    manifest.validate()?;
    Ok(manifest)
}

impl ExecutionCoverageManifest {
    pub fn gate_for(&self, metric: ExecutionMetric) -> Option<&MetricCoverageGate> {
        self.gates.iter().find(|gate| gate.metric == metric)
    }

    pub fn validate(&self) -> Result<(), CoverageManifestError> {
        if self.schema_version != EXECUTION_COVERAGE_SCHEMA_VERSION {
            return Err(CoverageManifestError::UnsupportedSchema(
                self.schema_version.clone(),
            ));
        }
        if self.compiler_version != EXECUTION_COVERAGE_COMPILER_VERSION {
            return Err(CoverageManifestError::UnsupportedCompiler(
                self.compiler_version.clone(),
            ));
        }
        let mut previous_card_id = None::<&str>;
        for card in &self.cards {
            if previous_card_id.is_some_and(|previous| previous >= card.card_id.as_str()) {
                return Err(CoverageManifestError::InvalidReference {
                    card: card.card_id.clone(),
                    detail: "card ids must be unique and strictly sorted".into(),
                });
            }
            previous_card_id = Some(&card.card_id);
            validate_card(card)?;
            let expected = compile_card(&card.source_record)?;
            if card != &expected {
                return Err(CoverageManifestError::InvalidReference {
                    card: card.card_id.clone(),
                    detail: "compiled leaves do not deterministically match the complete retained source record".into(),
                });
            }
        }

        let expected_gates = build_metric_gates(&self.cards);
        if self.gates != expected_gates {
            return Err(CoverageManifestError::GateMismatch);
        }
        if self.summary != manifest_summary(&self.cards) {
            return Err(CoverageManifestError::SummaryMismatch);
        }

        let expected_fingerprint = fingerprint_for_parts(
            &self.schema_version,
            &self.compiler_version,
            &self.provenance,
            &self.cards,
            &self.gates,
            &self.summary,
        )?;
        if self.fingerprint_sha256 != expected_fingerprint {
            return Err(CoverageManifestError::FingerprintMismatch);
        }
        Ok(())
    }

    pub fn compact_projection(
        &self,
    ) -> Result<CompactExecutionCoverageManifest, CoverageManifestError> {
        self.validate()?;
        let gates = self
            .gates
            .iter()
            .map(|gate| {
                let blockers = gate
                    .blockers
                    .iter()
                    .take(COMPACT_BLOCKER_SAMPLE_LIMIT)
                    .cloned()
                    .collect::<Vec<_>>();
                CompactMetricCoverageGate {
                    metric: gate.metric,
                    state: gate.state,
                    fully_executable_leaf_count: gate.fully_executable_leaf_count,
                    safely_irrelevant_leaf_count: gate.safely_irrelevant_leaf_count,
                    report_only_leaf_count: gate.report_only_leaf_count,
                    blocking_leaf_count: gate.blocking_leaf_count,
                    blocker_sample_truncated: gate.blockers.len() > blockers.len(),
                    blocker_sample_limit: COMPACT_BLOCKER_SAMPLE_LIMIT as u16,
                    blockers,
                }
            })
            .collect::<Vec<_>>();
        let mut compact = CompactExecutionCoverageManifest {
            schema_version: self.schema_version.clone(),
            compiler_version: self.compiler_version.clone(),
            provenance: self.provenance.clone(),
            gates,
            summary: self.summary.clone(),
            fingerprint_sha256: self.fingerprint_sha256.clone(),
            projection_sha256: String::new(),
        };
        compact.projection_sha256 = compact_projection_fingerprint(&compact)?;
        compact.validate()?;
        Ok(compact)
    }
}

impl CompactExecutionCoverageManifest {
    pub fn gate_for(&self, metric: ExecutionMetric) -> Option<&CompactMetricCoverageGate> {
        self.gates.iter().find(|gate| gate.metric == metric)
    }

    pub fn validate(&self) -> Result<(), CoverageManifestError> {
        if self.schema_version != EXECUTION_COVERAGE_SCHEMA_VERSION {
            return Err(CoverageManifestError::UnsupportedSchema(
                self.schema_version.clone(),
            ));
        }
        if self.compiler_version != EXECUTION_COVERAGE_COMPILER_VERSION {
            return Err(CoverageManifestError::UnsupportedCompiler(
                self.compiler_version.clone(),
            ));
        }
        if !is_sha256_hex(&self.fingerprint_sha256) {
            return Err(CoverageManifestError::InvalidCompactProjection(
                "full-manifest fingerprint is not a SHA-256 digest".into(),
            ));
        }
        if self.gates.len() != METRICS.len() {
            return Err(CoverageManifestError::InvalidCompactProjection(
                "every metric must appear exactly once".into(),
            ));
        }
        for (gate, expected_metric) in self.gates.iter().zip(METRICS) {
            if gate.metric != expected_metric {
                return Err(CoverageManifestError::InvalidCompactProjection(
                    "metric gates are not in canonical order".into(),
                ));
            }
            let covered_leaf_count = gate
                .fully_executable_leaf_count
                .saturating_add(gate.safely_irrelevant_leaf_count)
                .saturating_add(gate.report_only_leaf_count)
                .saturating_add(gate.blocking_leaf_count);
            if covered_leaf_count != self.summary.leaf_count {
                return Err(CoverageManifestError::InvalidCompactProjection(
                    "gate disposition counts do not equal the exact manifest leaf count".into(),
                ));
            }
            let expected_state = if gate.blocking_leaf_count > 0 {
                MetricGateState::Blocked
            } else if gate.report_only_leaf_count > 0 {
                MetricGateState::ReportOnly
            } else {
                MetricGateState::Executable
            };
            if gate.state != expected_state {
                return Err(CoverageManifestError::InvalidCompactProjection(
                    "gate state does not match its exact disposition counts".into(),
                ));
            }
            if usize::from(gate.blocker_sample_limit) != COMPACT_BLOCKER_SAMPLE_LIMIT {
                return Err(CoverageManifestError::InvalidCompactProjection(
                    "blocker sample limit does not match this compiler".into(),
                ));
            }
            let expected_sample_len =
                (gate.blocking_leaf_count as usize).min(COMPACT_BLOCKER_SAMPLE_LIMIT);
            if gate.blockers.len() != expected_sample_len
                || gate.blocker_sample_truncated
                    != (gate.blocking_leaf_count as usize > COMPACT_BLOCKER_SAMPLE_LIMIT)
            {
                return Err(CoverageManifestError::InvalidCompactProjection(
                    "blocker sample shape does not match the exact blocking count".into(),
                ));
            }
        }
        if !is_sha256_hex(&self.projection_sha256)
            || self.projection_sha256 != compact_projection_fingerprint(self)?
        {
            return Err(CoverageManifestError::ProjectionFingerprintMismatch);
        }
        Ok(())
    }
}

pub fn validate_oracle_partition(source: &str, spans: &[OracleSourceSpan]) -> Result<(), String> {
    validate_oracle_partition_at(source, spans, 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleRootBindingContext {
    normalized_root_sha256: String,
    clauses: Vec<RuntimeOracleClauseEvidence>,
}

type OracleBindingContexts = (
    BTreeMap<u32, RuntimeOracleClauseEvidence>,
    BTreeMap<u16, OracleRootBindingContext>,
);

fn oracle_binding_contexts(
    record: &CombinedCardRecord,
    faces: &[FaceCoverageManifest],
    spans: &[OracleSourceSpan],
) -> Result<OracleBindingContexts, CoverageManifestError> {
    let invalid = |detail: String| CoverageManifestError::InvalidReference {
        card: card_id(record),
        detail,
    };
    let mut roots = BTreeMap::<u16, OracleRootBindingContext>::new();
    for face in faces {
        let face_name = face.name.as_deref().unwrap_or(record.name.as_str());
        let face_type_line = face
            .type_line
            .as_deref()
            .unwrap_or(record.type_line.as_str());
        let normalized_root =
            normalize_oracle_clause_for_receipt(&face.oracle_source, face_name, face_type_line);
        roots.insert(
            face.face_index,
            OracleRootBindingContext {
                normalized_root_sha256: sha256_hex_lowercase(normalized_root.as_bytes()),
                clauses: Vec::new(),
            },
        );
    }

    let mut clause_by_span = BTreeMap::<u32, RuntimeOracleClauseEvidence>::new();
    for span in spans
        .iter()
        .filter(|span| span.kind == OracleSourceSpanKind::RulesText)
    {
        let face_index = span.face_index.unwrap_or_default();
        let face = faces.get(face_index as usize).ok_or_else(|| {
            invalid(format!(
                "Oracle rules span {} refers to missing face {face_index}",
                span.span_index
            ))
        })?;
        let root = roots.get_mut(&face_index).ok_or_else(|| {
            invalid(format!(
                "Oracle rules span {} has no face root context",
                span.span_index
            ))
        })?;
        let clause_index = u16::try_from(root.clauses.len()).map_err(|_| {
            invalid(format!(
                "face {face_index} has more Oracle clauses than the receipt schema can address"
            ))
        })?;
        let face_name = face.name.as_deref().unwrap_or(record.name.as_str());
        let face_type_line = face
            .type_line
            .as_deref()
            .unwrap_or(record.type_line.as_str());
        let normalized_clause =
            normalize_oracle_clause_for_receipt(&span.text, face_name, face_type_line);
        let clause = RuntimeOracleClauseEvidence {
            face_index,
            clause_index,
            normalized_clause_sha256: sha256_hex_lowercase(normalized_clause.as_bytes()),
        };
        root.clauses.push(clause.clone());
        clause_by_span.insert(span.span_index, clause);
    }

    Ok((clause_by_span, roots))
}

fn validate_oracle_partition_at(
    source: &str,
    spans: &[OracleSourceSpan],
    first_span_index: u32,
) -> Result<(), String> {
    if source.is_empty() {
        return spans
            .is_empty()
            .then_some(())
            .ok_or_else(|| "an empty Oracle source must have no spans".into());
    }
    if spans.is_empty() {
        return Err("a non-empty Oracle source must have at least one span".into());
    }

    let mut cursor = 0usize;
    let mut reconstructed = String::with_capacity(source.len());
    for (expected_index, span) in spans.iter().enumerate() {
        let expected_index = first_span_index.saturating_add(expected_index as u32);
        if span.span_index != expected_index {
            return Err(format!(
                "span index {} should be {}",
                span.span_index, expected_index
            ));
        }
        let start = span.start_byte as usize;
        let end = span.end_byte as usize;
        if start != cursor {
            return Err(format!(
                "span {} starts at byte {start}, expected {cursor}",
                span.span_index
            ));
        }
        if end <= start || end > source.len() {
            return Err(format!(
                "span {} has invalid byte range {start}..{end}",
                span.span_index
            ));
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(format!(
                "span {} does not end on UTF-8 character boundaries",
                span.span_index
            ));
        }
        if source.get(start..end) != Some(span.text.as_str()) {
            return Err(format!(
                "span {} text does not match its source byte range",
                span.span_index
            ));
        }
        reconstructed.push_str(&span.text);
        cursor = end;
    }
    if cursor != source.len() || reconstructed != source {
        return Err("spans do not round-trip the complete Oracle source".into());
    }
    Ok(())
}

fn compile_card(
    record: &CombinedCardRecord,
) -> Result<CardCoverageManifest, CoverageManifestError> {
    let id = card_id(record);
    let keywords = canonical_keywords(&record.keywords);
    let revision_material = CardRevisionMaterial {
        card_id: &id,
        source_record: record,
        canonical_keywords: &keywords,
    };
    let oracle_revision_sha256 = sha256_serialized(&revision_material)?;
    let runtime_receipts = compile_retained_runtime_receipts(record);
    let layout = record.layout.trim().to_ascii_lowercase();
    let has_exact_faces = !record.faces.is_empty();
    let name_parts = split_combined_field(&record.name);
    let type_parts = split_combined_field(&record.type_line);
    let cost_parts = record
        .mana_cost
        .as_deref()
        .map(split_combined_field)
        .unwrap_or_default();
    let mut next_span_index = 0u32;

    let (mut faces, mut spans, legacy_face_count) = if has_exact_faces {
        let mut spans = Vec::new();
        let faces = record
            .faces
            .iter()
            .enumerate()
            .map(|(index, face)| {
                let face_index = index as u16;
                let face_spans = partition_oracle_source(
                    &face.oracle_text,
                    OracleSourceReference::ExactFace,
                    Some(face_index),
                    false,
                    &mut next_span_index,
                );
                let oracle_span_indices = face_spans
                    .iter()
                    .map(|span| span.span_index)
                    .collect::<Vec<_>>();
                spans.extend(face_spans);
                FaceCoverageManifest {
                    face_index,
                    oracle_id: face.oracle_id.clone(),
                    layout: face.layout.clone(),
                    name: nonempty_string(&face.name),
                    mana_value: face.mana_value,
                    type_line: nonempty_string(&face.type_line),
                    mana_cost: face.mana_cost.clone(),
                    oracle_source: face.oracle_text.clone(),
                    oracle_source_sha256: sha256_hex(face.oracle_text.as_bytes()),
                    colors: face.colors.clone(),
                    color_indicator: face.color_indicator.clone(),
                    keywords: face.keywords.clone(),
                    produced_mana: face.produced_mana.clone(),
                    power: face.power.clone(),
                    toughness: face.toughness.clone(),
                    loyalty: face.loyalty.clone(),
                    defense: face.defense.clone(),
                    hand_modifier: face.hand_modifier.clone(),
                    life_modifier: face.life_modifier.clone(),
                    attraction_lights: face.attraction_lights.clone(),
                    image_uri: face.image_uri.clone(),
                    relationship: relationship_for_layout(&layout, index)
                        .unwrap_or(FaceRelationship::UnsupportedLayoutFace),
                    oracle_span_indices,
                    leaf_ids: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        (faces, spans, None)
    } else {
        let spans = partition_oracle_source(
            &record.oracle_text,
            OracleSourceReference::TopLevelCard,
            Some(0),
            true,
            &mut next_span_index,
        );
        let oracle_face_count = spans
            .iter()
            .filter_map(|span| span.face_index)
            .max()
            .map_or(1usize, |index| index as usize + 1);
        let inferred_face_count = [
            oracle_face_count,
            name_parts.len(),
            type_parts.len(),
            cost_parts.len(),
            1,
        ]
        .into_iter()
        .max()
        .unwrap_or(1);
        let face_count = if layout_requires_exact_faces(&layout) {
            inferred_face_count.max(2)
        } else {
            inferred_face_count
        };
        let faces = (0..face_count)
            .map(|index| {
                let single = face_count == 1;
                let oracle_source = if single {
                    record.oracle_text.clone()
                } else {
                    String::new()
                };
                FaceCoverageManifest {
                    face_index: index as u16,
                    oracle_id: single.then(|| record.oracle_id.clone()).flatten(),
                    layout: if single {
                        record.layout.clone()
                    } else {
                        String::new()
                    },
                    name: if single {
                        nonempty_string(&record.name)
                    } else {
                        aligned_part(&name_parts, face_count, index)
                    },
                    mana_value: single.then_some(record.root_mana_value).flatten(),
                    type_line: if single {
                        nonempty_string(&record.type_line)
                    } else {
                        aligned_part(&type_parts, face_count, index)
                    },
                    mana_cost: if single {
                        record.mana_cost.clone()
                    } else {
                        aligned_part(&cost_parts, face_count, index)
                    },
                    oracle_source: oracle_source.clone(),
                    oracle_source_sha256: sha256_hex(oracle_source.as_bytes()),
                    colors: if single {
                        record.colors.clone()
                    } else {
                        Vec::new()
                    },
                    color_indicator: if single {
                        record.color_indicator.clone()
                    } else {
                        Vec::new()
                    },
                    keywords: if single {
                        record.keywords.clone()
                    } else {
                        Vec::new()
                    },
                    produced_mana: if single {
                        record.produced_mana.clone()
                    } else {
                        Vec::new()
                    },
                    power: single.then(|| record.power.clone()).flatten(),
                    toughness: single.then(|| record.toughness.clone()).flatten(),
                    loyalty: single.then(|| record.loyalty.clone()).flatten(),
                    defense: single.then(|| record.defense.clone()).flatten(),
                    hand_modifier: single.then(|| record.hand_modifier.clone()).flatten(),
                    life_modifier: single.then(|| record.life_modifier.clone()).flatten(),
                    attraction_lights: if single {
                        record.attraction_lights.clone()
                    } else {
                        Vec::new()
                    },
                    image_uri: None,
                    relationship: legacy_relationship(&layout, face_count, index),
                    oracle_span_indices: spans
                        .iter()
                        .filter(|span| span.face_index == Some(index as u16))
                        .map(|span| span.span_index)
                        .collect(),
                    leaf_ids: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        (faces, spans, Some(inferred_face_count))
    };
    let multiface = faces.len() > 1 || layout_requires_exact_faces(&layout) || layout == "meld";
    let face_alignment_complete =
        face_alignment_complete(record, &layout, legacy_face_count, faces.len());
    let exact_normal_single_face = layout == "normal"
        && faces.len() == 1
        && face_alignment_complete
        && faces[0].relationship == FaceRelationship::SingleFace;
    let exact_modal_double_faced = has_exact_faces
        && layout == "modal_dfc"
        && faces.len() == 2
        && face_alignment_complete
        && faces[0].relationship == FaceRelationship::ModalDoubleFacedFront
        && faces[1].relationship == FaceRelationship::ModalDoubleFacedBack;
    let characteristic_receipts = faces
        .iter()
        .flat_map(|face| {
            let face_binding = if exact_normal_single_face {
                Some(CharacteristicFaceBinding::NormalSingleFace)
            } else if exact_modal_double_faced {
                match face.face_index {
                    0 => Some(CharacteristicFaceBinding::ModalDoubleFacedFront),
                    1 => Some(CharacteristicFaceBinding::ModalDoubleFacedBack),
                    _ => None,
                }
            } else {
                None
            };
            compile_characteristic_runtime_receipts(CharacteristicFaceInput {
                face_index: face.face_index,
                face_binding,
                name: face.name.as_deref().unwrap_or_default(),
                oracle_text: &face.oracle_source,
                mana_cost: face.mana_cost.as_deref(),
                mana_value: face.mana_value,
                colors: &face.colors,
                color_indicator: &face.color_indicator,
                power: face.power.as_deref(),
                toughness: face.toughness.as_deref(),
                type_line: face.type_line.as_deref(),
                keywords: &face.keywords,
                root_alignment: if exact_modal_double_faced {
                    CharacteristicRootAlignment::EXACT
                } else {
                    characteristic_root_alignment(record, face, has_exact_faces)
                },
            })
        })
        .collect::<Vec<_>>();
    let (oracle_clause_contexts, oracle_root_contexts) =
        oracle_binding_contexts(record, &faces, &spans)?;
    let mut leaves = Vec::new();

    let source_schema_complete =
        record.source_schema_version == crate::card_data::SCRYFALL_FIELD_CLASSIFICATION_VERSION;
    let source_schema_evidence = serde_json::to_vec(&(
        &record.source_schema_version,
        crate::card_data::SCRYFALL_FIELD_CLASSIFICATION_VERSION,
    ))?;
    push_leaf(
        &mut leaves,
        &mut faces,
        CoverageLeafKind::SourceSchemaCompleteness,
        CoverageLeafSubject::SourceSchemaCompleteness,
        None,
        Vec::new(),
        if source_schema_complete {
            safely_irrelevant_for_all(
                "reviewed-upstream-schema-capture",
                "This record was parsed with the current reviewed Scryfall field classification.",
            )
        } else {
            function_dispositions(CoverageBlocker {
                blocker_code: "card-source-schema-incomplete".into(),
                detail: format!(
                    "This card was captured with source schema `{}` instead of `{}`; newly retained fields may have been lost, so strict functional analysis requires a fresh Scryfall refresh.",
                    record.source_schema_version,
                    crate::card_data::SCRYFALL_FIELD_CLASSIFICATION_VERSION
                ),
            })
        },
        &source_schema_evidence,
    );

    for (field, value) in &record.unreviewed_fields {
        push_unreviewed_field_leaf(
            &mut leaves,
            &mut faces,
            None,
            &format!("card.{field}"),
            value,
        )?;
    }
    for (face_index, face) in record.faces.iter().enumerate() {
        for (field, value) in &face.unreviewed_fields {
            push_unreviewed_field_leaf(
                &mut leaves,
                &mut faces,
                Some(face_index as u16),
                &format!("cardFaces[{face_index}].{field}"),
                value,
            )?;
        }
    }
    for (component_index, component) in record.related_components.iter().enumerate() {
        for (field, value) in &component.unreviewed_fields {
            push_unreviewed_field_leaf(
                &mut leaves,
                &mut faces,
                None,
                &format!("allParts[{component_index}].{field}"),
                value,
            )?;
        }
    }

    for span in &mut spans {
        let (kind, subject, dispositions) = match span.kind {
            OracleSourceSpanKind::Formatting => (
                CoverageLeafKind::OracleFormatting,
                CoverageLeafSubject::OracleFormatting,
                safely_irrelevant_for_all(
                    "oracle-formatting",
                    "Whitespace formatting has no rules effect after exact source partitioning.",
                ),
            ),
            OracleSourceSpanKind::CombinedFaceDelimiter => (
                CoverageLeafKind::CombinedFaceDelimiter,
                CoverageLeafSubject::CombinedFaceDelimiter,
                safely_irrelevant_for_all(
                    "combined-record-face-delimiter",
                    "This delimiter was inserted by the current combined-record storage format.",
                ),
            ),
            OracleSourceSpanKind::RulesText => {
                let blocker = classify_rules_text(&span.text);
                let leaf_evidence_sha256 = sha256_hex(span.text.as_bytes());
                let clause_context = oracle_clause_contexts
                    .get(&span.span_index)
                    .expect("every rules span has an occurrence-addressed Oracle context");
                let root_context = oracle_root_contexts
                    .get(&clause_context.face_index)
                    .expect("every Oracle clause belongs to one face root");
                (
                    CoverageLeafKind::OracleRulesText,
                    CoverageLeafSubject::OracleRulesText,
                    runtime_receipt_dispositions(
                        &runtime_receipts,
                        RuntimeCoverageRequirement::Clause {
                            clause: clause_context,
                            root: root_context,
                        },
                        &oracle_revision_sha256,
                        &leaf_evidence_sha256,
                        blocker,
                    ),
                )
            }
        };
        let leaf_id = push_leaf(
            &mut leaves,
            &mut faces,
            kind,
            subject,
            span.face_index,
            vec![span.span_index],
            dispositions,
            span.text.as_bytes(),
        );
        span.leaf_id = leaf_id;
    }

    for index in 0..faces.len() {
        let face_index = Some(index as u16);
        let structure_blocker = structure_blocker(record, &layout, legacy_face_count, faces.len());
        let structural_evidence = serde_json::to_vec(&faces[index])?;
        let structure_dispositions = if layout == "normal"
            && faces.len() == 1
            && face_alignment_complete
            && faces[index].relationship == FaceRelationship::SingleFace
        {
            safely_irrelevant_for_all(
                "plain-single-face-layout",
                "The exact normal single-face layout adds no independent rules behavior beyond the retained Oracle root and printed fields.",
            )
        } else if exact_modal_double_faced {
            let leaf_evidence_sha256 = sha256_hex(&structural_evidence);
            characteristic_receipt_dispositions(
                &characteristic_receipts,
                &CoverageLeafSubject::FaceRelationship,
                face_index,
                &oracle_revision_sha256,
                &leaf_evidence_sha256,
                structure_blocker,
            )
        } else {
            function_dispositions(structure_blocker)
        };
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::FaceStructure,
            CoverageLeafSubject::FaceRelationship,
            face_index,
            Vec::new(),
            structure_dispositions,
            &structural_evidence,
        );
        push_face_characteristic_leaves(
            &mut leaves,
            &mut faces,
            index,
            &characteristic_receipts,
            &oracle_revision_sha256,
        )?;

        if let Some(cost) = faces[index]
            .mana_cost
            .clone()
            .filter(|cost| !cost.trim().is_empty())
        {
            for blocker in classify_printed_cost(&cost) {
                let leaf_evidence_sha256 = sha256_hex(cost.as_bytes());
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::PrintedManaCost,
                    CoverageLeafSubject::ManaCost,
                    face_index,
                    Vec::new(),
                    characteristic_receipt_dispositions(
                        &characteristic_receipts,
                        &CoverageLeafSubject::ManaCost,
                        face_index,
                        &oracle_revision_sha256,
                        &leaf_evidence_sha256,
                        blocker,
                    ),
                    cost.as_bytes(),
                );
            }
        }
        if has_exact_faces {
            for keyword in canonical_keywords(&faces[index].keywords) {
                let subject = CoverageLeafSubject::Keyword(keyword.clone());
                let leaf_evidence_sha256 = sha256_hex(keyword.as_bytes());
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::KeywordAbility,
                    subject.clone(),
                    face_index,
                    Vec::new(),
                    keyword_receipt_dispositions(
                        &runtime_receipts,
                        &characteristic_receipts,
                        &subject,
                        face_index,
                        &oracle_revision_sha256,
                        &leaf_evidence_sha256,
                        CoverageBlocker {
                            blocker_code: "face-keyword-executor-missing".into(),
                            detail: format!(
                                "Face keyword `{keyword}` has no versioned executable binding in {}.",
                                EXECUTION_COVERAGE_COMPILER_VERSION
                            ),
                        },
                    ),
                    keyword.as_bytes(),
                );
            }
        }
    }

    if has_exact_faces {
        for (index, face) in record.faces.iter().enumerate() {
            let face_index = Some(index as u16);
            let rules_spans = spans
                .iter()
                .filter(|span| {
                    span.face_index == face_index && span.kind == OracleSourceSpanKind::RulesText
                })
                .map(|span| span.span_index)
                .collect::<Vec<_>>();
            for blocker in detect_atomicity_blockers(&face.oracle_text) {
                let leaf_evidence_sha256 = sha256_hex(face.oracle_text.as_bytes());
                let root_context = oracle_root_contexts
                    .get(&(index as u16))
                    .expect("every exact face has an Oracle root context");
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::AtomicityGuard,
                    CoverageLeafSubject::AtomicityGuard,
                    face_index,
                    rules_spans.clone(),
                    runtime_receipt_dispositions(
                        &runtime_receipts,
                        RuntimeCoverageRequirement::CompleteRoot(root_context),
                        &oracle_revision_sha256,
                        &leaf_evidence_sha256,
                        blocker,
                    ),
                    face.oracle_text.as_bytes(),
                );
            }
        }
    } else {
        let all_rules_spans = spans
            .iter()
            .filter(|span| span.kind == OracleSourceSpanKind::RulesText)
            .map(|span| span.span_index)
            .collect::<Vec<_>>();
        for blocker in detect_atomicity_blockers(&record.oracle_text) {
            let leaf_evidence_sha256 = sha256_hex(record.oracle_text.as_bytes());
            let root_context = oracle_root_contexts
                .get(&0)
                .expect("the top-level Oracle source has a root context");
            push_leaf(
                &mut leaves,
                &mut faces,
                CoverageLeafKind::AtomicityGuard,
                CoverageLeafSubject::AtomicityGuard,
                None,
                all_rules_spans.clone(),
                runtime_receipt_dispositions(
                    &runtime_receipts,
                    RuntimeCoverageRequirement::CompleteRoot(root_context),
                    &oracle_revision_sha256,
                    &leaf_evidence_sha256,
                    blocker,
                ),
                record.oracle_text.as_bytes(),
            );
        }
    }
    for keyword in &keywords {
        let subject = CoverageLeafSubject::Keyword(keyword.clone());
        let leaf_evidence_sha256 = sha256_hex(keyword.as_bytes());
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::KeywordAbility,
            subject.clone(),
            None,
            Vec::new(),
            keyword_receipt_dispositions(
                &runtime_receipts,
                &characteristic_receipts,
                &subject,
                None,
                &oracle_revision_sha256,
                &leaf_evidence_sha256,
                CoverageBlocker {
                    blocker_code: "keyword-executor-missing".into(),
                    detail: format!(
                        "Keyword `{keyword}` has no versioned executable binding in {}.",
                        EXECUTION_COVERAGE_COMPILER_VERSION
                    ),
                },
            ),
            keyword.as_bytes(),
        );
    }

    for component in &record.related_components {
        let component_evidence = serde_json::to_vec(component)?;
        let component_complete = !component.id.trim().is_empty()
            && !component.component.trim().is_empty()
            && !component.name.trim().is_empty();
        let component_kind = component.component.trim().to_ascii_lowercase();
        let dispositions = if component_complete
            && matches!(component_kind.as_str(), "token" | "combo_piece")
        {
            safely_irrelevant_for_all(
                "provider-cross-link-does-not-execute",
                "This complete provider cross-link is retained for provenance and does not supply a game object or runtime action.",
            )
        } else {
            function_dispositions(CoverageBlocker {
                blocker_code: if component_complete {
                    "related-component-executor-unbound".into()
                } else {
                    "related-component-data-incomplete".into()
                },
                detail: if component_complete {
                    format!(
                        "Related `{}` component `{}` is retained by stable id, but its zone and relationship behavior has no executable binding.",
                        component.component, component.name
                    )
                } else {
                    "A related component is missing its stable id, component kind, or name; execution cannot infer the relationship.".into()
                },
            })
        };
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::RelatedComponent,
            CoverageLeafSubject::RelatedComponent(component.component.clone()),
            None,
            Vec::new(),
            dispositions,
            &component_evidence,
        );
    }

    let card = CardCoverageManifest {
        card_id: id,
        name: record.name.clone(),
        normalized_name: record.normalized_name.clone(),
        combined_type_line: record.type_line.clone(),
        combined_mana_cost: record.mana_cost.clone(),
        keywords,
        layout,
        oracle_source: record.oracle_text.clone(),
        oracle_revision_sha256,
        source_record: record.clone(),
        multiface,
        face_alignment_complete,
        faces,
        oracle_spans: spans,
        leaves,
    };
    validate_card(&card)?;
    Ok(card)
}

fn characteristic_root_alignment(
    record: &CombinedCardRecord,
    face: &FaceCoverageManifest,
    has_exact_faces: bool,
) -> CharacteristicRootAlignment {
    if !has_exact_faces {
        return CharacteristicRootAlignment {
            mana_cost: true,
            mana_value: true,
            colors: true,
            color_indicator: true,
            power: true,
            toughness: true,
            type_line: true,
            keywords: true,
        };
    }
    let runtime_mana_value = record.root_mana_value.unwrap_or(record.mana_value);
    CharacteristicRootAlignment {
        mana_cost: face.mana_cost.as_deref() == record.mana_cost.as_deref(),
        mana_value: face
            .mana_value
            .is_some_and(|mana_value| mana_value == runtime_mana_value),
        colors: face.colors == record.colors,
        color_indicator: face.color_indicator == record.color_indicator,
        power: face.power == record.power,
        toughness: face.toughness == record.toughness,
        type_line: face.type_line.as_deref() == nonempty_string(&record.type_line).as_deref(),
        keywords: canonical_keywords(&face.keywords) == canonical_keywords(&record.keywords),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RetainedRuntimeReceipt {
    Atomic(AtomicRuntimeReceipt),
    GraveyardReclamation(GraveyardReclamationRuntimeReceipt),
    SpellResolutionMana(SpellResolutionManaRuntimeReceipt),
    ConditionalManaSource(ConditionalManaSourceRuntimeReceipt),
    SacrificeSelfMana(SacrificeSelfManaRuntimeReceipt),
    Interaction(InteractionRuntimeReceipt),
    Tutor(TutorRuntimeReceipt),
    RestrictionProtection(RestrictionProtectionRuntimeReceipt),
    Reviewed(ReviewedRuntimeReceipt),
    Land(LandRuntimeReceipt),
    LiveAbility(LiveAbilityRuntimeReceipt),
}

fn compile_retained_runtime_receipts(record: &CombinedCardRecord) -> Vec<RetainedRuntimeReceipt> {
    let compiled = compile_retained_card(record);
    let mut receipts = Vec::with_capacity(4);
    if let Some(receipt) = compile_atomic_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::Atomic(receipt));
    }
    if let Some(receipt) = compile_graveyard_reclamation_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::GraveyardReclamation(receipt));
    }
    if let Some(receipt) = compile_spell_resolution_mana_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::SpellResolutionMana(receipt));
    }
    if let Some(receipt) = compile_conditional_mana_source_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::ConditionalManaSource(receipt));
    }
    if let Some(receipt) = compile_sacrifice_self_mana_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::SacrificeSelfMana(receipt));
    }
    if let Some(receipt) = compile_interaction_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::Interaction(receipt));
    }
    if let Some(receipt) = compile_tutor_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::Tutor(receipt));
    }
    if let Some(receipt) = compile_restriction_protection_runtime_receipt(&compiled) {
        receipts.push(RetainedRuntimeReceipt::RestrictionProtection(receipt));
    }
    receipts.extend(
        compile_reviewed_runtime_receipts(&compiled)
            .into_iter()
            .map(RetainedRuntimeReceipt::Reviewed),
    );
    let claimed_oracle_clauses = receipts
        .iter()
        .flat_map(|receipt| {
            runtime_receipt_parts(receipt)
                .2
                .covered_oracle_clauses
                .iter()
                .cloned()
        })
        .collect::<BTreeSet<_>>();
    receipts.extend(
        compile_land_runtime_receipts(&compiled)
            .into_iter()
            .filter(|receipt| {
                receipt
                    .source_evidence
                    .covered_oracle_clauses
                    .iter()
                    .all(|clause| !claimed_oracle_clauses.contains(clause))
            })
            .map(RetainedRuntimeReceipt::Land),
    );
    if !compiled.has(role::LAND) {
        let complete_root_claimed = receipts.iter().any(|receipt| {
            runtime_receipt_parts(receipt)
                .1
                .contains(&RuntimeCapability::CompleteOracleRoot)
        });
        if !complete_root_claimed {
            let claimed_clauses = receipts
                .iter()
                .flat_map(|receipt| {
                    runtime_receipt_parts(receipt)
                        .2
                        .covered_oracle_clauses
                        .iter()
                        .cloned()
                })
                .collect::<BTreeSet<_>>();
            receipts.extend(
                compile_live_ability_runtime_receipts(&compiled)
                    .into_iter()
                    .filter(|receipt| {
                        receipt
                            .source_evidence
                            .covered_oracle_clauses
                            .iter()
                            .all(|clause| !claimed_clauses.contains(clause))
                    })
                    .map(RetainedRuntimeReceipt::LiveAbility),
            );
        }
    }
    sort_retained_runtime_receipts(&mut receipts);
    receipts
}

fn sort_retained_runtime_receipts(receipts: &mut [RetainedRuntimeReceipt]) {
    receipts.sort_by(|left, right| {
        let (left_binding, _, left_evidence) = runtime_receipt_parts(left);
        let (right_binding, _, right_evidence) = runtime_receipt_parts(right);
        left_binding
            .executor_id
            .cmp(right_binding.executor_id)
            .then_with(|| {
                left_binding
                    .executor_version
                    .cmp(right_binding.executor_version)
            })
            .then_with(|| {
                left_evidence
                    .source_evidence_sha256
                    .cmp(&right_evidence.source_evidence_sha256)
            })
    });
}

fn compile_retained_card(record: &CombinedCardRecord) -> CompiledCard {
    let retained_definition = retained_card_definition(record);
    let root = OracleCardInput {
        name: &record.name,
        layout: &record.layout,
        type_line: &record.type_line,
        oracle_text: &record.oracle_text,
        has_face_records: !record.faces.is_empty(),
    };
    let ability_program = if record.faces.is_empty() {
        compile_executable_ability_program(root)
    } else {
        let face_inputs = record
            .faces
            .iter()
            .map(|face| OracleCardFaceInput {
                name: &face.name,
                type_line: &face.type_line,
                oracle_text: &face.oracle_text,
            })
            .collect::<Vec<_>>();
        compile_face_bound_ability_program(root, &face_inputs)
    };
    let roles = retained_type_role_mask(&record.type_line);
    let mut effects = crate::effects::compile_effect_descriptor(&retained_definition);
    effects.retain_exact_mana_network_program(&record.type_line, &ability_program);
    CompiledCard {
        name: record.name.clone(),
        normalized_name: record.normalized_name.clone(),
        type_line: record.type_line.clone(),
        colors: record.colors.clone(),
        quantity: 1,
        mana_value: record.root_mana_value.unwrap_or(record.mana_value),
        printed_power: record
            .power
            .as_deref()
            .and_then(|value| value.trim().parse::<i16>().ok()),
        printed_toughness: record
            .toughness
            .as_deref()
            .and_then(|value| value.trim().parse::<i16>().ok()),
        roles,
        is_commander: false,
        semantic_confidence: 1.0,
        effects,
        ability_program,
    }
}

fn retained_card_definition(record: &CombinedCardRecord) -> crate::domain::CardDefinition {
    crate::domain::CardDefinition {
        name: record.name.clone(),
        normalized_name: record.normalized_name.clone(),
        oracle_id: record.oracle_id.clone(),
        layout: record.layout.clone(),
        root_mana_value: record.root_mana_value,
        mana_value: record.mana_value,
        mana_cost: record.mana_cost.clone(),
        type_line: record.type_line.clone(),
        oracle_text: record.oracle_text.clone(),
        colors: record.colors.clone(),
        color_indicator: record.color_indicator.clone(),
        color_identity: record.color_identity.clone(),
        keywords: record.keywords.clone(),
        produced_mana: record.produced_mana.clone(),
        power: record.power.clone(),
        toughness: record.toughness.clone(),
        loyalty: record.loyalty.clone(),
        defense: record.defense.clone(),
        hand_modifier: record.hand_modifier.clone(),
        life_modifier: record.life_modifier.clone(),
        attraction_lights: record.attraction_lights.clone(),
        faces: record
            .faces
            .iter()
            .map(|face| crate::domain::CardFaceDefinition {
                oracle_id: face.oracle_id.clone(),
                layout: face.layout.clone(),
                name: face.name.clone(),
                mana_value: face.mana_value,
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                colors: face.colors.clone(),
                color_indicator: face.color_indicator.clone(),
                keywords: face.keywords.clone(),
                produced_mana: face.produced_mana.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                hand_modifier: face.hand_modifier.clone(),
                life_modifier: face.life_modifier.clone(),
                attraction_lights: face.attraction_lights.clone(),
                image_uri: face.image_uri.clone(),
                unreviewed_fields: face.unreviewed_fields.clone(),
            })
            .collect(),
        related_components: record
            .related_components
            .iter()
            .map(|component| crate::domain::RelatedCardComponentDefinition {
                id: component.id.clone(),
                component: component.component.clone(),
                name: component.name.clone(),
                type_line: component.type_line.clone(),
                uri: component.uri.clone(),
                unreviewed_fields: component.unreviewed_fields.clone(),
            })
            .collect(),
        game_changer: record.game_changer,
        commander_legality: record.commander_legality.clone(),
        legal_commander: record.legal_commander,
        unreviewed_fields: record.unreviewed_fields.clone(),
        source_schema_version: record.source_schema_version.clone(),
        ..crate::domain::CardDefinition::default()
    }
}

fn retained_type_role_mask(type_line: &str) -> u32 {
    // Keep this source-kind projection identical to `semantics::classify_card`.
    // The receipt digest masks out every strategic role, so only these exact
    // printed-type bits can influence runtime/coverage parity.
    let type_line = type_line.to_ascii_lowercase();
    let mut roles = 0u32;
    if type_line.contains("land") {
        roles |= role::LAND;
    }
    if type_line.contains("creature") {
        roles |= role::CREATURE;
    }
    if type_line.contains("artifact") {
        roles |= role::ARTIFACT;
    }
    if type_line.contains("enchantment") {
        roles |= role::ENCHANTMENT;
    }
    if type_line.contains("instant") || type_line.contains("sorcery") {
        roles |= role::INSTANT_SORCERY;
    }
    roles
}

fn push_face_characteristic_leaves(
    leaves: &mut Vec<ExecutionCoverageLeaf>,
    faces: &mut [FaceCoverageManifest],
    index: usize,
    characteristic_receipts: &[CharacteristicRuntimeReceipt],
    card_revision_sha256: &str,
) -> Result<(), CoverageManifestError> {
    let face_index = Some(index as u16);
    let retained = {
        let face = &faces[index];
        let mut retained = Vec::<(
            CoverageLeafSubject,
            &'static str,
            &'static str,
            &'static str,
            Vec<u8>,
        )>::new();
        macro_rules! retain {
            ($present:expr, $subject:expr, $path:literal, $code:literal, $detail:literal, $value:expr) => {
                if $present {
                    retained.push((
                        $subject,
                        $path,
                        $code,
                        $detail,
                        serde_json::to_vec(&($path, $value))?,
                    ));
                }
            };
        }
        retain!(
            face.mana_value.is_some(),
            CoverageLeafSubject::ManaValue,
            "manaValue",
            "printed-mana-value-executor-unbound",
            "A finite nonnegative integer mana value from the exact normal face is required by the shared characteristic executor.",
            &face.mana_value
        );
        retain!(
            face.type_line.is_some(),
            CoverageLeafSubject::TypeLine,
            "typeLine",
            "printed-type-line-executor-unbound",
            "Only a normal single-face line made entirely of reviewed card types and supertypes with an exact retained subtype suffix has an exact compiled type profile.",
            &face.type_line
        );
        retain!(
            !face.colors.is_empty(),
            CoverageLeafSubject::Colors,
            "colors",
            "printed-colors-executor-unbound",
            "The complete printed color set, including an explicit empty colorless set, must use only W, U, B, R, and G.",
            &face.colors
        );
        retain!(
            !face.color_indicator.is_empty(),
            CoverageLeafSubject::ColorIndicator,
            "colorIndicator",
            "printed-color-indicator-executor-unbound",
            "The complete color-indicator set, including an explicit empty set, must use only W, U, B, R, and G.",
            &face.color_indicator
        );
        retain!(
            !face.produced_mana.is_empty(),
            CoverageLeafSubject::ProducedMana,
            "producedMana",
            "printed-produced-mana-executor-unbound",
            "The retained produced-mana metadata is not a substitute for an exact executable mana ability.",
            &face.produced_mana
        );
        retain!(
            face.power.is_some(),
            CoverageLeafSubject::Power,
            "power",
            "printed-power-executor-unbound",
            "Creature power and toughness must both be exact canonical integers before either combat characteristic can bind.",
            &face.power
        );
        retain!(
            face.toughness.is_some(),
            CoverageLeafSubject::Toughness,
            "toughness",
            "printed-toughness-executor-unbound",
            "Creature power and toughness must both be exact canonical integers before either combat characteristic can bind.",
            &face.toughness
        );
        retain!(
            face.loyalty.is_some(),
            CoverageLeafSubject::Loyalty,
            "loyalty",
            "printed-loyalty-executor-unbound",
            "The exact printed loyalty is retained but planeswalker loyalty rules are not completely executable.",
            &face.loyalty
        );
        retain!(
            face.defense.is_some(),
            CoverageLeafSubject::Defense,
            "defense",
            "printed-defense-executor-unbound",
            "The exact printed defense is retained but battle defense and defeat rules are not completely executable.",
            &face.defense
        );
        retain!(
            face.hand_modifier.is_some(),
            CoverageLeafSubject::HandModifier,
            "handModifier",
            "printed-hand-modifier-executor-unbound",
            "The exact Vanguard hand modifier is retained but is not bound to strict game initialization.",
            &face.hand_modifier
        );
        retain!(
            face.life_modifier.is_some(),
            CoverageLeafSubject::LifeModifier,
            "lifeModifier",
            "printed-life-modifier-executor-unbound",
            "The exact Vanguard life modifier is retained but is not bound to strict game initialization.",
            &face.life_modifier
        );
        retain!(
            !face.attraction_lights.is_empty(),
            CoverageLeafSubject::AttractionLights,
            "attractionLights",
            "printed-attraction-lights-executor-unbound",
            "The exact Attraction lights are retained but Attraction rolling and visit behavior is not executable.",
            &face.attraction_lights
        );
        retained
    };

    for (subject, field_path, blocker_code, detail, evidence) in retained {
        let dispositions = if subject == CoverageLeafSubject::ProducedMana {
            safely_irrelevant_for_all(
                "provider-produced-mana-does-not-execute",
                "Produced-mana provider metadata is retained for provenance. Runtime mana authority comes only from an exact executable ability.",
            )
        } else {
            let leaf_evidence_sha256 = sha256_hex(&evidence);
            characteristic_receipt_dispositions(
                characteristic_receipts,
                &subject,
                face_index,
                card_revision_sha256,
                &leaf_evidence_sha256,
                CoverageBlocker {
                    blocker_code: blocker_code.into(),
                    detail: format!("Face field `{field_path}`: {detail}"),
                },
            )
        };
        push_leaf(
            leaves,
            faces,
            CoverageLeafKind::PrintedCharacteristics,
            subject,
            face_index,
            Vec::new(),
            dispositions,
            &evidence,
        );
    }
    Ok(())
}

fn printed_characteristic_leaf_count(face: &FaceCoverageManifest) -> usize {
    usize::from(face.mana_value.is_some())
        + usize::from(face.type_line.is_some())
        + usize::from(!face.colors.is_empty())
        + usize::from(!face.color_indicator.is_empty())
        + usize::from(!face.produced_mana.is_empty())
        + usize::from(face.power.is_some())
        + usize::from(face.toughness.is_some())
        + usize::from(face.loyalty.is_some())
        + usize::from(face.defense.is_some())
        + usize::from(face.hand_modifier.is_some())
        + usize::from(face.life_modifier.is_some())
        + usize::from(!face.attraction_lights.is_empty())
}

fn canonical_keywords(keywords: &[String]) -> Vec<String> {
    let mut canonical = keywords
        .iter()
        .map(|keyword| keyword.trim().to_string())
        .filter(|keyword| !keyword.is_empty())
        .collect::<Vec<_>>();
    canonical.sort_by_key(|keyword| keyword.to_ascii_lowercase());
    canonical.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    canonical
}

fn card_id(record: &CombinedCardRecord) -> String {
    if let Some(root_id) = record
        .oracle_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return root_id.to_string();
    }
    let face_ids = record
        .faces
        .iter()
        .map(|face| face.oracle_id.as_deref().map(str::trim).unwrap_or_default())
        .collect::<Vec<_>>();
    if !face_ids.is_empty() && face_ids.iter().all(|face_id| !face_id.is_empty()) {
        return format!(
            "face-oracles:{}:{}",
            record.layout.trim().to_ascii_lowercase(),
            face_ids.join("+")
        );
    }
    let normalized = record.normalized_name.trim();
    if normalized.is_empty() {
        record.name.trim().to_string()
    } else {
        normalized.to_string()
    }
}

fn split_combined_field(value: &str) -> Vec<String> {
    value
        .split(" // ")
        .map(str::trim)
        .map(str::to_string)
        .collect()
}

fn legacy_inferred_face_count(record: &CombinedCardRecord) -> usize {
    let oracle_face_count = record
        .oracle_text
        .split_inclusive('\n')
        .filter(|chunk| chunk.trim() == "//")
        .count()
        .saturating_add(1);
    [
        oracle_face_count,
        split_combined_field(&record.name).len(),
        split_combined_field(&record.type_line).len(),
        record
            .mana_cost
            .as_deref()
            .map(split_combined_field)
            .map_or(0, |parts| parts.len()),
        1,
    ]
    .into_iter()
    .max()
    .unwrap_or(1)
}

fn aligned_part(parts: &[String], face_count: usize, index: usize) -> Option<String> {
    (parts.len() == face_count)
        .then(|| parts.get(index).cloned())
        .flatten()
        .filter(|value| !value.is_empty())
}

fn nonempty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn partition_oracle_source(
    source: &str,
    source_reference: OracleSourceReference,
    fixed_face_index: Option<u16>,
    recognize_combined_delimiter: bool,
    next_span_index: &mut u32,
) -> Vec<OracleSourceSpan> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    let mut legacy_face_index = fixed_face_index.unwrap_or(0);
    for chunk in source.split_inclusive('\n') {
        let start = cursor;
        cursor += chunk.len();
        let trimmed = chunk.trim();
        let (kind, assigned_face) = if recognize_combined_delimiter && trimmed == "//" {
            let current = (OracleSourceSpanKind::CombinedFaceDelimiter, None);
            legacy_face_index = legacy_face_index.saturating_add(1);
            current
        } else if trimmed.is_empty() {
            (
                OracleSourceSpanKind::Formatting,
                if recognize_combined_delimiter {
                    Some(legacy_face_index)
                } else {
                    fixed_face_index
                },
            )
        } else {
            (
                OracleSourceSpanKind::RulesText,
                if recognize_combined_delimiter {
                    Some(legacy_face_index)
                } else {
                    fixed_face_index
                },
            )
        };
        spans.push(OracleSourceSpan {
            span_index: *next_span_index,
            start_byte: start as u32,
            end_byte: cursor as u32,
            text: chunk.to_string(),
            kind,
            source_reference,
            face_index: assigned_face,
            leaf_id: String::new(),
        });
        *next_span_index = next_span_index.saturating_add(1);
    }
    spans
}

fn layout_requires_exact_faces(layout: &str) -> bool {
    matches!(
        layout,
        "split"
            | "transform"
            | "adventure"
            | "modal_dfc"
            | "flip"
            | "reversible_card"
            | "double_faced_token"
    )
}

fn known_single_layout(layout: &str) -> bool {
    matches!(
        layout,
        "normal"
            | "leveler"
            | "class"
            | "case"
            | "saga"
            | "mutate"
            | "prototype"
            | "battle"
            | "planar"
            | "scheme"
            | "vanguard"
            | "token"
            | "emblem"
            | "augment"
            | "host"
            | "art_series"
    )
}

fn relationship_for_layout(layout: &str, index: usize) -> Option<FaceRelationship> {
    match (layout, index) {
        ("split", 0 | 1) => Some(FaceRelationship::SplitHalf),
        ("transform", 0) => Some(FaceRelationship::TransformFront),
        ("transform", 1) => Some(FaceRelationship::TransformBack),
        ("adventure", 0) => Some(FaceRelationship::AdventurePermanent),
        ("adventure", 1) => Some(FaceRelationship::AdventureSpell),
        ("modal_dfc", 0) => Some(FaceRelationship::ModalDoubleFacedFront),
        ("modal_dfc", 1) => Some(FaceRelationship::ModalDoubleFacedBack),
        ("flip", 0) => Some(FaceRelationship::FlipFront),
        ("flip", 1) => Some(FaceRelationship::FlipBack),
        ("reversible_card", 0) => Some(FaceRelationship::ReversibleFront),
        ("reversible_card", 1) => Some(FaceRelationship::ReversibleBack),
        ("double_faced_token", 0) => Some(FaceRelationship::DoubleFacedTokenFront),
        ("double_faced_token", 1) => Some(FaceRelationship::DoubleFacedTokenBack),
        ("meld", 0) => Some(FaceRelationship::MeldCard),
        (layout, 0) if known_single_layout(layout) => Some(FaceRelationship::SingleFace),
        _ => None,
    }
}

fn legacy_relationship(layout: &str, face_count: usize, index: usize) -> FaceRelationship {
    if layout.is_empty() {
        if face_count == 1 {
            FaceRelationship::LegacySingleFaceLayoutUnknown
        } else {
            FaceRelationship::LegacyCombinedMultifaceRelationshipUnknown
        }
    } else {
        relationship_for_layout(layout, index).unwrap_or(FaceRelationship::UnsupportedLayoutFace)
    }
}

fn meld_components_complete(record: &CombinedCardRecord) -> bool {
    let meld_parts = record
        .related_components
        .iter()
        .filter(|component| {
            component.component == "meld_part"
                && !component.id.trim().is_empty()
                && !component.name.trim().is_empty()
        })
        .count();
    let meld_results = record
        .related_components
        .iter()
        .filter(|component| {
            component.component == "meld_result"
                && !component.id.trim().is_empty()
                && !component.name.trim().is_empty()
        })
        .count();
    meld_parts >= 2 && meld_results >= 1
}

fn face_alignment_complete(
    record: &CombinedCardRecord,
    layout: &str,
    legacy_face_count: Option<usize>,
    compiled_face_count: usize,
) -> bool {
    if layout.is_empty() {
        return false;
    }
    if layout == "meld" {
        return record.faces.is_empty()
            && compiled_face_count == 1
            && legacy_face_count == Some(1)
            && meld_components_complete(record);
    }
    if layout_requires_exact_faces(layout) {
        return record.faces.len() == 2 && compiled_face_count == 2;
    }
    if known_single_layout(layout) {
        return compiled_face_count == 1
            && (record.faces.len() == 1 || legacy_face_count == Some(1));
    }
    false
}

fn structure_blocker(
    record: &CombinedCardRecord,
    layout: &str,
    legacy_face_count: Option<usize>,
    compiled_face_count: usize,
) -> CoverageBlocker {
    let (blocker_code, detail) = if layout.is_empty() {
        if compiled_face_count > 1 {
            (
                "legacy-multiface-data-missing",
                "The legacy record contains combined face fields but no retained layout or exact face records; face attribution is intentionally blocked.",
            )
        } else {
            (
                "legacy-layout-missing",
                "The legacy record has no retained Scryfall layout, so even a single apparent face cannot be certified against the schema-v5 coverage source.",
            )
        }
    } else if layout == "meld" {
        if !record.faces.is_empty() || legacy_face_count != Some(1) {
            (
                "layout-face-data-mismatch",
                "A meld card must retain its printed card as one top-level face plus related meld components.",
            )
        } else if !meld_components_complete(record) {
            (
                "meld-related-components-incomplete",
                "The meld relationship requires at least two identified meld parts and one identified meld result.",
            )
        } else {
            (
                "meld-relationship-executor-unbound",
                "The complete meld component relationship is retained, but meld assembly and resulting-object behavior are not executable.",
            )
        }
    } else if layout_requires_exact_faces(layout) {
        if record.faces.is_empty() {
            (
                "schema5-face-data-missing",
                "This layout requires two exact schema-v5 face records; combined top-level strings cannot substitute for them.",
            )
        } else if record.faces.len() != 2 || compiled_face_count != 2 {
            (
                "layout-face-count-mismatch",
                "The retained layout requires exactly two ordered face records, and the stored count does not match.",
            )
        } else {
            (
                "face-relationship-executor-unbound",
                "The ordered face sources and layout relationship are exact, but casting/transition behavior has no versioned executable binding.",
            )
        }
    } else if known_single_layout(layout) {
        if compiled_face_count != 1 {
            (
                "layout-face-data-mismatch",
                "A single-face layout conflicts with multiple retained or legacy-inferred faces.",
            )
        } else {
            (
                "face-characteristics-executor-unbound",
                "The retained single-face structure has no versioned executable binding for every printed and implicit characteristic.",
            )
        }
    } else {
        (
            "unsupported-layout",
            "The retained layout is unknown to this closed compiler version and cannot be assigned executable face semantics.",
        )
    };
    CoverageBlocker {
        blocker_code: blocker_code.into(),
        detail: detail.into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_leaf(
    leaves: &mut Vec<ExecutionCoverageLeaf>,
    faces: &mut [FaceCoverageManifest],
    kind: CoverageLeafKind,
    subject: CoverageLeafSubject,
    face_index: Option<u16>,
    source_span_indices: Vec<u32>,
    metric_dispositions: Vec<MetricDisposition>,
    evidence: &[u8],
) -> String {
    let leaf_id = format!("leaf-{:04}", leaves.len());
    leaves.push(ExecutionCoverageLeaf {
        leaf_id: leaf_id.clone(),
        kind,
        subject,
        face_index,
        source_span_indices,
        evidence_sha256: sha256_hex(evidence),
        metric_dispositions,
    });
    if let Some(face) = face_index.and_then(|index| faces.get_mut(index as usize)) {
        face.leaf_ids.push(leaf_id.clone());
    }
    leaf_id
}

fn push_unreviewed_field_leaf(
    leaves: &mut Vec<ExecutionCoverageLeaf>,
    faces: &mut [FaceCoverageManifest],
    face_index: Option<u16>,
    field_path: &str,
    value: &serde_json::Value,
) -> Result<(), CoverageManifestError> {
    let evidence = serde_json::to_vec(&(field_path, value))?;
    push_leaf(
        leaves,
        faces,
        CoverageLeafKind::UnreviewedUpstreamField,
        CoverageLeafSubject::UnreviewedField(field_path.into()),
        face_index,
        Vec::new(),
        function_dispositions(CoverageBlocker {
            blocker_code: "unreviewed-scryfall-field".into(),
            detail: format!(
                "Upstream field `{field_path}` was retained losslessly but is not classified by {}; functional analysis is blocked until its rules/analysis effect is reviewed.",
                EXECUTION_COVERAGE_COMPILER_VERSION
            ),
        }),
        &evidence,
    );
    Ok(())
}

fn classify_rules_text(text: &str) -> CoverageBlocker {
    let normalized = text.to_ascii_lowercase();
    if normalized.contains("draw")
        && (normalized.contains("then discard") || normalized.contains("discard, then draw"))
    {
        CoverageBlocker {
            blocker_code: "mandatory-sequence-requires-atomic-executor".into(),
            detail: "A recognized draw fragment cannot execute unless the mandatory discard and sequencing are also executable.".into(),
        }
    } else if normalized.contains("as an additional cost") {
        CoverageBlocker {
            blocker_code: "additional-cost-requires-atomic-executor".into(),
            detail: "The spell cannot execute unless its additional cost is staged and paid atomically with casting.".into(),
        }
    } else if normalized.contains("extra turn") && normalized.contains("lose the game") {
        CoverageBlocker {
            blocker_code: "delayed-drawback-requires-atomic-executor".into(),
            detail:
                "An extra-turn benefit cannot execute without its linked delayed loss condition."
                    .into(),
        }
    } else {
        CoverageBlocker {
            blocker_code: "oracle-rules-text-executor-unbound".into(),
            detail: "This complete Oracle rules span has no exact, versioned executable binding."
                .into(),
        }
    }
}

fn detect_atomicity_blockers(source: &str) -> Vec<CoverageBlocker> {
    let normalized = source
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let mut blockers = BTreeMap::<String, String>::new();
    if normalized.contains("draw") && normalized.contains("then discard") {
        blockers.insert(
            "mandatory-sequence-requires-atomic-executor".into(),
            "Drawing and the mandatory later discard must execute as one ordered ability.".into(),
        );
    }
    if normalized.contains("as an additional cost") {
        blockers.insert(
            "additional-cost-requires-atomic-executor".into(),
            "The additional cost and spell effect must be announced, paid, and executed atomically."
                .into(),
        );
    }
    if normalized.contains("extra turn") && normalized.contains("lose the game") {
        blockers.insert(
            "delayed-drawback-requires-atomic-executor".into(),
            "The extra turn and linked delayed loss condition must share one executable trace."
                .into(),
        );
    }
    blockers
        .into_iter()
        .map(|(blocker_code, detail)| CoverageBlocker {
            blocker_code,
            detail,
        })
        .collect()
}

fn classify_printed_cost(cost: &str) -> Vec<CoverageBlocker> {
    let normalized = cost.to_ascii_uppercase();
    let mut blockers = BTreeMap::<String, String>::new();
    if normalized.contains("{X}") || normalized.contains("{Y}") || normalized.contains("{Z}") {
        blockers.insert(
            "variable-mana-cost-executor-unbound".into(),
            "Variable mana requires a declared value and linked effect scaling; it cannot be treated as zero.".into(),
        );
    }
    if normalized.contains("/P}") {
        blockers.insert(
            "phyrexian-mana-cost-executor-unbound".into(),
            "Phyrexian mana requires an explicit mana-or-life choice and tracked life payment."
                .into(),
        );
    }
    if normalized.contains("{S}") {
        blockers.insert(
            "snow-mana-cost-executor-unbound".into(),
            "Snow payment requires provenance from a snow source and cannot be reduced to generic mana.".into(),
        );
    }
    if braced_symbols(&normalized)
        .iter()
        .any(|symbol| symbol.contains('/') && !symbol.ends_with("/P"))
    {
        blockers.insert(
            "hybrid-mana-cost-executor-unbound".into(),
            "Hybrid mana requires an explicit alternative payment choice tied to the selected face.".into(),
        );
    }
    if braced_symbols(&normalized)
        .iter()
        .any(|symbol| !known_cost_symbol(symbol))
    {
        blockers.insert(
            "unknown-mana-symbol-executor-unbound".into(),
            "At least one printed mana symbol is outside the closed cost vocabulary.".into(),
        );
    }
    if blockers.is_empty() {
        blockers.insert(
            "printed-mana-cost-executor-unbound".into(),
            "The printed cost has not yet been bound to the strict staged-payment executor.".into(),
        );
    }
    blockers
        .into_iter()
        .map(|(blocker_code, detail)| CoverageBlocker {
            blocker_code,
            detail,
        })
        .collect()
}

fn braced_symbols(cost: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    let mut remaining = cost;
    while let Some(start) = remaining.find('{') {
        let after_start = &remaining[start + 1..];
        let Some(end) = after_start.find('}') else {
            symbols.push(after_start.to_string());
            break;
        };
        symbols.push(after_start[..end].to_string());
        remaining = &after_start[end + 1..];
    }
    symbols
}

fn known_cost_symbol(symbol: &str) -> bool {
    if symbol.chars().all(|character| character.is_ascii_digit()) {
        return true;
    }
    if matches!(
        symbol,
        "W" | "U" | "B" | "R" | "G" | "C" | "S" | "X" | "Y" | "Z"
    ) {
        return true;
    }
    let parts = symbol.split('/').collect::<Vec<_>>();
    parts.len() == 2
        && parts
            .iter()
            .all(|part| matches!(*part, "W" | "U" | "B" | "R" | "G" | "C" | "P" | "2"))
}

#[derive(Debug, Clone, Copy)]
enum RuntimeCoverageRequirement<'a> {
    Clause {
        clause: &'a RuntimeOracleClauseEvidence,
        root: &'a OracleRootBindingContext,
    },
    CompleteRoot(&'a OracleRootBindingContext),
}

impl<'a> RuntimeCoverageRequirement<'a> {
    fn root(self) -> &'a OracleRootBindingContext {
        match self {
            Self::Clause { root, .. } | Self::CompleteRoot(root) => root,
        }
    }
}

fn runtime_receipt_claims_requirement(
    receipt: &RetainedRuntimeReceipt,
    requirement: RuntimeCoverageRequirement<'_>,
) -> bool {
    if !runtime_receipt_has_exact_contract(receipt) {
        return false;
    }
    let (_, capabilities, source_evidence) = runtime_receipt_parts(receipt);
    if capabilities.contains(&RuntimeCapability::CompleteOracleRoot) {
        return source_evidence.normalized_oracle_sha256
            == requirement.root().normalized_root_sha256;
    }
    if !capabilities.contains(&RuntimeCapability::ExactOracleClauseSet) {
        return false;
    }
    match requirement {
        RuntimeCoverageRequirement::Clause { clause, .. } => {
            source_evidence.covered_oracle_clauses.contains(clause)
        }
        RuntimeCoverageRequirement::CompleteRoot(_) => false,
    }
}

fn runtime_receipt_dispositions(
    receipts: &[RetainedRuntimeReceipt],
    requirement: RuntimeCoverageRequirement<'_>,
    card_revision_sha256: &str,
    leaf_evidence_sha256: &str,
    blocker: CoverageBlocker,
) -> Vec<MetricDisposition> {
    let matching_receipts = receipts
        .iter()
        .filter(|receipt| runtime_receipt_claims_requirement(receipt, requirement))
        .collect::<Vec<_>>();
    let receipt = (matching_receipts.len() == 1).then(|| matching_receipts[0]);
    let conflicting_claims = matching_receipts.len() > 1;
    let covered_oracle_clauses = receipt.map(|receipt| {
        let (_, capabilities, source_evidence) = runtime_receipt_parts(receipt);
        if capabilities.contains(&RuntimeCapability::CompleteOracleRoot) {
            requirement.root().clauses.as_slice()
        } else {
            source_evidence.covered_oracle_clauses.as_slice()
        }
    });
    METRICS
        .into_iter()
        .map(|metric| {
            let disposition = match metric {
                ExecutionMetric::RawOpeningComposition => {
                    CoverageDisposition::SafelyIrrelevant {
                        proof: IrrelevanceProof {
                            proof_code: "raw-opening-does-not-execute-card-functions".into(),
                            detail: "Physical opening-card sampling uses only canonical deck membership and zone placement; this metric makes no keep or card-function claim.".into(),
                        },
                    }
                }
                ExecutionMetric::SynergyDescription => CoverageDisposition::ReportOnly {
                    reason: "The exact runtime receipt may corroborate descriptive evidence, but descriptive relationships do not mutate game state or numeric scoring.".into(),
                },
                _ if conflicting_claims => CoverageDisposition::BlockingUnsupported {
                    blocker: CoverageBlocker {
                        blocker_code: "runtime-receipt-clause-claim-conflict".into(),
                        detail: "More than one validated runtime receipt claims the same Oracle obligation; execution remains blocked to prevent duplicate or order-dependent mutation.".into(),
                    },
                },
                metric if receipt.is_some_and(|receipt| {
                    runtime_receipt_supports_metric(receipt, metric)
                }) => CoverageDisposition::FullyExecutable {
                    binding: Box::new(executor_binding(
                        receipt.expect("supported metric requires a receipt"),
                        card_revision_sha256,
                        leaf_evidence_sha256,
                        covered_oracle_clauses
                            .expect("a selected runtime receipt has exact clause evidence"),
                    )),
                },
                _ => CoverageDisposition::BlockingUnsupported {
                    blocker: blocker.clone(),
                },
            };
            MetricDisposition {
                metric,
                disposition,
            }
        })
        .collect()
}

fn characteristic_receipt_dispositions(
    receipts: &[CharacteristicRuntimeReceipt],
    leaf_subject: &CoverageLeafSubject,
    face_index: Option<u16>,
    card_revision_sha256: &str,
    leaf_evidence_sha256: &str,
    blocker: CoverageBlocker,
) -> Vec<MetricDisposition> {
    let subject = characteristic_subject_for_leaf(leaf_subject);
    let receipt = subject.as_ref().and_then(|subject| {
        let mut matches = receipts.iter().filter(|receipt| {
            receipt.face_index == face_index.unwrap_or_default()
                && &receipt.subject == subject
                && receipt.has_exact_contract()
        });
        let receipt = matches.next()?;
        matches.next().is_none().then_some(receipt)
    });
    METRICS
        .into_iter()
        .map(|metric| {
            let disposition = match metric {
                ExecutionMetric::RawOpeningComposition => {
                    CoverageDisposition::SafelyIrrelevant {
                        proof: IrrelevanceProof {
                            proof_code: "raw-opening-does-not-execute-card-functions".into(),
                            detail: "Physical opening-card sampling uses only canonical deck membership and zone placement; this metric makes no keep or card-function claim.".into(),
                        },
                    }
                }
                ExecutionMetric::SynergyDescription => CoverageDisposition::ReportOnly {
                    reason: "The exact characteristic receipt may corroborate descriptive evidence, but descriptive relationships do not mutate game state or numeric scoring.".into(),
                },
                metric if receipt.is_some_and(|receipt| {
                    executor_id_supports_metric(
                        receipt.binding.executor_id,
                        receipt.binding.executor_version,
                        metric,
                    )
                }) => CoverageDisposition::FullyExecutable {
                    binding: Box::new(characteristic_executor_binding(
                        receipt.expect("supported metric requires a characteristic receipt"),
                        card_revision_sha256,
                        leaf_evidence_sha256,
                    )),
                },
                _ => CoverageDisposition::BlockingUnsupported {
                    blocker: blocker.clone(),
                },
            };
            MetricDisposition {
                metric,
                disposition,
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn keyword_receipt_dispositions(
    runtime_receipts: &[RetainedRuntimeReceipt],
    characteristic_receipts: &[CharacteristicRuntimeReceipt],
    leaf_subject: &CoverageLeafSubject,
    face_index: Option<u16>,
    card_revision_sha256: &str,
    leaf_evidence_sha256: &str,
    blocker: CoverageBlocker,
) -> Vec<MetricDisposition> {
    let CoverageLeafSubject::Keyword(keyword) = leaf_subject else {
        return characteristic_receipt_dispositions(
            characteristic_receipts,
            leaf_subject,
            face_index,
            card_revision_sha256,
            leaf_evidence_sha256,
            blocker,
        );
    };
    if keyword.eq_ignore_ascii_case("constellation") {
        return safely_irrelevant_for_all(
            "ability-word-has-no-rules-meaning",
            "Constellation is an ability word under CR 207.2c. The word itself has no rules meaning; the following Oracle ability remains independently bound to its executor.",
        );
    }
    if keyword.eq_ignore_ascii_case("equip") {
        let characteristic = characteristic_receipt_dispositions(
            characteristic_receipts,
            leaf_subject,
            face_index,
            card_revision_sha256,
            leaf_evidence_sha256,
            blocker.clone(),
        );
        if characteristic.iter().any(|entry| {
            matches!(
                entry.disposition,
                CoverageDisposition::FullyExecutable { .. }
            )
        }) {
            return characteristic;
        }
    }
    let required_capability = match keyword.trim().to_ascii_lowercase().as_str() {
        "flashback" => Some(RuntimeCapability::ExactFlashbackKeyword),
        "bargain" => Some(RuntimeCapability::ExactBargainKeyword),
        "overload" => Some(RuntimeCapability::ExactOverloadKeyword),
        "escape" => Some(RuntimeCapability::ExactEscapeKeyword),
        "enchant" => Some(RuntimeCapability::ExactEnchantKeyword),
        "equip" => Some(RuntimeCapability::ExactEquipKeyword),
        "scry" => Some(RuntimeCapability::ExactScryKeyword),
        _ => None,
    };
    let required_live_shape = match keyword.trim().to_ascii_lowercase().as_str() {
        "flash" => Some(LiveAbilityShape::FlashPermission),
        "ward" => Some(LiveAbilityShape::Ward),
        "enchant" => Some(LiveAbilityShape::AuraSpellTargeting),
        "scry" => Some(LiveAbilityShape::ScryResolution),
        _ => None,
    };
    let runtime_receipt = required_capability
        .and_then(|required| {
            let mut matches = runtime_receipts.iter().filter(|receipt| {
                runtime_receipt_has_exact_contract(receipt)
                    && runtime_receipt_parts(receipt).1.contains(&required)
                    && face_index.is_none_or(|face_index| {
                        runtime_receipt_parts(receipt)
                            .2
                            .covered_oracle_clauses
                            .iter()
                            .all(|clause| clause.face_index == face_index)
                    })
            });
            let receipt = matches.next()?;
            matches.next().is_none().then_some(receipt)
        })
        .or_else(|| {
            let required_shape = required_live_shape?;
            let mut matches = runtime_receipts.iter().filter(|receipt| {
                let RetainedRuntimeReceipt::LiveAbility(live) = receipt else {
                    return false;
                };
                live.shape == required_shape
                    && runtime_receipt_has_exact_contract(receipt)
                    && face_index.is_none_or(|face_index| {
                        live.source_evidence
                            .covered_oracle_clauses
                            .iter()
                            .all(|clause| clause.face_index == face_index)
                    })
            });
            let receipt = matches.next()?;
            matches.next().is_none().then_some(receipt)
        })
        .or_else(|| {
            if !keyword.eq_ignore_ascii_case("treasure") || face_index.is_some() {
                return None;
            }
            let mut matches = runtime_receipts.iter().filter(|receipt| {
                let RetainedRuntimeReceipt::LiveAbility(live) = receipt else {
                    return false;
                };
                live.owns_exact_treasure_token_keyword()
                    && runtime_receipt_has_exact_contract(receipt)
            });
            let receipt = matches.next()?;
            matches.next().is_none().then_some(receipt)
        });
    let Some(runtime_receipt) = runtime_receipt else {
        return characteristic_receipt_dispositions(
            characteristic_receipts,
            leaf_subject,
            face_index,
            card_revision_sha256,
            leaf_evidence_sha256,
            blocker,
        );
    };

    METRICS
        .into_iter()
        .map(|metric| {
            let disposition = match metric {
                ExecutionMetric::RawOpeningComposition => {
                    CoverageDisposition::SafelyIrrelevant {
                        proof: IrrelevanceProof {
                            proof_code: "raw-opening-does-not-execute-card-functions".into(),
                            detail: "Physical opening-card sampling uses only canonical deck membership and zone placement; this metric makes no keep or card-function claim.".into(),
                        },
                    }
                }
                ExecutionMetric::SynergyDescription => CoverageDisposition::ReportOnly {
                    reason: "The exact runtime receipt may corroborate descriptive evidence, but descriptive relationships do not mutate game state or numeric scoring.".into(),
                },
                metric if runtime_receipt_supports_metric(runtime_receipt, metric) => {
                    CoverageDisposition::FullyExecutable {
                        binding: Box::new(executor_binding(
                            runtime_receipt,
                            card_revision_sha256,
                            leaf_evidence_sha256,
                            runtime_receipt_parts(runtime_receipt)
                                .2
                                .covered_oracle_clauses
                                .as_slice(),
                        )),
                    }
                }
                _ => CoverageDisposition::BlockingUnsupported {
                    blocker: blocker.clone(),
                },
            };
            MetricDisposition {
                metric,
                disposition,
            }
        })
        .collect()
}

fn characteristic_subject_for_leaf(subject: &CoverageLeafSubject) -> Option<CharacteristicSubject> {
    match subject {
        CoverageLeafSubject::FaceRelationship => Some(CharacteristicSubject::FaceRelationship),
        CoverageLeafSubject::ManaCost => Some(CharacteristicSubject::ManaCost),
        CoverageLeafSubject::ManaValue => Some(CharacteristicSubject::ManaValue),
        CoverageLeafSubject::Colors => Some(CharacteristicSubject::Colors),
        CoverageLeafSubject::ColorIndicator => Some(CharacteristicSubject::ColorIndicator),
        CoverageLeafSubject::Power => Some(CharacteristicSubject::Power),
        CoverageLeafSubject::Toughness => Some(CharacteristicSubject::Toughness),
        CoverageLeafSubject::TypeLine => Some(CharacteristicSubject::CardTypeProfile),
        CoverageLeafSubject::Keyword(keyword) => Some(CharacteristicSubject::PrintedCombatKeyword(
            keyword.trim().to_ascii_lowercase(),
        )),
        _ => None,
    }
}

fn runtime_receipt_has_exact_contract(receipt: &RetainedRuntimeReceipt) -> bool {
    if let RetainedRuntimeReceipt::Land(receipt) = receipt {
        return receipt.has_exact_contract()
            && executor_id_matches_version(
                receipt.binding.executor_id,
                receipt.binding.executor_version,
            );
    }
    if let RetainedRuntimeReceipt::Interaction(receipt) = receipt {
        return receipt.has_exact_contract()
            && executor_id_matches_version(
                receipt.binding.executor_id,
                receipt.binding.executor_version,
            );
    }
    if let RetainedRuntimeReceipt::Tutor(receipt) = receipt {
        return receipt.has_exact_contract()
            && executor_id_matches_version(
                receipt.binding.executor_id,
                receipt.binding.executor_version,
            );
    }
    if let RetainedRuntimeReceipt::RestrictionProtection(receipt) = receipt {
        return receipt.has_exact_contract()
            && executor_id_matches_version(
                receipt.binding.executor_id,
                receipt.binding.executor_version,
            );
    }
    if let RetainedRuntimeReceipt::Reviewed(receipt) = receipt {
        return receipt.has_exact_contract()
            && executor_id_matches_version(
                receipt.binding.executor_id,
                receipt.binding.executor_version,
            );
    }
    let (
        binding,
        capabilities,
        source_evidence,
        expected_executor_version,
        mut expected_capabilities,
    ) = match receipt {
        RetainedRuntimeReceipt::Atomic(receipt) => {
            let required_terminal_capability = match &receipt.transaction {
                TypedAtomicTransaction::HandMana { .. } => {
                    RuntimeCapability::HandManaAbilityWithoutStack
                }
                _ => RuntimeCapability::CounteredSpellResolutionBoundary,
            };
            let mut expected_capabilities = vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::AtomicInitiationBoundary,
                RuntimeCapability::OrderedResolution,
                required_terminal_capability,
            ];
            if matches!(
                receipt.transaction,
                TypedAtomicTransaction::BargainSearchCastOrHand { .. }
            ) {
                expected_capabilities.push(RuntimeCapability::ExactBargainKeyword);
            }
            (
                &receipt.binding,
                receipt.capabilities.as_slice(),
                &receipt.source_evidence,
                ATOMIC_TRANSACTION_EXECUTOR_VERSION,
                expected_capabilities,
            )
        }
        RetainedRuntimeReceipt::GraveyardReclamation(receipt) => (
            &receipt.binding,
            receipt.capabilities.as_slice(),
            &receipt.source_evidence,
            GRAVEYARD_RECLAMATION_EXECUTOR_VERSION,
            vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::OrderedResolution,
                RuntimeCapability::CounteredSpellResolutionBoundary,
                RuntimeCapability::ExactFlashbackKeyword,
                RuntimeCapability::ExactPhysicalZoneObjectIdentity,
                RuntimeCapability::OptionalUncastSpellCopy,
                RuntimeCapability::OptionalLegalCopyRetarget,
                RuntimeCapability::SourceExitBeforeCopyResolution,
                RuntimeCapability::SourceReturnTriggersAboveCopy,
            ],
        ),
        RetainedRuntimeReceipt::SpellResolutionMana(receipt) => (
            &receipt.binding,
            receipt.capabilities.as_slice(),
            &receipt.source_evidence,
            SPELL_RESOLUTION_MANA_EXECUTOR_VERSION,
            vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::OrderedResolution,
                RuntimeCapability::CounteredSpellResolutionBoundary,
            ],
        ),
        RetainedRuntimeReceipt::ConditionalManaSource(receipt) => (
            &receipt.binding,
            receipt.capabilities.as_slice(),
            &receipt.source_evidence,
            CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION,
            vec![
                RuntimeCapability::CompleteOracleRoot,
                if receipt.source.is_entry_linked() {
                    RuntimeCapability::ExactPermanentEntryProcedure
                } else {
                    RuntimeCapability::LiveBattlefieldManaCondition
                },
            ],
        ),
        RetainedRuntimeReceipt::SacrificeSelfMana(receipt) => (
            &receipt.binding,
            receipt.capabilities.as_slice(),
            &receipt.source_evidence,
            SACRIFICE_SELF_MANA_EXECUTOR_VERSION,
            vec![
                RuntimeCapability::CompleteOracleRoot,
                RuntimeCapability::SacrificeSelfManaAbility,
            ],
        ),
        RetainedRuntimeReceipt::LiveAbility(receipt) => {
            let mut expected = vec![RuntimeCapability::ExactOracleClauseSet];
            if receipt.shape == LiveAbilityShape::EquipmentAttach {
                expected.push(RuntimeCapability::ExactEquipKeyword);
            }
            (
                &receipt.binding,
                receipt.capabilities.as_slice(),
                &receipt.source_evidence,
                LIVE_ABILITY_EXECUTOR_VERSION,
                expected,
            )
        }
        RetainedRuntimeReceipt::Interaction(_)
        | RetainedRuntimeReceipt::Tutor(_)
        | RetainedRuntimeReceipt::RestrictionProtection(_)
        | RetainedRuntimeReceipt::Reviewed(_) => {
            unreachable!(
                "closed interaction and tutor receipts return through their dedicated exact contract"
            )
        }
        RetainedRuntimeReceipt::Land(_) => {
            unreachable!("land receipts return through their dedicated exact contract")
        }
    };
    let Some(source_scope) = capabilities.first().copied() else {
        return false;
    };
    if !matches!(
        source_scope,
        RuntimeCapability::CompleteOracleRoot | RuntimeCapability::ExactOracleClauseSet
    ) {
        return false;
    }
    expected_capabilities[0] = source_scope;
    binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
        && binding.executor_version == expected_executor_version
        && executor_id_matches_version(binding.executor_id, binding.executor_version)
        && source_evidence.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
        && is_sha256_hex(&source_evidence.normalized_oracle_sha256)
        && source_evidence.has_exact_clause_contract()
        && is_sha256_hex(&source_evidence.type_line_sha256)
        && is_sha256_hex(&source_evidence.source_evidence_sha256)
        && capabilities == expected_capabilities
        && match receipt {
            RetainedRuntimeReceipt::LiveAbility(receipt) => receipt.has_exact_contract(),
            _ => true,
        }
}

fn runtime_receipt_supports_metric(
    receipt: &RetainedRuntimeReceipt,
    metric: ExecutionMetric,
) -> bool {
    let (binding, _, _) = runtime_receipt_parts(receipt);
    executor_id_supports_metric(binding.executor_id, binding.executor_version, metric)
}

fn executor_id_matches_version(executor_id: &str, executor_version: &str) -> bool {
    match executor_version {
        ATOMIC_TRANSACTION_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.atomic.hand-mana"
                | "abstract-play.atomic.sacrifice-ritual"
                | "abstract-play.atomic.name-linked-graveyard-ritual"
                | "abstract-play.atomic.sacrifice-tutor"
                | "abstract-play.atomic.threshold-ritual"
                | "abstract-play.atomic.search-random-discard-shuffle"
                | "abstract-play.atomic.temporary-land-sacrifice-mana-grant"
                | "abstract-play.atomic.bargain-search-cast-or-hand"
                | "abstract-play.atomic.opponent-choice-search-split"
        ),
        GRAVEYARD_RECLAMATION_EXECUTOR_VERSION => {
            executor_id == "abstract-play.graveyard-reclamation.sevinne"
        }
        SPELL_RESOLUTION_MANA_EXECUTOR_VERSION => {
            executor_id == "abstract-play.spell-resolution.fixed-mana"
        }
        CONDITIONAL_MANA_SOURCE_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.conditional-mana.imprint-linked-card-colors"
                | "abstract-play.conditional-mana.discard-land-or-fail-entry"
                | "abstract-play.conditional-mana.controlled-legendary-colors"
                | "abstract-play.conditional-mana.metalcraft-any-color"
        ),
        SACRIFICE_SELF_MANA_EXECUTOR_VERSION => {
            executor_id == "abstract-play.activated.sacrifice-self-any-color-mana"
        }
        LIVE_ABILITY_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.ability.static.flash-permission"
                | "abstract-play.ability.trigger.ward"
                | "abstract-play.ability.spell.aura-targeting"
                | "abstract-play.ability.resolution.scry"
                | "abstract-play.ability.static-creature-modifier"
                | "abstract-play.ability.trigger.controller-token"
                | "abstract-play.ability.trigger.draw"
                | "abstract-play.ability.trigger.upkeep-token-life"
                | "abstract-play.ability.lifecycle.quest-counter-token"
                | "abstract-play.ability.static.creature-type-choice"
                | "abstract-play.ability.static.chosen-creature-type"
                | "abstract-play.ability.static.trigger-multiplier"
                | "abstract-play.ability.static.spell-cost-reduction"
                | "abstract-play.ability.static.alternative-spell-cost"
                | "abstract-play.ability.activated.equip"
                | "abstract-play.ability.trigger.cumulative-upkeep"
                | "abstract-play.ability.activated.fixed-mana"
                | "abstract-play.ability.resolution.fixed-draw"
                | "abstract-play.ability.trigger.temporary-power-toughness"
                | "abstract-play.ability.trigger.become-monarch"
                | "abstract-play.ability.static.all-creature-types"
        ),
        INTERACTION_RUNTIME_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.interaction.targeted-destroy"
                | "abstract-play.interaction.targeted-exile"
                | "abstract-play.interaction.targeted-bounce"
                | "abstract-play.interaction.counterspell"
                | "abstract-play.interaction.counter-unless-payment"
                | "abstract-play.interaction.destroy-all"
        ),
        TUTOR_RUNTIME_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.tutor.upkeep-basic-lands"
                | "abstract-play.tutor.artifact-enchantment-top"
                | "abstract-play.tutor.enchantment-hand"
                | "abstract-play.tutor.any-card-hand-lose-three"
        ),
        RESTRICTION_PROTECTION_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.restriction.attack-tax"
                | "abstract-play.restriction.opponent-turn-actions"
                | "abstract-play.protection.keyword-grant"
                | "abstract-play.restriction.aura"
                | "abstract-play.protection.keyword-removal"
                | "abstract-play.protection.complete-turn"
        ),
        ALTERNATIVE_CAST_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.alternative-cast.overload-bounce"
                | "abstract-play.alternative-cast.overload-exile-compensation"
                | "abstract-play.alternative-cast.commander-free-counter"
                | "abstract-play.alternative-cast.escape-aura"
        ),
        CONTINUOUS_TRIGGER_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.continuous.creature-modifier"
                | "abstract-play.trigger.token-creation"
                | "abstract-play.trigger.life-gain"
                | "abstract-play.trigger.attachment-move"
                | "abstract-play.trigger.equipped-death-return"
                | "abstract-play.continuous.spell-cost-reduction"
                | "abstract-play.trigger.temporary-self-modifier"
        ),
        OBJECT_LIFECYCLE_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.lifecycle.linked-exile"
                | "abstract-play.lifecycle.delayed-exile-return"
                | "abstract-play.lifecycle.conditional-self-return"
                | "abstract-play.lifecycle.token-replacement"
                | "abstract-play.lifecycle.creature-entry-counters"
                | "abstract-play.lifecycle.modal-graveyard-return"
                | "abstract-play.lifecycle.aura-land-type-choice"
        ),
        UTILITY_MODAL_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.utility.top-library"
                | "abstract-play.utility.spell-scry-draw"
                | "abstract-play.utility.entry-scry"
                | "abstract-play.utility.damage-prevention"
                | "abstract-play.utility.activated-wipe"
                | "abstract-play.utility.retaliatory-destroy"
                | "abstract-play.utility.modal-creature-interaction"
                | "abstract-play.utility.faerie-threshold-counter"
        ),
        MANA_NETWORK_RUNTIME_EXECUTOR_VERSION => matches!(
            executor_id,
            COMMANDER_IDENTITY_MANA_EXECUTOR_ID
                | CONTROLLED_LAND_CAPABILITY_MANA_EXECUTOR_ID
                | CONTROLLED_LAND_ANY_COLOR_GRANT_EXECUTOR_ID
                | GLOBAL_BASIC_LAND_SUBTYPE_GRANT_EXECUTOR_ID
                | SELF_BOUNCE_DUAL_LAND_EXECUTOR_ID
        ),
        LAND_RUNTIME_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.land.basic-type-mana"
                | "abstract-play.land.fixed-mana"
                | "abstract-play.land.entry.always-tapped"
                | "abstract-play.land.entry.pay-two-life-or-tapped"
                | "abstract-play.land.entry.two-opponents"
                | "abstract-play.land.fetch-two-basic-land-types"
        ),
        CHARACTERISTIC_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.characteristic.modal-double-faced-relationship"
                | "abstract-play.characteristic.mana-cost"
                | "abstract-play.characteristic.mana-value"
                | "abstract-play.characteristic.colors"
                | "abstract-play.characteristic.color-indicator"
                | "abstract-play.characteristic.power"
                | "abstract-play.characteristic.toughness"
                | "abstract-play.characteristic.card-type-profile"
                | "abstract-play.characteristic.printed-combat-keyword.deathtouch"
                | "abstract-play.characteristic.printed-combat-keyword.double-strike"
                | "abstract-play.characteristic.printed-combat-keyword.first-strike"
                | "abstract-play.characteristic.printed-combat-keyword.flying"
                | "abstract-play.characteristic.printed-combat-keyword.haste"
                | "abstract-play.characteristic.printed-combat-keyword.hexproof"
                | "abstract-play.characteristic.printed-combat-keyword.indestructible"
                | "abstract-play.characteristic.printed-combat-keyword.lifelink"
                | "abstract-play.characteristic.printed-combat-keyword.menace"
                | "abstract-play.characteristic.printed-combat-keyword.reach"
                | "abstract-play.characteristic.printed-combat-keyword.shroud"
                | "abstract-play.characteristic.printed-combat-keyword.trample"
                | "abstract-play.characteristic.printed-combat-keyword.vigilance"
                | "abstract-play.characteristic.printed-combat-keyword.defender"
                | "abstract-play.characteristic.printed-keyword.equip"
                | "abstract-play.characteristic.printed-keyword.cumulative-upkeep"
        ),
        CHARACTERISTIC_ORACLE_EXECUTOR_VERSION => matches!(
            executor_id,
            "abstract-play.characteristic-oracle.combat-keyword"
                | "abstract-play.characteristic-oracle.devotion-toughness"
        ),
        _ => false,
    }
}

fn executor_id_supports_metric(
    executor_id: &str,
    executor_version: &str,
    metric: ExecutionMetric,
) -> bool {
    if !executor_id_matches_version(executor_id, executor_version) {
        return false;
    }
    match metric {
        ExecutionMetric::RawOpeningComposition | ExecutionMetric::SynergyDescription => false,
        ExecutionMetric::FunctionalMulligan
        | ExecutionMetric::ManaConsistency
        | ExecutionMetric::GoldfishTiming
        | ExecutionMetric::InterferenceTiming
        | ExecutionMetric::BracketRating => true,
    }
}

fn runtime_receipt_parts(
    receipt: &RetainedRuntimeReceipt,
) -> (
    &RuntimeExecutorBinding,
    &[RuntimeCapability],
    &RuntimeSourceEvidence,
) {
    match receipt {
        RetainedRuntimeReceipt::Atomic(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::GraveyardReclamation(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::SpellResolutionMana(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::ConditionalManaSource(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::SacrificeSelfMana(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::Interaction(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::Tutor(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::RestrictionProtection(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::Reviewed(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::Land(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
        RetainedRuntimeReceipt::LiveAbility(receipt) => (
            &receipt.binding,
            &receipt.capabilities,
            &receipt.source_evidence,
        ),
    }
}

fn executor_binding(
    receipt: &RetainedRuntimeReceipt,
    card_revision_sha256: &str,
    leaf_evidence_sha256: &str,
    covered_oracle_clauses: &[RuntimeOracleClauseEvidence],
) -> ExecutorBinding {
    let (runtime_binding, capabilities, source_evidence) = runtime_receipt_parts(receipt);
    let mut rule_dependencies = vec![if capabilities
        .contains(&RuntimeCapability::CompleteOracleRoot)
    {
        "typed-ability-program:complete-oracle-root".to_string()
    } else {
        "typed-ability-program:exact-oracle-clause-set".to_string()
    }];
    if capabilities.contains(&RuntimeCapability::OrderedResolution) {
        rule_dependencies.push("CR 608.2c".into());
    }
    if capabilities.contains(&RuntimeCapability::CounteredSpellResolutionBoundary) {
        rule_dependencies.push("CR 601.2".into());
        rule_dependencies.push("CR 701.5a".into());
    }
    if capabilities.iter().any(|capability| {
        matches!(
            capability,
            RuntimeCapability::HandManaAbilityWithoutStack
                | RuntimeCapability::LiveBattlefieldManaCondition
                | RuntimeCapability::SacrificeSelfManaAbility
        )
    }) {
        rule_dependencies.push("CR 605.3b".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactPermanentEntryProcedure) {
        rule_dependencies.push("CR 614.1c".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactCommanderColorIdentityMana) {
        rule_dependencies.push("CR 903.4".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactControlledLandManaCapabilities) {
        rule_dependencies.push("CR 106.7".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactControlledLandManaGrant) {
        rule_dependencies.push("CR 611.3b".into());
        rule_dependencies.push("CR 613.1f".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactGlobalBasicLandSubtypeGrant) {
        rule_dependencies.push("CR 305.6".into());
        rule_dependencies.push("CR 613.1d".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactSelfBounceDualLandLifecycle) {
        rule_dependencies.push("CR 603.6a".into());
    }
    if capabilities.contains(&RuntimeCapability::SacrificeSelfManaAbility) {
        rule_dependencies.push("CR 701.17a".into());
        rule_dependencies.push("CR 701.21a".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactFlashbackKeyword) {
        rule_dependencies.push("CR 702.34a".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactBargainKeyword) {
        rule_dependencies.push("CR 702.166a".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactOverloadKeyword) {
        rule_dependencies.push("CR 702.96a".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactEscapeKeyword) {
        rule_dependencies.push("CR 702.138a".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactEnchantKeyword) {
        rule_dependencies.push("CR 702.5".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactEquipKeyword) {
        rule_dependencies.push("CR 702.6".into());
    }
    if capabilities.contains(&RuntimeCapability::ExactScryKeyword) {
        rule_dependencies.push("CR 701.18".into());
    }
    if capabilities.contains(&RuntimeCapability::OptionalUncastSpellCopy) {
        rule_dependencies.push("CR 707.10".into());
        rule_dependencies.push("CR 707.10c".into());
    }
    if matches!(receipt, RetainedRuntimeReceipt::Interaction(_)) {
        rule_dependencies.push("CR 115.1".into());
    }
    if matches!(receipt, RetainedRuntimeReceipt::Tutor(_)) {
        rule_dependencies.push("CR 701.19".into());
        rule_dependencies.push("CR 701.20".into());
    }
    if matches!(receipt, RetainedRuntimeReceipt::RestrictionProtection(_)) {
        rule_dependencies.push("CR 613.1".into());
        rule_dependencies.push("CR 702.26".into());
    }
    if let RetainedRuntimeReceipt::LiveAbility(receipt) = receipt {
        match receipt.shape {
            LiveAbilityShape::FlashPermission => rule_dependencies.push("CR 702.8".into()),
            LiveAbilityShape::Ward => rule_dependencies.push("CR 702.21".into()),
            LiveAbilityShape::AuraSpellTargeting => rule_dependencies.push("CR 702.5".into()),
            LiveAbilityShape::ScryResolution => rule_dependencies.push("CR 701.18".into()),
            _ => {}
        }
        if receipt.owns_exact_treasure_token_keyword() {
            rule_dependencies.push("CR 111.10a".into());
        }
    }
    rule_dependencies.sort();
    rule_dependencies.dedup();

    let mut evidence_tests = vec![
        format!(
            "runtime-executor:{}@{}",
            runtime_binding.executor_id, runtime_binding.executor_version
        ),
        format!("runtime-receipt:{}", runtime_binding.receipt_schema_version),
        "runtime-source:oracle-clause-and-type-binding".into(),
    ];
    evidence_tests.sort();
    evidence_tests.dedup();

    ExecutorBinding {
        receipt_schema_version: runtime_binding.receipt_schema_version.into(),
        executor_id: runtime_binding.executor_id.into(),
        executor_version: runtime_binding.executor_version.into(),
        ability_program_version: source_evidence.ability_program_version.into(),
        runtime_source_evidence_sha256: source_evidence.source_evidence_sha256.clone(),
        normalized_oracle_sha256: source_evidence.normalized_oracle_sha256.clone(),
        normalized_oracle_clause_sha256s: covered_oracle_clauses
            .iter()
            .map(|clause| clause.normalized_clause_sha256.clone())
            .collect(),
        covered_oracle_clauses: covered_oracle_clauses
            .iter()
            .map(OracleClauseBinding::from)
            .collect(),
        type_line_sha256: source_evidence.type_line_sha256.clone(),
        relevant_type_role_mask: source_evidence.relevant_type_role_mask,
        card_revision_sha256: card_revision_sha256.into(),
        leaf_evidence_sha256: leaf_evidence_sha256.into(),
        rule_dependencies,
        evidence_tests,
    }
}

fn characteristic_executor_binding(
    receipt: &CharacteristicRuntimeReceipt,
    card_revision_sha256: &str,
    leaf_evidence_sha256: &str,
) -> ExecutorBinding {
    let mut rule_dependencies = match receipt.subject {
        CharacteristicSubject::FaceRelationship => vec!["CR 712".to_string()],
        CharacteristicSubject::ManaCost | CharacteristicSubject::ManaValue => {
            vec!["CR 202".to_string()]
        }
        CharacteristicSubject::Colors | CharacteristicSubject::ColorIndicator => {
            vec!["CR 105".to_string()]
        }
        CharacteristicSubject::Power | CharacteristicSubject::Toughness => {
            vec!["CR 208".to_string()]
        }
        CharacteristicSubject::CardTypeProfile => vec!["CR 205".to_string()],
        CharacteristicSubject::PrintedCombatKeyword(_) => vec!["CR 702".to_string()],
    };
    rule_dependencies.push(
        match receipt.face_binding {
            CharacteristicFaceBinding::NormalSingleFace => {
                "typed-characteristic:normal-single-face"
            }
            CharacteristicFaceBinding::ModalDoubleFacedFront => {
                "typed-characteristic:modal-double-faced-front"
            }
            CharacteristicFaceBinding::ModalDoubleFacedBack => {
                "typed-characteristic:modal-double-faced-back"
            }
        }
        .into(),
    );
    rule_dependencies.sort();
    rule_dependencies.dedup();

    let mut evidence_tests = vec![
        format!(
            "runtime-executor:{}@{}",
            receipt.binding.executor_id, receipt.binding.executor_version
        ),
        format!("runtime-receipt:{}", receipt.binding.receipt_schema_version),
        "runtime-capability:typed-characteristic".into(),
    ];
    evidence_tests.sort();

    ExecutorBinding {
        receipt_schema_version: receipt.binding.receipt_schema_version.into(),
        executor_id: receipt.binding.executor_id.into(),
        executor_version: receipt.binding.executor_version.into(),
        ability_program_version: receipt.source_evidence.ability_program_version.into(),
        runtime_source_evidence_sha256: receipt.source_evidence.source_evidence_sha256.clone(),
        normalized_oracle_sha256: receipt.source_evidence.normalized_oracle_sha256.clone(),
        normalized_oracle_clause_sha256s: receipt
            .source_evidence
            .normalized_oracle_clause_sha256s
            .clone(),
        covered_oracle_clauses: receipt
            .source_evidence
            .covered_oracle_clauses
            .iter()
            .map(OracleClauseBinding::from)
            .collect(),
        type_line_sha256: receipt.source_evidence.type_line_sha256.clone(),
        relevant_type_role_mask: receipt.source_evidence.relevant_type_role_mask,
        card_revision_sha256: card_revision_sha256.into(),
        leaf_evidence_sha256: leaf_evidence_sha256.into(),
        rule_dependencies,
        evidence_tests,
    }
}

fn function_dispositions(blocker: CoverageBlocker) -> Vec<MetricDisposition> {
    METRICS
        .into_iter()
        .map(|metric| {
            let disposition = match metric {
                ExecutionMetric::RawOpeningComposition => {
                    CoverageDisposition::SafelyIrrelevant {
                        proof: IrrelevanceProof {
                            proof_code: "raw-opening-does-not-execute-card-functions".into(),
                            detail: "Physical opening-card sampling uses only canonical deck membership and zone placement; this metric makes no keep or card-function claim.".into(),
                        },
                    }
                }
                ExecutionMetric::SynergyDescription => CoverageDisposition::ReportOnly {
                    reason: "The leaf may be retained as descriptive evidence, but it cannot mutate game state or numeric scoring.".into(),
                },
                _ => CoverageDisposition::BlockingUnsupported {
                    blocker: blocker.clone(),
                },
            };
            MetricDisposition {
                metric,
                disposition,
            }
        })
        .collect()
}

fn safely_irrelevant_for_all(proof_code: &str, detail: &str) -> Vec<MetricDisposition> {
    METRICS
        .into_iter()
        .map(|metric| MetricDisposition {
            metric,
            disposition: CoverageDisposition::SafelyIrrelevant {
                proof: IrrelevanceProof {
                    proof_code: proof_code.into(),
                    detail: detail.into(),
                },
            },
        })
        .collect()
}

fn build_metric_gates(cards: &[CardCoverageManifest]) -> Vec<MetricCoverageGate> {
    METRICS
        .into_iter()
        .map(|metric| {
            let mut fully_executable_leaf_count = 0u32;
            let mut safely_irrelevant_leaf_count = 0u32;
            let mut report_only_leaf_count = 0u32;
            let mut blocking_leaf_count = 0u32;
            let mut blockers = Vec::new();
            for card in cards {
                for leaf in &card.leaves {
                    let Some(metric_disposition) = leaf
                        .metric_dispositions
                        .iter()
                        .find(|candidate| candidate.metric == metric)
                    else {
                        continue;
                    };
                    match &metric_disposition.disposition {
                        CoverageDisposition::FullyExecutable { .. } => {
                            fully_executable_leaf_count =
                                fully_executable_leaf_count.saturating_add(1);
                        }
                        CoverageDisposition::SafelyIrrelevant { .. } => {
                            safely_irrelevant_leaf_count =
                                safely_irrelevant_leaf_count.saturating_add(1);
                        }
                        CoverageDisposition::ReportOnly { .. } => {
                            report_only_leaf_count = report_only_leaf_count.saturating_add(1);
                        }
                        CoverageDisposition::BlockingUnsupported { blocker } => {
                            blocking_leaf_count = blocking_leaf_count.saturating_add(1);
                            blockers.push(CoverageLeafReference {
                                card_id: card.card_id.clone(),
                                card_name: card.name.clone(),
                                face_index: leaf.face_index,
                                leaf_id: leaf.leaf_id.clone(),
                                blocker: blocker.clone(),
                            });
                        }
                    }
                }
            }
            let state = if blocking_leaf_count > 0 {
                MetricGateState::Blocked
            } else if report_only_leaf_count > 0 {
                MetricGateState::ReportOnly
            } else {
                MetricGateState::Executable
            };
            MetricCoverageGate {
                metric,
                state,
                fully_executable_leaf_count,
                safely_irrelevant_leaf_count,
                report_only_leaf_count,
                blocking_leaf_count,
                blockers,
            }
        })
        .collect()
}

fn manifest_summary(cards: &[CardCoverageManifest]) -> CoverageManifestSummary {
    CoverageManifestSummary {
        card_count: cards.len().min(u32::MAX as usize) as u32,
        face_count: cards
            .iter()
            .map(|card| card.faces.len() as u64)
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32,
        oracle_span_count: cards
            .iter()
            .map(|card| card.oracle_spans.len() as u64)
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32,
        leaf_count: cards
            .iter()
            .map(|card| card.leaves.len() as u64)
            .sum::<u64>()
            .min(u64::from(u32::MAX)) as u32,
    }
}

fn validate_leaf_disposition_shape(leaf: &ExecutionCoverageLeaf) -> Result<(), String> {
    if !leaf_subject_matches_kind(leaf) {
        return Err(format!(
            "leaf subject {:?} is incompatible with kind {:?}",
            leaf.subject, leaf.kind
        ));
    }
    let every_metric_irrelevant = leaf.metric_dispositions.iter().all(|entry| {
        matches!(
            entry.disposition,
            CoverageDisposition::SafelyIrrelevant { .. }
        )
    });
    let universally_irrelevant = matches!(
        leaf.kind,
        CoverageLeafKind::OracleFormatting | CoverageLeafKind::CombinedFaceDelimiter
    ) || (leaf.kind == CoverageLeafKind::SourceSchemaCompleteness
        && every_metric_irrelevant)
        || (leaf.kind == CoverageLeafKind::FaceStructure && every_metric_irrelevant)
        || (leaf.kind == CoverageLeafKind::KeywordAbility
            && matches!(
                &leaf.subject,
                CoverageLeafSubject::Keyword(keyword)
                    if keyword.eq_ignore_ascii_case("constellation")
            )
            && every_metric_irrelevant)
        || (matches!(leaf.subject, CoverageLeafSubject::ProducedMana) && every_metric_irrelevant)
        || (matches!(
            &leaf.subject,
            CoverageLeafSubject::RelatedComponent(component)
                if matches!(
                    component.trim().to_ascii_lowercase().as_str(),
                    "token" | "combo_piece"
                )
        ) && every_metric_irrelevant);
    for entry in &leaf.metric_dispositions {
        let valid = if universally_irrelevant {
            matches!(
                entry.disposition,
                CoverageDisposition::SafelyIrrelevant { .. }
            )
        } else {
            match entry.metric {
                ExecutionMetric::RawOpeningComposition => matches!(
                    entry.disposition,
                    CoverageDisposition::SafelyIrrelevant { .. }
                ),
                ExecutionMetric::SynergyDescription => {
                    matches!(entry.disposition, CoverageDisposition::ReportOnly { .. })
                }
                _ if characteristic_subject_for_leaf(&leaf.subject).is_some() => {
                    match &entry.disposition {
                        CoverageDisposition::FullyExecutable { binding } => {
                            executor_binding_shape_valid(binding, &leaf.evidence_sha256)
                                && (characteristic_binding_matches_leaf(binding, &leaf.subject)
                                    || exact_runtime_keyword_binding_matches_leaf(
                                        binding,
                                        &leaf.subject,
                                    ))
                                && executor_id_supports_metric(
                                    &binding.executor_id,
                                    &binding.executor_version,
                                    entry.metric,
                                )
                        }
                        CoverageDisposition::BlockingUnsupported { .. } => true,
                        _ => false,
                    }
                }
                _ if matches!(
                    leaf.kind,
                    CoverageLeafKind::OracleRulesText | CoverageLeafKind::AtomicityGuard
                ) =>
                {
                    match &entry.disposition {
                        CoverageDisposition::FullyExecutable { binding } => {
                            executor_binding_shape_valid(binding, &leaf.evidence_sha256)
                                && executor_id_supports_metric(
                                    &binding.executor_id,
                                    &binding.executor_version,
                                    entry.metric,
                                )
                        }
                        CoverageDisposition::BlockingUnsupported { .. } => true,
                        _ => false,
                    }
                }
                _ => matches!(
                    entry.disposition,
                    CoverageDisposition::BlockingUnsupported { .. }
                ),
            }
        };
        if !valid {
            return Err(format!(
                "disposition for {:?} is incompatible with compiler {}",
                entry.metric, EXECUTION_COVERAGE_COMPILER_VERSION
            ));
        }
    }
    Ok(())
}

fn exact_runtime_keyword_binding_matches_leaf(
    binding: &ExecutorBinding,
    subject: &CoverageLeafSubject,
) -> bool {
    let CoverageLeafSubject::Keyword(keyword) = subject else {
        return false;
    };
    match keyword.trim().to_ascii_lowercase().as_str() {
        "flashback" => {
            binding.executor_id == "abstract-play.graveyard-reclamation.sevinne"
                && binding.executor_version == GRAVEYARD_RECLAMATION_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.34a")
        }
        "bargain" => {
            binding.executor_id == "abstract-play.atomic.bargain-search-cast-or-hand"
                && binding.executor_version == ATOMIC_TRANSACTION_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.166a")
        }
        "flash" => {
            binding.executor_id == "abstract-play.ability.static.flash-permission"
                && binding.executor_version == LIVE_ABILITY_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.8")
        }
        "ward" => {
            binding.executor_id == "abstract-play.ability.trigger.ward"
                && binding.executor_version == LIVE_ABILITY_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.21")
        }
        "enchant" => {
            matches!(
                (
                    binding.executor_id.as_str(),
                    binding.executor_version.as_str()
                ),
                (
                    "abstract-play.ability.spell.aura-targeting",
                    LIVE_ABILITY_EXECUTOR_VERSION
                ) | (
                    "abstract-play.alternative-cast.escape-aura",
                    ALTERNATIVE_CAST_EXECUTOR_VERSION
                ) | (
                    "abstract-play.continuous.creature-modifier",
                    CONTINUOUS_TRIGGER_EXECUTOR_VERSION
                ) | (
                    "abstract-play.lifecycle.aura-land-type-choice",
                    OBJECT_LIFECYCLE_EXECUTOR_VERSION
                ) | (
                    "abstract-play.restriction.aura",
                    RESTRICTION_PROTECTION_EXECUTOR_VERSION
                )
            ) && binding
                .rule_dependencies
                .iter()
                .any(|dependency| dependency == "CR 702.5")
        }
        "scry" => {
            matches!(
                (
                    binding.executor_id.as_str(),
                    binding.executor_version.as_str()
                ),
                (
                    "abstract-play.ability.resolution.scry",
                    LIVE_ABILITY_EXECUTOR_VERSION
                ) | (
                    "abstract-play.utility.spell-scry-draw",
                    UTILITY_MODAL_EXECUTOR_VERSION
                ) | (
                    "abstract-play.utility.entry-scry",
                    UTILITY_MODAL_EXECUTOR_VERSION
                )
            ) && binding
                .rule_dependencies
                .iter()
                .any(|dependency| dependency == "CR 701.18")
        }
        "overload" => {
            matches!(
                binding.executor_id.as_str(),
                "abstract-play.alternative-cast.overload-bounce"
                    | "abstract-play.alternative-cast.overload-exile-compensation"
            ) && binding.executor_version == ALTERNATIVE_CAST_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.96a")
        }
        "escape" => {
            binding.executor_id == "abstract-play.alternative-cast.escape-aura"
                && binding.executor_version == ALTERNATIVE_CAST_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.138a")
        }
        "equip" => {
            binding.executor_id == "abstract-play.ability.activated.equip"
                && binding.executor_version == LIVE_ABILITY_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 702.6")
        }
        "treasure" => {
            binding.executor_id == "abstract-play.ability.trigger.draw"
                && binding.executor_version == LIVE_ABILITY_EXECUTOR_VERSION
                && binding
                    .rule_dependencies
                    .iter()
                    .any(|dependency| dependency == "CR 111.10a")
        }
        _ => false,
    }
}

fn characteristic_binding_matches_leaf(
    binding: &ExecutorBinding,
    subject: &CoverageLeafSubject,
) -> bool {
    if binding.executor_version != CHARACTERISTIC_EXECUTOR_VERSION {
        return false;
    }
    match subject {
        CoverageLeafSubject::FaceRelationship => {
            binding.executor_id == "abstract-play.characteristic.modal-double-faced-relationship"
        }
        CoverageLeafSubject::ManaCost => {
            binding.executor_id == "abstract-play.characteristic.mana-cost"
        }
        CoverageLeafSubject::ManaValue => {
            binding.executor_id == "abstract-play.characteristic.mana-value"
        }
        CoverageLeafSubject::Colors => binding.executor_id == "abstract-play.characteristic.colors",
        CoverageLeafSubject::ColorIndicator => {
            binding.executor_id == "abstract-play.characteristic.color-indicator"
        }
        CoverageLeafSubject::Power => binding.executor_id == "abstract-play.characteristic.power",
        CoverageLeafSubject::Toughness => {
            binding.executor_id == "abstract-play.characteristic.toughness"
        }
        CoverageLeafSubject::TypeLine => {
            binding.executor_id == "abstract-play.characteristic.card-type-profile"
        }
        CoverageLeafSubject::Keyword(_) => characteristic_subject_for_leaf(subject)
            .is_some_and(|subject| binding.executor_id == subject.executor_id()),
        _ => false,
    }
}

fn leaf_subject_matches_kind(leaf: &ExecutionCoverageLeaf) -> bool {
    match (&leaf.kind, &leaf.subject) {
        (CoverageLeafKind::OracleRulesText, CoverageLeafSubject::OracleRulesText)
        | (CoverageLeafKind::OracleFormatting, CoverageLeafSubject::OracleFormatting)
        | (CoverageLeafKind::CombinedFaceDelimiter, CoverageLeafSubject::CombinedFaceDelimiter)
        | (CoverageLeafKind::FaceStructure, CoverageLeafSubject::FaceRelationship)
        | (CoverageLeafKind::PrintedManaCost, CoverageLeafSubject::ManaCost)
        | (CoverageLeafKind::AtomicityGuard, CoverageLeafSubject::AtomicityGuard)
        | (
            CoverageLeafKind::SourceSchemaCompleteness,
            CoverageLeafSubject::SourceSchemaCompleteness,
        ) => true,
        (CoverageLeafKind::PrintedCharacteristics, subject) => matches!(
            subject,
            CoverageLeafSubject::ManaValue
                | CoverageLeafSubject::TypeLine
                | CoverageLeafSubject::Colors
                | CoverageLeafSubject::ColorIndicator
                | CoverageLeafSubject::ColorIdentity
                | CoverageLeafSubject::ProducedMana
                | CoverageLeafSubject::Power
                | CoverageLeafSubject::Toughness
                | CoverageLeafSubject::Loyalty
                | CoverageLeafSubject::Defense
                | CoverageLeafSubject::HandModifier
                | CoverageLeafSubject::LifeModifier
                | CoverageLeafSubject::AttractionLights
                | CoverageLeafSubject::CommanderLegality
                | CoverageLeafSubject::LegacyCommanderLegality
                | CoverageLeafSubject::GameChanger
        ),
        (CoverageLeafKind::KeywordAbility, CoverageLeafSubject::Keyword(_))
        | (CoverageLeafKind::RelatedComponent, CoverageLeafSubject::RelatedComponent(_))
        | (CoverageLeafKind::UnreviewedUpstreamField, CoverageLeafSubject::UnreviewedField(_)) => {
            true
        }
        _ => false,
    }
}

fn executor_binding_shape_valid(binding: &ExecutorBinding, leaf_evidence_sha256: &str) -> bool {
    let runtime_type_role_mask =
        role::LAND | role::CREATURE | role::ARTIFACT | role::ENCHANTMENT | role::INSTANT_SORCERY;
    binding.receipt_schema_version == RUNTIME_RECEIPT_SCHEMA_VERSION
        && executor_id_matches_version(&binding.executor_id, &binding.executor_version)
        && binding.ability_program_version == EXECUTABLE_ABILITY_PROGRAM_VERSION
        && is_sha256_hex(&binding.runtime_source_evidence_sha256)
        && is_sha256_hex(&binding.normalized_oracle_sha256)
        && !binding.normalized_oracle_clause_sha256s.is_empty()
        && binding
            .normalized_oracle_clause_sha256s
            .iter()
            .all(|digest| is_sha256_hex(digest))
        && !binding.covered_oracle_clauses.is_empty()
        && binding.covered_oracle_clauses.iter().all(|clause| {
            is_sha256_hex(&clause.normalized_clause_sha256)
                && binding
                    .normalized_oracle_clause_sha256s
                    .contains(&clause.normalized_clause_sha256)
        })
        && binding
            .covered_oracle_clauses
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && is_sha256_hex(&binding.type_line_sha256)
        && binding.relevant_type_role_mask & !runtime_type_role_mask == 0
        && is_sha256_hex(&binding.card_revision_sha256)
        && is_sha256_hex(&binding.leaf_evidence_sha256)
        && binding.leaf_evidence_sha256 == leaf_evidence_sha256
        && !binding.rule_dependencies.is_empty()
        && binding
            .rule_dependencies
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && !binding.evidence_tests.is_empty()
        && binding
            .evidence_tests
            .windows(2)
            .all(|pair| pair[0] < pair[1])
}

fn validate_card(card: &CardCoverageManifest) -> Result<(), CoverageManifestError> {
    let invalid_reference = |detail: String| CoverageManifestError::InvalidReference {
        card: card.card_id.clone(),
        detail,
    };
    if card.card_id != card_id(&card.source_record) {
        return Err(invalid_reference(
            "card id does not match the retained source record".into(),
        ));
    }
    let retained_keywords = canonical_keywords(&card.source_record.keywords);
    if card.name != card.source_record.name
        || card.normalized_name != card.source_record.normalized_name
        || card.combined_type_line != card.source_record.type_line
        || card.combined_mana_cost != card.source_record.mana_cost
        || card.oracle_source != card.source_record.oracle_text
        || card.layout != card.source_record.layout.trim().to_ascii_lowercase()
        || card.keywords != retained_keywords
    {
        return Err(invalid_reference(
            "duplicated card fields do not exactly match the retained source record".into(),
        ));
    }
    let revision_material = CardRevisionMaterial {
        card_id: &card.card_id,
        source_record: &card.source_record,
        canonical_keywords: &retained_keywords,
    };
    if sha256_serialized(&revision_material)? != card.oracle_revision_sha256 {
        return Err(invalid_reference(
            "Oracle revision fingerprint does not match the complete retained source record".into(),
        ));
    }

    for (index, span) in card.oracle_spans.iter().enumerate() {
        if span.span_index != index as u32 {
            return Err(invalid_reference(
                "Oracle span indices must be globally contiguous and ordered".into(),
            ));
        }
    }

    let legacy_face_count = if card.source_record.faces.is_empty() {
        Some(legacy_inferred_face_count(&card.source_record))
    } else {
        None
    };
    let expected_face_count = if card.source_record.faces.is_empty() {
        if layout_requires_exact_faces(&card.layout) {
            legacy_face_count.unwrap_or(1).max(2)
        } else {
            legacy_face_count.unwrap_or(1)
        }
    } else {
        card.source_record.faces.len()
    };
    if card.faces.len() != expected_face_count {
        return Err(invalid_reference(format!(
            "compiled face count {} does not match source-derived count {expected_face_count}",
            card.faces.len()
        )));
    }
    let expected_multiface = expected_face_count > 1
        || layout_requires_exact_faces(&card.layout)
        || card.layout == "meld";
    if card.multiface != expected_multiface
        || card.face_alignment_complete
            != face_alignment_complete(
                &card.source_record,
                &card.layout,
                legacy_face_count,
                card.faces.len(),
            )
    {
        return Err(invalid_reference(
            "multiface or face-alignment flags do not match retained layout data".into(),
        ));
    }

    if card.source_record.faces.is_empty() {
        if card
            .oracle_spans
            .iter()
            .any(|span| span.source_reference != OracleSourceReference::TopLevelCard)
        {
            return Err(invalid_reference(
                "a top-level Oracle source contains a span attributed to an exact face".into(),
            ));
        }
        validate_oracle_partition(&card.oracle_source, &card.oracle_spans).map_err(|detail| {
            CoverageManifestError::InvalidOraclePartition {
                card: card.card_id.clone(),
                detail,
            }
        })?;
    } else {
        if card
            .oracle_spans
            .iter()
            .any(|span| span.source_reference != OracleSourceReference::ExactFace)
        {
            return Err(invalid_reference(
                "exact face compilation contains a top-level Oracle span".into(),
            ));
        }
        let mut next_span_index = 0u32;
        for (index, source_face) in card.source_record.faces.iter().enumerate() {
            let face_index = index as u16;
            let face_spans = card
                .oracle_spans
                .iter()
                .filter(|span| span.face_index == Some(face_index))
                .cloned()
                .collect::<Vec<_>>();
            validate_oracle_partition_at(&source_face.oracle_text, &face_spans, next_span_index)
                .map_err(|detail| CoverageManifestError::InvalidOraclePartition {
                    card: card.card_id.clone(),
                    detail: format!("exact face {face_index}: {detail}"),
                })?;
            next_span_index = next_span_index.saturating_add(face_spans.len() as u32);
        }
        if next_span_index as usize != card.oracle_spans.len() {
            return Err(invalid_reference(
                "at least one exact-face Oracle span is not owned by a retained face".into(),
            ));
        }
    }

    let legacy_name_parts = split_combined_field(&card.source_record.name);
    let legacy_type_parts = split_combined_field(&card.source_record.type_line);
    let legacy_cost_parts = card
        .source_record
        .mana_cost
        .as_deref()
        .map(split_combined_field)
        .unwrap_or_default();
    for (index, face) in card.faces.iter().enumerate() {
        if face.face_index != index as u16 {
            return Err(invalid_reference(
                "face indices must be contiguous and ordered".into(),
            ));
        }
        if face.oracle_source_sha256 != sha256_hex(face.oracle_source.as_bytes()) {
            return Err(invalid_reference(format!(
                "face {} Oracle source fingerprint does not match",
                face.face_index
            )));
        }
        if let Some(source_face) = card.source_record.faces.get(index) {
            let expected_relationship = relationship_for_layout(&card.layout, index)
                .unwrap_or(FaceRelationship::UnsupportedLayoutFace);
            if face.oracle_id != source_face.oracle_id
                || face.layout != source_face.layout
                || face.name.as_deref() != nonempty_string(&source_face.name).as_deref()
                || face.mana_value != source_face.mana_value
                || face.type_line.as_deref() != nonempty_string(&source_face.type_line).as_deref()
                || face.mana_cost != source_face.mana_cost
                || face.oracle_source != source_face.oracle_text
                || face.colors != source_face.colors
                || face.color_indicator != source_face.color_indicator
                || face.keywords != source_face.keywords
                || face.produced_mana != source_face.produced_mana
                || face.power != source_face.power
                || face.toughness != source_face.toughness
                || face.loyalty != source_face.loyalty
                || face.defense != source_face.defense
                || face.hand_modifier != source_face.hand_modifier
                || face.life_modifier != source_face.life_modifier
                || face.attraction_lights != source_face.attraction_lights
                || face.image_uri != source_face.image_uri
                || face.relationship != expected_relationship
            {
                return Err(invalid_reference(format!(
                    "face {} does not exactly mirror its retained source face",
                    face.face_index
                )));
            }
        } else {
            let single = card.faces.len() == 1;
            let expected_name = if single {
                nonempty_string(&card.source_record.name)
            } else {
                aligned_part(&legacy_name_parts, card.faces.len(), index)
            };
            let expected_type_line = if single {
                nonempty_string(&card.source_record.type_line)
            } else {
                aligned_part(&legacy_type_parts, card.faces.len(), index)
            };
            let expected_mana_cost = if single {
                card.source_record.mana_cost.clone()
            } else {
                aligned_part(&legacy_cost_parts, card.faces.len(), index)
            };
            let expected_oracle_source = if single {
                card.source_record.oracle_text.as_str()
            } else {
                ""
            };
            if face.oracle_id
                != if single {
                    card.source_record.oracle_id.clone()
                } else {
                    None
                }
                || face.layout
                    != if single {
                        card.source_record.layout.clone()
                    } else {
                        String::new()
                    }
                || face.name != expected_name
                || face.mana_value
                    != if single {
                        card.source_record.root_mana_value
                    } else {
                        None
                    }
                || face.type_line != expected_type_line
                || face.mana_cost != expected_mana_cost
                || face.oracle_source != expected_oracle_source
                || face.colors
                    != if single {
                        card.source_record.colors.clone()
                    } else {
                        Vec::new()
                    }
                || face.color_indicator
                    != if single {
                        card.source_record.color_indicator.clone()
                    } else {
                        Vec::new()
                    }
                || face.keywords
                    != if single {
                        card.source_record.keywords.clone()
                    } else {
                        Vec::new()
                    }
                || face.produced_mana
                    != if single {
                        card.source_record.produced_mana.clone()
                    } else {
                        Vec::new()
                    }
                || face.power
                    != if single {
                        card.source_record.power.clone()
                    } else {
                        None
                    }
                || face.toughness
                    != if single {
                        card.source_record.toughness.clone()
                    } else {
                        None
                    }
                || face.loyalty
                    != if single {
                        card.source_record.loyalty.clone()
                    } else {
                        None
                    }
                || face.defense
                    != if single {
                        card.source_record.defense.clone()
                    } else {
                        None
                    }
                || face.hand_modifier
                    != if single {
                        card.source_record.hand_modifier.clone()
                    } else {
                        None
                    }
                || face.life_modifier
                    != if single {
                        card.source_record.life_modifier.clone()
                    } else {
                        None
                    }
                || face.attraction_lights
                    != if single {
                        card.source_record.attraction_lights.clone()
                    } else {
                        Vec::new()
                    }
                || face.image_uri.is_some()
                || face.relationship != legacy_relationship(&card.layout, card.faces.len(), index)
            {
                return Err(invalid_reference(format!(
                    "legacy face {} does not match its fail-closed source attribution",
                    face.face_index
                )));
            }
        }
    }
    let leaf_ids = card
        .leaves
        .iter()
        .map(|leaf| leaf.leaf_id.as_str())
        .collect::<BTreeSet<_>>();
    if leaf_ids.len() != card.leaves.len() {
        return Err(invalid_reference(
            "leaf ids must be unique within a card".into(),
        ));
    }
    for (index, leaf) in card.leaves.iter().enumerate() {
        if leaf.leaf_id != format!("leaf-{index:04}") {
            return Err(invalid_reference(
                "leaf ids must be contiguous and canonically ordered".into(),
            ));
        }
    }
    for span in &card.oracle_spans {
        if span
            .face_index
            .is_some_and(|face_index| face_index as usize >= card.faces.len())
        {
            return Err(invalid_reference(format!(
                "span {} references missing face {:?}",
                span.span_index, span.face_index
            )));
        }
        let Some(leaf) = card.leaves.iter().find(|leaf| leaf.leaf_id == span.leaf_id) else {
            return Err(invalid_reference(format!(
                "span {} references a missing leaf",
                span.span_index
            )));
        };
        if !leaf.source_span_indices.contains(&span.span_index) {
            return Err(invalid_reference(format!(
                "span {} and leaf `{}` do not reference each other",
                span.span_index, leaf.leaf_id
            )));
        }
        let expected_leaf_kind = match span.kind {
            OracleSourceSpanKind::RulesText => CoverageLeafKind::OracleRulesText,
            OracleSourceSpanKind::Formatting => CoverageLeafKind::OracleFormatting,
            OracleSourceSpanKind::CombinedFaceDelimiter => CoverageLeafKind::CombinedFaceDelimiter,
        };
        if leaf.kind != expected_leaf_kind || leaf.face_index != span.face_index {
            return Err(invalid_reference(format!(
                "span {} is bound to an incompatible primary Oracle leaf",
                span.span_index
            )));
        }
    }
    for leaf in &card.leaves {
        if leaf
            .face_index
            .is_some_and(|face_index| face_index as usize >= card.faces.len())
        {
            return Err(invalid_reference(format!(
                "leaf `{}` references missing face {:?}",
                leaf.leaf_id, leaf.face_index
            )));
        }
        let metrics = leaf
            .metric_dispositions
            .iter()
            .map(|entry| entry.metric)
            .collect::<Vec<_>>();
        if metrics != METRICS {
            return Err(invalid_reference(format!(
                "leaf `{}` must contain every metric exactly once in canonical order",
                leaf.leaf_id
            )));
        }
        for span_index in &leaf.source_span_indices {
            let Some(span) = card.oracle_spans.get(*span_index as usize) else {
                return Err(invalid_reference(format!(
                    "leaf `{}` references missing span {}",
                    leaf.leaf_id, span_index
                )));
            };
            if span.span_index != *span_index
                || leaf
                    .face_index
                    .is_some_and(|face_index| span.face_index != Some(face_index))
            {
                return Err(invalid_reference(format!(
                    "leaf `{}` references a span from an incompatible face",
                    leaf.leaf_id
                )));
            }
        }
        validate_leaf_disposition_shape(leaf)
            .map_err(|detail| invalid_reference(format!("leaf `{}`: {detail}", leaf.leaf_id)))?;
        if leaf.metric_dispositions.iter().any(|entry| {
            matches!(
                &entry.disposition,
                CoverageDisposition::FullyExecutable { binding }
                    if binding.card_revision_sha256 != card.oracle_revision_sha256
            )
        }) {
            return Err(invalid_reference(format!(
                "leaf `{}` executable receipt is not tied to this complete retained card revision",
                leaf.leaf_id
            )));
        }
    }
    if card.oracle_spans.iter().any(|span| {
        card.leaves
            .iter()
            .filter(|leaf| {
                matches!(
                    leaf.kind,
                    CoverageLeafKind::OracleRulesText
                        | CoverageLeafKind::OracleFormatting
                        | CoverageLeafKind::CombinedFaceDelimiter
                ) && leaf.source_span_indices.contains(&span.span_index)
            })
            .count()
            != 1
    }) {
        return Err(invalid_reference(
            "each Oracle source span must be covered by exactly one primary Oracle leaf".into(),
        ));
    }
    let required_face_structure_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::FaceStructure)
        .count();
    let related_component_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::RelatedComponent)
        .count();
    let printed_characteristic_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::PrintedCharacteristics)
        .count();
    let keyword_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::KeywordAbility)
        .count();
    let source_schema_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::SourceSchemaCompleteness)
        .count();
    let unreviewed_field_leaves = card
        .leaves
        .iter()
        .filter(|leaf| leaf.kind == CoverageLeafKind::UnreviewedUpstreamField)
        .count();
    let expected_unreviewed_field_leaves = card.source_record.unreviewed_fields.len()
        + card
            .source_record
            .faces
            .iter()
            .map(|face| face.unreviewed_fields.len())
            .sum::<usize>()
        + card
            .source_record
            .related_components
            .iter()
            .map(|component| component.unreviewed_fields.len())
            .sum::<usize>();
    let expected_keyword_leaves = canonical_keywords(&card.source_record.keywords).len()
        + card
            .source_record
            .faces
            .iter()
            .map(|face| canonical_keywords(&face.keywords).len())
            .sum::<usize>();
    let expected_printed_characteristic_leaves = card
        .faces
        .iter()
        .map(printed_characteristic_leaf_count)
        .sum::<usize>();
    if source_schema_leaves != 1
        || unreviewed_field_leaves != expected_unreviewed_field_leaves
        || required_face_structure_leaves != card.faces.len()
        || related_component_leaves != card.source_record.related_components.len()
        || printed_characteristic_leaves != expected_printed_characteristic_leaves
        || keyword_leaves != expected_keyword_leaves
    {
        return Err(invalid_reference(
            "coverage leaves do not account for source completeness, every unreviewed field, face, related component, and retained functional printed characteristic".into(),
        ));
    }
    for face in &card.faces {
        if face.leaf_ids.iter().collect::<BTreeSet<_>>().len() != face.leaf_ids.len() {
            return Err(invalid_reference(format!(
                "face {} contains duplicate leaf references",
                face.face_index
            )));
        }
        for span_index in &face.oracle_span_indices {
            if card
                .oracle_spans
                .get(*span_index as usize)
                .is_none_or(|span| span.face_index != Some(face.face_index))
            {
                return Err(invalid_reference(format!(
                    "face {} references a span assigned elsewhere",
                    face.face_index
                )));
            }
        }
        let expected_span_indices = card
            .oracle_spans
            .iter()
            .filter(|span| span.face_index == Some(face.face_index))
            .map(|span| span.span_index)
            .collect::<Vec<_>>();
        if face.oracle_span_indices != expected_span_indices {
            return Err(invalid_reference(format!(
                "face {} does not reference every and only its Oracle spans",
                face.face_index
            )));
        }
        let expected_face_leaf_ids = card
            .leaves
            .iter()
            .filter(|leaf| leaf.face_index == Some(face.face_index))
            .map(|leaf| leaf.leaf_id.as_str())
            .collect::<Vec<_>>();
        if face.leaf_ids.iter().map(String::as_str).collect::<Vec<_>>() != expected_face_leaf_ids {
            return Err(invalid_reference(format!(
                "face {} does not reference every and only its face leaves",
                face.face_index
            )));
        }
        for leaf_id in &face.leaf_ids {
            if !leaf_ids.contains(leaf_id.as_str()) {
                return Err(invalid_reference(format!(
                    "face {} references missing leaf `{leaf_id}`",
                    face.face_index
                )));
            }
        }
    }
    Ok(())
}

fn fingerprint_for_parts(
    schema_version: &str,
    compiler_version: &str,
    provenance: &CoverageSnapshotProvenance,
    cards: &[CardCoverageManifest],
    gates: &[MetricCoverageGate],
    summary: &CoverageManifestSummary,
) -> Result<String, CoverageManifestError> {
    Ok(sha256_serialized(&ManifestFingerprintMaterial {
        schema_version,
        compiler_version,
        provenance,
        cards,
        gates,
        summary,
    })?)
}

fn compact_projection_fingerprint(
    projection: &CompactExecutionCoverageManifest,
) -> Result<String, CoverageManifestError> {
    Ok(sha256_serialized(&CompactProjectionFingerprintMaterial {
        schema_version: &projection.schema_version,
        compiler_version: &projection.compiler_version,
        provenance: &projection.provenance,
        gates: &projection.gates,
        summary: &projection.summary,
        fingerprint_sha256: &projection.fingerprint_sha256,
    })?)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_serialized(value: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_vec(value).map(|bytes| sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:X}")
}

fn sha256_hex_lowercase(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
