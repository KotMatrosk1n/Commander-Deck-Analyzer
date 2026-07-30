import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  Activity,
  ArrowLeft,
  ArrowDownRight,
  ArrowUpRight,
  BookOpenCheck,
  Braces,
  CircleGauge,
  ClipboardCopy,
  Download,
  FlaskConical,
  Hand,
  Info,
  Network,
  RotateCcw,
  ShieldCheck,
  Sparkles,
  Target,
  TimerReset,
  TriangleAlert,
  Trophy,
} from "lucide-react";
import { isTauri, writeTextFile } from "./api";
import {
  formatReportMarkdown,
  hasStrictFunctionalRating,
  reportFileStem,
} from "./reportExport";
import { interactionProfileLabel } from "./interactionProfiles";
import type {
  AnalysisReport,
  CompactInteractionScenarioReport,
  ReportTab,
  TurnDistribution,
} from "./types";
import { UiErrorBoundary } from "./UiErrorBoundary";

interface ReportViewProps {
  report: AnalysisReport;
  activeTab: ReportTab;
  onTabChange: (tab: ReportTab) => void;
  stale: boolean;
}

const tabs: { id: ReportTab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "hands", label: "Opening hands" },
  { id: "speed", label: "Threat timing" },
  { id: "synergy", label: "Synergy" },
  { id: "method", label: "Method & coverage" },
];

export function ReportView({ report, activeTab, onTabChange, stale }: ReportViewProps) {
  const [actionStatus, setActionStatus] = useState<string | null>(null);

  const copySummary = async () => {
    try {
      await copyText(formatReportMarkdown(report));
      setActionStatus("Report copied");
    } catch {
      setActionStatus("Clipboard access was unavailable");
    }
  };

  const exportReport = async () => {
    try {
      const contents = formatReportMarkdown(report);
      if (!isTauri()) {
        downloadBrowserFile(`${reportFileStem(report)}.md`, contents);
        setActionStatus("Report downloaded");
        return;
      }
      const path = await save({
        defaultPath: `${reportFileStem(report)}.md`,
        filters: [{ name: "Markdown report", extensions: ["md"] }],
      });
      if (!path) return;
      await writeTextFile(path, contents);
      setActionStatus("Report saved");
    } catch {
      setActionStatus("The report could not be saved");
    }
  };

  return (
    <section className="report-view" aria-label="Analysis report">
      <div className="report-heading">
        <div>
          <div className="eyebrow">Analysis report</div>
          <h2>{report.deck.commanders.join(" + ") || "Commander deck"}</h2>
        </div>
        <div className="report-heading-actions">
          <div className="report-meta">
            <span>{report.cache.hit ? "Local cache" : report.elapsedMs < 1000 ? `${report.elapsedMs} ms` : `${(report.elapsedMs / 1000).toFixed(1)} s`}</span>
            <span aria-hidden="true">•</span>
            <span>{report.openingHands.simulations.toLocaleString()} hands</span>
            <span aria-hidden="true">•</span>
            <span>{report.winSpeed.simulations.toLocaleString()} paired trajectories</span>
          </div>
          <div className="report-actions">
            <button onClick={() => void copySummary()} type="button">
              <ClipboardCopy size={15} />
              Copy summary
            </button>
            <button onClick={() => void exportReport()} type="button">
              <Download size={15} />
              Export report
            </button>
          </div>
          <span className="report-action-status" role="status" aria-live="polite">
            {actionStatus}
          </span>
        </div>
      </div>

      {stale && (
        <div className="stale-banner" role="status">
          <Info size={16} />
          Deck changed since this report. Reanalyze to update the results.
        </div>
      )}

      {(report.winSpeed.fidelity !== "strictExecutable"
        || report.openingHands.policyFidelity !== "strictExecutable") && (
        <div className="model-boundary-banner" role="status">
          <TriangleAlert size={16} />
          <div>
            <strong>
              {report.winSpeed.fidelity === "blockedUnsupported"
                || report.openingHands.policyFidelity === "blockedUnsupported"
                ? "Strict functional analysis blocked"
                : "Legacy heuristic timing"}
            </strong>
            <span>{report.winSpeed.fidelityMessage}</span>
            {report.openingHands.policyFidelity !== "strictExecutable" && (
              <span>
                Functional keep/bottom decisions are{" "}
                {report.openingHands.policyFidelity === "blockedUnsupported"
                  ? "blocked by unsupported card functions"
                  : "legacy heuristic estimates"}
                ; opening-card composition remains the only raw sampling claim.
              </span>
            )}
          </div>
        </div>
      )}

      <nav className="report-tabs" aria-label="Report sections">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={activeTab === tab.id ? "active" : ""}
            onClick={() => onTabChange(tab.id)}
            type="button"
          >
            {tab.label}
          </button>
        ))}
      </nav>

      <div className="report-scroll">
        <UiErrorBoundary
          key={`${report.runId}:${activeTab}`}
          fallback={({ error, reset }) => (
            <ReportSectionRecovery
              activeTab={activeTab}
              error={error}
              onRetry={reset}
              onReturnToOverview={() => onTabChange("overview")}
            />
          )}
        >
          {activeTab === "overview" && <Overview report={report} />}
          {activeTab === "hands" && <OpeningHands report={report} />}
          {activeTab === "speed" && <WinSpeed report={report} />}
          {activeTab === "synergy" && <Synergy report={report} />}
          {activeTab === "method" && <Method report={report} />}
        </UiErrorBoundary>
      </div>
    </section>
  );
}

function ReportSectionRecovery({
  activeTab,
  error,
  onRetry,
  onReturnToOverview,
}: {
  activeTab: ReportTab;
  error: Error;
  onRetry: () => void;
  onReturnToOverview: () => void;
}) {
  return (
    <div className="report-section-recovery" role="alert">
      <div className="recovery-icon"><TriangleAlert size={22} /></div>
      <span className="eyebrow">Section recovery</span>
      <h3>This report section could not be displayed</h3>
      <p>
        The rest of the report is still available. You can retry this section
        or return to the overview without restarting the application.
      </p>
      <div className="recovery-actions">
        <button onClick={onRetry} type="button">
          <RotateCcw size={15} />
          Retry section
        </button>
        {activeTab !== "overview" && (
          <button onClick={onReturnToOverview} type="button">
            <ArrowLeft size={15} />
            Return to overview
          </button>
        )}
      </div>
      <details>
        <summary>Technical details</summary>
        <code>{error.message || "Unknown rendering error"}</code>
      </details>
    </div>
  );
}

function Overview({ report }: { report: AnalysisReport }) {
  const strictRatingAvailable = hasStrictFunctionalRating(report);
  const ratingGate = report.coverage.executionManifest?.gates.find(
    (gate) => gate.metric === "bracketRating",
  );
  const recoveryDetail = report.winSpeed.recoveryByMaxTurnRate == null
    ? "Recovery not sampled"
    : `${pct(report.winSpeed.recoveryByMaxTurnRate)} modeled recovery`;
  const metrics = [
    { label: "Mana", value: report.overview.manaScore, icon: CircleGauge, detail: `${pct(report.overview.commanderOnCurveRate)} commander on curve` },
    {
      label: "Consistency",
      value: report.overview.consistencyScore,
      icon: Hand,
      detail: report.openingHands.policyFidelity === "strictExecutable"
        ? `${pct(report.openingHands.keepableAfterMulligansRate)} keepable`
        : `${pct(report.openingHands.keepableAfterMulligansRate)} exploratory keep estimate`,
    },
    { label: "Speed", value: report.overview.speedScore, icon: Activity, detail: `Baseline · ${speedScoreDetail(report)}` },
    { label: "Interaction", value: report.overview.interactionScore, icon: ShieldCheck, detail: "Removal, counters & protection" },
    { label: "Synergy", value: report.overview.synergyScore, icon: Network, detail: `${report.synergy.detectedPlans.length} plan${report.synergy.detectedPlans.length === 1 ? "" : "s"} detected` },
    { label: "Resilience", value: report.overview.resilienceScore, icon: TimerReset, detail: recoveryDetail },
  ];

  return (
    <div className="report-page overview-page">
      <section className={`recommendation-card ${strictRatingAvailable ? "" : "rating-unavailable"}`}>
        <div
          className="bracket-orb"
          aria-label={`${strictRatingAvailable ? "Model" : "Exploratory model"} bracket estimate ${report.recommendation.likelyBracket}`}
        >
          <span>Estimated Bracket</span>
          <strong>{report.recommendation.likelyBracket}</strong>
        </div>
        <div className="recommendation-copy">
          <div className="recommendation-pills">
            <span>
              {`${strictRatingAvailable ? "Model" : "Exploratory model"} range ${report.recommendation.rangeLow}-${report.recommendation.rangeHigh}`}
            </span>
            {!strictRatingAvailable && <span>Strict functional rating unavailable</span>}
            <span className={`confidence ${report.recommendation.confidence}`}>
              {capitalize(report.recommendation.confidence)} model coverage
            </span>
            <span className={`legality ${report.policy.legality}`}>
              {capitalize(report.policy.legality)} list
            </span>
            {report.recommendation.rulesFloor && (
              <span>Policy floor B{report.recommendation.rulesFloor}</span>
            )}
          </div>
          <p>{report.recommendation.summary}</p>
          <p className="method-caveat">
            {strictRatingAvailable
              ? "This is an explainable, uncalibrated model estimate, not an official bracket ruling."
              : `Low-confidence warning: strict functional coverage is blocked by ${ratingGate?.blockingLeafCount ?? "unreported"} execution-preflight leaves. The estimate, range, and weights remain exploratory and uncalibrated; the official policy floor remains separate.`}
          </p>
        </div>
        <div
          className="probability-stack"
          aria-label={`${strictRatingAvailable ? "Uncalibrated" : "Exploratory uncalibrated"} bracket model weights`}
        >
          {report.recommendation.probabilities.map((entry) => (
            <div className="probability-row" key={entry.bracket}>
              <span>B{entry.bracket}</span>
              <div className="bar-track">
                <div
                  className={entry.bracket === report.recommendation.likelyBracket ? "bar-fill primary" : "bar-fill"}
                  style={{ width: `${Math.max(entry.probability * 100, 1)}%` }}
                />
              </div>
              <strong>{pct(entry.probability)}</strong>
            </div>
          ))}
          <small className="model-weight-note">
            {strictRatingAvailable ? "Model" : "Exploratory model"} weights · not observed win or bracket probabilities
          </small>
        </div>
      </section>

      <div className="metric-grid">
        {metrics.map(({ label, value, icon: Icon, detail }) => (
          <article className="metric-card" key={label}>
            <div className="metric-icon"><Icon size={18} /></div>
            <div className="metric-copy">
              <span>{label}</span>
              <strong>{value}<small>/100</small></strong>
              <p>{detail}</p>
            </div>
            <MiniRing value={value} />
          </article>
        ))}
      </div>

      <section className="evidence-panel">
        <div className="section-heading">
          <div>
            <span className="eyebrow">
              {strictRatingAvailable ? "Recommendation evidence" : "Exploratory evidence"}
            </span>
            <h3>
              {strictRatingAvailable ? "Why this range?" : "Why this exploratory range?"}
            </h3>
          </div>
          <span className="coverage-inline">{pct(report.coverage.semanticCoverage)} semantic coverage</span>
        </div>
        <div className="evidence-list">
          {report.evidence.map((item) => (
            <article className="evidence-item" key={`${item.title}-${item.detail}`}>
              <div className={`evidence-direction ${item.direction}`}>
                {item.direction === "raises" ? <ArrowUpRight size={17} /> : item.direction === "lowers" ? <ArrowDownRight size={17} /> : <Info size={17} />}
              </div>
              <div>
                <h4>{item.title}</h4>
                <p>{item.detail}</p>
              </div>
            </article>
          ))}
        </div>
      </section>
    </div>
  );
}

function OpeningHands({ report }: { report: AnalysisReport }) {
  const mana = report.openingHands.mana;
  const functionalPolicyExecutable =
    report.openingHands.policyFidelity === "strictExecutable";
  const rows = [
    ["Keepable seven", report.openingHands.keepableSevenRate],
    ["Keepable after mulligans", report.openingHands.keepableAfterMulligansRate],
    ["At least two lands", report.openingHands.twoLandRate],
    ["Third land by turn three", report.openingHands.threeLandByTurnThreeRate],
    ["Early ramp access", report.openingHands.rampAccessRate],
    ["Any engine/tutor in kept hand", report.openingHands.engineAccessRate],
    ["Primary-plan card proxy", report.overview.primaryPlanAccessRate],
  ] as const;
  return (
    <div className="report-page">
      <div className="stat-hero-grid">
        <StatHero label="Keepable after mulligans" value={pct(report.openingHands.keepableAfterMulligansRate)} detail={`Monte Carlo 95% margin ±${(report.openingHands.confidenceMargin * 100).toFixed(1)} points`} icon={<Hand />} />
        <StatHero label="Average mulligans" value={report.openingHands.averageMulligans.toFixed(2)} detail={`${report.openingHands.averageCardsKept.toFixed(2)} average cards kept`} icon={<TimerReset />} />
        <StatHero label="Turn-three color access" value={pct(mana.averageTurnThreeColorCoverage)} detail={`${words(mana.reliabilityBand)} colored-mana support`} icon={<CircleGauge />} />
      </div>
      <section className="panel-section">
        <div className="section-heading">
          <div><span className="eyebrow">Consistency profile</span><h3>{report.openingHands.simulations.toLocaleString()} deterministic trials</h3></div>
          <span className={`coverage-inline execution-state ${functionalPolicyExecutable ? "executable" : "blocked"}`}>
            Functional mulligan {words(report.openingHands.policyFidelity)}
          </span>
        </div>
        {!functionalPolicyExecutable && (
          <p className="policy-floor-reason">
            Hand composition was sampled from real deck objects, but keep and
            bottom decisions use exploratory legacy card-role estimates. Do not
            read “keepable” or engine-access values as strict functional
            consistency until every relevant card ability is executable.
          </p>
        )}
        <div className="horizontal-bars">
          {rows.map(([label, value]) => (
            <div className="horizontal-bar" key={label}>
              <div><span>{label}</span><strong>{pct(value)}</strong></div>
              <div className="bar-track large"><div className="bar-fill primary" style={{ width: `${value * 100}%` }} /></div>
            </div>
          ))}
        </div>
      </section>
      <section className="panel-section">
        <div className="section-heading">
          <div><span className="eyebrow">Colored mana model</span><h3>Requirements matched against weighted sources</h3></div>
          <span className={`mana-reliability ${mana.reliabilityBand}`}>{words(mana.reliabilityBand)} · {pct(mana.reliabilityScore)}</span>
        </div>
        <div className="detail-grid mana-summary-grid">
          <Detail label="Opening color coverage" value={pct(mana.averageOpeningColorCoverage)} />
          <Detail label="Turn-three color coverage" value={pct(mana.averageTurnThreeColorCoverage)} />
          <Detail label="Model confidence" value={pct(mana.modelConfidence)} />
          <Detail label="Land mana sources" value={String(mana.landSourceCount)} />
          <Detail label="Nonland mana sources" value={String(mana.nonlandSourceCount)} />
          <Detail label="Tapped / conditional lands" value={`${mana.entersTappedLandCount} / ${mana.conditionalSourceCount}`} />
        </div>
        {mana.colors.length ? (
          <div className="mana-source-list" aria-label="Mana sources by color">
            <div className="mana-source-header">
              <span>Color</span><span>Weighted sources</span><span>Exact</span><span>Conditional</span><span>Demand</span>
            </div>
            {mana.colors.map((source) => (
              <div className="mana-source-row" key={source.color}>
                <span className={`mana-color mana-${source.color.toLowerCase()}`}>{source.color}</span>
                <strong>{source.weightedSourceEquivalents.toFixed(1)}</strong>
                <span>{source.exactSources}</span>
                <span>{source.conditionalSources}</span>
                <span>{source.demandPipAppearances} pips</span>
              </div>
            ))}
          </div>
        ) : null}
        {mana.notes.length ? <p className="mana-notes">{mana.notes.slice(0, 3).join(" ")}</p> : null}
      </section>
      <div className="explanation-card">
        <FlaskConical size={20} />
        <div>
          <h4>What this simulation includes</h4>
          <p>Commander cards begin in the command zone. Each trial applies a multiplayer free mulligan, then London mulligans using the analyzer’s fixed aggressive route-search policy. “Any engine/tutor” is the broad simulated rate; the primary-plan value is a conservative deck-role proxy for seeing a card tied to the strongest detected plan, not proof that a complete line is ready. Colored-source coverage accounts for hybrid costs, commander-identity sources, conditional fetches, and tapped-land reliability; unsupported Oracle clauses lower model confidence. The seed is stored with the report, so the run can be reproduced.</p>
        </div>
      </div>
    </div>
  );
}

function WinSpeed({ report }: { report: AnalysisReport }) {
  const profile = interactionProfileLabel(report.assumptions.interactionProfile);
  const baselineResolved = report.winSpeed.baselineResolvedTableWin;
  const interferedResolved = report.winSpeed.interferedResolvedTableWin;
  const pairedResolvedDelay = report.winSpeed.pairedResolvedTableWinDelay;
  const recoveryObserved = report.winSpeed.recoveryOpportunities > 0
    && report.winSpeed.recoveryByMaxTurnRate != null;
  const stressTests = report.winSpeed.stressTests ?? [];
  const interactionScenarios = report.winSpeed.interactionScenarios ?? [];
  const attemptProvenance = report.winSpeed.attemptProvenance;
  const explicitRoutes = attemptProvenance?.explicitRoutes ?? [];
  const earlyBlockers = (attemptProvenance?.earlyTurnBlockers ?? [])
    .filter((blocker) => blocker.sample === "baseline")
    .sort((left, right) => right.rate - left.rate || left.turn - right.turn)
    .slice(0, 8);
  const baselineGenericMilestone = report.winSpeed.baselineGenericConversionMilestone;
  const interferedGenericMilestone = report.winSpeed.interferedGenericConversionMilestone;
  const earlyEvaluation = report.winSpeed.earlyTurnEvaluation;
  const omittedConversionRoutes = earlyEvaluation?.omittedNonTableWinLineCount ?? 0;
  return (
    <div className="report-page">
      <div className="stat-hero-grid">
        <StatHero label="Baseline credible threat" value={turnValue(report.winSpeed.baseline.median)} detail={`${pct(report.winSpeed.baseline.demonstratedRate)} presented one by turn ${report.assumptions.maximumTurn}`} icon={<Activity />} />
        <StatHero label="Baseline first win attempt" value={turnValue(report.winSpeed.baselineWinAttempt.median)} detail={`${pct(report.winSpeed.baselineWinAttempt.demonstratedRate)} presented an attempt by turn ${report.assumptions.maximumTurn}`} icon={<Target />} />
        <StatHero
          label="Baseline resolved table win"
          value={endpointTurnValue(baselineResolved)}
          detail={baselineResolved
            ? `${pct(baselineResolved.demonstratedRate)} had a typed table-lethal resolution proven by turn ${report.assumptions.maximumTurn}`
            : "Legacy endpoint not recorded · reanalyze"}
          icon={<Trophy />}
        />
      </div>
      <section className="panel-section">
        <div className="section-heading">
          <div><span className="eyebrow">Timing endpoints</span><h3>Credible threat, first attempt, and resolved table win are separate</h3></div>
          <span className="coverage-inline">{recoveryObserved ? `${pct(report.winSpeed.recoveryByMaxTurnRate ?? 0)} recovered after a stopped attempt` : "No recovery opportunity sampled"}</span>
        </div>
        <div className="detail-grid">
          <Detail label="Baseline credible threat" value={distributionLabel(report.winSpeed.baseline)} />
          <Detail label={`${profile} credible threat`} value={distributionLabel(report.winSpeed.interfered)} />
          <Detail label="Baseline first win attempt" value={distributionLabel(report.winSpeed.baselineWinAttempt)} />
          <Detail label={`${profile} first win attempt`} value={distributionLabel(report.winSpeed.interferedWinAttempt)} />
          <Detail label="Baseline resolved table win" value={distributionLabel(baselineResolved)} />
          <Detail label={`${profile} resolved table win`} value={distributionLabel(interferedResolved)} />
          <Detail label="First-attempt opportunities" value={report.winSpeed.firstAttemptOpportunities.toLocaleString()} />
          <Detail label="Stopped among opportunities" value={report.winSpeed.firstAttemptOpportunities > 0 ? pct(report.winSpeed.firstAttemptStoppedRate) : "Not observed"} />
          <Detail label="Recovery opportunities" value={report.winSpeed.recoveryOpportunities.toLocaleString()} />
          <Detail label="Recovery among stopped attempts" value={recoveryObserved ? pct(report.winSpeed.recoveryByMaxTurnRate ?? 0) : "Not observed"} />
          <Detail label="Paired observed attempt delays" value={report.winSpeed.pairedWinAttemptDelay.observedPairs.toLocaleString()} />
          <Detail label="Attempts prevented through turn cap" value={report.winSpeed.pairedWinAttemptDelay.preventedByTurnCap.toLocaleString()} />
          <Detail label="Median first-attempt delay" value={medianDelayLabel(report.winSpeed.winAttemptMedianDelay)} />
          <Detail
            label="Paired observed resolved-win delays"
            value={pairedResolvedDelay
              ? pairedResolvedDelay.observedPairs.toLocaleString()
              : "Legacy endpoint not recorded"}
          />
          <Detail
            label="Resolved wins prevented through turn cap"
            value={pairedResolvedDelay
              ? pairedResolvedDelay.preventedByTurnCap.toLocaleString()
              : "Legacy endpoint not recorded"}
          />
          <Detail
            label="Median resolved-win delay"
            value={resolvedDelayLabel(report.winSpeed.resolvedTableWinMedianDelay, pairedResolvedDelay != null)}
          />
        </div>
      </section>
      <section className="panel-section">
        <div className="section-heading">
          <div>
            <span className="eyebrow">Exact early-route access</span>
            <h3>T1/T2 table-win route skeletons and candidate-hand search envelope</h3>
          </div>
          <span className="coverage-inline">
            {earlyEvaluation
              ? `${earlyEvaluation.eligibleTableWinRouteCount} eligible explicit route${earlyEvaluation.eligibleTableWinRouteCount === 1 ? "" : "s"}${omittedConversionRoutes > 0 ? ` · ${omittedConversionRoutes} recognized route${omittedConversionRoutes === 1 ? "" : "s"} requiring conversion` : ""}`
              : "Evaluation not recorded"}
          </span>
        </div>
        {earlyEvaluation?.routes.length ? (
          <div className="stress-table" aria-label="Early explicit-route readiness">
            {earlyEvaluation.routes.map((route) => {
              const turnOne = route.turns.find((turn) => turn.turn === 1);
              const turnTwo = route.turns.find((turn) => turn.turn === 2);
              const executionWitness = earlyEvaluation.executionWitnesses.find(
                (witness) => witness.routeId === route.routeId,
              );
              return (
                <div className="stress-row" key={route.routeId}>
                  <span className="severity-dot info" />
                  <strong>{route.routeName}</strong>
                  <span>
                    {executionWitness
                      ? `Legal trajectory T${executionWitness.turn} ${executionWitness.resolvedTableWin ? "resolved table win" : "recognized attempt"}`
                      : "No legal T1/T2 trajectory found in the bounded witness search"}
                    {" · "}
                    T1 direct {pct(turnOne?.directSkeletonProbability ?? 0)}
                    {" · "}T2 direct {pct(turnTwo?.directSkeletonProbability ?? 0)}
                    {" · "}{route.aggressiveMulligan.candidateHands}-candidate envelope direct {pct(route.aggressiveMulligan.directSkeletonInAtLeastOneCandidate)}
                    {" · "}{words(route.modelingCeiling)}
                  </span>
                </div>
              );
            })}
          </div>
        ) : omittedConversionRoutes > 0 ? (
          <p className="empty-inline">
            {omittedConversionRoutes === 1
              ? "1 recognized route requires a table-lethal conversion, so it is not counted as an eligible table-win route for exact early access enumeration."
              : `${omittedConversionRoutes} recognized routes require a table-lethal conversion, so they are not counted as eligible table-win routes for exact early access enumeration.`}
          </p>
        ) : (
          <p className="empty-inline">
            No reviewed table-lethal route was eligible for exact early access enumeration. Generic engine density is not used as a replacement.
          </p>
        )}
        {earlyEvaluation?.routes.length && omittedConversionRoutes > 0 ? (
          <p className="empty-inline">
            {omittedConversionRoutes} additional recognized route{omittedConversionRoutes === 1 ? "" : "s"} require{omittedConversionRoutes === 1 ? "s" : ""} a table-lethal conversion and {omittedConversionRoutes === 1 ? "is" : "are"} excluded from the eligible table-win count above.
          </p>
        ) : null}
        <p className="model-definition">
          These are exact combination-weighted card-access diagnostics, not win-attempt or win probabilities. The independent candidate-hand envelope is not the production mulligan keep rate: it does not model stop-on-keep decisions or London bottoming. Tutor payment, colored mana, ordered sequencing, priority, protection, and opponent responses remain blocked until a legal trajectory supplies a witness.
        </p>
      </section>
      <section className="panel-section">
        <div className="section-heading">
          <div>
            <span className="eyebrow">Route provenance</span>
            <h3>What actually produced or blocked a win attempt</h3>
          </div>
          <span className="coverage-inline">
            {explicitRoutes.length
              ? `${explicitRoutes.length} recognized explicit route${explicitRoutes.length === 1 ? "" : "s"}`
              : "No recognized explicit table-win route"}
          </span>
        </div>
        <div className="detail-grid">
          <Detail
            label="Baseline generic conversion milestone"
            value={distributionLabel(baselineGenericMilestone)}
          />
          <Detail
            label={`${profile} generic conversion milestone`}
            value={distributionLabel(interferedGenericMilestone)}
          />
          <Detail
            label="Early blocker horizon"
            value={attemptProvenance ? `Turns 1-${attemptProvenance.earlyFailureHorizon}` : "Not recorded"}
          />
        </div>
        {explicitRoutes.length ? (
          <div className="stress-table" aria-label="Recognized explicit win routes">
            {explicitRoutes.map((route) => {
              const baselineTurnOne = cumulativeRateAtTurn(
                route.cumulativeBaselineAttemptRate,
                1,
              );
              const baselineTurnTwo = cumulativeRateAtTurn(
                route.cumulativeBaselineAttemptRate,
                2,
              );
              const interferedTurnOne = cumulativeRateAtTurn(
                route.cumulativeInterferedAttemptRate,
                1,
              );
              const interferedTurnTwo = cumulativeRateAtTurn(
                route.cumulativeInterferedAttemptRate,
                2,
              );
              return (
                <div className="stress-row" key={route.routeId}>
                  <span className="severity-dot info" />
                  <strong>{route.name}</strong>
                  <span>
                    Cumulative attempts: T1 baseline {pct(baselineTurnOne)}
                    {" / "}selected profile {pct(interferedTurnOne)}
                    {" · "}T2 baseline {pct(baselineTurnTwo)}
                    {" / "}selected profile {pct(interferedTurnTwo)}
                    {" · "}By T{report.assumptions.maximumTurn} baseline {pct(route.baselineRate)}
                    {" / "}selected profile {pct(route.interferedRate)}
                    {" · "}{route.cards.join(" + ")}
                  </span>
                </div>
              );
            })}
          </div>
        ) : (
          <p className="empty-inline">
            Generic engine or combat density is still shown below as a development milestone, but it cannot populate the first-win-attempt curve.
          </p>
        )}
        {earlyBlockers.length ? (
          <>
            <h4 className="subsection-title">Most frequent early explicit-route blockers</h4>
            <div className="stress-table" aria-label="Early explicit-route blockers">
              {earlyBlockers.map((blocker, index) => (
                <div
                  className="stress-row"
                  key={`${blocker.turn}-${blocker.routeId ?? "none"}-${blocker.reason}-${blocker.blockedCard ?? "none"}-${index}`}
                >
                  <span className="severity-dot warning" />
                  <strong>T{blocker.turn} · {words(blocker.reason)}</strong>
                  <span>
                    {pct(blocker.rate)}
                    {blocker.routeName ? ` · ${blocker.routeName}` : ""}
                    {blocker.blockedCard ? ` · missing ${blocker.blockedCard}` : ""}
                  </span>
                </div>
              ))}
            </div>
          </>
        ) : null}
        <p className="model-definition">
          “Generic conversion milestone” is deliberately not a win attempt. It records broad engine/combat development that the older model mixed into timing, while explicit routes retain the reviewed line and blocker that produced each result.
        </p>
      </section>
      <CumulativeCurve
        eyebrow="No-interference baseline"
        title="Cumulative credible-threat rate by turn"
        points={report.winSpeed.cumulativeThreatRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow={`${profile} profile`}
        title="Cumulative credible-threat rate under the selected profile"
        points={report.winSpeed.cumulativeInterferedThreatRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow="Development diagnostic · not an attempt"
        title="Cumulative generic engine/combat milestone rate"
        points={report.winSpeed.cumulativeGenericConversionMilestoneRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow={`${profile} profile · development diagnostic · not an attempt`}
        title="Cumulative generic milestone rate under the selected profile"
        points={report.winSpeed.cumulativeInterferedGenericConversionMilestoneRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow="No-interference baseline"
        title="Cumulative first-win-attempt rate by turn"
        points={report.winSpeed.cumulativeWinAttemptRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow={`${profile} profile`}
        title="Cumulative first-win-attempt rate under the selected profile"
        points={report.winSpeed.cumulativeInterferedWinAttemptRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow="No-interference baseline"
        title="Cumulative resolved-table-win rate by turn"
        points={report.winSpeed.cumulativeResolvedTableWinRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      <CumulativeCurve
        eyebrow={`${profile} profile`}
        title="Cumulative resolved-table-win rate under the selected profile"
        points={report.winSpeed.cumulativeInterferedResolvedTableWinRate}
        maximumTurn={report.assumptions.maximumTurn}
      />
      {interactionScenarios.length > 0 ? (
        <section className="panel-section">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Isolated paired scenarios</span>
              <h3>Eight explicit disruption and recovery checkpoints</h3>
            </div>
            <span className="coverage-inline">
              {interactionScenarios[0].measurement.label}
            </span>
          </div>
          <div className="scenario-grid">
            {interactionScenarios.map((scenario) => (
              <ScenarioCard
                key={scenario.directive.scenarioId}
                scenario={scenario}
              />
            ))}
          </div>
          <p className="model-definition">
            These eight fixed counterfactual checkpoints are independent of the selected
            {" "}aggregate profile ({profile}).{" "}
            {interactionScenarios[0].measurement.claimBoundary}
          </p>
        </section>
      ) : (
        <section className="panel-section">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Legacy stress summaries</span>
              <h3>How the plan reacts to disruption</h3>
            </div>
          </div>
          <div className="stress-table">
            {stressTests.length ? stressTests.map((test) => (
              <div className="stress-row" key={test.name}>
                <span className={`severity-dot ${test.severity}`} />
                <strong>{test.name}</strong>
                <span>{test.outcome}</span>
              </div>
            )) : <p className="empty-inline">No stress-test observations were recorded for this report.</p>}
          </div>
        </section>
      )}
      <div className="explanation-card">
        <Info size={20} />
        <div><h4>Three timing endpoints, not multiplayer win percentages</h4><p>A credible threat is an answer-demanding state. A first win attempt requires either a recognized reviewed table-lethal line or a rules-backed combat assignment that would eliminate every remaining opponent if its damage connects. A resolved table win is recorded only after the typed line resolves or the assigned combat damage actually connects and produces a terminal game state; structural labels and generic engine or combat milestones cannot populate either endpoint. The aggregate comparison uses the selected profile: {profile}. The eight isolated checkpoints are separate fixed counterfactual diagnostics. Censored runs remain censored at the turn cap, and one endpoint is never substituted for another. These are local paired-model outcomes, not observed pod win rates or three fully simulated opponents.</p></div>
      </div>
    </div>
  );
}

function ScenarioCard({
  scenario,
}: {
  scenario: NonNullable<AnalysisReport["winSpeed"]["interactionScenarios"]>[number];
}) {
  const counters = scenario.counters;
  const applicability = scenario.applicability;
  const delay = scenario.firstWinAttemptDelay;
  const resolvedDelay = scenario.resolvedTableWinDelay;
  const recovery = scenario.recovery;
  const status = counters.undeterminedEpisodes === counters.totalEpisodes
    ? "Undetermined"
    : counters.notApplicableEpisodes === counters.totalEpisodes
      ? "Not applicable"
      : counters.opportunityEpisodes === 0
        ? "No checkpoint opportunity observed"
        : `${counters.effectfulInterventionEpisodes.toLocaleString()} effectful pairs`;
  const delayLabel = delay.observedDelayMedianTurns == null
    ? delay.rightCensoredPairs > 0
      ? `${delay.rightCensoredPairs.toLocaleString()} right-censored delays`
      : "No observed paired delay"
    : `${signedTurns(delay.observedDelayMedianTurns)} median first-attempt delay`;
  const resolvedDelayLabel = !resolvedDelay
    ? "Resolved-win delay not recorded"
    : resolvedDelay.observedDelayMedianTurns == null
      ? resolvedDelay.rightCensoredPairs > 0
        ? `${resolvedDelay.rightCensoredPairs.toLocaleString()} right-censored resolved-win delays`
        : "No observed paired resolved-win delay"
      : `${signedTurns(resolvedDelay.observedDelayMedianTurns)} median resolved-win delay`;
  const recoveryLabel = recovery.opportunities === 0
    ? "Recovery not sampled"
      : recovery.recoveredByTurnCapRate == null
      ? `${recovery.opportunities.toLocaleString()} recovery opportunities`
      : `${pct(recovery.recoveredByTurnCapRate)} recovered by turn cap`;
  const applicabilityDetail = counters.undeterminedEpisodes === counters.totalEpisodes
    ? applicability.primaryUndeterminedReason
      ?? "Execution coverage could not determine whether the checkpoint applies."
    : counters.notApplicableEpisodes === counters.totalEpisodes
      ? scenarioInapplicabilityReason(applicability.primaryNotApplicableReason)
      : applicability.primaryUndeterminedReason
        ? `Some episodes were undetermined: ${applicability.primaryUndeterminedReason}`
        : null;

  return (
    <article className="scenario-card">
      <div className="scenario-card-heading">
        <strong>{scenarioLabel(scenario.directive.scenario)}</strong>
        <span>{status}</span>
      </div>
      <div className="scenario-card-result">{delayLabel}</div>
      <div className="scenario-card-result">{resolvedDelayLabel}</div>
      {applicabilityDetail && (
        <p className="scenario-card-boundary">{applicabilityDetail}</p>
      )}
      <div className="scenario-card-meta">
        <span>{recoveryLabel}</span>
        <span>
          {counters.applicableEpisodes.toLocaleString()} applicable ·{" "}
          {counters.notApplicableEpisodes.toLocaleString()} not applicable ·{" "}
          {counters.undeterminedEpisodes.toLocaleString()} undetermined
        </span>
      </div>
    </article>
  );
}

function CumulativeCurve({
  eyebrow,
  title,
  points,
  maximumTurn,
}: {
  eyebrow: string;
  title: string;
  points?: { turn: number; rate: number }[] | null;
  maximumTurn: number;
}) {
  const safePoints = Array.isArray(points) ? points : [];
  const maxRate = Math.max(...safePoints.map((point) => point.rate), 0.01);
  const finalRate = safePoints.length ? safePoints[safePoints.length - 1].rate : 0;
  return (
    <section className="panel-section">
      <div className="section-heading">
        <div><span className="eyebrow">{eyebrow}</span><h3>{title}</h3></div>
        <span className="coverage-inline">
          {safePoints.length ? `${pct(finalRate)} by turn ${maximumTurn}` : "Endpoint not recorded"}
        </span>
      </div>
      {safePoints.length ? (
        <div className="turn-chart" role="img" aria-label={`${title}; ${pct(finalRate)} by turn ${maximumTurn}`}>
          {safePoints.map((point) => (
            <div className="turn-column" key={point.turn}>
              <div className="turn-value">{point.rate >= 0.12 ? pct(point.rate) : ""}</div>
              <div className="turn-bar-shell">
                <div className="turn-bar" style={{ height: `${(point.rate / maxRate) * 100}%` }} />
              </div>
              <span>T{point.turn}</span>
            </div>
          ))}
        </div>
      ) : (
        <p className="empty-inline">No turn-by-turn timing samples were recorded for this report.</p>
      )}
    </section>
  );
}

function Synergy({ report }: { report: AnalysisReport }) {
  const maxRole = Math.max(...report.synergy.roleCounts.map((role) => role.count), 1);
  const graph = report.synergy.graph;
  const strategic = report.synergy.strategicProfile;
  const comboCatalogNotes = report.coverage.notes.filter((note) =>
    /commander spellbook|spellbook/i.test(note),
  );
  return (
    <div className="report-page">
      <div className="stat-hero-grid">
        <StatHero label="Cohesion" value={`${report.synergy.cohesionScore}/100`} detail={`${report.synergy.detectedPlans.length} supported strategic plan${report.synergy.detectedPlans.length === 1 ? "" : "s"}`} icon={<Network />} />
        <StatHero label="Commander dependence" value={pct(report.synergy.commanderDependence)} detail="Estimated share of plan identity tied to the command zone" icon={<Target />} />
        <StatHero label="Known lines" value={String(report.synergy.knownLines.length)} detail={report.synergy.knownLines.length ? "Compact documented combinations detected" : "No documented compact line matched"} icon={<Braces />} />
      </div>
      {comboCatalogNotes.length > 0 && (
        <div className="combo-source-note" role="note">
          <BookOpenCheck size={16} />
          <div>
            <strong>Combination catalog coverage</strong>
            {comboCatalogNotes.map((note) => <p key={note}>{note}</p>)}
          </div>
        </div>
      )}
      {strategic && (
        <section className="panel-section">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Structural strategy profile</span>
              <h3>{words(strategic.primaryArchetype)} · {words(strategic.primaryPosture)}</h3>
            </div>
            <span className="coverage-inline">{pct(strategic.confidence)} evidence confidence</span>
          </div>
          <div className="combo-source-note" role="note">
            <Sparkles size={16} />
            <div>
              <strong>Decklist-derived description, not declared player intent</strong>
              <p>
                This profile ranks structural posture and combo families from card roles and
                documented lines. It is report-only and cannot change simulation or bracket scoring.
              </p>
            </div>
          </div>
          <div className="plan-grid">
            {strategic.archetypeRanking.slice(0, 3).map((entry) => (
              <article className="plan-card" key={entry.archetype}>
                <div className="plan-score"><Target size={17} /><strong>{pct(entry.score)}</strong></div>
                <h4>{words(entry.archetype)}</h4>
                <p>{entry.evidence}</p>
              </article>
            ))}
          </div>
          <div className="combo-list">
            {strategic.comboFamilyRanking.slice(0, 4).map((family) => (
              <article key={family.family}>
                <div className="combo-heading">
                  <strong>{words(family.family)}</strong>
                  <span>{pct(family.score)} structural match</span>
                </div>
                <p className="combo-prerequisites">{family.evidence}</p>
              </article>
            ))}
          </div>
          {(strategic.comboRouteClusters?.length ?? 0) > 0 && (
            <>
              <div className="section-heading">
                <div>
                  <span className="eyebrow">Documented route structure</span>
                  <h3>Primary package and backup paths</h3>
                </div>
              </div>
              <div className="combo-list">
                {strategic.comboRouteClusters?.slice(0, 6).map((route) => {
                  const core = route.centralCards
                    .filter((card) => card.appearsInEveryLine)
                    .map((card) => card.name)
                    .join(" + ");
                  const recurring = route.centralCards
                    .filter((card) => !card.appearsInEveryLine)
                    .map((card) => `${card.name} (${card.lineCount}/${route.lineCount})`)
                    .join(" · ");
                  const unique = route.uniqueCards.slice(0, 8).join(" · ");
                  return (
                    <article key={route.routeId}>
                      <div className="combo-heading">
                        <strong>{words(route.rank)} route · {route.lineCount} documented line{route.lineCount === 1 ? "" : "s"}</strong>
                        <span>{pct(route.score)} structural rank</span>
                      </div>
                      <p className="combo-cards">
                        {core
                          ? `Every line uses: ${core}`
                          : `Standalone line: ${route.lineNames[0] ?? route.routeId}`}
                      </p>
                      {recurring && <p className="combo-prerequisites">Recurring branches: {recurring}</p>}
                      {unique && <p className="combo-prerequisites">Branches: {unique}</p>}
                      <div className="combo-meta">
                        <span>{words(route.conversion)}</span>
                        <span>{pct(route.bestConfidence)} best line confidence</span>
                        {route.hasReportOnlyRequirements && (
                          <span>{route.reportOnlyLineCount} report-only line{route.reportOnlyLineCount === 1 ? "" : "s"}</span>
                        )}
                      </div>
                    </article>
                  );
                })}
              </div>
            </>
          )}
        </section>
      )}
      <section className="panel-section">
        <div className="section-heading"><div><span className="eyebrow">Detected plans</span><h3>What the deck is organized to do</h3></div></div>
        <div className="plan-grid">
          {report.synergy.detectedPlans.length ? report.synergy.detectedPlans.map((plan) => (
            <article className="plan-card" key={plan.name}>
              <div className="plan-score"><Sparkles size={17} /><strong>{pct(plan.confidence)}</strong></div>
              <h4>{plan.name}</h4>
              <p>{plan.supportingCards.slice(0, 5).join(" · ")}</p>
            </article>
          )) : <p className="empty-inline">No sufficiently dense strategy cluster was found in the currently modeled cards.</p>}
        </div>
      </section>
      {graph && (
        <section className="panel-section">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Typed relationship graph</span>
              <h3>Why cards support one another</h3>
            </div>
            <span className="coverage-inline">
              {pct(graph.graphCoverage)} of modeled nonlands connected
            </span>
          </div>
          <div className="combo-source-note" role="note">
            <Network size={16} />
            <div>
              <strong>{graph.modelVersion} · {graph.abilityModelVersion}</strong>
              <p>
                These are conservative, report-only Oracle-text relationships. They explain
                resource and trigger matches but are not yet executed as game actions.
              </p>
              <div className="graph-summary">
                <span>{graph.edgeCount} explicit link{graph.edgeCount === 1 ? "" : "s"}</span>
                <span>{graph.resources.length} matched resource type{graph.resources.length === 1 ? "" : "s"}</span>
                <span>{graph.unsupportedClauseCount} unsupported clause{graph.unsupportedClauseCount === 1 ? "" : "s"} retained</span>
              </div>
            </div>
          </div>
          {graph.resources.length > 0 && (
            <div className="graph-resources" aria-label="Matched synergy resources">
              {graph.resources.slice(0, 10).map((resource) => (
                <span key={resource.resource}>
                  {resource.resource}
                  <small>{resource.producerCount} source{resource.producerCount === 1 ? "" : "s"} · {resource.consumerCount} payoff{resource.consumerCount === 1 ? "" : "s"}</small>
                </span>
              ))}
            </div>
          )}
          <div className="combo-list graph-link-list">
            {graph.links.length > 0 ? graph.links.slice(0, 12).map((link, index) => (
              <article key={`${link.sourceCard}-${link.targetCard}-${link.resource}-${index}`}>
                <div className="combo-heading">
                  <strong>{link.sourceCard} → {link.targetCard}</strong>
                  <span>{words(link.relation)}</span>
                </div>
                <p className="combo-cards">{link.resource}</p>
                <div className="combo-meta">
                  <span>{pct(link.confidence)} parser confidence</span>
                  {graph.commanderLinks.some((candidate) =>
                    candidate.sourceCard === link.sourceCard
                    && candidate.targetCard === link.targetCard
                    && candidate.resource === link.resource,
                  ) ? <span>Commander link</span> : null}
                </div>
                <p className="combo-prerequisites">{link.evidence}</p>
              </article>
            )) : (
              <p className="empty-inline">
                No explicit producer-to-payoff relationship cleared the current conservative parser.
              </p>
            )}
          </div>
        </section>
      )}
      {report.synergy.knownLines.length > 0 && (
        <section className="panel-section">
          <div className="section-heading"><div><span className="eyebrow">Known combinations</span><h3>Compact lines found in the list</h3></div></div>
          <div className="combo-list">
            {report.synergy.knownLines.map((line) => (
              <article key={`${line.name}-${line.cards.join()}`}>
                <div className="combo-heading">
                  <strong>{line.name}</strong>
                  <span className={line.tableLethalIfResolved ? "table-lethal" : "needs-payoff"}>
                    {line.tableLethalIfResolved ? "Table-lethal line" : words(line.outcome)}
                  </span>
                </div>
                <p className="combo-cards">{line.cards.join(" + ")}</p>
                <div className="combo-meta">
                  <span>{line.isInfinite ? "Unbounded / repeatable" : "Finite sequence"}</span>
                  {line.manaNeeded ? <span>Needs {line.manaNeeded}</span> : null}
                  <span>{pct(line.modelConfidence)} confidence</span>
                </div>
                {line.prerequisites.length ? <p className="combo-prerequisites">{line.prerequisites.join(" ")}</p> : null}
              </article>
            ))}
          </div>
        </section>
      )}
      <section className="panel-section">
        <div className="section-heading"><div><span className="eyebrow">Functional composition</span><h3>Modeled role density</h3></div><span className="coverage-inline">Modal cards can count twice</span></div>
        <div className="role-grid">
          {report.synergy.roleCounts.map((item) => (
            <div className="role-row" key={item.role}>
              <span>{item.role}</span>
              <div className="bar-track"><div className="bar-fill" style={{ width: `${(item.count / maxRole) * 100}%` }} /></div>
              <strong>{item.count}</strong>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

function Method({ report }: { report: AnalysisReport }) {
  const coverage = [
    ["Card identity resolution", report.coverage.identityResolution],
    ["Semantic role coverage", report.coverage.semanticCoverage],
    ["Simulation coverage", report.coverage.simulationCoverage],
  ] as const;
  const executionManifest = report.coverage.executionManifest;
  const ratingGate = executionManifest?.gates.find((gate) => gate.metric === "bracketRating");
  const timingGate = executionManifest?.gates.find((gate) => gate.metric === "goldfishTiming");
  const blockingCards = Array.from(
    new Map(
      (ratingGate?.blockers ?? []).map((blocker) => [
        blocker.cardId,
        `${blocker.cardName}: ${blocker.blocker.detail}`,
      ]),
    ).values(),
  );
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
  return (
    <div className="report-page">
      <section className="panel-section">
        <div className="section-heading"><div><span className="eyebrow">Coverage</span><h3>How much of this result is directly supported?</h3></div></div>
        <div className="coverage-cards">
          {coverage.map(([label, value]) => (
            <div className="coverage-card" key={label}>
              <div><span>{label}</span><strong>{pct(value)}</strong></div>
              <div className="bar-track large"><div className={`bar-fill ${value >= 0.8 ? "primary" : "warning"}`} style={{ width: `${value * 100}%` }} /></div>
            </div>
          ))}
        </div>
      </section>
      {executionManifest && (
        <section className="panel-section">
          <div className="section-heading">
            <div>
              <span className="eyebrow">Fail-closed execution gate</span>
              <h3>Every Oracle span must have an executable disposition</h3>
            </div>
            <span className={`coverage-inline execution-state ${ratingGate?.state ?? "blocked"}`}>
              Rating {words(ratingGate?.state ?? "blocked")}
            </span>
          </div>
          <div className="detail-grid">
            <Detail label="Manifest cards / faces" value={`${executionManifest.summary.cardCount} / ${executionManifest.summary.faceCount}`} />
            <Detail label="Oracle spans / coverage leaves" value={`${executionManifest.summary.oracleSpanCount} / ${executionManifest.summary.leafCount}`} />
            <Detail label="Goldfish timing gate" value={words(timingGate?.state ?? "blocked")} />
            <Detail label="Rating blockers" value={String(ratingGate?.blockingLeafCount ?? 0)} />
            <Detail label="Coverage compiler" value={executionManifest.compilerVersion} mono />
            <Detail label="Full preflight manifest SHA-256" value={executionManifest.fingerprintSha256} mono />
            <Detail label="Compact projection SHA-256" value={executionManifest.projectionSha256} mono />
          </div>
          {blockingCards.length > 0 && (
            <div className="execution-blocker-list">
              <strong>Deterministic blocker sample</strong>
              <ul>
                {blockingCards.slice(0, 12).map((blocker) => <li key={blocker}>{blocker}</li>)}
              </ul>
              {ratingGate?.blockerSampleTruncated ? (
                <p>
                  Showing {ratingGate.blockers.length} sampled leaves from{" "}
                  {ratingGate.blockingLeafCount} total blockers; the compact
                  report sample is capped at {ratingGate.blockerSampleLimit}.
                </p>
              ) : (
                <p>Every blocking leaf is included in this compact report.</p>
              )}
            </div>
          )}
        </section>
      )}
      {(report.coverage.unresolvedCards.length > 0 || report.coverage.approximatedCards.length > 0) && (
        <section className="panel-section split-section">
          <div>
            <span className="eyebrow">Unresolved cards</span>
            <h3>{report.coverage.unresolvedCards.length || "None"}</h3>
            <p>{report.coverage.unresolvedCards.slice(0, 12).join(" · ") || "Every printed name resolved to local card data."}</p>
          </div>
          <div>
            <span className="eyebrow">Higher approximation</span>
            <h3>{report.coverage.approximatedCards.length || "None"}</h3>
            <p>{report.coverage.approximatedCards.slice(0, 12).join(" · ") || "No high-impact approximation was flagged."}</p>
          </div>
        </section>
      )}
      <section className="panel-section">
        <div className="section-heading">
          <div><span className="eyebrow">Commander policy</span><h3>Legality and official bracket floor</h3></div>
          <span className={`coverage-inline policy-state ${report.policy.legality}`}>{capitalize(report.policy.legality)}</span>
        </div>
        <div className="detail-grid">
          <Detail label="Policy floor" value={report.policy.policyFloor ? `Bracket ${report.policy.policyFloor}` : "No deterministic floor"} />
          <Detail label="Game Changers" value={String(report.policy.gameChangerCount)} />
          <Detail label="Rules package" value={report.policy.packageVersion} mono />
          <Detail label="Effective date" value={report.policy.effectiveDate} />
        </div>
        <p className="policy-floor-reason">{report.policy.policyFloorReason}</p>
        {report.policy.gameChangers.length > 0 && (
          <p className="policy-card-list"><strong>Game Changers:</strong> {report.policy.gameChangers.join(" · ")}</p>
        )}
        {report.policy.bracketSignals.length > 0 && (
          <div className="policy-signal-list" aria-label="Bracket policy signals">
            {report.policy.bracketSignals.map((signal) => (
              <article className={`policy-signal ${signal.kind}`} key={`${signal.code}-${signal.title}`}>
                <div>
                  <strong>{signal.title}</strong>
                  <span>{policySignalKind(signal.kind)}{signal.recommendedFloor ? ` · Bracket ${signal.recommendedFloor}+ guidance` : ""}</span>
                </div>
                <p>{signal.detail}</p>
              </article>
            ))}
          </div>
        )}
        {policyIssues.length > 0 && (
          <div className="policy-review-list">
            {policyIssues.slice(0, 12).map((issue) => (
              <div key={issue}><Info size={14} /><span>{issue}</span></div>
            ))}
          </div>
        )}
      </section>
      <section className="panel-section">
        <div className="section-heading"><div><span className="eyebrow">Reproducibility</span><h3>Run assumptions and versions</h3></div></div>
        <div className="detail-grid">
          <Detail label="Opening hands" value={report.assumptions.openingHandSimulations.toLocaleString()} />
          <Detail label="Paired games" value={report.assumptions.gameSimulations.toLocaleString()} />
          <Detail label="Analyzer policy" value="Fixed competitive · aggressive route search" />
          <Detail label="Primary timing horizon" value={`Turns 1-${report.assumptions.maximumTurn}`} />
          <Detail
            label="Selected interaction profile"
            value={interactionProfileLabel(report.assumptions.interactionProfile)}
          />
          <Detail label="Isolated scenario suite" value="Eight independent fixed checkpoints" />
          <Detail label="Player-declared intent" value="Not used for evaluation" />
          <Detail label="Online card resolution" value={report.assumptions.allowOnlineCardResolution ? "Enabled for missing names" : "Disabled · local only"} />
          <Detail
            label="Random seed"
            value={report.assumptions.seedExact ?? String(report.assumptions.seed)}
            mono
          />
          <Detail label="Deck SHA-256" value={report.deck.canonicalDeckSha256} mono />
          {report.openingHands.candidateCohortSha256 && (
            <Detail
              label="Opening cohort SHA-256"
              value={report.openingHands.candidateCohortSha256}
              mono
            />
          )}
          <Detail label="Card data" value={report.versions.cardData} />
          {report.versions.cardSnapshotSha256 && <Detail label="Card snapshot SHA-256" value={report.versions.cardSnapshotSha256} mono />}
          <Detail label="Commander policy package" value={report.versions.rulesPackage} mono />
          {report.versions.rulesPackageOrigin && <Detail label="Commander policy origin" value={words(report.versions.rulesPackageOrigin)} />}
          {report.versions.rulesSnapshotSha256 && <Detail label="Commander policy SHA-256" value={report.versions.rulesSnapshotSha256} mono />}
          {report.versions.comprehensiveRulesEffectiveDate && <Detail label="Comprehensive Rules effective" value={report.versions.comprehensiveRulesEffectiveDate} mono />}
          {report.versions.comprehensiveRulesSnapshotSha256 && <Detail label="Comprehensive Rules SHA-256" value={report.versions.comprehensiveRulesSnapshotSha256} mono />}
          {report.versions.comprehensiveRulesParserVersion && <Detail label="Rules parser" value={report.versions.comprehensiveRulesParserVersion} mono />}
          {report.versions.ruleCapabilityModel && <Detail label="Rule capability model" value={report.versions.ruleCapabilityModel} mono />}
          <Detail label="Semantic compiler" value={report.versions.semanticModel} mono />
          {report.versions.semanticPackage && <Detail label="Semantic package snapshot" value={declaredSemanticPackage(report.versions.semanticPackage)} mono />}
          {report.versions.semanticPackageOrigin && <Detail label="Semantic package origin" value={words(report.versions.semanticPackageOrigin)} />}
          {report.versions.semanticSnapshotSha256 && <Detail label="Semantic snapshot SHA-256" value={report.versions.semanticSnapshotSha256} mono />}
          {report.versions.semanticImportedAt && <Detail label="Semantic package imported" value={formatTimestamp(report.versions.semanticImportedAt)} />}
          {report.versions.semanticAuthenticityBasis && <Detail label="Semantic package provenance" value={report.versions.semanticAuthenticityBasis} />}
          <Detail label="Simulation engine" value={report.versions.simulationEngine} mono />
          {report.versions.effectiveHandStrengthModel && (
            <Detail
              label="Opening-hand strength model"
              value={report.versions.effectiveHandStrengthModel}
              mono
            />
          )}
          <Detail
            label="Timing endpoint contract"
            value={report.winSpeed.timingEndpointVersion ?? "Legacy · not recorded"}
            mono
          />
          {report.versions.abilityProgram && <Detail label="Executable ability program" value={report.versions.abilityProgram} mono />}
          {report.versions.turnPlanner && <Detail label="Turn planner" value={report.versions.turnPlanner} mono />}
          {report.versions.strictEngine && <Detail label="Strict execution kernel" value={report.versions.strictEngine} mono />}
          {report.versions.executionCoverageCompiler && <Detail label="Execution coverage compiler" value={report.versions.executionCoverageCompiler} mono />}
          <Detail label="Bracket model" value={report.versions.bracketModel} mono />
          {report.versions.comboCatalog && <Detail label="Combo catalog" value={report.versions.comboCatalog} />}
          {report.versions.comboSnapshotSha256 && <Detail label="Combo SHA-256" value={report.versions.comboSnapshotSha256} mono />}
        </div>
      </section>
      <section className="notes-panel">
        <BookOpenCheck size={21} />
        <div><h3>Interpretation notes</h3><ul>{report.coverage.notes.map((note) => <li key={note}>{note}</li>)}</ul></div>
      </section>
    </div>
  );
}

function MiniRing({ value }: { value: number }) {
  return (
    <div className="mini-ring" style={{ "--score": value } as React.CSSProperties}>
      <span>{value}</span>
    </div>
  );
}

function StatHero({ label, value, detail, icon }: { label: string; value: string; detail: string; icon: React.ReactNode }) {
  return <article className="stat-hero"><div className="metric-icon">{icon}</div><span>{label}</span><strong>{value}</strong><p>{detail}</p></article>;
}

function Detail({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return <div className="detail-item"><span>{label}</span><strong className={mono ? "mono" : ""}>{value}</strong></div>;
}

const pct = (value: number) => `${Math.round(value * 100)}%`;
const cumulativeRateAtTurn = (
  curve: { turn: number; rate: number }[],
  turn: number,
) => curve.find((point) => point.turn === turn)?.rate ?? 0;
const capitalize = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);
const words = (value: string) => capitalize(value.replace(/[-_]/g, " ").replace(/([A-Z])/g, " $1"));
const policySignalKind = (kind: AnalysisReport["policy"]["bracketSignals"][number]["kind"]) => {
  switch (kind) {
    case "deterministicFloor": return "Deterministic package rule";
    case "modeledGuidance": return "Modeled bracket guidance";
    default: return "Manual review";
  }
};
const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
const turnValue = (turn?: number | null) =>
  isFiniteNumber(turn) ? `Turn ${turn.toFixed(1)}` : "Median not reached";
const endpointTurnValue = (distribution?: TurnDistribution | null) =>
  distribution ? turnValue(distribution.median) : "Not recorded";
const declaredSemanticPackage = (value: string) => value
  .replace(" · effective ", " · declared effective ")
  .replace(" · verified ", " · declared verified ");
const formatTimestamp = (value: string) => {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
};
const attemptTurnLabel = (distribution?: TurnDistribution | null) =>
  isFiniteNumber(distribution?.median)
    ? `Turn ${distribution.median.toFixed(1)} population median first win attempt`
    : `Population median first win attempt not reached by cap${isFiniteNumber(distribution?.conditionalMedian)
      ? ` · successful-run median T${distribution.conditionalMedian.toFixed(1)}`
      : ""
    } · ${pct(distribution?.demonstratedRate ?? 0)} demonstrated by cap`;
type SpeedScoreBasis =
  | "recognizedWinAttempt"
  | "genericConversionMilestone"
  | "proactiveDevelopment"
  | "credibleThreat"
  | "structuralPace";
const speedBasisTurnLabel = (
  distribution: TurnDistribution | null | undefined,
  endpoint: string,
) => isFiniteNumber(distribution?.median)
  ? `Turn ${distribution.median.toFixed(1)} population median ${endpoint}`
  : `Population median ${endpoint} not reached by cap${isFiniteNumber(distribution?.conditionalMedian)
    ? ` · successful-run median T${distribution.conditionalMedian.toFixed(1)}`
    : ""
  } · ${pct(distribution?.demonstratedRate ?? 0)} demonstrated by cap`;
const speedScoreDetail = (report: AnalysisReport) => {
  const basis = (report.overview as AnalysisReport["overview"] & {
    speedScoreBasis?: SpeedScoreBasis;
  }).speedScoreBasis;
  const modelPace = (report.winSpeed as AnalysisReport["winSpeed"] & {
    baselineModelPace?: TurnDistribution;
  }).baselineModelPace;
  switch (basis) {
    case "recognizedWinAttempt":
      return attemptTurnLabel(report.winSpeed.baselineWinAttempt);
    case "genericConversionMilestone":
      return speedBasisTurnLabel(
        report.winSpeed.baselineGenericConversionMilestone,
        "generic conversion milestone",
      );
    case "proactiveDevelopment":
      return isFiniteNumber(modelPace?.median)
        ? `Proactive development · population median T${modelPace.median.toFixed(1)} · not a win attempt or probability`
        : `Proactive development · population median not reached by cap${isFiniteNumber(modelPace?.conditionalMedian)
          ? ` · successful-run median T${modelPace.conditionalMedian.toFixed(1)}`
          : ""
        } · ${pct(modelPace?.demonstratedRate ?? 0)} demonstrated by cap · not a win attempt or probability`;
    case "credibleThreat":
      return speedBasisTurnLabel(report.winSpeed.baseline, "credible threat");
    case "structuralPace":
      return `Structural setup proxy · no modeled milestone by T${report.assumptions.maximumTurn}`;
    default:
      return attemptTurnLabel(report.winSpeed.baselineWinAttempt);
  }
};
const medianDelayLabel = (delay?: number | null) =>
  isFiniteNumber(delay)
    ? `${delay.toFixed(1)} turn median first-attempt delay`
    : "Median first-attempt delay not available";
const resolvedDelayLabel = (
  delay: number | null | undefined,
  endpointRecorded: boolean,
) => {
  if (!endpointRecorded) return "Legacy endpoint not recorded";
  return isFiniteNumber(delay)
    ? `${delay.toFixed(1)} turn median resolved-win delay`
    : "Median resolved-win delay not available";
};
const distributionLabel = (distribution?: TurnDistribution | null) => {
  if (!distribution) return "Legacy endpoint not recorded · reanalyze";
  const range = distributionQuantileLabel(distribution);
  if (!isFiniteNumber(distribution.median)) {
    const successfulOnly = isFiniteNumber(distribution.conditionalMedian)
      ? ` · successful-run median T${distribution.conditionalMedian.toFixed(1)}`
      : "";
    return `Population median not reached by turn cap${range}${successfulOnly}`;
  }
  return `Population median T${distribution.median.toFixed(1)}${range}`;
};
const distributionQuantileLabel = (distribution?: TurnDistribution | null) => {
  if (!distribution) return "";
  if (isFiniteNumber(distribution.p10) && isFiniteNumber(distribution.p90)) {
    return ` · P10-P90 ${distribution.p10.toFixed(1)}-${distribution.p90.toFixed(1)}`;
  }
  if (isFiniteNumber(distribution.p10)) {
    return ` · P10 T${distribution.p10.toFixed(1)} · P90 not reached by cap`;
  }
  if (isFiniteNumber(distribution.p90)) {
    return ` · P10 not reached by cap · P90 T${distribution.p90.toFixed(1)}`;
  }
  return "";
};
const signedTurns = (value: number) =>
  Number.isFinite(value) ? `${value >= 0 ? "+" : ""}${value.toFixed(1)} turns` : "Unknown";
const scenarioInapplicabilityReason = (
  reason?: CompactInteractionScenarioReport["applicability"]["primaryNotApplicableReason"],
) => {
  switch (reason) {
    case "noEligibleNoncommanderPermanent":
      return "No eligible strategic noncommander permanent exists in this plan.";
    case "noCommanderSubject":
      return "No commander subject can be established by the bounded plan.";
    case "noRelevantSpellClass":
      return "No relevant spell class exists for this checkpoint.";
    case "noRelevantCreatureBoardPlan":
      return "No relevant multi-creature board plan exists.";
    case "noGraveyardDependency":
      return "No executable graveyard-dependent action exists.";
    case "noTaxableActionClass":
      return "No taxable action class exists.";
    case "noMultispellPlan":
      return "No executable multi-spell turn plan exists.";
    case "noRepresentableWinAttempt":
      return "No representable first win attempt exists.";
    default:
      return "The scenario is structurally not applicable to this bounded plan.";
  }
};
const scenarioLabel = (
  scenario: NonNullable<AnalysisReport["winSpeed"]["interactionScenarios"]>[number]["directive"]["scenario"],
) => {
  switch (scenario) {
    case "targetedPermanentRemoval":
      return "Targeted permanent removal";
    case "commanderRemovalRecast":
      return "Commander removal and recast";
    case "firstRelevantSpellCountered":
      return "First relevant spell countered";
    case "creatureWipe":
      return "Creature wipe";
    case "graveyardShutdown":
      return "Graveyard shutdown";
    case "genericTaxStax":
      return "Generic tax / stax";
    case "ruleOfLawCap":
      return "Rule of Law cap";
    case "firstWinAttemptStopped":
      return "First win attempt stopped";
  }
};

function downloadBrowserFile(filename: string, contents: string) {
  const url = URL.createObjectURL(new Blob([contents], { type: "text/markdown;charset=utf-8" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

async function copyText(contents: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(contents);
      return;
    } catch {
      // The native webview may deny the modern API; keep a local-only fallback.
    }
  }
  const textarea = document.createElement("textarea");
  textarea.value = contents;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("Clipboard unavailable");
}
