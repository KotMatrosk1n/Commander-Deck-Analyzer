import { assertCurrentReportTimingSemantics } from "./reportCompatibility";
import { interactionProfileLabel } from "./interactionProfiles";
import type { AnalysisReport } from "./types";

export function hasStrictFunctionalRating(report: AnalysisReport): boolean {
  const ratingGate = report.coverage.executionManifest?.gates.find(
    (gate) => gate.metric === "bracketRating",
  );
  return (
    ratingGate?.state === "executable"
    && report.openingHands.policyFidelity === "strictExecutable"
    && report.winSpeed.fidelity === "strictExecutable"
  );
}

export function reportFileStem(report: AnalysisReport): string {
  const identity = report.deck.commanders.join("-and-") || "commander-deck";
  const safe = identity
    .normalize("NFKD")
    .replace(/[^\w.-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
  return `${safe || "commander-deck"}-analysis`;
}

export function formatReportMarkdown(report: AnalysisReport): string {
  assertCurrentReportTimingSemantics(report);
  const baselineResolved = report.winSpeed.baselineResolvedTableWin!;
  const interferedResolved = report.winSpeed.interferedResolvedTableWin!;
  const pairedResolvedDelay = report.winSpeed.pairedResolvedTableWinDelay!;
  const selectedInteractionProfile = interactionProfileLabel(
    report.assumptions.interactionProfile,
  );
  const bracket = report.recommendation;
  const strictRatingAvailable = hasStrictFunctionalRating(report);
  const policyFloor = report.policy.policyFloor
    ? `Bracket ${report.policy.policyFloor}`
    : "No deterministic floor";
  const recovery = report.winSpeed.recoveryByMaxTurnRate == null
    ? "Not observed"
    : `${pct(report.winSpeed.recoveryByMaxTurnRate)} (${report.winSpeed.recoveredAttempts}/${report.winSpeed.recoveryOpportunities})`;
  const probabilityRows = bracket.probabilities
    .map((entry) => `| ${entry.bracket} | ${pct(entry.probability)} |`)
    .join("\n");
  const evidence = report.evidence
    .map((item) => `- **${item.title}:** ${item.detail}`)
    .join("\n");
  const lines = report.synergy.knownLines.length
    ? report.synergy.knownLines
        .map((line) => {
          const outcome = line.tableLethalIfResolved
            ? "table-lethal if resolved"
            : words(line.outcome);
          const mana = line.manaNeeded ? ` Mana needed: ${line.manaNeeded}.` : "";
          const prerequisites = line.prerequisites.length
            ? ` Prerequisites: ${line.prerequisites.join(" ")}`
            : "";
          return `- **${line.name}:** ${line.cards.join(" + ")}: ${outcome}; ${pct(line.modelConfidence)} model confidence.${mana}${prerequisites}`;
        })
        .join("\n")
    : "- No compact line in the current knowledge package matched this list.";
  const graph = report.synergy.graph;
  const strategic = report.synergy.strategicProfile;
  const strategicSection = strategic
    ? `## Structural strategy profile

- Primary structural archetype: **${words(strategic.primaryArchetype)}**
- Primary posture: **${words(strategic.primaryPosture)}**
- Evidence confidence: ${pct(strategic.confidence)}
- Use policy: report-only; does not affect simulation, scoring, or asserted pilot intent

### Archetype ranking

${strategic.archetypeRanking
    .slice(0, 4)
    .map((entry) => `- **${words(entry.archetype)} (${pct(entry.score)}):** ${entry.evidence}`)
    .join("\n")}

### Combo-family ranking

${strategic.comboFamilyRanking
    .slice(0, 5)
    .map((entry) => `- **${words(entry.family)} (${pct(entry.score)}):** ${entry.evidence}`)
    .join("\n")}

### Documented combo routes

${strategic.comboRouteClusters?.length
    ? strategic.comboRouteClusters
        .slice(0, 8)
        .map((route) => {
          const core = route.centralCards
            .filter((card) => card.appearsInEveryLine)
            .map((card) => card.name)
            .join(" + ");
          const recurring = route.centralCards
            .filter((card) => !card.appearsInEveryLine)
            .map((card) => `${card.name} (${card.lineCount}/${route.lineCount})`)
            .join(" · ");
          const branchCards = route.uniqueCards.join(" · ");
          const packageText = core
            ? `every line uses ${core}`
            : `standalone line ${route.lineNames[0] ?? route.routeId}`;
          const recurringText = recurring ? `; recurring branches ${recurring}` : "";
          const branches = branchCards ? `; branches ${branchCards}` : "";
          const review = route.hasReportOnlyRequirements
            ? `; ${route.reportOnlyLineCount} report-only line${route.reportOnlyLineCount === 1 ? "" : "s"}`
            : "";
          return `- **${words(route.rank)} (${pct(route.score)}):** ${packageText}${recurringText}${branches}; ${words(route.conversion)}${review}`;
        })
        .join("\n")
    : "- No documented line route was available to cluster."}
`
    : `## Structural strategy profile

This report predates the report-only structural strategy classifier.
`;
  const graphLinks = graph?.links.length
    ? graph.links
        .slice(0, 12)
        .map(
          (link) =>
            `- **${link.sourceCard} → ${link.targetCard}** (${words(link.relation)}, ${link.resource}, ${pct(link.confidence)} parser confidence): ${link.evidence}`,
        )
        .join("\n")
    : "- No explicit producer-to-payoff relationship cleared the current conservative parser.";
  const gameChangers = report.policy.gameChangers.length
    ? report.policy.gameChangers.map((card) => `- ${card}`).join("\n")
    : "- None detected.";
  const policyIssues = Array.from(
    new Set([
      ...report.policy.formatViolations.map((violation) => violation.message),
      ...report.policy.colorIdentityViolations.map(
        (violation) =>
          `${violation.cardName} is outside the commander color identity (${violation.cardColors.join("") || "colorless"} vs ${violation.commanderColors.join("") || "colorless"}).`,
      ),
      ...report.policy.duplicateViolations.map(
        (violation) =>
          `${violation.cardName} appears ${violation.quantity} times without a modeled copy-count exception.`,
      ),
      ...report.policy.commanderEligibility
        .filter((commander) => commander.status !== "legal")
        .map((commander) => `${commander.cardName}: ${commander.reason}`),
      ...report.policy.unresolvedCards.map(
        (card) => `${card} could not be resolved, so its policy checks are incomplete.`,
      ),
      ...report.policy.manualReviewReasons,
    ]),
  );
  const policyReview = policyIssues.length
    ? policyIssues.map((issue) => `- ${issue}`).join("\n")
    : "- No modeled policy issues detected.";
  const policySignals = report.policy.bracketSignals.length
    ? report.policy.bracketSignals
        .map((signal) => {
          const kind = words(signal.kind);
          const floor = signal.recommendedFloor
            ? ` Recommended floor: Bracket ${signal.recommendedFloor}.`
            : "";
          const sources = signal.sourceUrls.length
            ? ` Sources: ${signal.sourceUrls.join(", ")}.`
            : "";
          return `- **${signal.title}** (${kind}): ${signal.detail}${floor}${sources}`;
        })
        .join("\n")
    : "- No additional bracket-policy signals.";
  const manaRows = report.openingHands.mana.colors.length
    ? report.openingHands.mana.colors
        .map(
          (color) =>
            `| ${color.color} | ${color.weightedSourceEquivalents.toFixed(1)} | ${color.exactSources} | ${color.conditionalSources} | ${color.demandPipAppearances} |`,
        )
        .join("\n")
    : "| Not available | Not available | Not available | Not available | Not available |";
  const notes = report.coverage.notes.map((note) => `- ${note}`).join("\n");
  const semanticPackage = report.versions.semanticPackage
    ? declaredSemanticPackage(report.versions.semanticPackage)
    : "Not recorded";
  const executionManifest = report.coverage.executionManifest;
  const ratingGate = executionManifest?.gates.find((gate) => gate.metric === "bracketRating");
  const executionBlockers = ratingGate?.blockers.length
    ? ratingGate.blockers
        .slice(0, 20)
        .map((blocker) => `- **${blocker.cardName}:** ${blocker.blocker.detail}`)
        .join("\n")
    : "- No strict rating blocker was recorded.";
  const blockerSampleNote = ratingGate?.blockerSampleTruncated
    ? `Only the deterministic first ${ratingGate.blockers.length} of ${ratingGate.blockingLeafCount} blocking leaves are included in this compact report (sample limit ${ratingGate.blockerSampleLimit}).`
    : "The compact report includes every recorded blocking leaf.";
  const executionCoverageSection = executionManifest
    ? `## Fail-closed execution coverage

- Compiler: \`${executionManifest.compilerVersion}\`
- Full preflight manifest SHA-256: \`${executionManifest.fingerprintSha256}\`
- Compact projection SHA-256: \`${executionManifest.projectionSha256}\`
- Cards / faces: ${executionManifest.summary.cardCount} / ${executionManifest.summary.faceCount}
- Oracle spans / leaves: ${executionManifest.summary.oracleSpanCount} / ${executionManifest.summary.leafCount}
- Strict bracket gate: **${words(ratingGate?.state ?? "blocked")}**
- Blocking leaves: ${ratingGate?.blockingLeafCount ?? 0}

${blockerSampleNote}

${executionBlockers}
`
    : `## Fail-closed execution coverage

No execution-coverage manifest was attached to this report.
`;
  const interactionScenarioSection = formatInteractionScenariosMarkdown(report);
  const provenance = report.winSpeed.attemptProvenance;
  const routeRows = provenance?.explicitRoutes.length
    ? provenance.explicitRoutes
        .map((route) =>
          `| ${singleLine(route.name)} | ${route.cards.map(singleLine).join(" + ")} | ${pct(route.baselineRate)} | ${pct(route.interferedRate)} |`,
        )
        .join("\n")
    : "| None recognized | Not available | 0% | 0% |";
  const blockerRows = provenance?.earlyTurnBlockers.length
    ? provenance.earlyTurnBlockers
        .filter((blocker) => blocker.sample === "baseline")
        .sort((left, right) => right.rate - left.rate || left.turn - right.turn)
        .slice(0, 12)
        .map((blocker) =>
          `| T${blocker.turn} | ${words(blocker.reason)} | ${singleLine(blocker.routeName ?? "No recognized route")} | ${singleLine(blocker.blockedCard ?? "Not available")} | ${pct(blocker.rate)} |`,
        )
        .join("\n")
    : "| Not available | Not recorded | Not available | Not available | Not available |";
  const routeProvenanceSection = `## Explicit win-route provenance

Only a recognized reviewed table-lethal line or a rules-backed combat
assignment that would eliminate every remaining opponent if its damage
connects can populate first-win-attempt timing. Broad engine/combat density is
recorded separately and is not a win attempt.

| Explicit route | Cards | Baseline attempt rate | Response-pressure attempt rate |
| --- | --- | ---: | ---: |
${routeRows}

### Most frequent baseline early-turn blockers

| Turn | Reason | Route | Blocked card | Episode rate |
| ---: | --- | --- | --- | ---: |
${blockerRows}
`;
  const earlyEvaluation = report.winSpeed.earlyTurnEvaluation;
  const omittedConversionRoutes = earlyEvaluation?.omittedNonTableWinLineCount ?? 0;
  const earlyRouteRows = earlyEvaluation?.routes.length
    ? earlyEvaluation.routes
        .map((route) => {
          const turnOne = route.turns.find((entry) => entry.turn === 1);
          const turnTwo = route.turns.find((entry) => entry.turn === 2);
          const witness = earlyEvaluation.executionWitnesses.find(
            (entry) => entry.routeId === route.routeId,
          );
          const witnessLabel = witness
            ? `T${witness.turn} ${witness.resolvedTableWin ? "resolved table win" : "recognized attempt"}`
            : "Not found in bounded search";
          return `| ${singleLine(route.routeName)} | ${witnessLabel} | ${pct(turnOne?.directSkeletonProbability ?? 0)} | ${pct(turnOne?.typedTutorSkeletonProbability ?? 0)} | ${pct(turnTwo?.directSkeletonProbability ?? 0)} | ${pct(turnTwo?.typedTutorSkeletonProbability ?? 0)} | ${pct(route.aggressiveMulligan.directSkeletonInAtLeastOneCandidate)} | ${words(route.modelingCeiling)} |`;
        })
        .join("\n")
    : omittedConversionRoutes > 0
      ? `| No eligible table-win route; ${omittedConversionRoutes} recognized route${omittedConversionRoutes === 1 ? "" : "s"} require${omittedConversionRoutes === 1 ? "s" : ""} conversion | Not available | Not available | Not available | Not available | Not available | Not available | Not available |`
      : "| No eligible explicit table-win route | Not available | Not available | Not available | Not available | Not available | Not available | Not available |";
  const earlyRouteCounts = earlyEvaluation
    ? `Eligible explicit table-win routes: **${earlyEvaluation.eligibleTableWinRouteCount}**. Recognized routes requiring a table-lethal conversion: **${omittedConversionRoutes}**.`
    : "Early-route evaluation was not recorded.";
  const earlyEvaluationSection = `## Exact T1/T2 route-skeleton access

This deterministic combination enumeration includes a separate independent candidate-hand search envelope. That envelope is not the production mulligan probability: it does not model stop-on-keep decisions or London bottoming.
These figures describe card access only; they are not attempted-win or win probabilities.
Tutor payment, colored mana, ordered sequencing, priority, protection, and opponent responses still require a legal trajectory witness.

${earlyRouteCounts}

| Explicit route | Legal trajectory witness | T1 direct | T1 with typed tutor access | T2 direct | T2 with typed tutor access | Direct in independent candidate envelope | Modeling ceiling |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
${earlyRouteRows}
`;
  const markdownHardBreak = "  ";
  const ratingHeader = `**${strictRatingAvailable ? "Model" : "Exploratory model"} bracket estimate:** ${bracket.likelyBracket}${markdownHardBreak}
**Uncalibrated model range:** ${bracket.rangeLow}-${bracket.rangeHigh}${markdownHardBreak}
**Model coverage:** ${capitalize(bracket.confidence)}${markdownHardBreak}
**Strict functional rating:** ${strictRatingAvailable ? "Available" : "Unavailable"}${markdownHardBreak}`;
  const ratingSummary = strictRatingAvailable
    ? bracket.summary
    : `${bracket.summary}

> Low-confidence warning: strict functional coverage is blocked by ${ratingGate?.blockingLeafCount ?? "unreported"} execution-preflight leaves. The estimate, range, and weights below remain exploratory and uncalibrated; the official policy floor remains separate.`;
  const modelWeightsSection = `## Bracket model weights

These normalized weights are ${strictRatingAvailable ? "" : "exploratory "}outputs of the current uncalibrated model. They are not observed bracket frequencies or multiplayer win probabilities.

| Bracket | Model weight |
| ---: | ---: |
${probabilityRows}`;
  const coreMetricsHeading = strictRatingAvailable
    ? "## Core metrics"
    : "## Exploratory core diagnostics";

  return `# Commander Deck Analyzer report

**Commander:** ${report.deck.commanders.join(" + ") || "Not specified"}${markdownHardBreak}
**Cards:** ${report.deck.cardCount} (${report.deck.uniqueCardCount} unique)${markdownHardBreak}
${ratingHeader}
**Calibration status:** ${words(bracket.calibrationStatus)}${markdownHardBreak}
**Timing fidelity:** ${words(report.winSpeed.fidelity)}${markdownHardBreak}
**Timing endpoint contract:** \`${report.winSpeed.timingEndpointVersion}\`${markdownHardBreak}
**Functional mulligan fidelity:** ${words(report.openingHands.policyFidelity)}${markdownHardBreak}
**Legality:** ${capitalize(report.policy.legality)}${markdownHardBreak}
**Policy floor:** ${policyFloor}

${ratingSummary}

> ${report.winSpeed.fidelityMessage}

${report.openingHands.policyFidelity === "strictExecutable"
    ? ""
    : "> Functional keep/bottom decisions are exploratory legacy estimates because at least one relevant card function is not strictly executable. Raw opening-card composition remains valid sampling."}

${modelWeightsSection}

## Commander policy

- Legality: **${capitalize(report.policy.legality)}**
- Policy floor: **${policyFloor}**
- Floor reason: ${report.policy.policyFloorReason}
- Rules package: \`${report.policy.packageVersion}\` (effective ${report.policy.effectiveDate})

### Game Changers

${gameChangers}

### Bracket-policy signals

${policySignals}

### Policy review

${policyReview}

${coreMetricsHeading}

| Metric | Result |
| --- | ---: |
| Mana | ${report.overview.manaScore}/100 |
| Consistency | ${report.overview.consistencyScore}/100 |
| Speed | ${report.overview.speedScore}/100 |
| Speed evidence basis | ${words(report.overview.speedScoreBasis ?? "not recorded")} |
| Interaction | ${report.overview.interactionScore}/100 |
| Synergy | ${report.overview.synergyScore}/100 |
| Resilience | ${report.overview.resilienceScore}/100 |
| ${report.openingHands.policyFidelity === "strictExecutable" ? "Keepable after mulligans" : "Exploratory keepable-after-mulligans estimate"} | ${pct(report.openingHands.keepableAfterMulligansRate)} |
| Any engine/tutor in kept hand | ${pct(report.openingHands.engineAccessRate)} |
| Primary-plan card proxy | ${pct(report.overview.primaryPlanAccessRate)} |
| Baseline threat population P10 | ${turn(report.winSpeed.baseline.p10)} |
| Population median baseline threat | ${turn(report.winSpeed.baseline.median)} |
| Baseline threat population P90 | ${turn(report.winSpeed.baseline.p90)} |
| ${selectedInteractionProfile} threat population P10 | ${turn(report.winSpeed.interfered.p10)} |
| ${selectedInteractionProfile} threat population median | ${turn(report.winSpeed.interfered.median)} |
| ${selectedInteractionProfile} threat population P90 | ${turn(report.winSpeed.interfered.p90)} |
| Baseline first-win-attempt population P10 | ${turn(report.winSpeed.baselineWinAttempt.p10)} |
| Population median baseline first win attempt | ${turn(report.winSpeed.baselineWinAttempt.median)} |
| Baseline first-win-attempt population P90 | ${turn(report.winSpeed.baselineWinAttempt.p90)} |
| ${selectedInteractionProfile} first-win-attempt population P10 | ${turn(report.winSpeed.interferedWinAttempt.p10)} |
| ${selectedInteractionProfile} first-win-attempt population median | ${turn(report.winSpeed.interferedWinAttempt.median)} |
| ${selectedInteractionProfile} first-win-attempt population P90 | ${turn(report.winSpeed.interferedWinAttempt.p90)} |
| Baseline first win attempt demonstrated by turn cap | ${pct(report.winSpeed.baselineWinAttempt.demonstratedRate)} |
| Successful-run-only baseline first-attempt median | ${conditionalTurn(report.winSpeed.baselineWinAttempt.conditionalMedian)} |
| Baseline generic engine/combat milestone | ${turn(report.winSpeed.baselineGenericConversionMilestone?.median)} |
| Generic milestone demonstrated by turn cap | ${pct(report.winSpeed.baselineGenericConversionMilestone?.demonstratedRate ?? 0)} |
| Baseline proactive-development population median | ${turn(report.winSpeed.baselineModelPace?.median)} |
| Successful-run-only proactive-development median | ${conditionalTurn(report.winSpeed.baselineModelPace?.conditionalMedian)} |
| Proactive development demonstrated by turn cap | ${pct(report.winSpeed.baselineModelPace?.demonstratedRate ?? 0)} |
| Baseline resolved-table-win population P10 | ${turn(baselineResolved.p10)} |
| Population median baseline resolved table win | ${turn(baselineResolved.median)} |
| Baseline resolved-table-win population P90 | ${turn(baselineResolved.p90)} |
| ${selectedInteractionProfile} resolved-table-win population P10 | ${turn(interferedResolved.p10)} |
| ${selectedInteractionProfile} resolved-table-win population median | ${turn(interferedResolved.median)} |
| ${selectedInteractionProfile} resolved-table-win population P90 | ${turn(interferedResolved.p90)} |
| Baseline resolved table win demonstrated by turn cap | ${pct(baselineResolved.demonstratedRate)} |
| Successful-run-only baseline resolved-win median | ${conditionalTurn(baselineResolved.conditionalMedian)} |
| Recovery after a stopped attempt | ${recovery} |
| Paired observed attempt delays | ${report.winSpeed.pairedWinAttemptDelay.observedPairs} |
| Baseline attempts prevented through turn cap | ${report.winSpeed.pairedWinAttemptDelay.preventedByTurnCap} |
| Paired observed resolved-win delays | ${pairedResolvedDelay.observedPairs} |
| Baseline resolved wins prevented through turn cap | ${pairedResolvedDelay.preventedByTurnCap} |

The primary-plan value is a conservative deck-role opening proxy, not a claim that a complete line is executable. The speed evidence basis identifies which deck-specific signal supplied the overview score. Proactive development is the earlier per-episode explicit attempt or generic milestone and is not itself a win attempt or probability; structural pace is a capped list-observable fallback, not a timing claim.
A credible threat is an answer-demanding state. A first win attempt requires either a recognized reviewed table-lethal line or a rules-backed combat assignment that would eliminate every remaining opponent if its damage connects. A generic engine/combat milestone is a separate development diagnostic and cannot populate attempt timing. A resolved table win is recorded only when the typed line resolves or the assigned combat damage actually connects and produces a terminal game state. The aggregate comparison uses the selected profile: **${selectedInteractionProfile}**. The eight isolated interaction scenarios are independent fixed counterfactual diagnostics. Censored runs remain censored at the turn cap; these endpoints are never substituted for one another and are not multiplayer pod win percentages.
${report.openingHands.policyFidelity === "strictExecutable"
    ? ""
    : " Functional mulligan, keepability, and engine-access values are not strict consistency claims for this report."}

${executionCoverageSection}

${routeProvenanceSection}

${earlyEvaluationSection}

${interactionScenarioSection}

## Colored mana

- Source reliability: **${capitalize(report.openingHands.mana.reliabilityBand)}** (${pct(report.openingHands.mana.reliabilityScore)})
- Opening color coverage: ${pct(report.openingHands.mana.averageOpeningColorCoverage)}
- Turn-three color coverage: ${pct(report.openingHands.mana.averageTurnThreeColorCoverage)}
- Model confidence: ${pct(report.openingHands.mana.modelConfidence)}
- Sources: ${report.openingHands.mana.landSourceCount} land, ${report.openingHands.mana.nonlandSourceCount} nonland, ${report.openingHands.mana.conditionalSourceCount} conditional

| Color | Weighted sources | Exact | Conditional | Demand pips |
| :---: | ---: | ---: | ---: | ---: |
${manaRows}

## Recommendation evidence

${evidence}

${strategicSection}

## Known compact lines

${lines}

## Typed synergy relationships

${graph
    ? `- Graph model: \`${graph.modelVersion}\`
- Ability model: \`${graph.abilityModelVersion}\`
- Connected modeled nonlands: ${pct(graph.graphCoverage)}
- Explicit links: ${graph.edgeCount} (${graph.displayedEdgeCount} retained in the report)
- Unsupported clauses retained for review: ${graph.unsupportedClauseCount}

These relationships are report-only. They explain typed Oracle-text resource and trigger matches but are not yet executed as game actions.

${graphLinks}`
    : "This report predates the typed synergy graph."}

## Coverage

| Layer | Coverage |
| --- | ---: |
| Card identity | ${pct(report.coverage.identityResolution)} |
| Semantic roles | ${pct(report.coverage.semanticCoverage)} |
| Simulation | ${pct(report.coverage.simulationCoverage)} |

## Reproducibility

- Canonical deck SHA-256: \`${report.deck.canonicalDeckSha256}\`
- Opening-hand trials: ${report.assumptions.openingHandSimulations}
- Paired game trials: ${report.assumptions.gameSimulations}
- Analyzer policy: Fixed competitive policy with aggressive route search
- Primary timing horizon: turns 1-${report.assumptions.maximumTurn}
- Selected interaction profile: ${selectedInteractionProfile}
- Isolated scenario suite: Eight independent fixed checkpoints
- Player-declared intent: Not used for evaluation
- Online missing-card resolution: ${report.assumptions.allowOnlineCardResolution ? "Enabled (unresolved names may be sent to Scryfall)" : "Disabled; local data only"}
- Seed: ${report.assumptions.seedExact ?? String(report.assumptions.seed)}
- Opening candidate cohort: ${report.openingHands.candidateCohortVersion || "Not recorded"}
- Opening candidate cohort SHA-256: ${report.openingHands.candidateCohortSha256 || "Not recorded"}
- Result source: ${report.cache.hit ? `Local cache (${report.cache.createdAt ?? "timestamp unavailable"})` : "Fresh local analysis"}
- Cache key format: ${report.cache.keyVersion}
- Card data: ${report.versions.cardData}
- Card snapshot SHA-256: ${report.versions.cardSnapshotSha256 ?? "No full snapshot installed"}
- Commander policy package: ${report.versions.rulesPackage}
- Commander policy origin: ${report.versions.rulesPackageOrigin ?? "Not recorded"}
- Commander policy SHA-256: ${report.versions.rulesSnapshotSha256 ?? "Not recorded"}
- Comprehensive Rules effective date: ${report.versions.comprehensiveRulesEffectiveDate ?? "Not installed"}
- Comprehensive Rules SHA-256: ${report.versions.comprehensiveRulesSnapshotSha256 ?? "Not installed"}
- Comprehensive Rules parser: ${report.versions.comprehensiveRulesParserVersion ?? "Not installed"}
- Rule capability model: ${report.versions.ruleCapabilityModel ?? "Not installed"}
- Structural strategy model: ${report.versions.strategicProfileModel ?? "Not recorded"}
- Semantic compiler: ${report.versions.semanticModel}
- Semantic package snapshot (package-declared dates): ${semanticPackage}
- Semantic package origin: ${report.versions.semanticPackageOrigin ?? "Not recorded"}
- Semantic snapshot SHA-256: ${report.versions.semanticSnapshotSha256 ?? "Not recorded"}
- Semantic package imported: ${report.versions.semanticImportedAt ?? "Bundled with this app build or not recorded"}
- Semantic package provenance: ${report.versions.semanticAuthenticityBasis ?? "Not recorded"}
- Simulation engine: ${report.versions.simulationEngine}
- Opening-hand strength model: ${report.versions.effectiveHandStrengthModel ?? "Not recorded"}
- Timing endpoint contract: ${report.winSpeed.timingEndpointVersion}
- Executable ability program: ${report.versions.abilityProgram ?? "Not recorded"}
- Turn planner: ${report.versions.turnPlanner ?? "Not recorded"}
- Strict execution kernel: ${report.versions.strictEngine ?? "Not recorded"}
- Execution coverage compiler: ${report.versions.executionCoverageCompiler ?? "Not recorded"}
- Bracket model: ${report.versions.bracketModel}
- Combo catalog: ${report.versions.comboCatalog ?? "Built-in verified lines only"}
- Combo snapshot SHA-256: ${report.versions.comboSnapshotSha256 ?? "Not installed"}

### Canonical analyzed deck

\`\`\`text
${report.deck.canonicalDeck}
\`\`\`

## Interpretation notes

${notes}
`;
}

const pct = (value: number) => `${Math.round(value * 100)}%`;
const turn = (value?: number | null) =>
  typeof value === "number" && Number.isFinite(value)
    ? `Turn ${value.toFixed(1)}`
    : "Population quantile not reached by turn cap";
const conditionalTurn = (value?: number | null) =>
  typeof value === "number" && Number.isFinite(value)
    ? `Turn ${value.toFixed(1)}`
    : "Successful-run quantile unavailable";
const capitalize = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);
const words = (value: string) =>
  value.replace(/([a-z])([A-Z])/g, "$1 $2").replace(/[-_]/g, " ").toLowerCase();
const declaredSemanticPackage = (value: string) => value
  .replace(" · effective ", " · declared effective ")
  .replace(" · verified ", " · declared verified ");

function formatInteractionScenariosMarkdown(report: AnalysisReport): string {
  const scenarios = report.winSpeed.interactionScenarios ?? [];
  const selectedInteractionProfile = interactionProfileLabel(
    report.assumptions.interactionProfile,
  );
  if (!scenarios.length) {
    return `## Isolated interaction scenarios

This report contains only the legacy stress summaries; no canonical paired
scenario suite was attached.
`;
  }
  const rows = scenarios
    .map((scenario) => {
      const delay = scenario.firstWinAttemptDelay;
      const resolvedDelay = scenario.resolvedTableWinDelay;
      const recovery = scenario.recovery;
      const applicability = scenario.applicability;
      const status = scenario.counters.undeterminedEpisodes === scenario.counters.totalEpisodes
        ? `Undetermined${applicability.primaryUndeterminedReason
          ? `: ${singleLine(applicability.primaryUndeterminedReason)}`
          : ""}`
        : scenario.counters.notApplicableEpisodes === scenario.counters.totalEpisodes
          ? `Not applicable: ${scenarioNotApplicableReason(
            applicability.primaryNotApplicableReason,
          )}`
          : `${scenario.counters.applicableEpisodes} applicable / ${scenario.counters.notApplicableEpisodes} N/A / ${scenario.counters.undeterminedEpisodes} undetermined`;
      const median = delay.observedDelayMedianTurns == null
        ? "Not observed"
        : `${delay.observedDelayMedianTurns >= 0 ? "+" : ""}${delay.observedDelayMedianTurns.toFixed(1)}`;
      const resolvedMedian = !resolvedDelay
        ? "Not recorded"
        : resolvedDelay.observedDelayMedianTurns == null
          ? "Not observed"
          : `${resolvedDelay.observedDelayMedianTurns >= 0 ? "+" : ""}${resolvedDelay.observedDelayMedianTurns.toFixed(1)}`;
      const resolvedRightCensored = resolvedDelay
        ? String(resolvedDelay.rightCensoredPairs)
        : "Not recorded";
      const recoveryRate = recovery.recoveredByTurnCapRate == null
        ? "Not sampled"
        : pct(recovery.recoveredByTurnCapRate);
      return `| ${scenarioName(scenario.directive.scenario)} | ${status} | ${scenario.counters.opportunityEpisodes} | ${scenario.counters.effectfulInterventionEpisodes} | ${median} | ${delay.rightCensoredPairs} | ${resolvedMedian} | ${resolvedRightCensored} | ${recoveryRate} |`;
    })
    .join("\n");
  const measurement = scenarios[0].measurement;
  return `## Isolated interaction scenarios

**Measurement:** ${measurement.label}. ${measurement.claimBoundary}

**Profile separation:** These eight fixed counterfactual checkpoints are
independent of the selected aggregate profile (**${selectedInteractionProfile}**).

**Sampling:** ${scenarios[0].sampling.episodeCount} paired episodes through turn
${scenarios[0].sampling.maximumTurn}; master seed
\`${scenarios[0].sampling.masterSeedExact ?? String(scenarios[0].sampling.masterSeed)}\`;
derivation \`${scenarios[0].sampling.seedDerivationVersion}\`.

| Scenario | Applicability | Opportunities | Effectful pairs | Observed median first-attempt delay | First-attempt right-censored | Observed median resolved-win delay | Resolved-win right-censored | Recovered by cap |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
${rows}

Not-applicable and undetermined episodes remain separate from absent
opportunities. Right-censored delays are not replaced with invented turns.
`;
}

function scenarioName(
  scenario: NonNullable<AnalysisReport["winSpeed"]["interactionScenarios"]>[number]["directive"]["scenario"],
): string {
  return scenario
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^./, (letter) => letter.toUpperCase());
}

function scenarioNotApplicableReason(
  reason?: NonNullable<
    AnalysisReport["winSpeed"]["interactionScenarios"]
  >[number]["applicability"]["primaryNotApplicableReason"],
): string {
  switch (reason) {
    case "noEligibleNoncommanderPermanent":
      return "no eligible strategic noncommander permanent";
    case "noCommanderSubject":
      return "no commander subject";
    case "noRelevantSpellClass":
      return "no relevant spell class";
    case "noRelevantCreatureBoardPlan":
      return "no relevant multi-creature board plan";
    case "noGraveyardDependency":
      return "no executable graveyard dependency";
    case "noTaxableActionClass":
      return "no taxable action class";
    case "noMultispellPlan":
      return "no executable multi-spell plan";
    case "noRepresentableWinAttempt":
      return "no representable first win attempt";
    default:
      return "structurally absent from the bounded plan";
  }
}

function singleLine(value: string): string {
  return value
    .replace(/\s+/g, " ")
    .replace(/\\/g, "\\\\")
    .replace(/\|/g, "\\|")
    .trim();
}
