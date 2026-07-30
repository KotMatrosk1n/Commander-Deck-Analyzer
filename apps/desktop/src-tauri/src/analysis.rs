use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::cache::{AnalysisCache, AnalysisCacheData, CACHE_KEY_VERSION};
use crate::card_data::CardRepository;
use crate::combo_store::{
    ComboDeckCard, ComboStore, LocalComboMatch, MatchRelevance, TableLethality,
};
use crate::comprehensive_rules::ComprehensiveRulesSnapshot;
use crate::domain::{
    AnalysisOptions, AnalysisProgress, AnalysisReport, AnalysisStage, AnalyzeRequest,
    CardDefinition, DeckEntry, DeckIntent, InteractionProfile, KnownLine, KnownLineOutcome,
    LineRequirement, MulliganPolicy, PilotPolicy,
};
use crate::execution_coverage::{
    CombinedCardRecord, CoverageSnapshotProvenance, ExecutionMetric,
    build_execution_coverage_manifest,
};
use crate::mana::build_mana_model_with_commanders;
use crate::parser::{normalize_card_name, parse_decklist};
use crate::policy_store::PolicyPackageSnapshot;
use crate::scoring::{ScoreInputs, score_analysis};
use crate::semantic_store::SemanticPackageSnapshot;
use crate::semantics::{COMBO_CATALOG_VERSION, compile_deck_with_rules_and_semantic_overrides};
use crate::simulation::{
    SIMULATION_ENGINE_VERSION, simulate_opening_hands_with_mana, simulate_win_speed_with_mana,
};

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("Analysis cancelled.")]
    Cancelled,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    CardData(#[from] crate::card_data::CardDataError),
    #[error("{0}")]
    Policy(#[from] crate::rules::PolicyPackageError),
    #[error("{0}")]
    Simulation(#[from] crate::simulation::SimulationError),
    #[error("{0}")]
    ExecutionCoverage(#[from] crate::execution_coverage::CoverageManifestError),
    #[error("The analysis worker stopped unexpectedly: {0}")]
    Worker(String),
}

#[derive(Debug, Clone)]
pub struct AnalysisSnapshots {
    pub policy: PolicyPackageSnapshot,
    pub semantics: SemanticPackageSnapshot,
    pub comprehensive_rules: Option<ComprehensiveRulesSnapshot>,
}

struct InternalAnalysisOptions {
    cache: Option<AnalysisCache>,
}

#[derive(Debug)]
struct ResolvedDeckIdentity {
    entries: Vec<DeckEntry>,
    commander_names: Vec<String>,
    canonical_text: String,
    unique_card_count: usize,
}

const ALLOWED_PRODUCTION_SIMULATION_COUNTS: [u32; 3] = [1_000, 5_000, 10_000];
const MINIMUM_PRODUCTION_TURN: u8 = 2;
const MAXIMUM_PRODUCTION_TURN: u8 = 12;

fn interaction_progress_label(profile: InteractionProfile) -> &'static str {
    match profile {
        InteractionProfile::None => "Simulate no-interaction paired control",
        InteractionProfile::Light => "Apply mild interaction pressure",
        InteractionProfile::Typical => "Apply moderate interaction pressure",
        InteractionProfile::HighPower => "Apply cEDH interaction pressure",
    }
}

pub async fn analyze(
    repository: CardRepository,
    combo_store: ComboStore,
    snapshots: AnalysisSnapshots,
    cache: Option<AnalysisCache>,
    request: AnalyzeRequest,
    cancellation: Arc<AtomicBool>,
    report: impl Fn(AnalysisProgress) + Clone + Send + Sync + 'static,
) -> Result<AnalysisReport, AnalysisError> {
    let request = with_objective_production_policy(request)?;
    analyze_internal(
        repository,
        combo_store,
        snapshots,
        InternalAnalysisOptions { cache },
        request,
        cancellation,
        report,
    )
    .await
}

/// The analyzer has one reproducible pilot policy while preserving the
/// caller's explicit interaction profile. Legacy mulligan, pilot, and declared
/// intent knobs are ignored; workload and interaction controls remain explicit
/// reproducibility inputs. Workloads are validated instead of being silently
/// overwritten or clamped.
///
/// Online card-resolution consent and an explicit reproducibility seed are
/// operational inputs rather than evaluation policy, so those two values are
/// preserved.
fn with_objective_production_policy(
    mut request: AnalyzeRequest,
) -> Result<AnalyzeRequest, AnalysisError> {
    let opening_hand_simulations = request.options.opening_hand_simulations;
    let game_simulations = request.options.game_simulations;
    let maximum_turn = request.options.maximum_turn;
    if opening_hand_simulations != game_simulations {
        return Err(AnalysisError::Validation(
            "Opening-hand and paired-game trial counts must match.".into(),
        ));
    }
    #[cfg(not(test))]
    let local_diagnostic_count_allowed = false;
    if !ALLOWED_PRODUCTION_SIMULATION_COUNTS.contains(&opening_hand_simulations)
        && !local_diagnostic_count_allowed
    {
        return Err(AnalysisError::Validation(
            "Analysis trials must be exactly 1,000, 5,000, or 10,000.".into(),
        ));
    }
    if !(MINIMUM_PRODUCTION_TURN..=MAXIMUM_PRODUCTION_TURN).contains(&maximum_turn) {
        return Err(AnalysisError::Validation(format!(
            "Analysis maximumTurn must be between {MINIMUM_PRODUCTION_TURN} and {MAXIMUM_PRODUCTION_TURN}."
        )));
    }

    let allow_online_card_resolution = request.options.allow_online_card_resolution;
    let interaction_profile = request.options.interaction_profile;
    let seed = request.options.seed;
    request.options = AnalysisOptions {
        opening_hand_simulations,
        game_simulations,
        maximum_turn,
        mulligan_policy: MulliganPolicy::Aggressive,
        pilot_policy: PilotPolicy::Race,
        interaction_profile,
        // Declared intent is subjective and must not bias the rating. The
        // aggressive mulligan policy independently selects the competitive
        // search envelope in the simulator.
        declared_intent: DeckIntent::Unspecified,
        allow_online_card_resolution,
        seed,
    };
    Ok(request)
}

async fn analyze_internal(
    repository: CardRepository,
    combo_store: ComboStore,
    snapshots: AnalysisSnapshots,
    internal_options: InternalAnalysisOptions,
    request: AnalyzeRequest,
    cancellation: Arc<AtomicBool>,
    report: impl Fn(AnalysisProgress) + Clone + Send + Sync + 'static,
) -> Result<AnalysisReport, AnalysisError> {
    let InternalAnalysisOptions { cache } = internal_options;
    let AnalysisSnapshots {
        policy: policy_snapshot,
        semantics: semantic_snapshot,
        comprehensive_rules,
    } = snapshots;
    let started = Instant::now();
    ensure_not_cancelled(&cancellation)?;
    emit(
        &report,
        &request.run_id,
        AnalysisStage::Validating,
        "Validate decklist",
        0,
        1,
        0.02,
        "Parsing quantities, sections, and commanders",
    );
    let parsed = parse_decklist(&request.deck_text);
    if !parsed.is_commander_sized {
        return Err(AnalysisError::Validation(format!(
            "Commander analysis requires exactly 100 cards including commanders; this list contains {}.",
            parsed.card_count
        )));
    }

    let submitted_commander_names = if request.commander_names.is_empty() {
        parsed.commanders.clone()
    } else {
        request.commander_names.clone()
    };
    if submitted_commander_names.is_empty() {
        return Err(AnalysisError::Validation(
            "Select at least one commander before analysis.".into(),
        ));
    }
    emit(
        &report,
        &request.run_id,
        AnalysisStage::Validating,
        "Validate decklist",
        1,
        1,
        0.07,
        &format!(
            "{} cards parsed; {} commander{} selected",
            parsed.card_count,
            submitted_commander_names.len(),
            if submitted_commander_names.len() == 1 {
                ""
            } else {
                "s"
            }
        ),
    );
    ensure_not_cancelled(&cancellation)?;

    let combo_status = combo_store.status().ok();
    let unique_names = parsed
        .entries
        .iter()
        .map(|entry| (normalize_card_name(&entry.name), entry.name.clone()))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let mut resolution_names = unique_names.clone();
    resolution_names.extend(submitted_commander_names.iter().cloned());
    let mut definitions = repository.get_many(&resolution_names)?;
    let missing = resolution_names
        .iter()
        .filter(|name| !definitions.contains_key(&normalize_card_name(name)))
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    emit(
        &report,
        &request.run_id,
        AnalysisStage::ResolvingCards,
        "Resolve card identities",
        resolved_name_count(&unique_names, &definitions),
        unique_names.len() as u32,
        0.10,
        if missing.is_empty() {
            "All card identities loaded from the local database"
        } else if !request.options.allow_online_card_resolution {
            "Online identity resolution is disabled; unresolved cards will remain visible as coverage gaps"
        } else {
            "Resolving missing cards and adding them to the local cache"
        },
    );

    if !missing.is_empty() && request.options.allow_online_card_resolution {
        match repository.resolve_missing(&missing).await {
            Ok((_resolved, _not_found)) => {
                // Re-read by the deck's requested names so multiface aliases
                // (for example either side of "Fire // Ice") map correctly.
                definitions = repository.get_many(&resolution_names)?;
            }
            Err(error) => {
                emit(
                    &report,
                    &request.run_id,
                    AnalysisStage::ResolvingCards,
                    "Resolve card identities",
                    resolved_name_count(&unique_names, &definitions),
                    unique_names.len() as u32,
                    0.17,
                    &format!("Offline analysis: {error}"),
                );
            }
        }
    }
    ensure_not_cancelled(&cancellation)?;

    let unresolved_names = unique_names
        .iter()
        .filter(|name| !definitions.contains_key(&normalize_card_name(name)))
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let resolved_cards = parsed
        .entries
        .iter()
        .filter(|entry| definitions.contains_key(&normalize_card_name(&entry.name)))
        .map(|entry| entry.quantity as u32)
        .sum::<u32>();
    let resolved_identity =
        resolve_deck_identity(&parsed.entries, &submitted_commander_names, &definitions)?;
    definitions = canonical_definition_index(&definitions)?;
    let analysis_card_status = repository.status()?;

    if let Some(cache) = &cache
        && let Ok(key) = cache.key(
            &resolved_identity.canonical_text,
            &resolved_identity.commander_names,
            &request.options,
            AnalysisCacheData {
                card_data: &analysis_card_status,
                combo_data: combo_status.as_ref(),
                policy_data: &policy_snapshot,
                semantic_data: &semantic_snapshot,
                comprehensive_rules: comprehensive_rules.as_ref(),
            },
        )
        && let Ok(Some(cached)) = cache.get(&key)
    {
        ensure_not_cancelled(&cancellation)?;
        emit(
            &report,
            &request.run_id,
            AnalysisStage::Scoring,
            "Load exact cached analysis",
            1,
            1,
            0.96,
            "Resolved deck, options, card, combo, policy, rules, and model versions exactly match a local report",
        );
        let mut cached_report = cached.report;
        cached_report.run_id = request.run_id.clone();
        cached_report.deck.commanders = submitted_commander_names.clone();
        cached_report.deck.canonical_deck = parsed.canonical_text.clone();
        cached_report.deck.canonical_deck_sha256 = sha256_hex(&parsed.canonical_text);
        cached_report.elapsed_ms = started.elapsed().as_millis();
        cached_report.cache.hit = true;
        cached_report.cache.created_at = cached.created_at;
        cached_report.cache.key_version = CACHE_KEY_VERSION.into();
        ensure_not_cancelled(&cancellation)?;
        emit(
            &report,
            &request.run_id,
            AnalysisStage::Complete,
            "Analysis complete",
            1,
            1,
            1.0,
            &format!(
                "Loaded an exact cached report: likely bracket {} with {:?} confidence",
                cached_report.recommendation.likely_bracket,
                cached_report.recommendation.confidence
            ),
        );
        return Ok(cached_report);
    }

    let policy_definitions = definitions
        .values()
        .cloned()
        .map(|card| (card.normalized_name.clone(), card))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let mut policy = crate::rules::evaluate_commander_policy(
        &policy_snapshot.package,
        &resolved_identity.entries,
        &policy_definitions,
        &resolved_identity.commander_names,
    );
    emit(
        &report,
        &request.run_id,
        AnalysisStage::ResolvingCards,
        "Resolve card identities",
        resolved_cards,
        parsed.card_count,
        0.22,
        &format!(
            "{resolved_cards} / {} card identities resolved",
            parsed.card_count
        ),
    );

    let selected_commanders = resolved_identity
        .commander_names
        .iter()
        .map(|name| normalize_card_name(name))
        .collect::<HashSet<_>>();
    let combo_cards = resolved_identity
        .entries
        .iter()
        .map(|entry| {
            let normalized = normalize_card_name(&entry.name);
            let mut combo_card = ComboDeckCard::new(
                &entry.name,
                entry.quantity as u32,
                entry.is_commander || selected_commanders.contains(&normalized),
            );
            combo_card.oracle_id = definitions
                .get(&normalized)
                .and_then(|definition| definition.oracle_id.clone());
            combo_card
        })
        .collect::<Vec<_>>();
    let (spellbook_lines, combo_note) = if combo_status.as_ref().is_some_and(|status| status.ready)
    {
        let worker_store = combo_store.clone();
        let matches = tokio::task::spawn_blocking(move || {
            worker_store.find_fully_satisfied_matches(&combo_cards, &[])
        })
        .await
        .map_err(|error| AnalysisError::Worker(error.to_string()));
        match matches {
            Ok(Ok(matches)) => {
                let lines = matches
                    .into_iter()
                    .filter_map(spellbook_match_to_known_line)
                    .take(64)
                    .collect::<Vec<_>>();
                let version = combo_status
                    .as_ref()
                    .and_then(|status| status.upstream_version.as_deref())
                    .unwrap_or("version unavailable");
                let note = format!(
                    "Local Commander Spellbook {version} contributed {} exact-card line match{} as report-only evidence; variants with unresolved generic templates remain excluded, and external line timing is not simulated until every starting zone and prerequisite has a machine-checkable mapping.",
                    lines.len(),
                    if lines.len() == 1 { "" } else { "es" }
                );
                (lines, note)
            }
            Ok(Err(error)) => (
                Vec::new(),
                format!(
                    "The optional local Commander Spellbook catalog could not be queried ({error}); built-in verified lines were still checked."
                ),
            ),
            Err(error) => (
                Vec::new(),
                format!(
                    "The optional local Commander Spellbook worker failed ({error}); built-in verified lines were still checked."
                ),
            ),
        }
    } else {
        (
                Vec::new(),
                "The optional local Commander Spellbook catalog is not installed; built-in verified lines were checked.".into(),
            )
    };

    emit(
        &report,
        &request.run_id,
        AnalysisStage::Compiling,
        "Compile semantic model",
        0,
        resolved_identity.unique_card_count as u32,
        0.24,
        "Classifying mana, interaction, engines, payoffs, and known lines",
    );
    let (compiled, semantic_override_summary) = compile_deck_with_rules_and_semantic_overrides(
        &resolved_identity.entries,
        &definitions,
        &resolved_identity.commander_names,
        &spellbook_lines,
        &semantic_snapshot.package,
        comprehensive_rules.as_ref(),
    );
    let mut coverage_records = std::collections::BTreeMap::<String, CombinedCardRecord>::new();
    for entry in &resolved_identity.entries {
        let normalized = normalize_card_name(&entry.name);
        if let Some(definition) = definitions.get(&normalized) {
            let record = CombinedCardRecord::from(definition);
            let identity = definition
                .oracle_id
                .clone()
                .unwrap_or_else(|| definition.normalized_name.clone());
            insert_unique_coverage_record(&mut coverage_records, identity, record)?;
        } else {
            insert_unique_coverage_record(
                &mut coverage_records,
                normalized.clone(),
                CombinedCardRecord {
                    oracle_id: None,
                    name: entry.name.clone(),
                    normalized_name: normalized,
                    mana_cost: None,
                    type_line: String::new(),
                    oracle_text: String::new(),
                    keywords: vec!["__unresolved_card_identity__".into()],
                    ..CombinedCardRecord::default()
                },
            )?;
        }
    }
    let execution_coverage = build_execution_coverage_manifest(
        CoverageSnapshotProvenance {
            card_snapshot_sha256: analysis_card_status.snapshot_sha256.clone(),
            comprehensive_rules_snapshot_sha256: comprehensive_rules
                .as_ref()
                .map(|rules| rules.snapshot_sha256.clone()),
            comprehensive_rules_effective_date: comprehensive_rules
                .as_ref()
                .map(|rules| rules.effective_date.clone()),
        },
        &coverage_records.into_values().collect::<Vec<_>>(),
    )?;
    let compact_execution_coverage = execution_coverage.compact_projection()?;
    let mana_model = build_mana_model_with_commanders(
        &resolved_identity.entries,
        &definitions,
        &resolved_identity.commander_names,
    );
    emit(
        &report,
        &request.run_id,
        AnalysisStage::Compiling,
        "Compile semantic model",
        resolved_identity.unique_card_count as u32,
        resolved_identity.unique_card_count as u32,
        0.30,
        &format!(
            "{:.0}% weighted semantic coverage; {} strategic plan{} detected",
            compiled.semantic_coverage * 100.0,
            compiled.synergy.detected_plans.len(),
            if compiled.synergy.detected_plans.len() == 1 {
                ""
            } else {
                "s"
            }
        ),
    );

    let seed = request
        .options
        .seed
        .unwrap_or_else(|| deterministic_seed(&resolved_identity.canonical_text));
    let opening_deck = compiled.clone();
    let opening_mana = mana_model.clone();
    let opening_options = request.options.clone();
    let opening_cancel = cancellation.clone();
    let opening_report = report.clone();
    let opening_run_id = request.run_id.clone();
    let opening_hands = tokio::task::spawn_blocking(move || {
        simulate_opening_hands_with_mana(
            &opening_deck,
            &opening_mana,
            &opening_options,
            seed,
            &opening_cancel,
            |completed, total| {
                let ratio = completed as f32 / total.max(1) as f32;
                emit(
                    &opening_report,
                    &opening_run_id,
                    AnalysisStage::OpeningHands,
                    "Simulate opening hands",
                    completed,
                    total,
                    0.30 + ratio * 0.20,
                    &format!("{completed} / {total} opening hands"),
                );
            },
        )
    })
    .await
    .map_err(|error| AnalysisError::Worker(error.to_string()))??;

    ensure_not_cancelled(&cancellation)?;
    let game_deck = compiled.clone();
    let game_mana = mana_model.clone();
    let game_options = request.options.clone();
    let game_cancel = cancellation.clone();
    let game_report = report.clone();
    let game_run_id = request.run_id.clone();
    let (mut win_speed, early_execution_witnesses) = tokio::task::spawn_blocking(move || {
        let interaction_label = interaction_progress_label(game_options.interaction_profile);
        let progress = |interference: bool, completed: u32, total: u32| {
            let ratio = completed as f32 / total.max(1) as f32;
            let (stage, label, base) = if interference {
                (AnalysisStage::Interference, interaction_label, 0.72)
            } else {
                (AnalysisStage::Goldfish, "Simulate baseline plans", 0.50)
            };
            emit(
                &game_report,
                &game_run_id,
                stage,
                label,
                completed,
                total,
                base + ratio * 0.22,
                &format!("{completed} / {total} modeled games"),
            );
        };
        simulate_win_speed_with_mana(
            &game_deck,
            &game_mana,
            &game_options,
            seed,
            &game_cancel,
            progress,
        )
        .and_then(|report| {
            crate::simulation::find_early_route_execution_witnesses(
                &game_deck,
                &game_mana,
                &game_options,
                &game_cancel,
            )
            .map(|witnesses| (report, witnesses))
        })
    })
    .await
    .map_err(|error| AnalysisError::Worker(error.to_string()))??;
    let mut early_turn_evaluation =
        crate::early_turn_evaluator::evaluate_early_turn_routes(&compiled, &mana_model);
    early_turn_evaluation.execution_witnesses = early_execution_witnesses;
    win_speed.early_turn_evaluation = Some(early_turn_evaluation);

    ensure_not_cancelled(&cancellation)?;
    crate::rules::apply_compiled_bracket_guidance(
        &policy_snapshot.package,
        &compiled,
        &win_speed,
        &mut policy,
    );
    emit(
        &report,
        &request.run_id,
        AnalysisStage::Scoring,
        "Build recommendation",
        0,
        1,
        0.95,
        "Assembling uncalibrated bracket weights, evidence, and coverage",
    );
    let status = analysis_card_status;
    let card_data_version = status
        .last_updated
        .clone()
        .unwrap_or_else(|| format!("{} local cards", status.card_count));
    let mut report_result = score_analysis(ScoreInputs {
        run_id: &request.run_id,
        deck: &compiled,
        card_count: parsed.card_count,
        unique_card_count: resolved_identity.unique_card_count,
        commander_names: submitted_commander_names.clone(),
        resolved_cards,
        unresolved_cards: unresolved_names,
        canonical_deck: parsed.canonical_text.clone(),
        canonical_deck_sha256: sha256_hex(&parsed.canonical_text),
        policy,
        opening_hands,
        win_speed,
        options: &request.options,
        seed,
        card_data_version,
        execution_coverage: &execution_coverage,
        compact_execution_coverage: &compact_execution_coverage,
        elapsed_ms: started.elapsed().as_millis(),
    });
    report_result.win_speed.coverage_manifest_sha256 =
        Some(execution_coverage.fingerprint_sha256.clone());
    if execution_coverage
        .gate_for(ExecutionMetric::GoldfishTiming)
        .is_none_or(|gate| !gate.can_execute())
    {
        let blockers = execution_coverage
            .gate_for(ExecutionMetric::GoldfishTiming)
            .map_or(0, |gate| gate.blocking_leaf_count);
        report_result.win_speed.fidelity = crate::domain::SimulationFidelity::BlockedUnsupported;
        report_result.win_speed.fidelity_message = format!(
            "Strict per-ability timing is blocked by {blockers} execution-coverage leaf{}. The displayed trajectories use bounded observable-state planning plus a narrow typed ability and reviewed-line executor, but remain legacy heuristic engineering estimates rather than complete Magic games.",
            if blockers == 1 { "" } else { "s" }
        );
    }
    if execution_coverage
        .gate_for(ExecutionMetric::FunctionalMulligan)
        .is_none_or(|gate| !gate.can_execute())
    {
        report_result.opening_hands.policy_fidelity =
            crate::domain::SimulationFidelity::BlockedUnsupported;
    }
    report_result.versions.card_snapshot_sha256 = status.snapshot_sha256.clone();
    report_result.versions.rules_snapshot_sha256 =
        Some(policy_snapshot.provenance.snapshot_sha256.clone());
    report_result.versions.rules_package_origin =
        Some(policy_snapshot.provenance.origin.as_cache_value().into());
    report_result.versions.semantic_package = Some(format!(
        "{} · effective {} · package-declared verified {}",
        semantic_snapshot.package.package_version,
        semantic_snapshot.package.effective_date,
        semantic_snapshot.package.verified_at
    ));
    report_result.versions.semantic_snapshot_sha256 =
        Some(semantic_snapshot.provenance.snapshot_sha256.clone());
    report_result.versions.semantic_package_origin =
        Some(semantic_snapshot.provenance.origin.as_cache_value().into());
    report_result.versions.semantic_imported_at = semantic_snapshot.provenance.imported_at.clone();
    report_result.versions.semantic_authenticity_basis =
        Some(semantic_snapshot.provenance.authenticity_basis.clone());
    if let Some(rules) = comprehensive_rules.as_ref() {
        report_result.versions.comprehensive_rules_effective_date =
            Some(rules.effective_date.clone());
        report_result.versions.comprehensive_rules_snapshot_sha256 =
            Some(rules.snapshot_sha256.clone());
        report_result.versions.comprehensive_rules_parser_version =
            Some(rules.parser_version.clone());
        report_result.versions.rule_capability_model =
            Some(crate::rules_capabilities::RULE_CAPABILITY_MODEL_VERSION.into());
        report_result.coverage.notes.push(format!(
            "Rules-backed semantic annotations used the official Comprehensive Rules effective {}. Recognized mechanics remain report-only unless a typed executor explicitly supports them.",
            rules.effective_date
        ));
        if !semantic_override_summary.rules_backed_mechanics.is_empty() {
            report_result.coverage.notes.push(format!(
                "Reviewed rules-backed mechanics found: {}.",
                semantic_override_summary
                    .rules_backed_mechanics
                    .iter()
                    .take(12)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !semantic_override_summary.rules_report_only_cards.is_empty() {
            report_result.coverage.notes.push(format!(
                "{} unique card{} contain{} rules-recognized or unreviewed keyword behavior that remains report-only in this engine version.",
                semantic_override_summary.rules_report_only_cards.len(),
                if semantic_override_summary.rules_report_only_cards.len() == 1 {
                    ""
                } else {
                    "s"
                },
                if semantic_override_summary.rules_report_only_cards.len() == 1 {
                    "s"
                } else {
                    ""
                }
            ));
        }
    } else {
        report_result.coverage.notes.push(
            "The official Comprehensive Rules are not installed; rules-backed keyword annotations were unavailable."
                .into(),
        );
    }
    if let Some(warning) = &policy_snapshot.provenance.warning {
        report_result.coverage.notes.push(warning.clone());
    }
    if let Some(warning) = &semantic_snapshot.provenance.warning {
        report_result.coverage.notes.push(warning.clone());
    }
    if matches!(
        semantic_snapshot.provenance.origin,
        crate::semantic_store::SemanticPackageOrigin::LocalImport
    ) {
        report_result.coverage.notes.push(format!(
            "Semantic package trust: {}",
            semantic_snapshot.provenance.authenticity_basis
        ));
    }
    if !semantic_snapshot.package.overrides.is_empty() {
        report_result.coverage.notes.push(format!(
            "Semantic package {} applied package-supplied annotations to {} unique deck card{}.",
            semantic_snapshot.package.package_version,
            semantic_override_summary.applied_cards.len(),
            if semantic_override_summary.applied_cards.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    if !semantic_override_summary.unguarded_applied_cards.is_empty() {
        report_result.coverage.notes.push(format!(
            "Semantic overrides for {} had no Oracle-text SHA-256 guard; re-verify them after Oracle text changes.",
            semantic_override_summary.unguarded_applied_cards.join(", ")
        ));
    }
    if !semantic_override_summary
        .oracle_text_guard_mismatches
        .is_empty()
    {
        report_result.coverage.notes.push(format!(
            "Semantic overrides were not applied to {} because current Oracle text did not match the package SHA-256 guard.",
            semantic_override_summary
                .oracle_text_guard_mismatches
                .join(", ")
        ));
    }
    report_result.versions.combo_catalog = Some(format!("Built-in {COMBO_CATALOG_VERSION}"));
    if let Some(status) = combo_status.as_ref().filter(|status| status.ready) {
        report_result.versions.combo_catalog = Some(format!(
            "Built-in {COMBO_CATALOG_VERSION} · Commander Spellbook {} · {} variants · published {}",
            status
                .upstream_version
                .as_deref()
                .unwrap_or("version unavailable"),
            status.variant_count,
            status
                .upstream_timestamp
                .as_deref()
                .unwrap_or("timestamp unavailable")
        ));
        report_result.versions.combo_snapshot_sha256 = status.snapshot_sha256.clone();
    }
    report_result.coverage.notes.push(combo_note);
    report_result.cache.hit = false;
    report_result.cache.created_at = chrono::Utc::now().to_rfc3339();
    report_result.cache.key_version = CACHE_KEY_VERSION.into();
    if let Some(cache) = &cache
        && let Ok(cache_key) = cache.key(
            &resolved_identity.canonical_text,
            &resolved_identity.commander_names,
            &request.options,
            AnalysisCacheData {
                card_data: &status,
                combo_data: combo_status.as_ref(),
                policy_data: &policy_snapshot,
                semantic_data: &semantic_snapshot,
                comprehensive_rules: comprehensive_rules.as_ref(),
            },
        )
    {
        // Caching is an optimization. A locked or damaged cache must never discard a valid
        // freshly-computed report, and AnalysisCache::put rejects incomplete card resolution.
        let _ = cache.put(&cache_key, &report_result);
    }
    emit(
        &report,
        &request.run_id,
        AnalysisStage::Complete,
        "Analysis complete",
        1,
        1,
        1.0,
        &format!(
            "Likely bracket {} with {:?} confidence",
            report_result.recommendation.likely_bracket, report_result.recommendation.confidence
        ),
    );
    Ok(report_result)
}

fn resolve_deck_identity(
    submitted_entries: &[DeckEntry],
    submitted_commanders: &[String],
    submitted_definitions: &HashMap<String, CardDefinition>,
) -> Result<ResolvedDeckIdentity, AnalysisError> {
    let mut seen_commanders = HashSet::new();
    let commander_names = submitted_commanders
        .iter()
        .filter_map(|submitted_name| {
            let normalized = normalize_card_name(submitted_name);
            let resolved_name = submitted_definitions
                .get(&normalized)
                .map(|definition| definition.name.clone())
                .unwrap_or_else(|| submitted_name.clone());
            let resolved_normalized = normalize_card_name(&resolved_name);
            seen_commanders
                .insert(resolved_normalized)
                .then_some(resolved_name)
        })
        .collect::<Vec<_>>();
    let selected_commanders = commander_names
        .iter()
        .map(|name| normalize_card_name(name))
        .collect::<HashSet<_>>();

    let mut aggregated = BTreeMap::<String, DeckEntry>::new();
    for submitted_entry in submitted_entries {
        let submitted_normalized = normalize_card_name(&submitted_entry.name);
        let definition = submitted_definitions.get(&submitted_normalized);
        let resolved_name = definition
            .map(|definition| definition.name.clone())
            .unwrap_or_else(|| submitted_entry.name.clone());
        let resolved_normalized = definition
            .map(|definition| definition.normalized_name.clone())
            .filter(|normalized| !normalized.is_empty())
            .unwrap_or_else(|| normalize_card_name(&resolved_name));
        let is_commander =
            submitted_entry.is_commander || selected_commanders.contains(&resolved_normalized);

        if let Some(existing) = aggregated.get_mut(&resolved_normalized) {
            existing.quantity = existing
                .quantity
                .checked_add(submitted_entry.quantity)
                .ok_or_else(|| {
                    AnalysisError::Validation(format!(
                        "The total quantity for {} exceeds the supported decklist range.",
                        existing.name
                    ))
                })?;
            existing.is_commander |= is_commander;
            existing.line_number = existing.line_number.min(submitted_entry.line_number);
        } else {
            aggregated.insert(
                resolved_normalized,
                DeckEntry {
                    quantity: submitted_entry.quantity,
                    name: resolved_name,
                    line_number: submitted_entry.line_number,
                    is_commander,
                },
            );
        }
    }

    let commander_order = commander_names
        .iter()
        .enumerate()
        .map(|(index, name)| (normalize_card_name(name), index))
        .collect::<HashMap<_, _>>();
    let mut entries = aggregated.into_values().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        let left_name = normalize_card_name(&left.name);
        let right_name = normalize_card_name(&right.name);
        let left_rank = commander_order.get(&left_name).copied();
        let right_rank = commander_order.get(&right_name).copied();
        match (left_rank, right_rank) {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(&right_rank),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_name.cmp(&right_name),
        }
    });

    let canonical_text = entries
        .iter()
        .map(|entry| (normalize_card_name(&entry.name), u32::from(entry.quantity)))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .map(|(name, quantity)| format!("{quantity} {name}"))
        .collect::<Vec<_>>()
        .join("\n");
    let unique_card_count = entries.len();

    Ok(ResolvedDeckIdentity {
        entries,
        commander_names,
        canonical_text,
        unique_card_count,
    })
}

fn canonical_definition_index(
    submitted_definitions: &HashMap<String, CardDefinition>,
) -> Result<HashMap<String, CardDefinition>, AnalysisError> {
    let mut canonical = HashMap::<String, CardDefinition>::new();
    for definition in submitted_definitions.values() {
        let normalized = if definition.normalized_name.is_empty() {
            normalize_card_name(&definition.name)
        } else {
            definition.normalized_name.clone()
        };
        if let Some(existing) = canonical.get(&normalized)
            && existing != definition
        {
            return Err(AnalysisError::Validation(format!(
                "Local card data returned conflicting records for {}.",
                definition.name
            )));
        }
        canonical
            .entry(normalized)
            .or_insert_with(|| definition.clone());
    }
    Ok(canonical)
}

fn resolved_name_count(
    submitted_names: &[String],
    submitted_definitions: &HashMap<String, CardDefinition>,
) -> u32 {
    submitted_names
        .iter()
        .filter(|name| submitted_definitions.contains_key(&normalize_card_name(name)))
        .count() as u32
}

fn insert_unique_coverage_record(
    records: &mut BTreeMap<String, CombinedCardRecord>,
    identity: String,
    record: CombinedCardRecord,
) -> Result<(), AnalysisError> {
    if let Some(existing) = records.get(&identity) {
        if existing == &record {
            return Ok(());
        }
        return Err(AnalysisError::Validation(format!(
            "Card data identity collision for `{identity}`: `{}` and `{}` retain different function, face, or component records. Analysis stopped instead of omitting one record.",
            existing.name, record.name
        )));
    }
    records.insert(identity, record);
    Ok(())
}

fn spellbook_match_to_known_line(candidate: LocalComboMatch) -> Option<KnownLine> {
    if matches!(
        candidate.relevance,
        MatchRelevance::NotRelevant | MatchRelevance::Unknown
    ) || candidate.commander_legal == Some(false)
    {
        return None;
    }

    let table_lethal_if_resolved = matches!(
        candidate.table_lethality,
        TableLethality::DocumentedTableWin | TableLethality::LikelyTableLethal
    );
    let feature_text = candidate
        .produces
        .iter()
        .map(|feature| feature.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let feature_lower = feature_text.to_ascii_lowercase();
    let outcome = if table_lethal_if_resolved {
        KnownLineOutcome::TableWin
    } else if candidate.has_unbounded_result && feature_lower.contains("mana") {
        KnownLineOutcome::InfiniteMana
    } else if candidate.has_unbounded_result {
        KnownLineOutcome::InfiniteEngine
    } else {
        KnownLineOutcome::Engine
    };

    let mut cards = Vec::new();
    let mut prerequisites = Vec::new();
    for requirement in &candidate.cards {
        for _ in 0..requirement.quantity.min(16) {
            cards.push(requirement.name.clone());
        }
        if requirement.must_be_commander {
            prerequisites.push(format!("{} must be in the command zone.", requirement.name));
        }
        if !requirement.zone_locations.is_empty() {
            prerequisites.push(format!(
                "{} uses the catalog zone requirement: {}.",
                requirement.name,
                requirement.zone_locations.join(", ")
            ));
        }
        for (label, state) in [
            ("battlefield", requirement.battlefield_state.as_deref()),
            ("exile", requirement.exile_state.as_deref()),
            ("library", requirement.library_state.as_deref()),
            ("graveyard", requirement.graveyard_state.as_deref()),
        ] {
            if let Some(state) = state.filter(|state| !state.trim().is_empty()) {
                prerequisites.push(format!(
                    "{} {label} state: {}",
                    requirement.name,
                    bounded_text(state, 320)
                ));
            }
        }
    }
    for text in [
        candidate.easy_prerequisites.as_deref(),
        candidate.notable_prerequisites.as_deref(),
        candidate.notes.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|text| !text.trim().is_empty())
    {
        prerequisites.push(bounded_text(text, 500));
    }
    if candidate.mana_needed.is_some() && !candidate.mana_minimum_confirmed {
        prerequisites.push(
            "Commander Spellbook reports this mana requirement but does not mark it as a proven minimum."
                .into(),
        );
    }
    prerequisites.sort();
    prerequisites.dedup();
    prerequisites.truncate(12);

    if cards.is_empty() {
        return None;
    }
    let compactness = cards.len().min(u8::MAX as usize) as u8;
    let label = if feature_text.trim().is_empty() {
        format!("Spellbook line {}", candidate.variant_id)
    } else {
        format!("Spellbook · {}", bounded_text(&feature_text, 90))
    };
    let relevance_penalty = if matches!(candidate.relevance, MatchRelevance::Borderline) {
        0.10
    } else {
        0.0
    };
    let lethality_confidence: f32 = match candidate.table_lethality {
        TableLethality::DocumentedTableWin => 0.92,
        TableLethality::LikelyTableLethal => 0.84,
        TableLethality::RequiresPayoffOrConversion => 0.80,
        TableLethality::Unknown => 0.68,
    };
    let has_unmodeled_prerequisites = !prerequisites.is_empty();
    let mut simulation_requirements = Vec::new();
    if candidate.mana_needed.is_some() {
        // Commander Spellbook reports execution mana from its documented
        // starting zones. It is useful report metadata, but it is not an
        // additional post-cast activation cost and must not be double-paid.
        simulation_requirements.push(LineRequirement::TotalExecutionMana);
    }
    // The bulk catalog preserves explicit ingredient zones and state prose,
    // but this mapper does not yet prove a complete, typed action sequence.
    // Even a zero-mana record with empty prose has an unspecified starting
    // zone rather than a machine-checked default. Keep every external line as
    // report evidence until those constraints are compiled card-by-card.
    simulation_requirements.push(LineRequirement::Unmodeled);

    Some(KnownLine {
        name: label,
        cards,
        compactness,
        is_infinite: candidate.has_unbounded_result,
        table_lethal_if_resolved,
        outcome,
        mana_needed: candidate.mana_needed.map(|mana| bounded_text(&mana, 80)),
        prerequisites,
        model_confidence: (lethality_confidence
            - relevance_penalty
            - if has_unmodeled_prerequisites {
                0.04
            } else {
                0.0
            })
        .clamp(0.45, 0.95),
        simulation_requirements,
    })
}

fn bounded_text(value: &str, maximum_characters: usize) -> String {
    let trimmed = value.trim();
    let mut bounded = trimmed.chars().take(maximum_characters).collect::<String>();
    if trimmed.chars().count() > maximum_characters {
        bounded.push('…');
    }
    bounded
}

fn deterministic_seed(canonical_deck: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(canonical_deck.as_bytes());
    hasher.update(SIMULATION_ENGINE_VERSION.as_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[0..8].try_into().expect("eight-byte digest prefix"))
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn ensure_not_cancelled(cancellation: &AtomicBool) -> Result<(), AnalysisError> {
    if cancellation.load(Ordering::Relaxed) {
        Err(AnalysisError::Cancelled)
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    report: &impl Fn(AnalysisProgress),
    run_id: &str,
    stage: AnalysisStage,
    stage_label: &str,
    completed_units: u32,
    total_units: u32,
    overall_progress: f32,
    detail: &str,
) {
    report(AnalysisProgress {
        run_id: run_id.into(),
        stage,
        stage_label: stage_label.into(),
        completed_units,
        total_units,
        overall_progress: overall_progress.clamp(0.0, 1.0),
        detail: detail.into(),
    });
}
