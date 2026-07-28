//! Fail-closed execution-coverage contracts.
//!
//! This module does not promote the existing heuristic semantic model to an
//! executable rules engine. It provides a lossless, versioned manifest that a
//! future strict simulator can use as a preflight gate. Current combined card
//! records are split only where their stored separators make that safe; any
//! missing face relationship, cost attribution, Oracle clause, keyword, or
//! executor remains explicitly blocking for functional metrics.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const EXECUTION_COVERAGE_SCHEMA_VERSION: &str = "commander-execution-coverage-manifest/v4";
pub const EXECUTION_COVERAGE_COMPILER_VERSION: &str = "execution-coverage-0.4";
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
    pub executor_id: String,
    pub executor_version: String,
    pub rule_dependencies: Vec<String>,
    pub evidence_tests: Vec<String>,
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
    FullyExecutable { binding: ExecutorBinding },
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
        let (kind, dispositions) = match span.kind {
            OracleSourceSpanKind::Formatting => (
                CoverageLeafKind::OracleFormatting,
                safely_irrelevant_for_all(
                    "oracle-formatting",
                    "Whitespace formatting has no rules effect after exact source partitioning.",
                ),
            ),
            OracleSourceSpanKind::CombinedFaceDelimiter => (
                CoverageLeafKind::CombinedFaceDelimiter,
                safely_irrelevant_for_all(
                    "combined-record-face-delimiter",
                    "This delimiter was inserted by the current combined-record storage format.",
                ),
            ),
            OracleSourceSpanKind::RulesText => {
                let blocker = classify_rules_text(&span.text);
                (
                    CoverageLeafKind::OracleRulesText,
                    function_dispositions(blocker),
                )
            }
        };
        let leaf_id = push_leaf(
            &mut leaves,
            &mut faces,
            kind,
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
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::FaceStructure,
            face_index,
            Vec::new(),
            function_dispositions(structure_blocker),
            &structural_evidence,
        );
        let characteristics_evidence = serde_json::to_vec(&(
            &faces[index].oracle_id,
            &faces[index].layout,
            faces[index].mana_value,
            &faces[index].type_line,
            &faces[index].colors,
            &faces[index].color_indicator,
            &faces[index].produced_mana,
            &faces[index].power,
            &faces[index].toughness,
            &faces[index].loyalty,
            &faces[index].defense,
            &faces[index].hand_modifier,
            &faces[index].life_modifier,
            &faces[index].attraction_lights,
        ))?;
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::PrintedCharacteristics,
            face_index,
            Vec::new(),
            function_dispositions(CoverageBlocker {
                blocker_code: "printed-characteristics-executor-unbound".into(),
                detail: "The exact printed characteristics are retained, but types, subtypes, supertypes, power/toughness, loyalty, defense, colors, color indicators, and implicit rules are not bound to an executable model.".into(),
            }),
            &characteristics_evidence,
        );

        if let Some(cost) = faces[index].mana_cost.clone() {
            for blocker in classify_printed_cost(&cost) {
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::PrintedManaCost,
                    face_index,
                    Vec::new(),
                    function_dispositions(blocker),
                    cost.as_bytes(),
                );
            }
        }
        if has_exact_faces {
            for keyword in canonical_keywords(&faces[index].keywords) {
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::KeywordAbility,
                    face_index,
                    Vec::new(),
                    function_dispositions(CoverageBlocker {
                        blocker_code: "face-keyword-executor-missing".into(),
                        detail: format!(
                            "Face keyword `{keyword}` has no versioned executable binding in {}.",
                            EXECUTION_COVERAGE_COMPILER_VERSION
                        ),
                    }),
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
                push_leaf(
                    &mut leaves,
                    &mut faces,
                    CoverageLeafKind::AtomicityGuard,
                    face_index,
                    rules_spans.clone(),
                    function_dispositions(blocker),
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
            push_leaf(
                &mut leaves,
                &mut faces,
                CoverageLeafKind::AtomicityGuard,
                None,
                all_rules_spans.clone(),
                function_dispositions(blocker),
                record.oracle_text.as_bytes(),
            );
        }
    }
    for keyword in &keywords {
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::KeywordAbility,
            None,
            Vec::new(),
            function_dispositions(CoverageBlocker {
                blocker_code: "keyword-executor-missing".into(),
                detail: format!(
                    "Keyword `{keyword}` has no versioned executable binding in {}.",
                    EXECUTION_COVERAGE_COMPILER_VERSION
                ),
            }),
            keyword.as_bytes(),
        );
    }

    // Hash the complete retained source record for this top-level leaf. This
    // deliberately includes every current root characteristic and future
    // retained field, avoiding tuple-size/omission hazards.
    let top_characteristics = serde_json::to_vec(record)?;
    push_leaf(
        &mut leaves,
        &mut faces,
        CoverageLeafKind::PrintedCharacteristics,
        None,
        Vec::new(),
        function_dispositions(CoverageBlocker {
            blocker_code: "top-level-characteristics-executor-unbound".into(),
            detail: "Oracle identity, layout, exact root mana value, colors, color identity, produced mana, top-level combat/loyalty/defense values, Vanguard modifiers, Attraction lights, Game Changer designation, and exact Commander legality are retained but not all bound to strict execution.".into(),
        }),
        &top_characteristics,
    );

    for component in &record.related_components {
        let component_evidence = serde_json::to_vec(component)?;
        let component_complete = !component.id.trim().is_empty()
            && !component.component.trim().is_empty()
            && !component.name.trim().is_empty();
        push_leaf(
            &mut leaves,
            &mut faces,
            CoverageLeafKind::RelatedComponent,
            None,
            Vec::new(),
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
            }),
            &component_evidence,
        );
    }

    let revision_material = CardRevisionMaterial {
        card_id: &id,
        source_record: record,
        canonical_keywords: &keywords,
    };
    let oracle_revision_sha256 = sha256_serialized(&revision_material)?;
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
                "The legacy record has no retained Scryfall layout, so even a single apparent face cannot be certified against the schema-4 source.",
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
                "schema4-face-data-missing",
                "This layout requires two exact schema-4 face records; combined top-level strings cannot substitute for them.",
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

fn push_leaf(
    leaves: &mut Vec<ExecutionCoverageLeaf>,
    faces: &mut [FaceCoverageManifest],
    kind: CoverageLeafKind,
    face_index: Option<u16>,
    source_span_indices: Vec<u32>,
    metric_dispositions: Vec<MetricDisposition>,
    evidence: &[u8],
) -> String {
    let leaf_id = format!("leaf-{:04}", leaves.len());
    leaves.push(ExecutionCoverageLeaf {
        leaf_id: leaf_id.clone(),
        kind,
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
    let universally_irrelevant = matches!(
        leaf.kind,
        CoverageLeafKind::OracleFormatting | CoverageLeafKind::CombinedFaceDelimiter
    ) || (leaf.kind == CoverageLeafKind::SourceSchemaCompleteness
        && leaf.metric_dispositions.iter().all(|entry| {
            matches!(
                entry.disposition,
                CoverageDisposition::SafelyIrrelevant { .. }
            )
        }));
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
    if source_schema_leaves != 1
        || unreviewed_field_leaves != expected_unreviewed_field_leaves
        || required_face_structure_leaves != card.faces.len()
        || related_component_leaves != card.source_record.related_components.len()
        || printed_characteristic_leaves != card.faces.len() + 1
        || keyword_leaves != expected_keyword_leaves
    {
        return Err(invalid_reference(
            "coverage leaves do not account for source completeness, every unreviewed field, face, related component, and top-level characteristic set".into(),
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
