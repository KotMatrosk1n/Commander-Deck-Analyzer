use crate::domain::{
    AnalysisAssumptions, AnalysisCacheInfo, AnalysisOptions, AnalysisReport, AnalyzedDeckSummary,
    BracketProbability, BracketRecommendation, CalibrationStatus, ConfidenceLevel, CoverageReport,
    DataVersions, EvidenceDirection, EvidenceItem, KnownLine, ManaReliabilityBand,
    OpeningHandReport, OverviewMetrics, SpeedScoreBasis, TurnDistribution, WinSpeedReport,
};
use crate::effects::TutorScope;
use crate::execution_coverage::{
    CompactExecutionCoverageManifest, EXECUTION_COVERAGE_COMPILER_VERSION,
    ExecutionCoverageManifest, ExecutionMetric,
};
use crate::rules::{BracketPolicySignalKind, LegalityStatus, PolicyEvaluation};
use crate::semantics::{CompiledDeck, card_supports_strategy_plan, role};
use crate::simulation::estimate_commander_on_curve;

pub(crate) const SEMANTIC_MODEL_VERSION: &str = "roles-0.6";
const SIMULATION_ENGINE_VERSION: &str = crate::simulation::SIMULATION_ENGINE_VERSION;
pub(crate) const BRACKET_MODEL_VERSION: &str = "evidence-score-0.8-uncalibrated";

pub struct ScoreInputs<'a> {
    pub run_id: &'a str,
    pub deck: &'a CompiledDeck,
    pub card_count: u32,
    pub unique_card_count: usize,
    pub commander_names: Vec<String>,
    pub resolved_cards: u32,
    pub unresolved_cards: Vec<String>,
    pub canonical_deck: String,
    pub canonical_deck_sha256: String,
    pub policy: PolicyEvaluation,
    pub opening_hands: OpeningHandReport,
    pub win_speed: WinSpeedReport,
    pub options: &'a AnalysisOptions,
    pub seed: u64,
    pub card_data_version: String,
    pub execution_coverage: &'a ExecutionCoverageManifest,
    pub compact_execution_coverage: &'a CompactExecutionCoverageManifest,
    pub elapsed_ms: u128,
}

pub fn score_analysis(inputs: ScoreInputs<'_>) -> AnalysisReport {
    let rating_gate = inputs
        .execution_coverage
        .gate_for(ExecutionMetric::BracketRating);
    let goldfish_gate = inputs
        .execution_coverage
        .gate_for(ExecutionMetric::GoldfishTiming);
    let strict_rating_blocked = rating_gate.is_none_or(|gate| !gate.can_execute());
    let strict_goldfish_blocked = goldfish_gate.is_none_or(|gate| !gate.can_execute());
    let fast_mana = role_count(inputs.deck, "Fast mana") as f32;
    let tutors = role_count(inputs.deck, "Tutors") as f32;
    let interaction = role_count(inputs.deck, "Spot interaction") as f32;
    let wipes = role_count(inputs.deck, "Board wipes") as f32;
    let protection = role_count(inputs.deck, "Protection") as f32;
    let ramp = role_count(inputs.deck, "Ramp") as f32;
    let lands = role_count(inputs.deck, "Lands") as f32;

    let modeled_mana_support =
        if inputs.opening_hands.mana.reliability_band == ManaReliabilityBand::Unknown {
            land_configuration_score(lands)
        } else {
            inputs.opening_hands.mana.reliability_score
        };
    let sampled_color_coverage =
        if inputs.opening_hands.mana.reliability_band == ManaReliabilityBand::Unknown {
            land_configuration_score(lands)
        } else {
            inputs.opening_hands.mana.average_turn_three_color_coverage
        };
    let mana_score = percentage_score(
        inputs.opening_hands.three_land_by_turn_three_rate * 0.38
            + inputs.opening_hands.ramp_access_rate * 0.18
            + land_configuration_score(lands) * 0.14
            + modeled_mana_support * 0.18
            + sampled_color_coverage * 0.12,
    );
    let consistency_score = percentage_score(
        inputs.opening_hands.keepable_after_mulligans_rate * 0.47
            + inputs.opening_hands.engine_access_rate * 0.33
            + (tutors / 8.0).min(1.0) * 0.20,
    );
    let primary_plan_access_rate = primary_plan_opening_proxy(inputs.deck, &inputs.opening_hands);
    let structural_pace_score = structural_pace_score(
        inputs.deck,
        &inputs.opening_hands,
        fast_mana,
        tutors,
        primary_plan_access_rate,
    );
    let (speed_score, speed_score_basis) = speed_score(&inputs.win_speed, structural_pace_score);
    let interaction_score = percentage_score(
        (interaction / 12.0).min(1.0) * 0.62
            + (wipes / 4.0).min(1.0) * 0.16
            + (protection / 8.0).min(1.0) * 0.22,
    );
    let synergy_score = inputs.deck.synergy.cohesion_score;
    let resilience_score = if let Some(recovery_rate) = inputs.win_speed.recovery_by_max_turn_rate {
        percentage_score(
            recovery_rate * 0.50
                + (1.0 - inputs.deck.synergy.commander_dependence) * 0.34
                + (protection / 8.0).min(1.0) * 0.16,
        )
    } else {
        // No stopped attempt means recovery was not tested. Score only
        // the observable structural signals instead of treating an absent
        // opportunity as perfect recovery.
        percentage_score(
            (1.0 - inputs.deck.synergy.commander_dependence) * 0.68
                + (protection / 8.0).min(1.0) * 0.32,
        )
    };
    let commander_on_curve_rate = estimate_commander_on_curve(
        inputs.deck,
        &inputs.opening_hands,
        &inputs.opening_hands.mana,
    );

    let overview = OverviewMetrics {
        mana_score,
        consistency_score,
        speed_score,
        speed_score_basis: Some(speed_score_basis),
        interaction_score,
        synergy_score,
        resilience_score,
        commander_on_curve_rate,
        primary_plan_access_rate,
    };

    let mut latent = 1.35
        + speed_score as f32 / 100.0 * 1.18
        + consistency_score as f32 / 100.0 * 0.72
        + synergy_score as f32 / 100.0 * 0.58
        + mana_score as f32 / 100.0 * 0.34
        + interaction_score as f32 / 100.0 * 0.27
        + resilience_score as f32 / 100.0 * 0.27;
    let modeled_lines = inputs
        .deck
        .known_lines
        .iter()
        .filter(|line| line_contributes_to_score(line));
    let table_lethal_lines = modeled_lines
        .clone()
        .filter(|line| line.table_lethal_if_resolved)
        .count();
    let resource_lines = modeled_lines
        .filter(|line| !line.table_lethal_if_resolved)
        .count();
    if table_lethal_lines > 0 {
        latent += 0.32 + (table_lethal_lines.min(3) as f32 - 1.0) * 0.09;
    }
    if resource_lines > 0 {
        latent += 0.10 + (resource_lines.min(3) as f32 - 1.0) * 0.04;
    }
    if fast_mana >= 3.0 {
        latent += 0.18;
    }
    if tutors >= 5.0 {
        latent += 0.16;
    }
    latent = latent.clamp(1.4, 4.78);

    let identity_resolution = if inputs.card_count == 0 {
        0.0
    } else {
        inputs.resolved_cards as f32 / inputs.card_count as f32
    };
    let mana_model_coverage = inputs.opening_hands.mana.model_confidence;
    let simulation_coverage = if strict_goldfish_blocked {
        0.0
    } else {
        simulation_model_coverage(inputs.deck, mana_model_coverage, identity_resolution)
    };
    let overall_coverage = (identity_resolution * 0.37
        + inputs.deck.semantic_coverage * 0.34
        + simulation_coverage * 0.19
        + mana_model_coverage * 0.10)
        .clamp(0.0, 1.0);
    let sigma = if overall_coverage >= 0.86 {
        0.58
    } else if overall_coverage >= 0.68 {
        0.76
    } else {
        1.02
    };
    let probabilities = apply_policy_floor(
        bracket_probabilities(latent, sigma),
        inputs.policy.policy_floor,
    );
    let likely_bracket = probabilities
        .iter()
        .max_by(|left, right| left.probability.total_cmp(&right.probability))
        .map(|probability| probability.bracket)
        .unwrap_or(3);
    let (range_low, range_high) = if strict_rating_blocked {
        (inputs.policy.policy_floor.unwrap_or(1), 5)
    } else {
        credible_range(&probabilities)
    };
    // Until a real independently reviewed corpus passes the calibration gate,
    // this level describes model/input coverage only. "High" is deliberately
    // unavailable; Monte Carlo sample size cannot establish external accuracy.
    let confidence = if strict_rating_blocked {
        ConfidenceLevel::Low
    } else if overall_coverage >= 0.62 {
        ConfidenceLevel::Medium
    } else {
        ConfidenceLevel::Low
    };

    let recommendation = BracketRecommendation {
        likely_bracket,
        range_low,
        range_high,
        confidence,
        rules_floor: inputs.policy.policy_floor,
        probabilities,
        summary: recommendation_summary(
            likely_bracket,
            &inputs.win_speed,
            speed_score_basis,
            overall_coverage,
            rating_gate.map_or(0, |gate| gate.blocking_leaf_count),
        ),
        calibration_status: CalibrationStatus::Uncalibrated,
    };

    let mut evidence = build_evidence(
        inputs.deck,
        &inputs.opening_hands,
        &inputs.win_speed,
        speed_score,
        speed_score_basis,
        fast_mana as u16,
        tutors as u16,
        ramp as u16,
        interaction as u16,
        &inputs.policy,
    );
    if strict_rating_blocked {
        evidence.insert(
            0,
            EvidenceItem {
                direction: EvidenceDirection::Neutral,
                title: "Strict executable rating is blocked".into(),
                detail: rating_gate.map_or_else(
                    || "The execution-coverage gate was unavailable.".into(),
                    |gate| {
                        format!(
                            "{} coverage leaf{} can affect bracket evidence but lack{} a complete executable binding. The displayed bracket weights remain exploratory heuristics.",
                            gate.blocking_leaf_count,
                            if gate.blocking_leaf_count == 1 { "" } else { "s" },
                            if gate.blocking_leaf_count == 1 { "s" } else { "" },
                        )
                    },
                ),
                weight: 0.0,
            },
        );
    }
    let coverage = CoverageReport {
        identity_resolution,
        semantic_coverage: inputs.deck.semantic_coverage,
        simulation_coverage,
        approximated_cards: inputs.deck.approximated_cards.clone(),
        unresolved_cards: inputs.unresolved_cards.clone(),
        notes: {
            let mut notes = vec![
            "Turn estimates use a bounded functional model, not a complete Magic rules engine."
                .into(),
            "Interference scenarios model response pressure; they are not multiplayer win rates."
                .into(),
            "Known-line timing uses a conservative minimal zone/sequence model and exact additional activation payments. Unsupported lines remain report-only; the engine still does not implement a full stack, priority, attachments, subtypes, or opponent combat."
                .into(),
            "Mana production executes only typed clean spell-resolution, reusable tap-source, and immediate self-sacrifice lifecycles. Unsupported activation costs, restrictions, triggers, and timing remain coverage gaps instead of reusable mana."
                .into(),
            "Bracket weights are an uncalibrated model distribution, not observed real-world probabilities; high confidence remains disabled until an independently reviewed corpus passes the calibration gate."
                .into(),
            "The model uses only deck-observable evidence; player-declared intent and self-selected table labels are not accepted as rating inputs.".into(),
            "The deterministic policy package enforces legality, Game Changer floors, and sufficiently confident mass-land-denial floors. Combo timing, extra-turn chaining, and published turn expectations remain explicitly labeled guidance or manual-review context."
                .into(),
            format!(
                "Commander policy {} was applied independently of performance scoring.",
                inputs.policy.package_version
            ),
                format!(
                    "Mana source parsing used {} with {:.0}% model confidence.",
                    crate::mana::MANA_MODEL_VERSION,
                    inputs.opening_hands.mana.model_confidence * 100.0
                ),
            ];
            if !inputs.unresolved_cards.is_empty() {
                notes.push(if inputs.options.allow_online_card_resolution {
                    "Some card names remained unresolved after the optional Scryfall lookup."
                        .into()
                } else {
                    "Online card resolution was disabled; install the full local snapshot or explicitly enable missing-name lookup to improve coverage."
                        .into()
                });
            }
            if !inputs.deck.approximated_cards.is_empty() {
                notes.push(
                    "Cards listed as approximated contain low-confidence or unsupported Oracle-text behavior; those clauses were not silently treated as fully simulated."
                        .into(),
                );
            }
            if strict_rating_blocked {
                notes.push(
                    "The fail-closed execution manifest blocks strict functional rating for this deck. Legacy role-based timing and bracket weights remain visible only as exploratory engineering output."
                        .into(),
                );
            }
            notes.extend(inputs.opening_hands.mana.notes.iter().cloned());
            notes
        },
        execution_manifest: Some(inputs.compact_execution_coverage.clone()),
    };

    AnalysisReport {
        run_id: inputs.run_id.into(),
        deck: AnalyzedDeckSummary {
            card_count: inputs.card_count,
            unique_card_count: inputs.unique_card_count,
            commanders: inputs.commander_names,
            resolved_cards: inputs.resolved_cards,
            unresolved_cards: inputs.unresolved_cards,
            canonical_deck: inputs.canonical_deck,
            canonical_deck_sha256: inputs.canonical_deck_sha256,
        },
        recommendation,
        overview,
        opening_hands: inputs.opening_hands,
        win_speed: inputs.win_speed,
        synergy: inputs.deck.synergy.clone(),
        coverage,
        evidence,
        policy: inputs.policy.clone(),
        assumptions: AnalysisAssumptions {
            opening_hand_simulations: inputs.options.opening_hand_simulations,
            game_simulations: inputs.options.game_simulations,
            maximum_turn: inputs.options.maximum_turn,
            mulligan_policy: inputs.options.mulligan_policy,
            pilot_policy: inputs.options.pilot_policy,
            interaction_profile: inputs.options.interaction_profile,
            declared_intent: inputs.options.declared_intent,
            allow_online_card_resolution: inputs.options.allow_online_card_resolution,
            seed_exact: inputs.seed.to_string(),
            seed: inputs.seed,
        },
        versions: DataVersions {
            card_data: inputs.card_data_version,
            card_snapshot_sha256: None,
            rules_package: inputs.policy.package_version,
            rules_snapshot_sha256: None,
            rules_package_origin: None,
            semantic_model: format!(
                "{SEMANTIC_MODEL_VERSION}+{}+{}",
                crate::semantics::ANNOTATION_MODEL_VERSION,
                crate::effects::EFFECT_DESCRIPTOR_VERSION
            ),
            semantic_package: None,
            semantic_snapshot_sha256: None,
            semantic_package_origin: None,
            semantic_imported_at: None,
            semantic_authenticity_basis: None,
            comprehensive_rules_effective_date: None,
            comprehensive_rules_snapshot_sha256: None,
            comprehensive_rules_parser_version: None,
            rule_capability_model: None,
            strategic_profile_model: Some(
                crate::strategic_profile::STRATEGIC_PROFILE_MODEL_VERSION.into(),
            ),
            simulation_engine: SIMULATION_ENGINE_VERSION.into(),
            effective_hand_strength_model: Some(
                crate::simulation::EFFECTIVE_HAND_STRENGTH_VERSION.into(),
            ),
            ability_program: Some(
                crate::ability_program::EXECUTABLE_ABILITY_PROGRAM_VERSION.into(),
            ),
            turn_planner: Some(crate::turn_planner::TURN_PLANNER_VERSION.into()),
            strict_engine: Some(crate::strict_engine::STRICT_ENGINE_VERSION.into()),
            execution_coverage_compiler: Some(EXECUTION_COVERAGE_COMPILER_VERSION.into()),
            bracket_model: BRACKET_MODEL_VERSION.into(),
            combo_catalog: None,
            combo_snapshot_sha256: None,
        },
        cache: AnalysisCacheInfo::default(),
        elapsed_ms: inputs.elapsed_ms,
    }
}

fn role_count(deck: &CompiledDeck, label: &str) -> u16 {
    deck.synergy
        .role_counts
        .iter()
        .find(|role| role.role == label)
        .map(|role| role.count)
        .unwrap_or(0)
}

/// Conservative probability proxy that a kept hand contains a card tied to
/// the highest-confidence detected plan. The opening-hand simulator's
/// `engine_access_rate` is intentionally broader (any engine or tutor), so it
/// must not be presented as direct access to the primary plan.
fn primary_plan_opening_proxy(deck: &CompiledDeck, opening: &OpeningHandReport) -> f32 {
    let Some(primary_plan) = deck.synergy.detected_plans.first() else {
        return 0.0;
    };
    let library_size = deck.library.len() as f32;
    if library_size <= 0.0 {
        return 0.0;
    }

    let effective_plan_slots = deck
        .library
        .iter()
        .filter_map(|index| deck.cards.get(*index).map(|card| (*index, card)))
        .map(|(card_index, card)| {
            if card_supports_strategy_plan(card, &primary_plan.name) {
                1.0
            } else if card.has(role::TUTOR) {
                // A tutor is plan access only when the typed spell-resolution
                // descriptor can actually find a different legal library
                // target tied to that plan. Unsupported searches and
                // restricted tutors with no matching target add no proxy slot.
                let has_legal_plan_target = card.effects.tutor.is_executable_on_spell_resolution()
                    && card.effects.tutor.instructions.iter().any(|instruction| {
                        deck.library.iter().copied().any(|candidate_index| {
                            candidate_index != card_index
                                && deck.cards.get(candidate_index).is_some_and(|candidate| {
                                    instruction.target.matches(candidate.effects.card_types)
                                        && card_supports_strategy_plan(
                                            candidate,
                                            &primary_plan.name,
                                        )
                                })
                        })
                    });
                if !has_legal_plan_target {
                    0.0
                } else {
                    match card.effects.tutor_scope {
                        TutorScope::AnyCard => 0.75,
                        TutorScope::Restricted => 0.35,
                        TutorScope::None => 0.0,
                    }
                }
            } else {
                0.0
            }
        })
        .sum::<f32>()
        .min(library_size);
    if effective_plan_slots <= 0.0 {
        return 0.0;
    }

    let sample_size = opening.average_cards_kept.clamp(0.0, 7.0);
    if sample_size <= 0.0 {
        return 0.0;
    }
    let lower = sample_size.floor() as u8;
    let upper = sample_size.ceil() as u8;
    let lower_rate = at_least_one_slot_rate(library_size, effective_plan_slots, lower);
    if lower == upper {
        return lower_rate;
    }
    let upper_rate = at_least_one_slot_rate(library_size, effective_plan_slots, upper);
    lower_rate + (upper_rate - lower_rate) * sample_size.fract()
}

fn at_least_one_slot_rate(library_size: f32, effective_slots: f32, draws: u8) -> f32 {
    let mut misses = 1.0;
    for draw in 0..draws {
        let remaining = library_size - draw as f32;
        if remaining <= 0.0 {
            break;
        }
        let remaining_misses = (library_size - effective_slots - draw as f32).max(0.0);
        misses *= (remaining_misses / remaining).clamp(0.0, 1.0);
    }
    (1.0 - misses).clamp(0.0, 1.0)
}

fn line_contributes_to_score(line: &KnownLine) -> bool {
    line.model_confidence >= 0.70
        && !line.simulation_requirements.iter().any(|requirement| {
            matches!(
                requirement,
                crate::domain::LineRequirement::TotalExecutionMana
                    | crate::domain::LineRequirement::CombatAccess
                    | crate::domain::LineRequirement::Unmodeled
            )
        })
}

fn percentage_score(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn mana_band_label(band: ManaReliabilityBand) -> &'static str {
    match band {
        ManaReliabilityBand::Unknown => "Unknown",
        ManaReliabilityBand::Fragile => "Fragile",
        ManaReliabilityBand::Mixed => "Mixed",
        ManaReliabilityBand::Supported => "Supported",
    }
}

fn simulation_model_coverage(
    deck: &CompiledDeck,
    mana_model_coverage: f32,
    identity_resolution: f32,
) -> f32 {
    // Coverage describes what the engine can represent, never how quickly or
    // successfully this particular deck performs. A well-understood slow deck
    // must not lose coverage merely because it presents few threats.
    let known_line_coverage = if deck.known_lines.is_empty() {
        1.0
    } else {
        deck.known_lines
            .iter()
            .map(|line| {
                if line
                    .simulation_requirements
                    .contains(&crate::domain::LineRequirement::Unmodeled)
                {
                    0.0
                } else {
                    line.model_confidence.clamp(0.0, 1.0)
                }
            })
            .sum::<f32>()
            / deck.known_lines.len() as f32
    };

    (deck.semantic_coverage * 0.55
        + mana_model_coverage.clamp(0.0, 1.0) * 0.25
        + identity_resolution.clamp(0.0, 1.0) * 0.10
        + known_line_coverage * 0.10)
        .clamp(0.0, 1.0)
}

fn land_configuration_score(lands: f32) -> f32 {
    let distance = (lands - 36.0).abs();
    (1.0 - distance / 28.0).clamp(0.15, 1.0)
}

fn speed_score(report: &WinSpeedReport, structural_pace_score: u8) -> (u8, SpeedScoreBasis) {
    if let Some(score) = timing_distribution_speed_score(&report.baseline_model_pace) {
        let basis = match (
            report.baseline_win_attempt.demonstrated_rate > 0.0,
            report
                .baseline_generic_conversion_milestone
                .demonstrated_rate
                > 0.0,
        ) {
            (true, true) => SpeedScoreBasis::ProactiveDevelopment,
            (true, false) => SpeedScoreBasis::RecognizedWinAttempt,
            (false, true) => SpeedScoreBasis::GenericConversionMilestone,
            (false, false) => {
                debug_assert!(false, "model pace requires an underlying pace endpoint");
                SpeedScoreBasis::ProactiveDevelopment
            }
        };
        return (score, basis);
    }
    if let Some(score) = timing_distribution_speed_score(&report.baseline) {
        return (score, SpeedScoreBasis::CredibleThreat);
    }
    (structural_pace_score, SpeedScoreBasis::StructuralPace)
}

fn timing_distribution_speed_score(distribution: &TurnDistribution) -> Option<u8> {
    if !distribution.demonstrated_rate.is_finite() || distribution.demonstrated_rate <= 0.0 {
        return None;
    }
    let turn = distribution.median.or(distribution.conditional_median)?;
    if !turn.is_finite() {
        return None;
    }
    let turn_score = turn_speed_bucket(turn);
    let demonstrated_rate = distribution.demonstrated_rate.clamp(0.0, 1.0);
    Some((turn_score * (0.72 + demonstrated_rate * 0.28)).round() as u8)
}

fn turn_speed_bucket(turn: f32) -> f32 {
    match turn {
        turn if turn <= 3.2 => 100.0,
        turn if turn <= 4.2 => 91.0,
        turn if turn <= 5.2 => 79.0,
        turn if turn <= 6.2 => 66.0,
        turn if turn <= 7.2 => 53.0,
        turn if turn <= 8.2 => 42.0,
        turn if turn <= 9.2 => 32.0,
        _ => 23.0,
    }
}

fn structural_pace_score(
    deck: &CompiledDeck,
    opening: &OpeningHandReport,
    fast_mana: f32,
    tutors: f32,
    primary_plan_access_rate: f32,
) -> u8 {
    // This fallback is deliberately capped below the upper half of the scale:
    // list-observable setup can distinguish unsupported decks, but cannot by
    // itself establish a fast timing endpoint.
    percentage_score(
        (fast_mana / 14.0).min(1.0) * 0.35
            + (tutors / 10.0).min(1.0) * 0.20
            + opening.ramp_access_rate.clamp(0.0, 1.0) * 0.15
            + primary_plan_access_rate.clamp(0.0, 1.0) * 0.15
            + (f32::from(deck.synergy.cohesion_score) / 100.0) * 0.15,
    )
    .min(50)
}

fn bracket_probabilities(latent: f32, sigma: f32) -> Vec<BracketProbability> {
    let weights = (1..=5)
        .map(|bracket| {
            let distance = bracket as f32 - latent;
            let weight = (-distance * distance / (2.0 * sigma * sigma)).exp();
            (bracket, weight)
        })
        .collect::<Vec<_>>();
    let total = weights
        .iter()
        .map(|(_, weight)| *weight)
        .sum::<f32>()
        .max(0.001);
    weights
        .into_iter()
        .map(|(bracket, weight)| BracketProbability {
            bracket,
            probability: weight / total,
        })
        .collect()
}

fn apply_policy_floor(
    mut probabilities: Vec<BracketProbability>,
    policy_floor: Option<u8>,
) -> Vec<BracketProbability> {
    let Some(floor) = policy_floor else {
        return probabilities;
    };
    for entry in &mut probabilities {
        if entry.bracket < floor {
            entry.probability = 0.0;
        }
    }
    let total = probabilities
        .iter()
        .map(|entry| entry.probability)
        .sum::<f32>()
        .max(f32::EPSILON);
    for entry in &mut probabilities {
        entry.probability /= total;
    }
    probabilities
}

fn credible_range(probabilities: &[BracketProbability]) -> (u8, u8) {
    let likely = probabilities
        .iter()
        .max_by(|left, right| left.probability.total_cmp(&right.probability))
        .map(|entry| entry.bracket)
        .unwrap_or(3);
    let mut included = vec![likely];
    let mut mass = probabilities
        .iter()
        .find(|entry| entry.bracket == likely)
        .map(|entry| entry.probability)
        .unwrap_or(0.0);
    while mass < 0.80 && included.len() < 5 {
        let candidate = probabilities
            .iter()
            .filter(|entry| !included.contains(&entry.bracket))
            .max_by(|left, right| left.probability.total_cmp(&right.probability));
        if let Some(candidate) = candidate {
            included.push(candidate.bracket);
            mass += candidate.probability;
        } else {
            break;
        }
    }
    (
        *included.iter().min().unwrap_or(&likely),
        *included.iter().max().unwrap_or(&likely),
    )
}

fn speed_basis_description(report: &WinSpeedReport, basis: SpeedScoreBasis) -> String {
    match basis {
        SpeedScoreBasis::RecognizedWinAttempt => timing_basis_description(
            "recognized win-attempt pace",
            &report.baseline_model_pace,
        ),
        SpeedScoreBasis::GenericConversionMilestone => timing_basis_description(
            "generic engine/combat development pace (not a win attempt)",
            &report.baseline_model_pace,
        ),
        SpeedScoreBasis::ProactiveDevelopment => timing_basis_description(
            "proactive development pace across explicit attempts and generic milestones (not a win probability)",
            &report.baseline_model_pace,
        ),
        SpeedScoreBasis::CredibleThreat => {
            timing_basis_description("credible-threat pace", &report.baseline)
        }
        SpeedScoreBasis::StructuralPace => {
            "a capped structural setup proxy because no modeled pace or credible-threat milestone was demonstrated by the turn cap".into()
        }
    }
}

fn timing_basis_description(label: &str, distribution: &TurnDistribution) -> String {
    let timing = distribution
        .median
        .map(|turn| format!("population median turn {turn:.1}"))
        .or_else(|| {
            distribution
                .conditional_median
                .map(|turn| format!("successful-run median turn {turn:.1}"))
        })
        .unwrap_or_else(|| "no observed turn".into());
    format!(
        "{label}: {timing}, demonstrated in {:.0}% of runs",
        distribution.demonstrated_rate * 100.0
    )
}

fn speed_basis_evidence(
    win_speed: &WinSpeedReport,
    speed_score: u8,
    basis: SpeedScoreBasis,
) -> EvidenceItem {
    let direction = if speed_score >= 60 {
        EvidenceDirection::Raises
    } else if speed_score < 35 {
        EvidenceDirection::Lowers
    } else {
        EvidenceDirection::Neutral
    };
    match basis {
        SpeedScoreBasis::RecognizedWinAttempt => EvidenceItem {
            direction,
            title: format!("Recognized win-attempt pace: {speed_score} / 100"),
            detail: format!(
                "{}. This endpoint records a reviewed table-lethal route presented if unanswered; it is not a resolved-win rate.",
                timing_basis_description(
                    "Baseline recognized attempt",
                    &win_speed.baseline_model_pace,
                )
            ),
            weight: 0.92,
        },
        SpeedScoreBasis::GenericConversionMilestone => EvidenceItem {
            direction,
            title: format!("Generic development pace: {speed_score} / 100"),
            detail: format!(
                "{}. This is broad engine/combat development and is deliberately not labeled a win attempt.",
                timing_basis_description(
                    "Baseline generic development",
                    &win_speed.baseline_model_pace,
                )
            ),
            weight: 0.82,
        },
        SpeedScoreBasis::ProactiveDevelopment => EvidenceItem {
            direction,
            title: format!("Proactive development pace: {speed_score} / 100"),
            detail: format!(
                "{}. Per episode this uses the earlier explicit attempt or generic development milestone; it is not a win probability.",
                timing_basis_description(
                    "Baseline proactive development",
                    &win_speed.baseline_model_pace,
                )
            ),
            weight: 0.88,
        },
        SpeedScoreBasis::CredibleThreat => EvidenceItem {
            direction,
            title: format!("Credible-threat pace: {speed_score} / 100"),
            detail: format!(
                "{}. No explicit attempt or generic conversion milestone was demonstrated, so this earlier threat signal supplies the overview speed basis.",
                timing_basis_description("Baseline credible threat", &win_speed.baseline)
            ),
            weight: 0.72,
        },
        SpeedScoreBasis::StructuralPace => EvidenceItem {
            direction,
            title: format!("Structural setup pace proxy: {speed_score} / 100"),
            detail: "No explicit attempt, generic development milestone, or credible threat was demonstrated by the turn cap. The capped fallback uses list-observable fast mana, tutors, executable ramp access, primary-plan access, and modeled cohesion; it is not a turn or win claim.".into(),
            weight: 0.62,
        },
    }
}

fn recommendation_summary(
    bracket: u8,
    win_speed: &WinSpeedReport,
    speed_score_basis: SpeedScoreBasis,
    coverage: f32,
    strict_blocking_leaves: u32,
) -> String {
    let speed = speed_basis_description(win_speed, speed_score_basis);
    let coverage_note = if coverage >= 0.82 {
        "The model has strong coverage of this list."
    } else if coverage >= 0.62 {
        "Some interactions are approximated, so the adjacent bracket remains plausible."
    } else {
        "Coverage is limited; treat this as a preliminary range until unresolved cards are modeled."
    };
    if strict_blocking_leaves > 0 {
        return format!(
            "Strict rating is blocked by {strict_blocking_leaves} execution-coverage leaf{}. Bracket {bracket} is shown only as an exploratory uncalibrated heuristic based on {speed}; the strict model range remains wide.",
            if strict_blocking_leaves == 1 { "" } else { "s" }
        );
    }
    format!(
        "Bracket {bracket} is the best current uncalibrated model fit, based on {speed}. {coverage_note}"
    )
}

#[allow(clippy::too_many_arguments)]
fn build_evidence(
    deck: &CompiledDeck,
    opening: &OpeningHandReport,
    win_speed: &WinSpeedReport,
    speed_score: u8,
    speed_score_basis: SpeedScoreBasis,
    fast_mana: u16,
    tutors: u16,
    ramp: u16,
    interaction: u16,
    policy: &PolicyEvaluation,
) -> Vec<EvidenceItem> {
    let mut evidence = Vec::new();
    evidence.push(speed_basis_evidence(
        win_speed,
        speed_score,
        speed_score_basis,
    ));
    let modeled_lines = deck
        .known_lines
        .iter()
        .filter(|line| line_contributes_to_score(line))
        .collect::<Vec<_>>();
    if !modeled_lines.is_empty() {
        let table_lethal = modeled_lines
            .iter()
            .filter(|line| line.table_lethal_if_resolved)
            .count();
        let resource_engines = modeled_lines.len().saturating_sub(table_lethal);
        evidence.push(EvidenceItem {
            direction: EvidenceDirection::Raises,
            title: if table_lethal > 0 {
                format!(
                    "{table_lethal} table-lethal compact line{} detected",
                    if table_lethal == 1 { "" } else { "s" }
                )
            } else {
                format!(
                    "{resource_engines} compact resource engine{} detected",
                    if resource_engines == 1 { "" } else { "s" }
                )
            },
            detail: modeled_lines
                .iter()
                .map(|line| {
                    if line.table_lethal_if_resolved {
                        line.name.clone()
                    } else {
                        format!("{} (needs payoff)", line.name)
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
            weight: if table_lethal > 0 { 0.88 } else { 0.62 },
        });
    }
    let report_only_lines = deck
        .known_lines
        .iter()
        .filter(|line| !line_contributes_to_score(line))
        .collect::<Vec<_>>();
    if !report_only_lines.is_empty() {
        evidence.push(EvidenceItem {
            direction: EvidenceDirection::Neutral,
            title: format!(
                "{} report-only compact line{} detected",
                report_only_lines.len(),
                if report_only_lines.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            detail: format!(
                "{}. {}",
                report_only_lines
                    .iter()
                    .map(|line| line.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                "These lines are shown for review but excluded from the numeric recommendation until every prerequisite is machine-checkable."
            ),
            weight: 0.36,
        });
    }
    evidence.push(EvidenceItem {
        direction: if opening.keepable_after_mulligans_rate >= 0.78 {
            EvidenceDirection::Raises
        } else {
            EvidenceDirection::Lowers
        },
        title: format!(
            "{:.0}% keepable after mulligans",
            opening.keepable_after_mulligans_rate * 100.0
        ),
        detail: format!(
            "{:.1} average mulligans under the fixed aggressive policy; 95% sampling margin ±{:.1} points.",
            opening.average_mulligans,
            opening.confidence_margin * 100.0
        ),
        weight: 0.72,
    });
    let mana = &opening.mana;
    evidence.push(EvidenceItem {
        direction: match mana.reliability_band {
            ManaReliabilityBand::Supported => EvidenceDirection::Raises,
            ManaReliabilityBand::Fragile => EvidenceDirection::Lowers,
            ManaReliabilityBand::Mixed | ManaReliabilityBand::Unknown => {
                EvidenceDirection::Neutral
            }
        },
        title: format!(
            "{} mana-source support ({:.0} / 100)",
            mana_band_label(mana.reliability_band),
            mana.reliability_score * 100.0
        ),
        detail: format!(
            "{:.0}% average required-color coverage by turn three; {} conditional and {} tapped land slot equivalents.",
            mana.average_turn_three_color_coverage * 100.0,
            mana.conditional_source_count,
            mana.enters_tapped_land_count
        ),
        weight: 0.70,
    });
    if fast_mana > 0 || tutors > 0 {
        evidence.push(EvidenceItem {
            direction: EvidenceDirection::Raises,
            title: format!("{fast_mana} fast-mana and {tutors} tutor slot equivalents"),
            detail: "Early acceleration and broad access increase both speed and repeatability."
                .into(),
            weight: 0.66,
        });
    }
    evidence.push(EvidenceItem {
        direction: if deck.synergy.cohesion_score >= 68 {
            EvidenceDirection::Raises
        } else {
            EvidenceDirection::Lowers
        },
        title: format!("{} / 100 modeled cohesion", deck.synergy.cohesion_score),
        detail: deck
            .synergy
            .detected_plans
            .first()
            .map(|plan| format!("The strongest detected plan is {}.", plan.name))
            .unwrap_or_else(|| "No dense primary plan was detected from modeled roles.".into()),
        weight: 0.64,
    });
    evidence.push(EvidenceItem {
        direction: if interaction >= 8 {
            EvidenceDirection::Raises
        } else {
            EvidenceDirection::Lowers
        },
        title: format!("{interaction} spot-interaction and {ramp} ramp slot equivalents"),
        detail:
            "These counts are semantic role estimates; modal cards may fill more than one role."
                .into(),
        weight: 0.48,
    });
    if let Some(floor) = policy.policy_floor {
        evidence.push(EvidenceItem {
            direction: EvidenceDirection::Raises,
            title: format!("Official policy floor: Bracket {floor}"),
            detail: policy.policy_floor_reason.clone(),
            weight: 1.0,
        });
    }
    for signal in policy
        .bracket_signals
        .iter()
        .filter(|signal| signal.kind != BracketPolicySignalKind::DeterministicFloor)
    {
        evidence.push(EvidenceItem {
            direction: match signal.kind {
                BracketPolicySignalKind::ModeledGuidance if signal.recommended_floor.is_some() => {
                    EvidenceDirection::Raises
                }
                BracketPolicySignalKind::ModeledGuidance
                | BracketPolicySignalKind::ManualReview => EvidenceDirection::Neutral,
                BracketPolicySignalKind::DeterministicFloor => unreachable!(),
            },
            title: signal.title.clone(),
            detail: signal.detail.clone(),
            weight: match signal.kind {
                BracketPolicySignalKind::ModeledGuidance => 0.76,
                BracketPolicySignalKind::ManualReview => 0.44,
                BracketPolicySignalKind::DeterministicFloor => unreachable!(),
            },
        });
    }
    if policy.legality != LegalityStatus::Legal {
        evidence.push(EvidenceItem {
            direction: EvidenceDirection::Neutral,
            title: match policy.legality {
                LegalityStatus::Illegal => "Commander legality issue detected".into(),
                LegalityStatus::Unknown => "Commander legality requires review".into(),
                LegalityStatus::Legal => unreachable!(),
            },
            detail: if policy.legality == LegalityStatus::Illegal {
                format!(
                    "{} deterministic rule violation{} found; performance results remain informational.",
                    policy.format_violations.len()
                        + policy.color_identity_violations.len()
                        + policy.duplicate_violations.len(),
                    if policy.format_violations.len()
                        + policy.color_identity_violations.len()
                        + policy.duplicate_violations.len()
                        == 1
                    {
                        " was"
                    } else {
                        "s were"
                    }
                )
            } else {
                policy
                    .manual_review_reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "At least one policy check could not be established.".into())
            },
            weight: 0.98,
        });
    }
    evidence.sort_by(|left, right| right.weight.total_cmp(&left.weight));
    evidence
}
