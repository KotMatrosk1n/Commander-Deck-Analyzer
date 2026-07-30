use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::ability_ir::{ABILITY_IR_VERSION, SYNERGY_GRAPH_VERSION};
use crate::ability_program::EXECUTABLE_ABILITY_PROGRAM_VERSION;
use crate::bounded_oracle_consumer::BOUNDED_ORACLE_CONSUMER_VERSION;
use crate::bounded_oracle_runtime::BOUNDED_ORACLE_RUNTIME_VERSION;
use crate::bounded_oracle_simulation::BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION;
use crate::combo_store::ComboStoreStatus;
use crate::comprehensive_rules::ComprehensiveRulesSnapshot;
use crate::domain::{AnalysisOptions, AnalysisReport, DataStatus};
use crate::domain::{DeckIntent, MulliganPolicy, PilotPolicy};
use crate::early_turn_evaluator::{
    EARLY_ROUTE_EXECUTION_WITNESS_VERSION, EARLY_TURN_EVALUATOR_VERSION,
};
use crate::effects::EFFECT_DESCRIPTOR_VERSION;
use crate::execution_coverage::{
    EXECUTION_COVERAGE_COMPILER_VERSION, EXECUTION_COVERAGE_SCHEMA_VERSION,
};
use crate::interaction_scenarios::{
    INTERACTION_CHECKPOINT_VERSION, INTERACTION_DIRECTIVE_VERSION,
    INTERACTION_SCENARIO_INPUT_VERSION, INTERACTION_SCENARIO_REPORT_VERSION,
};
use crate::keyword_rules_runtime::{KEYWORD_RULES_EVIDENCE_VERSION, KEYWORD_RULES_RUNTIME_VERSION};
use crate::mana::MANA_MODEL_VERSION;
use crate::mechanic_runtime::MECHANIC_RUNTIME_VERSION;
use crate::parser::normalize_card_name;
use crate::policy_store::PolicyPackageSnapshot;
use crate::rules_capabilities::RULE_CAPABILITY_MODEL_VERSION;
use crate::runtime_receipts::{
    KEYWORD_RULES_EXECUTION_BRIDGE_VERSION, RUNTIME_RECEIPT_SCHEMA_VERSION,
};
use crate::scoring::{BRACKET_MODEL_VERSION, SEMANTIC_MODEL_VERSION};
use crate::semantic_store::SemanticPackageSnapshot;
use crate::semantics::{ANNOTATION_MODEL_VERSION, COMBO_CATALOG_VERSION};
use crate::simulation::{
    EFFECTIVE_HAND_STRENGTH_VERSION, INTERACTION_SCENARIO_SEED_DERIVATION_VERSION,
    MAX_INTERACTION_SCENARIO_EPISODES, OPENING_CANDIDATE_COHORT_VERSION, SIMULATION_ENGINE_VERSION,
    TIMING_ENDPOINT_VERSION,
};
use crate::strategic_profile::STRATEGIC_PROFILE_MODEL_VERSION;
use crate::strict_engine::STRICT_ENGINE_VERSION;
use crate::turn_planner::TURN_PLANNER_VERSION;

const CACHE_SCHEMA_VERSION: &str = "1";
pub(crate) const CACHE_KEY_VERSION: &str = "analysis-cache-49";
const ANALYSIS_IMPLEMENTATION_SHA256: &str = env!("CDA_ANALYSIS_IMPLEMENTATION_SHA256");
const MAX_CACHE_ENTRIES: usize = 64;
const ALLOWED_PRODUCTION_SIMULATION_COUNTS: [u32; 3] = [1_000, 5_000, 10_000];
const MINIMUM_PRODUCTION_TURN: u8 = 2;
const MAXIMUM_PRODUCTION_TURN: u8 = 12;
const EARLY_FAILURE_DIAGNOSTIC_HORIZON: u8 = 6;

#[derive(Debug, thiserror::Error)]
pub enum AnalysisCacheError {
    #[error("Analysis cache database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Analysis cache serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Analysis cache file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Analysis cache identity is incomplete: {0}")]
    IncompleteIdentity(&'static str),
}

#[derive(Debug, Clone)]
pub struct AnalysisCache {
    database_path: PathBuf,
}

#[derive(Debug)]
pub struct CachedAnalysis {
    pub created_at: String,
    pub report: AnalysisReport,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheKeyMaterial<'a> {
    key_version: &'static str,
    analysis_implementation_sha256: &'a str,
    canonical_deck: &'a str,
    commanders: Vec<String>,
    options: &'a AnalysisOptions,
    card_data: CardDataFingerprint<'a>,
    combo_data: ComboDataFingerprint<'a>,
    policy_data: PolicyDataFingerprint<'a>,
    semantic_data: SemanticDataFingerprint<'a>,
    comprehensive_rules: ComprehensiveRulesFingerprint<'a>,
    ability_ir: &'static str,
    synergy_graph: &'static str,
    role_model: &'static str,
    annotation_model: &'static str,
    effect_descriptor: &'static str,
    combo_catalog: &'static str,
    strategic_profile: &'static str,
    mana_model: &'static str,
    simulation_engine: &'static str,
    timing_endpoint: &'static str,
    effective_hand_strength: &'static str,
    opening_candidate_cohort: &'static str,
    early_turn_evaluator: &'static str,
    interaction_scenario_input: &'static str,
    interaction_scenario_report: &'static str,
    interaction_directive: &'static str,
    interaction_checkpoint: &'static str,
    interaction_seed_derivation: &'static str,
    interaction_scenario_episode_cap: u32,
    ability_program: &'static str,
    turn_planner: &'static str,
    execution_contract: ExecutionContractFingerprint<'a>,
    scoring_model: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionContractFingerprint<'a> {
    strict_engine: &'a str,
    coverage_schema: &'a str,
    coverage_compiler: &'a str,
    runtime_receipt_schema: &'a str,
    bounded_oracle_runtime: &'a str,
    bounded_oracle_consumer: &'a str,
    bounded_oracle_simulation_bridge: &'a str,
    mechanic_runtime: &'a str,
    keyword_rules_runtime: &'a str,
    keyword_rules_evidence: &'a str,
    keyword_rules_execution_bridge: &'a str,
}

impl ExecutionContractFingerprint<'static> {
    fn current() -> Self {
        Self {
            strict_engine: STRICT_ENGINE_VERSION,
            coverage_schema: EXECUTION_COVERAGE_SCHEMA_VERSION,
            coverage_compiler: EXECUTION_COVERAGE_COMPILER_VERSION,
            runtime_receipt_schema: RUNTIME_RECEIPT_SCHEMA_VERSION,
            bounded_oracle_runtime: BOUNDED_ORACLE_RUNTIME_VERSION,
            bounded_oracle_consumer: BOUNDED_ORACLE_CONSUMER_VERSION,
            bounded_oracle_simulation_bridge: BOUNDED_ORACLE_SIMULATION_BRIDGE_VERSION,
            mechanic_runtime: MECHANIC_RUNTIME_VERSION,
            keyword_rules_runtime: KEYWORD_RULES_RUNTIME_VERSION,
            keyword_rules_evidence: KEYWORD_RULES_EVIDENCE_VERSION,
            keyword_rules_execution_bridge: KEYWORD_RULES_EXECUTION_BRIDGE_VERSION,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardDataFingerprint<'a> {
    card_count: u64,
    last_updated: &'a Option<String>,
    source: &'a str,
    snapshot_sha256: &'a Option<String>,
    schema_version: &'a str,
    ingestor_version: Option<&'a str>,
    alias_catalog_version: Option<&'a str>,
    alias_catalog_sha256: Option<&'a str>,
    alias_catalog_record_count: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComboDataFingerprint<'a> {
    ready: bool,
    upstream_version: Option<&'a str>,
    upstream_timestamp: Option<&'a str>,
    snapshot_sha256: Option<&'a str>,
    variant_count: u64,
    match_version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyDataFingerprint<'a> {
    origin: &'static str,
    schema_version: u16,
    package_version: &'a str,
    effective_date: &'a str,
    snapshot_sha256: &'a str,
    warning: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticDataFingerprint<'a> {
    origin: &'static str,
    schema_version: u16,
    package_version: &'a str,
    effective_date: &'a str,
    verified_at: &'a str,
    snapshot_sha256: &'a str,
    imported_at: Option<&'a str>,
    authenticity_basis: &'a str,
    warning: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComprehensiveRulesFingerprint<'a> {
    schema_version: Option<&'a str>,
    effective_date: Option<&'a str>,
    snapshot_sha256: Option<&'a str>,
    parser_version: Option<&'a str>,
    capability_model: &'static str,
}

pub(crate) struct AnalysisCacheData<'a> {
    pub card_data: &'a DataStatus,
    pub combo_data: Option<&'a ComboStoreStatus>,
    pub policy_data: &'a PolicyPackageSnapshot,
    pub semantic_data: &'a SemanticPackageSnapshot,
    pub comprehensive_rules: Option<&'a ComprehensiveRulesSnapshot>,
}

impl AnalysisCache {
    pub fn new(database_path: impl Into<PathBuf>) -> Result<Self, AnalysisCacheError> {
        let cache = Self {
            database_path: database_path.into(),
        };
        if let Some(parent) = cache.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = cache.open()?;
        initialize_schema(&connection)?;
        Ok(cache)
    }

    pub fn key(
        &self,
        canonical_deck: &str,
        commander_names: &[String],
        options: &AnalysisOptions,
        data: AnalysisCacheData<'_>,
    ) -> Result<String, AnalysisCacheError> {
        self.key_with_analysis_contract(
            canonical_deck,
            commander_names,
            options,
            data,
            ANALYSIS_IMPLEMENTATION_SHA256,
            ExecutionContractFingerprint::current(),
        )
    }

    fn key_with_analysis_contract(
        &self,
        canonical_deck: &str,
        commander_names: &[String],
        options: &AnalysisOptions,
        data: AnalysisCacheData<'_>,
        analysis_implementation_sha256: &str,
        execution_contract: ExecutionContractFingerprint<'_>,
    ) -> Result<String, AnalysisCacheError> {
        validate_cache_identity(analysis_implementation_sha256, &data)?;

        let mut commanders = commander_names
            .iter()
            .map(|name| normalize_card_name(name))
            .collect::<Vec<_>>();
        commanders.sort_unstable();
        commanders.dedup();

        let material = CacheKeyMaterial {
            key_version: CACHE_KEY_VERSION,
            analysis_implementation_sha256,
            canonical_deck,
            commanders,
            options,
            card_data: CardDataFingerprint {
                card_count: data.card_data.card_count,
                last_updated: &data.card_data.last_updated,
                source: &data.card_data.source,
                snapshot_sha256: &data.card_data.snapshot_sha256,
                schema_version: &data.card_data.schema_version,
                ingestor_version: data.card_data.ingestor_version.as_deref(),
                alias_catalog_version: data.card_data.alias_catalog_version.as_deref(),
                alias_catalog_sha256: data.card_data.alias_catalog_sha256.as_deref(),
                alias_catalog_record_count: data.card_data.alias_catalog_record_count,
            },
            combo_data: ComboDataFingerprint {
                ready: data.combo_data.is_some_and(|status| status.ready),
                upstream_version: data
                    .combo_data
                    .and_then(|status| status.upstream_version.as_deref()),
                upstream_timestamp: data
                    .combo_data
                    .and_then(|status| status.upstream_timestamp.as_deref()),
                snapshot_sha256: data
                    .combo_data
                    .and_then(|status| status.snapshot_sha256.as_deref()),
                variant_count: data.combo_data.map_or(0, |status| status.variant_count),
                match_version: crate::combo_store::COMBO_STORE_MATCH_VERSION,
            },
            policy_data: PolicyDataFingerprint {
                origin: data.policy_data.provenance.origin.as_cache_value(),
                schema_version: data.policy_data.package.schema_version,
                package_version: &data.policy_data.package.package_version,
                effective_date: &data.policy_data.package.effective_date,
                snapshot_sha256: &data.policy_data.provenance.snapshot_sha256,
                warning: data.policy_data.provenance.warning.as_deref(),
            },
            semantic_data: SemanticDataFingerprint {
                origin: data.semantic_data.provenance.origin.as_cache_value(),
                schema_version: data.semantic_data.package.schema_version,
                package_version: &data.semantic_data.package.package_version,
                effective_date: &data.semantic_data.package.effective_date,
                verified_at: &data.semantic_data.package.verified_at,
                snapshot_sha256: &data.semantic_data.provenance.snapshot_sha256,
                imported_at: data.semantic_data.provenance.imported_at.as_deref(),
                authenticity_basis: &data.semantic_data.provenance.authenticity_basis,
                warning: data.semantic_data.provenance.warning.as_deref(),
            },
            comprehensive_rules: ComprehensiveRulesFingerprint {
                schema_version: data
                    .comprehensive_rules
                    .map(|rules| rules.schema_version.as_str()),
                effective_date: data
                    .comprehensive_rules
                    .map(|rules| rules.effective_date.as_str()),
                snapshot_sha256: data
                    .comprehensive_rules
                    .map(|rules| rules.snapshot_sha256.as_str()),
                parser_version: data
                    .comprehensive_rules
                    .map(|rules| rules.parser_version.as_str()),
                capability_model: RULE_CAPABILITY_MODEL_VERSION,
            },
            ability_ir: ABILITY_IR_VERSION,
            synergy_graph: SYNERGY_GRAPH_VERSION,
            role_model: SEMANTIC_MODEL_VERSION,
            annotation_model: ANNOTATION_MODEL_VERSION,
            effect_descriptor: EFFECT_DESCRIPTOR_VERSION,
            combo_catalog: COMBO_CATALOG_VERSION,
            strategic_profile: STRATEGIC_PROFILE_MODEL_VERSION,
            mana_model: MANA_MODEL_VERSION,
            simulation_engine: SIMULATION_ENGINE_VERSION,
            timing_endpoint: TIMING_ENDPOINT_VERSION,
            effective_hand_strength: EFFECTIVE_HAND_STRENGTH_VERSION,
            opening_candidate_cohort: OPENING_CANDIDATE_COHORT_VERSION,
            early_turn_evaluator: EARLY_TURN_EVALUATOR_VERSION,
            interaction_scenario_input: INTERACTION_SCENARIO_INPUT_VERSION,
            interaction_scenario_report: INTERACTION_SCENARIO_REPORT_VERSION,
            interaction_directive: INTERACTION_DIRECTIVE_VERSION,
            interaction_checkpoint: INTERACTION_CHECKPOINT_VERSION,
            interaction_seed_derivation: INTERACTION_SCENARIO_SEED_DERIVATION_VERSION,
            interaction_scenario_episode_cap: MAX_INTERACTION_SCENARIO_EPISODES,
            ability_program: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            turn_planner: TURN_PLANNER_VERSION,
            execution_contract,
            scoring_model: BRACKET_MODEL_VERSION,
        };
        let encoded = serde_json::to_vec(&material)?;
        let digest = Sha256::digest(encoded);
        Ok(format!("{digest:x}"))
    }

    pub fn get(&self, key: &str) -> Result<Option<CachedAnalysis>, AnalysisCacheError> {
        let connection = self.open()?;
        let cached = connection
            .query_row(
                "SELECT created_at, report_json
                 FROM analysis_reports
                 WHERE cache_key = ?1",
                [key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;

        let Some((created_at, report_json)) = cached else {
            return Ok(None);
        };
        let report: AnalysisReport = match serde_json::from_str(&report_json) {
            Ok(report) => report,
            Err(_) => {
                connection.execute("DELETE FROM analysis_reports WHERE cache_key = ?1", [key])?;
                return Ok(None);
            }
        };
        if !report_matches_execution_contract(&report) {
            connection.execute("DELETE FROM analysis_reports WHERE cache_key = ?1", [key])?;
            return Ok(None);
        }
        connection.execute(
            "UPDATE analysis_reports SET last_accessed_at = ?2 WHERE cache_key = ?1",
            params![key, Utc::now().to_rfc3339()],
        )?;
        Ok(Some(CachedAnalysis { created_at, report }))
    }

    pub fn put(&self, key: &str, report: &AnalysisReport) -> Result<(), AnalysisCacheError> {
        if report.deck.resolved_cards != report.deck.card_count
            || !report.deck.unresolved_cards.is_empty()
            || !report_matches_execution_contract(report)
        {
            return Ok(());
        }

        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let now = Utc::now().to_rfc3339();
        let report_json = serde_json::to_string(report)?;
        transaction.execute(
            "INSERT INTO analysis_reports (
                cache_key, created_at, last_accessed_at, report_json
             ) VALUES (?1, ?2, ?2, ?3)
             ON CONFLICT(cache_key) DO UPDATE SET
                created_at = excluded.created_at,
                last_accessed_at = excluded.last_accessed_at,
                report_json = excluded.report_json",
            params![key, now, report_json],
        )?;
        transaction.execute(
            "DELETE FROM analysis_reports
             WHERE cache_key IN (
                SELECT cache_key
                FROM analysis_reports
                ORDER BY last_accessed_at DESC
                LIMIT -1 OFFSET ?1
             )",
            [MAX_CACHE_ENTRIES as i64],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn open(&self) -> Result<Connection, rusqlite::Error> {
        let connection = Connection::open(&self.database_path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        Ok(connection)
    }
}

fn report_matches_execution_contract(report: &AnalysisReport) -> bool {
    let Some(manifest) = report.coverage.execution_manifest.as_ref() else {
        return false;
    };
    manifest.validate().is_ok()
        && manifest.schema_version == EXECUTION_COVERAGE_SCHEMA_VERSION
        && manifest.compiler_version == EXECUTION_COVERAGE_COMPILER_VERSION
        && report.win_speed.coverage_manifest_sha256.as_deref()
            == Some(manifest.fingerprint_sha256.as_str())
        && report.win_speed.timing_endpoint_version.as_deref() == Some(TIMING_ENDPOINT_VERSION)
        && report.overview.speed_score_basis.is_some()
        && report.win_speed.baseline_resolved_table_win.is_some()
        && report.win_speed.interfered_resolved_table_win.is_some()
        && report.win_speed.paired_resolved_table_win_delay.is_some()
        && report
            .win_speed
            .cumulative_resolved_table_win_rate
            .is_some()
        && report
            .win_speed
            .cumulative_interfered_resolved_table_win_rate
            .is_some()
        && report_matches_paired_delay_contract(report)
        && report_matches_attempt_recovery_contract(report)
        && report_matches_interaction_scenario_contract(report)
        && report_matches_generic_milestone_contract(report)
        && report.versions.strict_engine.as_deref() == Some(STRICT_ENGINE_VERSION)
        && report.versions.simulation_engine == SIMULATION_ENGINE_VERSION
        && report.versions.effective_hand_strength_model.as_deref()
            == Some(EFFECTIVE_HAND_STRENGTH_VERSION)
        && report.opening_hands.candidate_cohort_version == OPENING_CANDIDATE_COHORT_VERSION
        && is_lowercase_sha256(&report.opening_hands.candidate_cohort_sha256)
        && report.versions.ability_program.as_deref() == Some(EXECUTABLE_ABILITY_PROGRAM_VERSION)
        && report.versions.turn_planner.as_deref() == Some(TURN_PLANNER_VERSION)
        && report.versions.execution_coverage_compiler.as_deref()
            == Some(EXECUTION_COVERAGE_COMPILER_VERSION)
        && report.versions.bracket_model == BRACKET_MODEL_VERSION
        && report.cache.key_version == CACHE_KEY_VERSION
        && report_matches_versioned_data_contract(report)
        && report_matches_deck_identity_contract(report)
        && report_matches_workload_contract(report)
        && matches!(
            report.assumptions.mulligan_policy,
            MulliganPolicy::Aggressive
        )
        && matches!(report.assumptions.pilot_policy, PilotPolicy::Race)
        && matches!(report.assumptions.declared_intent, DeckIntent::Unspecified)
        && report
            .win_speed
            .early_turn_evaluation
            .as_ref()
            .is_some_and(|evaluation| {
                evaluation.model_version == EARLY_TURN_EVALUATOR_VERSION
                    && evaluation.execution_witness_version == EARLY_ROUTE_EXECUTION_WITNESS_VERSION
                    && evaluation.fixed_policy.opening_hand_size == 7
                    && evaluation.fixed_policy.natural_draws_before_turn_one == 1
                    && evaluation.fixed_policy.natural_draws_before_turn_two == 2
                    && evaluation.fixed_policy.aggressive_candidate_hands == 4
            })
        && report.assumptions.seed_exact == report.assumptions.seed.to_string()
}

fn report_matches_versioned_data_contract(report: &AnalysisReport) -> bool {
    let versions = &report.versions;
    let expected_semantic_model =
        format!("{SEMANTIC_MODEL_VERSION}+{ANNOTATION_MODEL_VERSION}+{EFFECT_DESCRIPTOR_VERSION}");
    let expected_combo_prefix = format!("Built-in {COMBO_CATALOG_VERSION}");
    let comprehensive_rules_match = match (
        versions.comprehensive_rules_effective_date.as_deref(),
        versions.comprehensive_rules_snapshot_sha256.as_deref(),
        versions.comprehensive_rules_parser_version.as_deref(),
        versions.rule_capability_model.as_deref(),
    ) {
        (None, None, None, None) => true,
        (Some(effective_date), Some(snapshot), Some(parser), Some(capability_model)) => {
            !effective_date.trim().is_empty()
                && is_lowercase_sha256(snapshot)
                && !parser.trim().is_empty()
                && capability_model == RULE_CAPABILITY_MODEL_VERSION
        }
        _ => false,
    };

    !versions.card_data.trim().is_empty()
        && versions
            .card_snapshot_sha256
            .as_deref()
            .is_none_or(is_lowercase_sha256)
        && !versions.rules_package.trim().is_empty()
        && versions
            .rules_snapshot_sha256
            .as_deref()
            .is_some_and(is_lowercase_sha256)
        && versions
            .rules_package_origin
            .as_deref()
            .is_some_and(|origin| !origin.trim().is_empty())
        && versions.semantic_model == expected_semantic_model
        && versions
            .semantic_package
            .as_deref()
            .is_some_and(|package| !package.trim().is_empty())
        && versions
            .semantic_snapshot_sha256
            .as_deref()
            .is_some_and(is_lowercase_sha256)
        && versions
            .semantic_package_origin
            .as_deref()
            .is_some_and(|origin| !origin.trim().is_empty())
        && versions
            .semantic_authenticity_basis
            .as_deref()
            .is_some_and(|basis| !basis.trim().is_empty())
        && comprehensive_rules_match
        && versions.strategic_profile_model.as_deref() == Some(STRATEGIC_PROFILE_MODEL_VERSION)
        && versions
            .combo_catalog
            .as_deref()
            .is_some_and(|catalog| catalog.starts_with(&expected_combo_prefix))
        && versions
            .combo_snapshot_sha256
            .as_deref()
            .is_none_or(is_lowercase_sha256)
}

fn report_matches_deck_identity_contract(report: &AnalysisReport) -> bool {
    let canonical_deck = report.deck.canonical_deck.as_bytes();
    let expected = format!("{:x}", Sha256::digest(canonical_deck));
    is_lowercase_sha256(&report.deck.canonical_deck_sha256)
        && report.deck.canonical_deck_sha256 == expected
}

fn report_matches_workload_contract(report: &AnalysisReport) -> bool {
    let assumptions = &report.assumptions;
    assumptions.opening_hand_simulations == assumptions.game_simulations
        && ALLOWED_PRODUCTION_SIMULATION_COUNTS.contains(&assumptions.opening_hand_simulations)
        && (MINIMUM_PRODUCTION_TURN..=MAXIMUM_PRODUCTION_TURN).contains(&assumptions.maximum_turn)
        && report.opening_hands.simulations == assumptions.opening_hand_simulations
        && report.win_speed.simulations == assumptions.game_simulations
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_cache_identity(
    analysis_implementation_sha256: &str,
    data: &AnalysisCacheData<'_>,
) -> Result<(), AnalysisCacheError> {
    require_sha256(
        analysis_implementation_sha256,
        "analysis implementation fingerprint",
    )?;
    optional_sha256(
        data.card_data.snapshot_sha256.as_deref(),
        "card data snapshot",
    )?;
    if data.card_data.schema_version.trim().is_empty() {
        return Err(AnalysisCacheError::IncompleteIdentity(
            "card data schema version",
        ));
    }
    optional_sha256(
        data.card_data.alias_catalog_sha256.as_deref(),
        "card alias catalog",
    )?;
    let alias_identity_fields = [
        data.card_data.alias_catalog_version.is_some(),
        data.card_data.alias_catalog_sha256.is_some(),
        data.card_data.alias_catalog_record_count.is_some(),
    ];
    if alias_identity_fields.iter().any(|present| *present)
        && alias_identity_fields.iter().any(|present| !*present)
    {
        return Err(AnalysisCacheError::IncompleteIdentity(
            "card alias catalog metadata",
        ));
    }

    if let Some(combo) = data.combo_data {
        if combo.ready && combo.snapshot_sha256.is_none() {
            return Err(AnalysisCacheError::IncompleteIdentity(
                "ready combo data has no snapshot fingerprint",
            ));
        }
        optional_sha256(combo.snapshot_sha256.as_deref(), "combo data snapshot")?;
    }

    require_sha256(
        &data.policy_data.provenance.snapshot_sha256,
        "policy package snapshot",
    )?;
    require_sha256(
        &data.semantic_data.provenance.snapshot_sha256,
        "semantic package snapshot",
    )?;

    if let Some(rules) = data.comprehensive_rules {
        require_sha256(&rules.snapshot_sha256, "Comprehensive Rules snapshot")?;
    }
    Ok(())
}

fn require_sha256(value: &str, label: &'static str) -> Result<(), AnalysisCacheError> {
    if is_lowercase_sha256(value) {
        Ok(())
    } else {
        Err(AnalysisCacheError::IncompleteIdentity(label))
    }
}

fn optional_sha256(value: Option<&str>, label: &'static str) -> Result<(), AnalysisCacheError> {
    value.map_or(Ok(()), |value| require_sha256(value, label))
}

const RATE_TOLERANCE: f32 = 0.000_001;

fn checked_u32_sum(values: impl IntoIterator<Item = u32>) -> Option<u32> {
    values
        .into_iter()
        .try_fold(0u32, |sum, value| sum.checked_add(value))
}

fn checked_u64_sum(values: impl IntoIterator<Item = u64>) -> Option<u64> {
    values
        .into_iter()
        .try_fold(0u64, |sum, value| sum.checked_add(value))
}

fn rate_matches_count(rate: f32, count: u32, denominator: u32) -> bool {
    denominator > 0
        && rate.is_finite()
        && (0.0..=1.0).contains(&rate)
        && count <= denominator
        && (rate - count as f32 / denominator as f32).abs() <= RATE_TOLERANCE
}

fn rate_uses_denominator(rate: f32, denominator: u32) -> bool {
    if denominator == 0 || !rate.is_finite() || !(0.0..=1.0).contains(&rate) {
        return false;
    }
    let nearest_count = (rate * denominator as f32).round() as u32;
    rate_matches_count(rate, nearest_count, denominator)
}

fn turn_quantile_is_valid(value: Option<f32>, maximum_turn: u8) -> bool {
    value.is_none_or(|turn| turn.is_finite() && (1.0..=f32::from(maximum_turn)).contains(&turn))
}

fn ordered_turn_quantiles(p10: Option<f32>, median: Option<f32>, p90: Option<f32>) -> bool {
    p10.zip(median).is_none_or(|(low, middle)| low <= middle)
        && median.zip(p90).is_none_or(|(middle, high)| middle <= high)
        && p10.zip(p90).is_none_or(|(low, high)| low <= high)
}

fn distribution_matches_contract(
    distribution: &crate::domain::TurnDistribution,
    simulations: u32,
    maximum_turn: u8,
) -> bool {
    if simulations == 0
        || !distribution.demonstrated_rate.is_finite()
        || !(0.0..=1.0).contains(&distribution.demonstrated_rate)
        || !distribution.right_censored_rate.is_finite()
        || !(0.0..=1.0).contains(&distribution.right_censored_rate)
        || (distribution.demonstrated_rate + distribution.right_censored_rate - 1.0).abs()
            > 0.000_01
        || !rate_uses_denominator(distribution.demonstrated_rate, simulations)
    {
        return false;
    }

    let demonstrated_episodes =
        (distribution.demonstrated_rate * simulations as f32).round() as u32;
    let population_rank = |probability: f32| {
        (probability.clamp(0.0, 1.0) * simulations as f32)
            .ceil()
            .max(1.0) as u32
    };
    let population_presence_matches = distribution.p10.is_some()
        == (demonstrated_episodes >= population_rank(0.10))
        && distribution.median.is_some() == (demonstrated_episodes >= population_rank(0.50))
        && distribution.p90.is_some() == (demonstrated_episodes >= population_rank(0.90));
    let conditional_presence_matches = if demonstrated_episodes == 0 {
        distribution.conditional_median.is_none()
            && distribution.conditional_p10.is_none()
            && distribution.conditional_p90.is_none()
    } else {
        distribution.conditional_median.is_some()
            && distribution.conditional_p10.is_some()
            && distribution.conditional_p90.is_some()
    };

    population_presence_matches
        && conditional_presence_matches
        && [
            distribution.p10,
            distribution.median,
            distribution.p90,
            distribution.conditional_p10,
            distribution.conditional_median,
            distribution.conditional_p90,
        ]
        .into_iter()
        .all(|value| turn_quantile_is_valid(value, maximum_turn))
        && ordered_turn_quantiles(distribution.p10, distribution.median, distribution.p90)
        && ordered_turn_quantiles(
            distribution.conditional_p10,
            distribution.conditional_median,
            distribution.conditional_p90,
        )
}

fn paired_delay_matches_contract(
    delay: &crate::domain::PairedTurnDelayReport,
    simulations: u32,
    maximum_turn: u8,
) -> bool {
    let categories = checked_u32_sum([
        delay.observed_pairs,
        delay.prevented_by_turn_cap,
        delay.baseline_not_demonstrated,
        delay.stressed_only,
    ]);
    let quantiles_present = delay.median.is_some() && delay.p10.is_some() && delay.p90.is_some();
    let quantiles_absent = delay.median.is_none() && delay.p10.is_none() && delay.p90.is_none();
    let quantiles_match_observations = if delay.observed_pairs == 0 {
        quantiles_absent
    } else {
        quantiles_present
    };
    let maximum_delay = f32::from(maximum_turn.saturating_sub(1));

    categories == Some(simulations)
        && quantiles_match_observations
        && [delay.p10, delay.median, delay.p90]
            .into_iter()
            .all(|value| {
                value.is_none_or(|turns| turns.is_finite() && turns.abs() <= maximum_delay)
            })
        && ordered_turn_quantiles(delay.p10, delay.median, delay.p90)
}

fn optional_rate_matches(left: Option<f32>, right: Option<f32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.is_finite() && right.is_finite() && (left - right).abs() <= RATE_TOLERANCE
        }
        (None, None) => true,
        _ => false,
    }
}

fn report_matches_paired_delay_contract(report: &AnalysisReport) -> bool {
    let simulations = report.win_speed.simulations;
    let maximum_turn = report.assumptions.maximum_turn;
    let resolved = report.win_speed.paired_resolved_table_win_delay.as_ref();

    paired_delay_matches_contract(
        &report.win_speed.paired_threat_delay,
        simulations,
        maximum_turn,
    ) && paired_delay_matches_contract(
        &report.win_speed.paired_win_attempt_delay,
        simulations,
        maximum_turn,
    ) && resolved
        .is_some_and(|delay| paired_delay_matches_contract(delay, simulations, maximum_turn))
        && optional_rate_matches(
            report.win_speed.median_delay,
            report.win_speed.paired_threat_delay.median,
        )
        && optional_rate_matches(
            report.win_speed.win_attempt_median_delay,
            report.win_speed.paired_win_attempt_delay.median,
        )
        && optional_rate_matches(
            report.win_speed.resolved_table_win_median_delay,
            resolved.and_then(|delay| delay.median),
        )
}

fn report_matches_attempt_recovery_contract(report: &AnalysisReport) -> bool {
    let win_speed = &report.win_speed;
    let simulations = win_speed.simulations;
    let stopped_denominator = win_speed.first_attempt_opportunities.max(1);
    let expected_stopped_rate =
        win_speed.recovery_opportunities as f32 / stopped_denominator as f32;
    let recovery_rate_matches = if win_speed.recovery_opportunities == 0 {
        win_speed.recovery_by_max_turn_rate.is_none()
    } else {
        win_speed.recovery_by_max_turn_rate.is_some_and(|rate| {
            rate_matches_count(
                rate,
                win_speed.recovered_attempts,
                win_speed.recovery_opportunities,
            )
        })
    };

    win_speed.first_attempt_opportunities <= simulations
        && win_speed.recovery_opportunities <= win_speed.first_attempt_opportunities
        && win_speed.first_attempt_stopped_rate.is_finite()
        && (win_speed.first_attempt_stopped_rate - expected_stopped_rate).abs() <= RATE_TOLERANCE
        && win_speed.recovered_attempts <= win_speed.recovery_opportunities
        && recovery_rate_matches
}

fn compact_delay_quantiles_match(
    delay: &crate::interaction_scenarios::CompactPairedDelayDistribution,
    maximum_turn: u8,
) -> bool {
    let observed_present = delay.observed_delay_p10_turns.is_some()
        && delay.observed_delay_median_turns.is_some()
        && delay.observed_delay_p90_turns.is_some();
    let observed_absent = delay.observed_delay_p10_turns.is_none()
        && delay.observed_delay_median_turns.is_none()
        && delay.observed_delay_p90_turns.is_none();
    let observed_match = if delay.observed_pairs == 0 {
        observed_absent
    } else {
        observed_present
    };
    let observed_values = [
        delay.observed_delay_p10_turns,
        delay.observed_delay_median_turns,
        delay.observed_delay_p90_turns,
    ];
    let observed_ordered =
        observed_values[0] <= observed_values[1] && observed_values[1] <= observed_values[2];
    let maximum_delay = f64::from(maximum_turn.saturating_sub(1));

    let censored_present = delay.censored_bound_min_turns.is_some()
        && delay.censored_bound_median_turns.is_some()
        && delay.censored_bound_max_turns.is_some();
    let censored_absent = delay.censored_bound_min_turns.is_none()
        && delay.censored_bound_median_turns.is_none()
        && delay.censored_bound_max_turns.is_none();
    let censored_match = if delay.right_censored_pairs == 0 {
        censored_absent
    } else {
        censored_present
    };
    let censored_ordered = delay
        .censored_bound_min_turns
        .zip(delay.censored_bound_median_turns)
        .is_none_or(|(low, middle)| f64::from(low) <= middle)
        && delay
            .censored_bound_median_turns
            .zip(delay.censored_bound_max_turns)
            .is_none_or(|(middle, high)| middle <= f64::from(high));

    observed_match
        && observed_values.into_iter().all(|value| {
            value.is_none_or(|turns| turns.is_finite() && turns.abs() <= maximum_delay)
        })
        && (delay.observed_pairs == 0 || observed_ordered)
        && censored_match
        && delay
            .censored_bound_min_turns
            .is_none_or(|turns| (0..=i32::from(maximum_turn)).contains(&turns))
        && delay.censored_bound_median_turns.is_none_or(|turns| {
            turns.is_finite() && (0.0..=f64::from(maximum_turn)).contains(&turns)
        })
        && delay
            .censored_bound_max_turns
            .is_none_or(|turns| (0..=i32::from(maximum_turn)).contains(&turns))
        && censored_ordered
}

fn compact_delay_matches_contract(
    delay: &crate::interaction_scenarios::CompactPairedDelayDistribution,
    expected_metric: crate::interaction_scenarios::DelayMetric,
    simulations: u32,
    applicable_episodes: u32,
    effectful_episodes: u32,
    maximum_turn: u8,
) -> bool {
    let category_sum = checked_u32_sum([
        delay.observed_pairs,
        delay.right_censored_pairs,
        delay.no_op_invariant_pairs,
        delay.non_estimable_pairs,
        delay.excluded_pairs,
    ]);
    let effectful_sum = checked_u32_sum([
        delay.observed_pairs,
        delay.right_censored_pairs,
        delay.non_estimable_pairs,
    ]);

    delay.metric == expected_metric
        && delay.total_episode_pairs == simulations
        && delay.applicable_pairs == applicable_episodes
        && delay.effectful_pairs == effectful_episodes
        && category_sum == Some(simulations)
        && effectful_sum == Some(effectful_episodes)
        && delay.no_op_invariant_pairs.checked_add(effectful_episodes) == Some(applicable_episodes)
        && delay.applicable_pairs.checked_add(delay.excluded_pairs) == Some(simulations)
        && compact_delay_quantiles_match(delay, maximum_turn)
}

fn compact_recovery_matches_contract(
    recovery: &crate::interaction_scenarios::CompactRecoverySummary,
    effectful_episodes: u32,
    maximum_turn: u8,
) -> bool {
    let outcomes = recovery.recovered.checked_add(recovery.right_censored);
    let expected_rate = if recovery.opportunities == 0 {
        None
    } else {
        Some(recovery.recovered as f64 / recovery.opportunities as f64)
    };
    let rate_matches = match (recovery.recovered_by_turn_cap_rate, expected_rate) {
        (Some(actual), Some(expected)) => {
            actual.is_finite()
                && (0.0..=1.0).contains(&actual)
                && (actual - expected).abs() <= 0.000_000_001
        }
        (None, None) => true,
        _ => false,
    };
    let quantiles_present = recovery.observed_recovery_p10_turn.is_some()
        && recovery.observed_recovery_median_turn.is_some()
        && recovery.observed_recovery_p90_turn.is_some();
    let quantiles_absent = recovery.observed_recovery_p10_turn.is_none()
        && recovery.observed_recovery_median_turn.is_none()
        && recovery.observed_recovery_p90_turn.is_none();
    let quantiles_match = if recovery.recovered == 0 {
        quantiles_absent
    } else {
        quantiles_present
    };
    let quantiles = [
        recovery.observed_recovery_p10_turn,
        recovery.observed_recovery_median_turn,
        recovery.observed_recovery_p90_turn,
    ];
    let quantiles_ordered = quantiles[0] <= quantiles[1] && quantiles[1] <= quantiles[2];

    recovery.opportunities == effectful_episodes
        && outcomes == Some(recovery.opportunities)
        && rate_matches
        && quantiles_match
        && quantiles.into_iter().all(|turn| {
            turn.is_none_or(|turn| {
                turn.is_finite() && (1.0..=f64::from(maximum_turn)).contains(&turn)
            })
        })
        && (recovery.recovered == 0 || quantiles_ordered)
}

fn report_matches_interaction_scenario_contract(report: &AnalysisReport) -> bool {
    use crate::interaction_scenarios::{
        DelayMetric, INTERACTION_SCENARIO_REPORT_VERSION, InteractionScenario,
        RESPONSE_PRESSURE_LABEL, ScenarioExecutionSource, directive_for,
    };

    let simulations = report.win_speed.simulations;
    let scenario_simulations = simulations.min(MAX_INTERACTION_SCENARIO_EPISODES);
    let maximum_turn = report.assumptions.maximum_turn;
    let scenarios = &report.win_speed.interaction_scenarios;

    simulations > 0
        && scenarios.len() == InteractionScenario::ALL.len()
        && scenarios
            .iter()
            .zip(InteractionScenario::ALL)
            .all(|(scenario, expected_scenario)| {
                let applicable = u32::try_from(scenario.counters.applicable_episodes).ok();
                let effectful =
                    u32::try_from(scenario.counters.effectful_intervention_episodes).ok();
                let applicability_sum = checked_u64_sum([
                    scenario.applicability.applicable_episodes,
                    scenario.applicability.not_applicable_episodes,
                    scenario.applicability.undetermined_episodes,
                ]);
                let counter_applicability_sum = checked_u64_sum([
                    scenario.counters.applicable_episodes,
                    scenario.counters.not_applicable_episodes,
                    scenario.counters.undetermined_episodes,
                ]);
                let applicable_opportunity_sum = scenario
                    .counters
                    .applicable_without_opportunity_episodes
                    .checked_add(scenario.counters.opportunity_episodes);

                scenario.schema_version == INTERACTION_SCENARIO_REPORT_VERSION
                    && scenario.directive == directive_for(expected_scenario)
                    && scenario.measurement.label == RESPONSE_PRESSURE_LABEL
                    && matches!(
                        scenario.measurement.execution_source,
                        ScenarioExecutionSource::ResponsePressure
                    )
                    && !scenario.measurement.claim_boundary.trim().is_empty()
                    && scenario.sampling.master_seed == report.assumptions.seed
                    && scenario.sampling.master_seed_exact
                        == scenario.sampling.master_seed.to_string()
                    && scenario.sampling.seed_derivation_version
                        == INTERACTION_SCENARIO_SEED_DERIVATION_VERSION
                    && scenario.sampling.episode_count == scenario_simulations
                    && scenario.sampling.maximum_turn == u16::from(maximum_turn)
                    && scenario.counters.total_episodes == u64::from(scenario_simulations)
                    && applicability_sum == Some(u64::from(scenario_simulations))
                    && counter_applicability_sum == Some(u64::from(scenario_simulations))
                    && scenario.applicability.applicable_episodes
                        == scenario.counters.applicable_episodes
                    && scenario.applicability.not_applicable_episodes
                        == scenario.counters.not_applicable_episodes
                    && scenario.applicability.undetermined_episodes
                        == scenario.counters.undetermined_episodes
                    && applicable_opportunity_sum == Some(scenario.counters.applicable_episodes)
                    && scenario.counters.effectful_intervention_episodes
                        <= scenario.counters.opportunity_episodes
                    && applicable
                        .zip(effectful)
                        .is_some_and(|(applicable, effectful)| {
                            compact_delay_matches_contract(
                                &scenario.credible_threat_delay,
                                DelayMetric::CredibleThreat,
                                scenario_simulations,
                                applicable,
                                effectful,
                                maximum_turn,
                            ) && compact_delay_matches_contract(
                                &scenario.first_win_attempt_delay,
                                DelayMetric::FirstWinAttempt,
                                scenario_simulations,
                                applicable,
                                effectful,
                                maximum_turn,
                            ) && scenario
                                .resolved_table_win_delay
                                .as_ref()
                                .is_some_and(|delay| {
                                    compact_delay_matches_contract(
                                        delay,
                                        DelayMetric::ResolvedTableWin,
                                        scenario_simulations,
                                        applicable,
                                        effectful,
                                        maximum_turn,
                                    )
                                })
                                && compact_recovery_matches_contract(
                                    &scenario.recovery,
                                    effectful,
                                    maximum_turn,
                                )
                        })
            })
}

fn report_matches_generic_milestone_contract(report: &AnalysisReport) -> bool {
    let maximum_turn = report.assumptions.maximum_turn;
    let simulations = report.win_speed.simulations;
    let curve_is_current = |curve: &[crate::domain::TurnRate]| {
        curve.len() == usize::from(maximum_turn)
            && curve.iter().enumerate().all(|(index, point)| {
                point.turn == (index + 1) as u8
                    && point.rate.is_finite()
                    && (0.0..=1.0).contains(&point.rate)
                    && rate_uses_denominator(point.rate, simulations)
                    && (index == 0 || point.rate >= curve[index - 1].rate)
            })
    };
    let curve_matches_distribution =
        |curve: &[crate::domain::TurnRate], distribution: &crate::domain::TurnDistribution| {
            curve_is_current(curve)
                && curve.last().is_some_and(|point| {
                    (point.rate - distribution.demonstrated_rate).abs() <= RATE_TOLERANCE
                })
        };
    let route_curve_is_current = |curve: &[crate::domain::TurnRate], attempts: u32, rate: f32| {
        simulations > 0
            && attempts <= simulations
            && rate.is_finite()
            && (0.0..=1.0).contains(&rate)
            && curve_is_current(curve)
            && curve
                .last()
                .is_some_and(|point| (point.rate - rate).abs() <= RATE_TOLERANCE)
            && (rate - attempts as f32 / simulations as f32).abs() <= RATE_TOLERANCE
    };
    let provenance = &report.win_speed.attempt_provenance;
    let required_kinds = [
        crate::domain::GenericMilestoneKind::Engine,
        crate::domain::GenericMilestoneKind::Combat,
        crate::domain::GenericMilestoneKind::EngineAndCombat,
    ];

    let baseline_milestone_sum = provenance
        .generic_milestone_kinds
        .iter()
        .try_fold(0u32, |sum, milestone| {
            sum.checked_add(milestone.baseline_episodes)
        });
    let interfered_milestone_sum = provenance
        .generic_milestone_kinds
        .iter()
        .try_fold(0u32, |sum, milestone| {
            sum.checked_add(milestone.interfered_episodes)
        });

    distribution_matches_contract(&report.win_speed.baseline, simulations, maximum_turn)
        && distribution_matches_contract(&report.win_speed.interfered, simulations, maximum_turn)
        && distribution_matches_contract(
            &report.win_speed.baseline_win_attempt,
            simulations,
            maximum_turn,
        )
        && distribution_matches_contract(
            &report.win_speed.interfered_win_attempt,
            simulations,
            maximum_turn,
        )
        && report
            .win_speed
            .baseline_resolved_table_win
            .as_ref()
            .is_some_and(|distribution| {
                distribution_matches_contract(distribution, simulations, maximum_turn)
            })
        && report
            .win_speed
            .interfered_resolved_table_win
            .as_ref()
            .is_some_and(|distribution| {
                distribution_matches_contract(distribution, simulations, maximum_turn)
            })
        && distribution_matches_contract(
            &report.win_speed.baseline_model_pace,
            simulations,
            maximum_turn,
        )
        && distribution_matches_contract(
            &report.win_speed.interfered_model_pace,
            simulations,
            maximum_turn,
        )
        && distribution_matches_contract(
            &report.win_speed.baseline_generic_conversion_milestone,
            simulations,
            maximum_turn,
        )
        && distribution_matches_contract(
            &report.win_speed.interfered_generic_conversion_milestone,
            simulations,
            maximum_turn,
        )
        && curve_matches_distribution(
            &report.win_speed.cumulative_threat_rate,
            &report.win_speed.baseline,
        )
        && curve_matches_distribution(
            &report.win_speed.cumulative_interfered_threat_rate,
            &report.win_speed.interfered,
        )
        && curve_matches_distribution(
            &report.win_speed.cumulative_win_attempt_rate,
            &report.win_speed.baseline_win_attempt,
        )
        && curve_matches_distribution(
            &report.win_speed.cumulative_interfered_win_attempt_rate,
            &report.win_speed.interfered_win_attempt,
        )
        && report
            .win_speed
            .cumulative_resolved_table_win_rate
            .as_ref()
            .zip(report.win_speed.baseline_resolved_table_win.as_ref())
            .is_some_and(|(curve, distribution)| curve_matches_distribution(curve, distribution))
        && report
            .win_speed
            .cumulative_interfered_resolved_table_win_rate
            .as_ref()
            .zip(report.win_speed.interfered_resolved_table_win.as_ref())
            .is_some_and(|(curve, distribution)| curve_matches_distribution(curve, distribution))
        && curve_matches_distribution(
            &report
                .win_speed
                .cumulative_generic_conversion_milestone_rate,
            &report.win_speed.baseline_generic_conversion_milestone,
        )
        && curve_matches_distribution(
            &report
                .win_speed
                .cumulative_interfered_generic_conversion_milestone_rate,
            &report.win_speed.interfered_generic_conversion_milestone,
        )
        && provenance.explicit_routes.iter().all(|route| {
            distribution_matches_contract(&route.baseline_first_attempt, simulations, maximum_turn)
                && distribution_matches_contract(
                    &route.interfered_first_attempt,
                    simulations,
                    maximum_turn,
                )
                && (route.baseline_first_attempt.demonstrated_rate - route.baseline_rate).abs()
                    <= RATE_TOLERANCE
                && (route.interfered_first_attempt.demonstrated_rate - route.interfered_rate).abs()
                    <= RATE_TOLERANCE
                && route_curve_is_current(
                    &route.cumulative_baseline_attempt_rate,
                    route.baseline_attempts,
                    route.baseline_rate,
                )
                && route_curve_is_current(
                    &route.cumulative_interfered_attempt_rate,
                    route.interfered_attempts,
                    route.interfered_rate,
                )
        })
        && provenance.early_failure_horizon == maximum_turn.min(EARLY_FAILURE_DIAGNOSTIC_HORIZON)
        && provenance.early_turn_blockers.iter().all(|blocker| {
            (1..=provenance.early_failure_horizon).contains(&blocker.turn)
                && blocker.episodes <= simulations
                && rate_matches_count(blocker.rate, blocker.episodes, simulations)
        })
        && provenance.generic_milestone_kinds.len() == required_kinds.len()
        && required_kinds.into_iter().all(|kind| {
            provenance
                .generic_milestone_kinds
                .iter()
                .filter(|report| report.kind == kind)
                .count()
                == 1
        })
        && provenance.generic_milestone_kinds.iter().all(|milestone| {
            milestone.baseline_episodes <= report.win_speed.simulations
                && milestone.interfered_episodes <= report.win_speed.simulations
                && rate_matches_count(
                    milestone.baseline_rate,
                    milestone.baseline_episodes,
                    simulations,
                )
                && rate_matches_count(
                    milestone.interfered_rate,
                    milestone.interfered_episodes,
                    simulations,
                )
        })
        && baseline_milestone_sum.is_some_and(|sum| {
            rate_matches_count(
                report
                    .win_speed
                    .baseline_generic_conversion_milestone
                    .demonstrated_rate,
                sum,
                simulations,
            )
        })
        && interfered_milestone_sum.is_some_and(|sum| {
            rate_matches_count(
                report
                    .win_speed
                    .interfered_generic_conversion_milestone
                    .demonstrated_rate,
                sum,
                simulations,
            )
        })
}

fn initialize_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS cache_metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
         );",
    )?;
    let current_version = connection
        .query_row(
            "SELECT value FROM cache_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if current_version.as_deref() != Some(CACHE_SCHEMA_VERSION) {
        connection.execute_batch(
            "DROP TABLE IF EXISTS analysis_reports;
             DELETE FROM cache_metadata;",
        )?;
        connection.execute(
            "INSERT INTO cache_metadata (key, value)
             VALUES ('schema_version', ?1)",
            [CACHE_SCHEMA_VERSION],
        )?;
    }
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS analysis_reports (
            cache_key TEXT PRIMARY KEY NOT NULL,
            created_at TEXT NOT NULL,
            last_accessed_at TEXT NOT NULL,
            report_json TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS analysis_reports_last_accessed
         ON analysis_reports(last_accessed_at);",
    )?;
    Ok(())
}
