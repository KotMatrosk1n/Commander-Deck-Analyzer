import type {
  AnalysisOptions,
  AnalysisReport,
  AttemptProvenanceReport,
  CompactInteractionScenarioReport,
  GenericMilestoneKind,
  InteractionScenario,
  PairedTurnDelayReport,
  TurnDistribution,
} from "./types";

export const CURRENT_SIMULATION_ENGINE_VERSION = "abstract-play-0.48";
export const CURRENT_TIMING_ENDPOINT_VERSION = "commander-timing-endpoints/v3";
export const CURRENT_EFFECTIVE_HAND_STRENGTH_VERSION =
  "mtg-effective-hand-strength/v4";
export const CURRENT_OPENING_CANDIDATE_COHORT_VERSION =
  "opening-candidate-cohort/v1";
export const CURRENT_ABILITY_PROGRAM_VERSION =
  "executable-ability-program/v22";
export const CURRENT_TURN_PLANNER_VERSION = "bounded-beam-0.7";
export const CURRENT_STRICT_ENGINE_VERSION = "strict-kernel-0.1";
export const CURRENT_EXECUTION_COVERAGE_COMPILER_VERSION =
  "execution-coverage-0.9";
export const CURRENT_BRACKET_MODEL_VERSION = "evidence-score-0.8-uncalibrated";
export const CURRENT_CACHE_KEY_VERSION = "analysis-cache-46";
const ALLOWED_TRIAL_COUNTS = [1000, 5000, 10000] as const;
const MINIMUM_TURN_HORIZON = 2;
const MAXIMUM_TURN_HORIZON = 12;
const MAXIMUM_INTERACTION_SCENARIO_EPISODES = 1000;
const RATE_TOLERANCE = 0.000_001;
const DISTRIBUTION_KEYS = [
  "baseline",
  "interfered",
  "baselineWinAttempt",
  "interferedWinAttempt",
  "baselineModelPace",
  "interferedModelPace",
  "baselineResolvedTableWin",
  "interferedResolvedTableWin",
] as const;
const RESOLVED_CURVE_KEYS = [
  "cumulativeResolvedTableWinRate",
  "cumulativeInterferedResolvedTableWinRate",
] as const;
const PRIMARY_CURVE_CONTRACTS = [
  ["cumulativeThreatRate", "baseline"],
  ["cumulativeInterferedThreatRate", "interfered"],
  ["cumulativeWinAttemptRate", "baselineWinAttempt"],
  ["cumulativeInterferedWinAttemptRate", "interferedWinAttempt"],
] as const;
const EXPLICIT_TIMING_FIELDS = [
  "conditionalMedian",
  "conditionalP10",
  "conditionalP90",
  "rightCensoredRate",
] as const;
const PAIRED_DELAY_FIELDS = [
  "observedPairs",
  "preventedByTurnCap",
  "baselineNotDemonstrated",
  "stressedOnly",
  "median",
  "p10",
  "p90",
] as const;
const CURRENT_GENERIC_MILESTONE_KINDS = [
  "engine",
  "combat",
  "engineAndCombat",
] satisfies GenericMilestoneKind[];
const CURRENT_INTERACTION_SCENARIOS = [
  ["targetedPermanentRemoval", "targeted-permanent-removal"],
  ["commanderRemovalRecast", "commander-removal-recast"],
  ["firstRelevantSpellCountered", "first-relevant-spell-countered"],
  ["creatureWipe", "creature-wipe"],
  ["graveyardShutdown", "graveyard-shutdown"],
  ["genericTaxStax", "generic-tax-stax"],
  ["ruleOfLawCap", "rule-of-law-cap"],
  ["firstWinAttemptStopped", "first-win-attempt-stopped"],
] as const satisfies readonly (readonly [InteractionScenario, string])[];

/**
 * Rejects reports whose legacy successful-run quantiles could otherwise be
 * mistaken for population quantiles. The desktop app currently has no
 * standalone analysis-report import, so this protects the backend/cache result
 * boundary and report export.
 */
export function assertCurrentReportTimingSemantics(
  report: AnalysisReport,
  expected?: {
    runId: string;
    options: AnalysisOptions;
    canonicalDeck: string;
    commanderNames: readonly string[];
  },
): void {
  const behaviorVersionMismatch = [
    [
      "simulation engine",
      report.versions?.simulationEngine,
      CURRENT_SIMULATION_ENGINE_VERSION,
    ],
    [
      "executable ability program",
      report.versions?.abilityProgram,
      CURRENT_ABILITY_PROGRAM_VERSION,
    ],
    [
      "turn planner",
      report.versions?.turnPlanner,
      CURRENT_TURN_PLANNER_VERSION,
    ],
    [
      "strict execution kernel",
      report.versions?.strictEngine,
      CURRENT_STRICT_ENGINE_VERSION,
    ],
    [
      "execution coverage compiler",
      report.versions?.executionCoverageCompiler,
      CURRENT_EXECUTION_COVERAGE_COMPILER_VERSION,
    ],
  ].find(([, actual, expected]) => actual !== expected);
  if (behaviorVersionMismatch) {
    const [label, actual, expected] = behaviorVersionMismatch;
    throw new Error(
      `This analysis report does not declare the current ${label} (${expected}); received ${JSON.stringify(actual ?? "missing")}. Reanalyze the deck; reports from a different executable behavior contract will not be displayed as current analysis.`,
    );
  }
  if (report.cache?.keyVersion !== CURRENT_CACHE_KEY_VERSION) {
    throw new Error(
      `This analysis report does not declare the current cache contract (${CURRENT_CACHE_KEY_VERSION}); received ${JSON.stringify(report.cache?.keyVersion ?? "missing")}. Reanalyze the deck; cached reports from a different analysis contract will not be displayed as current analysis.`,
    );
  }
  if (report.versions?.bracketModel !== CURRENT_BRACKET_MODEL_VERSION) {
    throw new Error(
      `This analysis report does not declare the current overview scoring contract (${CURRENT_BRACKET_MODEL_VERSION}). Reanalyze the deck; legacy speed and bracket scoring will not be displayed as current analysis.`,
    );
  }
  const assumptions = report.assumptions;
  const openingHandSimulations = assumptions?.openingHandSimulations;
  const gameSimulations = assumptions?.gameSimulations;
  const maximumTurn = assumptions?.maximumTurn ?? Number.NaN;
  if (
    !ALLOWED_TRIAL_COUNTS.includes(
      openingHandSimulations as (typeof ALLOWED_TRIAL_COUNTS)[number],
    )
    || gameSimulations !== openingHandSimulations
    || !Number.isInteger(maximumTurn)
    || maximumTurn < MINIMUM_TURN_HORIZON
    || maximumTurn > MAXIMUM_TURN_HORIZON
    || assumptions?.mulliganPolicy !== "aggressive"
    || assumptions?.pilotPolicy !== "race"
    || assumptions?.interactionProfile !== "highPower"
    || assumptions?.declaredIntent !== "unspecified"
    || report.openingHands?.simulations !== openingHandSimulations
    || report.winSpeed?.simulations !== gameSimulations
  ) {
    throw new Error(
      "This report was not produced with a supported analysis workload (equal 1,000, 5,000, or 10,000 opening-hand and paired-trajectory trials; a turn horizon from 2 through 12; aggressive mulligans; proactive route search; and standardized high-power response pressure). Reanalyze the deck.",
    );
  }
  if (
    expected
    && (
      report.runId !== expected.runId
      || assumptions.openingHandSimulations
        !== expected.options.openingHandSimulations
      || assumptions.gameSimulations !== expected.options.gameSimulations
      || assumptions.maximumTurn !== expected.options.maximumTurn
      || assumptions.mulliganPolicy !== expected.options.mulliganPolicy
      || assumptions.pilotPolicy !== expected.options.pilotPolicy
      || assumptions.interactionProfile !== expected.options.interactionProfile
      || assumptions.declaredIntent !== expected.options.declaredIntent
      || assumptions.allowOnlineCardResolution
        !== expected.options.allowOnlineCardResolution
      || (
        expected.options.seed !== undefined
        && assumptions.seed !== expected.options.seed
      )
    )
  ) {
    throw new Error(
      "The analyzer returned a report that does not match the submitted run identity or analysis settings. The result will not be displayed; reanalyze the deck.",
    );
  }
  if (
    expected
    && (
      report.deck?.canonicalDeck !== expected.canonicalDeck
      || normalizedCommanderSelection(report.deck?.commanders)
        !== normalizedCommanderSelection(expected.commanderNames)
    )
  ) {
    throw new Error(
      "The analyzer returned a report that does not match the submitted deck or commander selection. The result will not be displayed; reanalyze the deck.",
    );
  }
  if (
    ![
      "recognizedWinAttempt",
      "genericConversionMilestone",
      "proactiveDevelopment",
      "credibleThreat",
      "structuralPace",
    ].includes(report.overview?.speedScoreBasis ?? "")
    || !hasExplicitTimingFields(report.winSpeed?.baselineModelPace)
    || !hasExplicitTimingFields(report.winSpeed?.interferedModelPace)
  ) {
    throw new Error(
      "This analysis report is missing the current overview speed-basis or proactive-development timing contract. Reanalyze the deck; censored or generic development will not be relabeled as explicit win timing.",
    );
  }
  if (
    report.versions?.effectiveHandStrengthModel
    !== CURRENT_EFFECTIVE_HAND_STRENGTH_VERSION
  ) {
    throw new Error(
      `This analysis report does not declare the current opening-hand strength model (${CURRENT_EFFECTIVE_HAND_STRENGTH_VERSION}). Reanalyze the deck; legacy mulligan decisions will not be mixed with the current fixed policy.`,
    );
  }

  if (
    report.openingHands?.candidateCohortVersion
      !== CURRENT_OPENING_CANDIDATE_COHORT_VERSION
    || !/^[0-9a-f]{64}$/.test(
      report.openingHands?.candidateCohortSha256 ?? "",
    )
  ) {
    throw new Error(
      `This analysis report is missing the current opening candidate cohort contract (${CURRENT_OPENING_CANDIDATE_COHORT_VERSION}) or its lowercase SHA-256 identity. Reanalyze the deck; opening-hand deltas will not be presented as reproducibly paired without a complete cohort binding.`,
    );
  }

  const endpointVersion = report.winSpeed?.timingEndpointVersion;
  if (endpointVersion !== CURRENT_TIMING_ENDPOINT_VERSION) {
    throw new Error(
      `This analysis report does not declare the current timing endpoint contract (${CURRENT_TIMING_ENDPOINT_VERSION}); received ${JSON.stringify(endpointVersion ?? "missing")}. Reanalyze the deck; older threat or attempt fields will not be relabeled as resolved table wins.`,
    );
  }

  const simulations = gameSimulations as number;
  const incomplete = DISTRIBUTION_KEYS.find((key) =>
    !hasExplicitTimingFields(report.winSpeed?.[key]),
  );
  if (incomplete) {
    throw new Error(
      `This analysis report is missing explicit three-endpoint population-versus-successful-run timing fields for ${incomplete}. Reanalyze the deck with the current engine; ambiguous legacy quantiles will not be displayed or exported.`,
    );
  }

  const invalidDistribution = DISTRIBUTION_KEYS.find((key) =>
    !hasCurrentTurnDistribution(
      report.winSpeed?.[key],
      simulations,
      maximumTurn,
    ),
  );
  if (invalidDistribution) {
    throw new Error(
      `This analysis report contains a workload-inconsistent timing distribution for ${invalidDistribution}. Reanalyze the deck; rates, censoring, and quantiles must match the submitted paired-trajectory workload and selected horizon.`,
    );
  }

  const invalidPrimaryCurve = PRIMARY_CURVE_CONTRACTS.find(
    ([curveKey, distributionKey]) =>
      !hasCurrentCumulativeCurveMatchingDistribution(
        report.winSpeed?.[curveKey],
        maximumTurn,
        simulations,
        report.winSpeed?.[distributionKey],
      ),
  );
  if (invalidPrimaryCurve) {
    throw new Error(
      `This analysis report is missing or inconsistent with the selected horizon for cumulative timing curve ${invalidPrimaryCurve[0]}. Reanalyze the deck; a terminal value from a different turn horizon will not be relabeled as current timing.`,
    );
  }

  const missingCurve = RESOLVED_CURVE_KEYS.find(
    (key, index) => !hasCurrentCumulativeCurveMatchingDistribution(
      report.winSpeed?.[key],
      maximumTurn,
      simulations,
      index === 0
        ? report.winSpeed?.baselineResolvedTableWin
        : report.winSpeed?.interferedResolvedTableWin,
    ),
  );
  if (missingCurve) {
    throw new Error(
      `This analysis report is missing the explicit resolved-table-win cumulative curve ${missingCurve}. Reanalyze the deck; first-attempt curves will not be relabeled as resolved wins.`,
    );
  }

  const invalidPairedDelay = [
    ["pairedThreatDelay", "medianDelay"],
    ["pairedWinAttemptDelay", "winAttemptMedianDelay"],
    ["pairedResolvedTableWinDelay", "resolvedTableWinMedianDelay"],
  ].find(([delayKey, medianKey]) =>
    !Object.prototype.hasOwnProperty.call(report.winSpeed ?? {}, medianKey)
    || !hasCurrentPairedDelay(
      Reflect.get(report.winSpeed ?? {}, delayKey),
      simulations,
      maximumTurn,
    )
    || !nullableNumbersEqual(
      Reflect.get(report.winSpeed ?? {}, medianKey),
      Reflect.get(
        Reflect.get(report.winSpeed ?? {}, delayKey) ?? {},
        "median",
      ),
    )
  );
  if (invalidPairedDelay) {
    throw new Error(
      `This analysis report is missing the explicit resolved-table-win paired delay contract or contains a workload-inconsistent paired delay for ${invalidPairedDelay[0]}. Reanalyze the deck; paired endpoint categories must account for every trajectory and first-attempt delay will not be relabeled as resolved-win delay.`,
    );
  }

  if (
    !hasCurrentTurnDistribution(
      report.winSpeed?.baselineGenericConversionMilestone,
      simulations,
      maximumTurn,
    )
    || !hasCurrentTurnDistribution(
      report.winSpeed?.interferedGenericConversionMilestone,
      simulations,
      maximumTurn,
    )
    || !hasCurrentCumulativeCurveMatchingDistribution(
      report.winSpeed?.cumulativeGenericConversionMilestoneRate,
      maximumTurn,
      simulations,
      report.winSpeed?.baselineGenericConversionMilestone,
    )
    || !hasCurrentCumulativeCurveMatchingDistribution(
      report.winSpeed?.cumulativeInterferedGenericConversionMilestoneRate,
      maximumTurn,
      simulations,
      report.winSpeed?.interferedGenericConversionMilestone,
    )
    || !hasCurrentAttemptProvenance(
      report.winSpeed?.attemptProvenance,
      maximumTurn,
      Math.min(maximumTurn, 6),
      simulations,
      report.winSpeed?.baselineGenericConversionMilestone,
      report.winSpeed?.interferedGenericConversionMilestone,
    )
  ) {
    throw new Error(
      "This analysis report predates explicit win-route provenance and generic-milestone separation. Reanalyze the deck; broad engine/combat heuristics will not be displayed as win attempts.",
    );
  }

  if (!hasCurrentAggregateAttemptAndRecoveryRates(report, simulations)) {
    throw new Error(
      "This analysis report contains attempt-stop or recovery counts that do not match the submitted paired-trajectory workload. Reanalyze the deck; workload-derived rates will not be displayed from inconsistent counts.",
    );
  }

  if (
    !hasCurrentInteractionScenarioSuite(
      report.winSpeed?.interactionScenarios,
      simulations,
      maximumTurn,
      assumptions.seed,
      assumptions.seedExact,
    )
  ) {
    throw new Error(
      "This analysis report is missing or inconsistent with the current eight-scenario response-pressure contract. Reanalyze the deck; scenario coverage, seeds, workloads, and paired-delay denominators must match the submitted analysis.",
    );
  }

  const early = report.winSpeed?.earlyTurnEvaluation;
  if (
    !early
    || early.modelVersion !== "early-turn-route-skeleton/v5"
    || !Array.isArray(early.routes)
    || !Array.isArray(early.blockers)
    || early.fixedPolicy?.openingHandSize !== 7
    || early.fixedPolicy?.naturalDrawsBeforeTurnOne !== 1
    || early.fixedPolicy?.naturalDrawsBeforeTurnTwo !== 2
    || early.fixedPolicy?.aggressiveCandidateHands !== 4
  ) {
    throw new Error(
      "This analysis report is missing the fixed-policy T1/T2 explicit-route readiness evaluation. Reanalyze the deck; generic development will not be substituted for early win-route access.",
    );
  }
}

function normalizedCommanderSelection(
  commanderNames: readonly string[] | null | undefined,
): string {
  if (!Array.isArray(commanderNames)) return "";
  return commanderNames
    .map((name) => name.trim().replace(/\s+/g, " ").toLocaleLowerCase("en-US"))
    .filter(Boolean)
    .sort()
    .join("\n");
}

function hasExplicitTimingFields(
  distribution: TurnDistribution | null | undefined,
): distribution is TurnDistribution {
  if (!distribution || typeof distribution !== "object") return false;
  return EXPLICIT_TIMING_FIELDS.every((field) =>
    Object.prototype.hasOwnProperty.call(distribution, field),
  );
}

function hasCurrentTurnDistribution(
  distribution: TurnDistribution | null | undefined,
  simulations: number,
  maximumTurn: number,
): distribution is TurnDistribution {
  if (
    !distribution
    || typeof distribution !== "object"
    || ![
      "median",
      "p10",
      "p90",
      "conditionalMedian",
      "conditionalP10",
      "conditionalP90",
      "demonstratedRate",
      "rightCensoredRate",
    ].every((field) => Object.prototype.hasOwnProperty.call(distribution, field))
  ) {
    return false;
  }

  const rightCensoredRate = distribution.rightCensoredRate;
  if (typeof rightCensoredRate !== "number") return false;
  const demonstratedEpisodes = episodesForRate(
    distribution.demonstratedRate,
    simulations,
  );
  const rightCensoredEpisodes = episodesForRate(
    rightCensoredRate,
    simulations,
  );
  if (
    demonstratedEpisodes === null
    || rightCensoredEpisodes === null
    || demonstratedEpisodes + rightCensoredEpisodes !== simulations
    || Math.abs(
      distribution.demonstratedRate + rightCensoredRate - 1,
    ) > RATE_TOLERANCE
  ) {
    return false;
  }

  const populationQuantiles = [
    [distribution.p10, 0.10],
    [distribution.median, 0.50],
    [distribution.p90, 0.90],
  ] as const;
  if (
    !populationQuantiles.every(([value, probability]) => {
      const isIdentifiable = demonstratedEpisodes
        >= Math.ceil(probability * simulations);
      return isIdentifiable
        ? isTurnWithinHorizon(value, maximumTurn)
        : value === null;
    })
    || !numbersAreNondecreasing([
      distribution.p10,
      distribution.median,
      distribution.p90,
    ])
  ) {
    return false;
  }

  const conditional = [
    distribution.conditionalP10,
    distribution.conditionalMedian,
    distribution.conditionalP90,
  ];
  return demonstratedEpisodes === 0
    ? conditional.every((value) => value === null)
    : conditional.every((value) => isTurnWithinHorizon(value, maximumTurn))
      && numbersAreNondecreasing(conditional);
}

function hasCurrentPairedDelay(
  value: unknown,
  simulations: number,
  maximumTurn: number,
): value is PairedTurnDelayReport {
  if (
    !value
    || typeof value !== "object"
    || !PAIRED_DELAY_FIELDS.every((field) =>
      Object.prototype.hasOwnProperty.call(value, field)
    )
  ) {
    return false;
  }
  const delay = value as PairedTurnDelayReport;
  const counts = [
    delay.observedPairs,
    delay.preventedByTurnCap,
    delay.baselineNotDemonstrated,
    delay.stressedOnly,
  ];
  if (
    !counts.every((count) => isBoundedInteger(count, simulations))
    || counts.reduce((sum, count) => sum + count, 0) !== simulations
  ) {
    return false;
  }

  const quantiles = [delay.p10, delay.median, delay.p90];
  if (delay.observedPairs === 0) {
    return quantiles.every((value) => value === null);
  }
  const maximumAbsoluteDelay = Math.max(0, maximumTurn - 1);
  return quantiles.every((turn) =>
    typeof turn === "number"
    && Number.isFinite(turn)
    && turn >= -maximumAbsoluteDelay
    && turn <= maximumAbsoluteDelay
  ) && numbersAreNondecreasing(quantiles);
}

function hasCurrentCumulativeCurve(
  value: unknown,
  maximumTurn: number,
  simulations: number,
): value is { turn: number; rate: number }[] {
  if (!Array.isArray(value) || value.length !== maximumTurn) return false;
  let previousRate = 0;
  return value.every((point, index) => {
    if (!point || typeof point !== "object") return false;
    const turn = Reflect.get(point, "turn");
    const rate = Reflect.get(point, "rate");
    const valid = turn === index + 1
      && typeof rate === "number"
      && Number.isFinite(rate)
      && rate + RATE_TOLERANCE >= previousRate
      && rate >= 0
      && rate <= 1
      && episodesForRate(rate, simulations) !== null;
    if (valid) previousRate = rate;
    return valid;
  });
}

function hasCurrentCumulativeCurveMatchingDistribution(
  value: unknown,
  maximumTurn: number,
  simulations: number,
  distribution: TurnDistribution | null | undefined,
): boolean {
  if (
    !hasCurrentCumulativeCurve(value, maximumTurn, simulations)
    || !distribution
    || typeof distribution.demonstratedRate !== "number"
    || !Number.isFinite(distribution.demonstratedRate)
  ) {
    return false;
  }
  const terminalRate = value[value.length - 1]?.rate;
  return typeof terminalRate === "number"
    && Math.abs(terminalRate - distribution.demonstratedRate)
      <= RATE_TOLERANCE;
}

function hasCurrentAttemptProvenance(
  value: unknown,
  maximumTurn: number,
  earlyFailureHorizon: number,
  simulations: number,
  baselineGenericMilestone: TurnDistribution | null | undefined,
  interferedGenericMilestone: TurnDistribution | null | undefined,
): value is AttemptProvenanceReport {
  if (!value || typeof value !== "object") return false;
  const provenance = value as Partial<AttemptProvenanceReport>;
  if (
    provenance.earlyFailureHorizon !== earlyFailureHorizon
    || !Array.isArray(provenance.explicitRoutes)
    || !Array.isArray(provenance.genericMilestoneKinds)
    || !Array.isArray(provenance.earlyTurnBlockers)
    || provenance.genericMilestoneKinds.length
      !== CURRENT_GENERIC_MILESTONE_KINDS.length
  ) {
    return false;
  }

  const routeIds = new Set<string>();
  if (
    !provenance.explicitRoutes.every((route) =>
      typeof route?.routeId === "string"
      && route.routeId.length > 0
      && !routeIds.has(route.routeId)
      && routeIds.add(route.routeId)
      && isBoundedInteger(route.baselineAttempts, simulations)
      && isBoundedInteger(route.interferedAttempts, simulations)
      && rateMatchesEpisodes(
        route.baselineRate,
        route.baselineAttempts,
        simulations,
      )
      && rateMatchesEpisodes(
        route.interferedRate,
        route.interferedAttempts,
        simulations,
      )
      && hasCurrentTurnDistribution(
        route.baselineFirstAttempt,
        simulations,
        maximumTurn,
      )
      && hasCurrentTurnDistribution(
        route.interferedFirstAttempt,
        simulations,
        maximumTurn,
      )
      && Math.abs(
        route.baselineFirstAttempt.demonstratedRate - route.baselineRate,
      ) <= RATE_TOLERANCE
      && Math.abs(
        route.interferedFirstAttempt.demonstratedRate - route.interferedRate,
      ) <= RATE_TOLERANCE
      && hasCurrentCumulativeCurve(
        route?.cumulativeBaselineAttemptRate,
        maximumTurn,
        simulations,
      )
      && hasCurrentCumulativeCurve(
        route?.cumulativeInterferedAttemptRate,
        maximumTurn,
        simulations,
      )
      && curveEndsAtSummaryRate(
        route.cumulativeBaselineAttemptRate,
        route.baselineRate,
      )
      && curveEndsAtSummaryRate(
        route.cumulativeInterferedAttemptRate,
        route.interferedRate,
      )
    )
  ) {
    return false;
  }
  const kinds = provenance.genericMilestoneKinds.map((entry) => entry?.kind);
  if (
    !CURRENT_GENERIC_MILESTONE_KINDS.every(
      (kind) => kinds.filter((candidate) => candidate === kind).length === 1,
    )
  ) {
    return false;
  }

  let baselineGenericEpisodes = 0;
  let interferedGenericEpisodes = 0;
  for (const milestone of provenance.genericMilestoneKinds) {
    if (
      !isBoundedInteger(milestone.baselineEpisodes, simulations)
      || !isBoundedInteger(milestone.interferedEpisodes, simulations)
      || !rateMatchesEpisodes(
        milestone.baselineRate,
        milestone.baselineEpisodes,
        simulations,
      )
      || !rateMatchesEpisodes(
        milestone.interferedRate,
        milestone.interferedEpisodes,
        simulations,
      )
    ) {
      return false;
    }
    baselineGenericEpisodes += milestone.baselineEpisodes;
    interferedGenericEpisodes += milestone.interferedEpisodes;
  }
  if (
    baselineGenericEpisodes > simulations
    || interferedGenericEpisodes > simulations
    || !baselineGenericMilestone
    || !interferedGenericMilestone
    || !rateMatchesEpisodes(
      baselineGenericMilestone.demonstratedRate,
      baselineGenericEpisodes,
      simulations,
    )
    || !rateMatchesEpisodes(
      interferedGenericMilestone.demonstratedRate,
      interferedGenericEpisodes,
      simulations,
    )
  ) {
    return false;
  }

  return provenance.earlyTurnBlockers.every((blocker) =>
    (blocker.sample === "baseline" || blocker.sample === "interfered")
    && Number.isInteger(blocker.turn)
    && blocker.turn >= 1
    && blocker.turn <= earlyFailureHorizon
    && isBoundedInteger(blocker.episodes, simulations)
    && rateMatchesEpisodes(blocker.rate, blocker.episodes, simulations)
  );
}

function curveEndsAtSummaryRate(
  curve: { turn: number; rate: number }[],
  summaryRate: unknown,
): boolean {
  if (
    typeof summaryRate !== "number"
    || !Number.isFinite(summaryRate)
    || summaryRate < 0
    || summaryRate > 1
  ) {
    return false;
  }
  const terminalRate = curve[curve.length - 1]?.rate;
  return typeof terminalRate === "number"
    && Math.abs(terminalRate - summaryRate) <= RATE_TOLERANCE;
}

function hasCurrentAggregateAttemptAndRecoveryRates(
  report: AnalysisReport,
  simulations: number,
): boolean {
  const winSpeed = report.winSpeed;
  if (
    !isBoundedInteger(winSpeed.firstAttemptOpportunities, simulations)
    || !isBoundedInteger(
      winSpeed.recoveryOpportunities,
      winSpeed.firstAttemptOpportunities,
    )
    || !isBoundedInteger(
      winSpeed.recoveredAttempts,
      winSpeed.recoveryOpportunities,
    )
    || !rateMatchesEpisodes(
      winSpeed.firstAttemptStoppedRate,
      winSpeed.recoveryOpportunities,
      Math.max(1, winSpeed.firstAttemptOpportunities),
    )
  ) {
    return false;
  }
  if (winSpeed.recoveryOpportunities === 0) {
    return winSpeed.recoveryByMaxTurnRate === null;
  }
  return rateMatchesEpisodes(
    winSpeed.recoveryByMaxTurnRate,
    winSpeed.recoveredAttempts,
    winSpeed.recoveryOpportunities,
  );
}

function hasCurrentInteractionScenarioSuite(
  value: unknown,
  simulations: number,
  maximumTurn: number,
  masterSeed: number,
  masterSeedExact: string | null | undefined,
): value is CompactInteractionScenarioReport[] {
  if (
    !Array.isArray(value)
    || value.length !== CURRENT_INTERACTION_SCENARIOS.length
    || !Number.isFinite(masterSeed)
    || !Number.isInteger(masterSeed)
    || typeof masterSeedExact !== "string"
    || !/^(0|[1-9]\d*)$/.test(masterSeedExact)
  ) {
    return false;
  }
  const expectedEpisodeCount = Math.min(
    simulations,
    MAXIMUM_INTERACTION_SCENARIO_EPISODES,
  );
  return value.every((scenario, index) =>
    hasCurrentInteractionScenario(
      scenario,
      CURRENT_INTERACTION_SCENARIOS[index],
      expectedEpisodeCount,
      maximumTurn,
      masterSeed,
      masterSeedExact,
    )
  );
}

function hasCurrentInteractionScenario(
  value: unknown,
  expected: readonly [InteractionScenario, string],
  episodeCount: number,
  maximumTurn: number,
  masterSeed: number,
  masterSeedExact: string,
): value is CompactInteractionScenarioReport {
  if (!value || typeof value !== "object") return false;
  const scenario = value as CompactInteractionScenarioReport;
  const [expectedScenario, expectedScenarioId] = expected;
  if (
    scenario.schemaVersion !== "commander-interaction-scenarios/report/v2"
    || scenario.directive?.directiveVersion
      !== "commander-interaction-directives/v1"
    || scenario.directive?.checkpointVersion
      !== "commander-interaction-checkpoints/v1"
    || scenario.directive?.scenario !== expectedScenario
    || scenario.directive?.scenarioId !== expectedScenarioId
    || scenario.directive?.selection?.occurrence !== "first"
    || !arraysEqual(
      scenario.directive?.selection?.tieBreakers,
      ["eventSequence", "stableObjectId", "stablePlayerId"],
    )
    || scenario.measurement?.label !== "response-pressure"
    || scenario.measurement?.executionSource?.kind !== "responsePressure"
    || scenario.sampling?.seedDerivationVersion
      !== "splitmix64/master-gold-index/v1"
    || scenario.sampling?.masterSeed !== masterSeed
    || scenario.sampling?.masterSeedExact !== masterSeedExact
    || scenario.sampling?.episodeCount !== episodeCount
    || scenario.sampling?.maximumTurn !== maximumTurn
  ) {
    return false;
  }

  const applicability = scenario.applicability;
  const counters = scenario.counters;
  if (
    !isBoundedInteger(applicability?.applicableEpisodes, episodeCount)
    || !isBoundedInteger(applicability?.notApplicableEpisodes, episodeCount)
    || !isBoundedInteger(applicability?.undeterminedEpisodes, episodeCount)
    || applicability.applicableEpisodes
      + applicability.notApplicableEpisodes
      + applicability.undeterminedEpisodes !== episodeCount
    || !hasCurrentScenarioCounters(
      counters,
      applicability.applicableEpisodes,
      applicability.notApplicableEpisodes,
      applicability.undeterminedEpisodes,
      episodeCount,
    )
  ) {
    return false;
  }

  return hasCurrentCompactScenarioDelay(
    scenario.credibleThreatDelay,
    "credibleThreat",
    counters,
    applicability,
    episodeCount,
    maximumTurn,
  )
    && hasCurrentCompactScenarioDelay(
      scenario.firstWinAttemptDelay,
      "firstWinAttempt",
      counters,
      applicability,
      episodeCount,
      maximumTurn,
    )
    && hasCurrentCompactScenarioDelay(
      scenario.resolvedTableWinDelay,
      "resolvedTableWin",
      counters,
      applicability,
      episodeCount,
      maximumTurn,
    )
    && hasCurrentCompactRecovery(
      scenario.recovery,
      counters.effectfulInterventionEpisodes,
      maximumTurn,
    );
}

function hasCurrentScenarioCounters(
  counters: CompactInteractionScenarioReport["counters"] | null | undefined,
  applicableEpisodes: number,
  notApplicableEpisodes: number,
  undeterminedEpisodes: number,
  episodeCount: number,
): counters is CompactInteractionScenarioReport["counters"] {
  if (!counters) return false;
  const boundedFields = [
    counters.totalEpisodes,
    counters.applicableEpisodes,
    counters.notApplicableEpisodes,
    counters.undeterminedEpisodes,
    counters.applicableWithoutOpportunityEpisodes,
    counters.opportunityEpisodes,
    counters.checkpointEvents,
    counters.opportunityEvents,
    counters.directiveAttemptEvents,
    counters.directiveAppliedEvents,
    counters.directiveRejectedEvents,
    counters.directiveNoOpEvents,
    counters.effectfulInterventionEpisodes,
  ];
  return boundedFields.every((count) => isBoundedInteger(count, episodeCount))
    && Number.isInteger(counters.affectedGameEvents)
    && counters.affectedGameEvents >= 0
    && counters.totalEpisodes === episodeCount
    && counters.applicableEpisodes === applicableEpisodes
    && counters.notApplicableEpisodes === notApplicableEpisodes
    && counters.undeterminedEpisodes === undeterminedEpisodes
    && counters.applicableWithoutOpportunityEpisodes
      + counters.opportunityEpisodes === applicableEpisodes
    && counters.checkpointEvents === counters.opportunityEvents
    && counters.opportunityEvents === counters.opportunityEpisodes
    && counters.directiveAttemptEvents
      === counters.directiveAppliedEvents + counters.directiveRejectedEvents
    && counters.directiveAttemptEvents <= counters.opportunityEvents
    && counters.directiveNoOpEvents <= counters.directiveAppliedEvents
    && counters.effectfulInterventionEpisodes
      === counters.directiveAppliedEvents - counters.directiveNoOpEvents
    && (
      counters.effectfulInterventionEpisodes === 0
        ? counters.affectedGameEvents === 0
        : counters.affectedGameEvents > 0
    );
}

function hasCurrentCompactScenarioDelay(
  delay: CompactInteractionScenarioReport["credibleThreatDelay"] | null | undefined,
  metric: "credibleThreat" | "firstWinAttempt" | "resolvedTableWin",
  counters: CompactInteractionScenarioReport["counters"],
  applicability: CompactInteractionScenarioReport["applicability"],
  episodeCount: number,
  maximumTurn: number,
): boolean {
  if (!delay || delay.metric !== metric) return false;
  const partition = [
    delay.observedPairs,
    delay.rightCensoredPairs,
    delay.noOpInvariantPairs,
    delay.nonEstimablePairs,
    delay.excludedPairs,
  ];
  if (
    ![
      delay.totalEpisodePairs,
      delay.applicablePairs,
      delay.effectfulPairs,
      ...partition,
    ].every((count) => isBoundedInteger(count, episodeCount))
    || delay.totalEpisodePairs !== episodeCount
    || delay.applicablePairs !== applicability.applicableEpisodes
    || delay.effectfulPairs !== counters.effectfulInterventionEpisodes
    || delay.excludedPairs !== applicability.notApplicableEpisodes
      + applicability.undeterminedEpisodes
    || partition.reduce((sum, count) => sum + count, 0) !== episodeCount
    || delay.applicablePairs
      !== delay.effectfulPairs + delay.noOpInvariantPairs
    || delay.effectfulPairs
      !== delay.observedPairs
        + delay.rightCensoredPairs
        + delay.nonEstimablePairs
  ) {
    return false;
  }

  const observedQuantiles = [
    delay.observedDelayP10Turns,
    delay.observedDelayMedianTurns,
    delay.observedDelayP90Turns,
  ];
  const maximumAbsoluteDelay = Math.max(0, maximumTurn - 1);
  if (
    delay.observedPairs === 0
      ? !observedQuantiles.every((turn) => turn === null)
      : !observedQuantiles.every((turn) =>
        typeof turn === "number"
        && Number.isFinite(turn)
        && turn >= -maximumAbsoluteDelay
        && turn <= maximumAbsoluteDelay
      ) || !numbersAreNondecreasing(observedQuantiles)
  ) {
    return false;
  }

  const censoredBounds = [
    delay.censoredBoundMinTurns,
    delay.censoredBoundMedianTurns,
    delay.censoredBoundMaxTurns,
  ];
  return delay.rightCensoredPairs === 0
    ? censoredBounds.every((turn) => turn === null)
    : censoredBounds.every((turn) =>
      typeof turn === "number"
      && Number.isFinite(turn)
      && turn >= -maximumAbsoluteDelay
      && turn <= maximumAbsoluteDelay
    ) && numbersAreNondecreasing(censoredBounds);
}

function hasCurrentCompactRecovery(
  recovery: CompactInteractionScenarioReport["recovery"] | null | undefined,
  effectfulEpisodes: number,
  maximumTurn: number,
): boolean {
  if (
    !recovery
    || recovery.opportunities !== effectfulEpisodes
    || !isBoundedInteger(recovery.recovered, recovery.opportunities)
    || !isBoundedInteger(recovery.rightCensored, recovery.opportunities)
    || recovery.recovered + recovery.rightCensored !== recovery.opportunities
  ) {
    return false;
  }
  const quantiles = [
    recovery.observedRecoveryP10Turn,
    recovery.observedRecoveryMedianTurn,
    recovery.observedRecoveryP90Turn,
  ];
  if (recovery.opportunities === 0) {
    return recovery.recoveredByTurnCapRate === null
      && quantiles.every((turn) => turn === null);
  }
  if (
    !rateMatchesEpisodes(
      recovery.recoveredByTurnCapRate,
      recovery.recovered,
      recovery.opportunities,
    )
  ) {
    return false;
  }
  return recovery.recovered === 0
    ? quantiles.every((turn) => turn === null)
    : quantiles.every((turn) => isTurnWithinHorizon(turn, maximumTurn))
      && numbersAreNondecreasing(quantiles);
}

function isBoundedInteger(value: unknown, maximum: number): value is number {
  return typeof value === "number"
    && Number.isInteger(value)
    && value >= 0
    && value <= maximum;
}

function episodesForRate(rate: unknown, simulations: number): number | null {
  if (
    typeof rate !== "number"
    || !Number.isFinite(rate)
    || rate < 0
    || rate > 1
    || !Number.isInteger(simulations)
    || simulations <= 0
  ) {
    return null;
  }
  const episodes = Math.round(rate * simulations);
  return rateMatchesEpisodes(rate, episodes, simulations) ? episodes : null;
}

function rateMatchesEpisodes(
  rate: unknown,
  episodes: number,
  denominator: number,
): boolean {
  if (
    typeof rate !== "number"
    || !Number.isFinite(rate)
    || !Number.isInteger(episodes)
    || episodes < 0
    || !Number.isInteger(denominator)
    || denominator <= 0
    || episodes > denominator
  ) {
    return false;
  }
  return Math.abs(rate - episodes / denominator) <= RATE_TOLERANCE;
}

function isTurnWithinHorizon(
  value: unknown,
  maximumTurn: number,
): value is number {
  return typeof value === "number"
    && Number.isFinite(value)
    && value >= 1
    && value <= maximumTurn;
}

function numbersAreNondecreasing(
  values: readonly (number | null | undefined)[],
): boolean {
  const present = values.filter(
    (value): value is number =>
      typeof value === "number" && Number.isFinite(value),
  );
  return present.every((value, index) =>
    index === 0 || value >= present[index - 1]
  );
}

function nullableNumbersEqual(left: unknown, right: unknown): boolean {
  if (left === null && right === null) return true;
  return typeof left === "number"
    && typeof right === "number"
    && Number.isFinite(left)
    && Number.isFinite(right)
    && Math.abs(left - right) <= RATE_TOLERANCE;
}

function arraysEqual(
  left: readonly unknown[] | null | undefined,
  right: readonly unknown[],
): boolean {
  return Array.isArray(left)
    && left.length === right.length
    && left.every((value, index) => value === right[index]);
}
