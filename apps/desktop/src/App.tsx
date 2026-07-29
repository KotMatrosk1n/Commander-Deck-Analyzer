import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BarChart3,
  BookOpenCheck,
  Braces,
  Check,
  CheckCircle2,
  ChevronRight,
  Circle,
  ClipboardPaste,
  CloudDownload,
  Database,
  FileText,
  Gauge,
  Globe2,
  Import,
  Info,
  Layers3,
  LoaderCircle,
  Menu,
  Play,
  RefreshCw,
  RotateCcw,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Square,
  Target,
  WifiOff,
  X,
} from "lucide-react";
import {
  analyzeDeck,
  cancelAnalysis,
  checkForKnowledgeUpdates,
  getComboDataStatus,
  getComprehensiveRulesStatus,
  getDataStatus,
  getPolicyPackageStatus,
  getSemanticPackageStatus,
  importDeckUrl,
  importPolicyPackage,
  importSemanticPackage,
  parseDeck,
  readDeckFile,
  resetPolicyPackage,
  resetSemanticPackage,
  updateCardDatabase,
  updateComboDatabase,
  updateComprehensiveRules,
} from "./api";
import { CreditsPanel } from "./CreditsPanel";
import { ReportView } from "./ReportView";
import type {
  AnalysisOptions,
  AnalysisProgress,
  AnalysisReport,
  AnalysisStage,
  AnalysisTrialCount,
  ComboStoreStatus,
  ComboUpdateProgress,
  ComprehensiveRulesStatus,
  ComprehensiveRulesUpdateProgress,
  DataStatus,
  DataUpdateProgress,
  DeckParseResult,
  ImportResult,
  KnowledgeUpdateCheck,
  PolicyPackageStatus,
  ReportTab,
  SemanticPackageStatus,
} from "./types";
import "./App.css";

const EMPTY_PARSE: DeckParseResult = {
  entries: [],
  cardCount: 0,
  uniqueCardCount: 0,
  ignoredLineCount: 0,
  commanders: [],
  issues: [],
  canonicalText: "",
  isCommanderSized: false,
};

const DEFAULT_OPTIONS: AnalysisOptions = {
  openingHandSimulations: 1000,
  gameSimulations: 1000,
  maximumTurn: 6,
  mulliganPolicy: "aggressive",
  pilotPolicy: "race",
  interactionProfile: "highPower",
  declaredIntent: "unspecified",
  allowOnlineCardResolution: false,
};
const ANALYSIS_TRIAL_OPTIONS = [1000, 5000, 10000] as const;
const MINIMUM_ANALYSIS_TURN = 2;
const MAXIMUM_ANALYSIS_TURN = 12;

interface AnalysisRunSettings {
  trials: AnalysisTrialCount;
  maximumTurn: number;
}

const DIALOG_FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

const stageOrder: { id: AnalysisStage; label: string; description: string }[] = [
  { id: "validating", label: "Validate decklist", description: "Quantities, sections, commanders" },
  { id: "resolvingCards", label: "Resolve card identities", description: "Local database, then missing cards" },
  { id: "compiling", label: "Compile semantic model", description: "Roles, plans, known combinations" },
  { id: "openingHands", label: "Simulate opening hands", description: "Fixed aggressive London mulligans" },
  { id: "goldfish", label: "Simulate baseline plans", description: "Bounded no-interference trajectories" },
  { id: "interference", label: "Apply table interaction", description: "Standardized cEDH disruption and recovery" },
  { id: "scoring", label: "Build recommendation", description: "Evidence, uncertainty, coverage" },
];

type AppPhase = "empty" | "ready" | "importing" | "analyzing" | "complete" | "error";

function App() {
  const [deckText, setDeckText] = useState("");
  const [parseResult, setParseResult] = useState<DeckParseResult>(EMPTY_PARSE);
  const [parsedDeckText, setParsedDeckText] = useState("");
  const [commanderInput, setCommanderInput] = useState("");
  const [deckUrl, setDeckUrl] = useState("");
  const [deckName, setDeckName] = useState("");
  const [source, setSource] = useState<ImportResult | null>(null);
  const [sourceDirty, setSourceDirty] = useState(false);
  const [phase, setPhase] = useState<AppPhase>("empty");
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [lastAnalyzedFingerprint, setLastAnalyzedFingerprint] = useState("");
  const [activeTab, setActiveTab] = useState<ReportTab>("overview");
  const [progress, setProgress] = useState<AnalysisProgress | null>(null);
  const [stageProgress, setStageProgress] = useState<Partial<Record<AnalysisStage, AnalysisProgress>>>({});
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dataStatus, setDataStatus] = useState<DataStatus | null>(null);
  const [privacyOpen, setPrivacyOpen] = useState(false);
  const [dataOpen, setDataOpen] = useState(false);
  const [creditsOpen, setCreditsOpen] = useState(false);
  const [analysisSetupOpen, setAnalysisSetupOpen] = useState(false);
  const [analysisSettings, setAnalysisSettings] = useState<AnalysisRunSettings>({
    trials: DEFAULT_OPTIONS.gameSimulations,
    maximumTurn: DEFAULT_OPTIONS.maximumTurn,
  });
  const [allowOnlineCardResolution, setAllowOnlineCardResolution] = useState(false);
  const options = useMemo<AnalysisOptions>(
    () => ({
      ...DEFAULT_OPTIONS,
      openingHandSimulations: analysisSettings.trials,
      gameSimulations: analysisSettings.trials,
      maximumTurn: analysisSettings.maximumTurn,
      allowOnlineCardResolution,
    }),
    [allowOnlineCardResolution, analysisSettings],
  );
  const [checkingKnowledgeUpdates, setCheckingKnowledgeUpdates] = useState(false);
  const [knowledgeUpdateCheck, setKnowledgeUpdateCheck] =
    useState<KnowledgeUpdateCheck | null>(null);
  const [knowledgeUpdateNotice, setKnowledgeUpdateNotice] = useState<string | null>(null);
  const [knowledgeUpdateError, setKnowledgeUpdateError] = useState<string | null>(null);
  const [updateConfirmationOpen, setUpdateConfirmationOpen] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<DataUpdateProgress | null>(null);
  const [updatingData, setUpdatingData] = useState(false);
  const [comboStatus, setComboStatus] = useState<ComboStoreStatus | null>(null);
  const [comboUpdateProgress, setComboUpdateProgress] = useState<ComboUpdateProgress | null>(null);
  const [updatingCombos, setUpdatingCombos] = useState(false);
  const [comboError, setComboError] = useState<string | null>(null);
  const [comboNotice, setComboNotice] = useState<string | null>(null);
  const [comprehensiveRulesStatus, setComprehensiveRulesStatus] =
    useState<ComprehensiveRulesStatus | null>(null);
  const [comprehensiveRulesProgress, setComprehensiveRulesProgress] =
    useState<ComprehensiveRulesUpdateProgress | null>(null);
  const [updatingComprehensiveRules, setUpdatingComprehensiveRules] = useState(false);
  const [comprehensiveRulesError, setComprehensiveRulesError] = useState<string | null>(null);
  const [comprehensiveRulesNotice, setComprehensiveRulesNotice] = useState<string | null>(null);
  const [policyStatus, setPolicyStatus] = useState<PolicyPackageStatus | null>(null);
  const [importingPolicy, setImportingPolicy] = useState(false);
  const [resettingPolicy, setResettingPolicy] = useState(false);
  const [policyError, setPolicyError] = useState<string | null>(null);
  const [policyNotice, setPolicyNotice] = useState<string | null>(null);
  const [semanticStatus, setSemanticStatus] = useState<SemanticPackageStatus | null>(null);
  const [importingSemantics, setImportingSemantics] = useState(false);
  const [resettingSemantics, setResettingSemantics] = useState(false);
  const [semanticError, setSemanticError] = useState<string | null>(null);
  const [semanticNotice, setSemanticNotice] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const activeRunIdRef = useRef<string | null>(null);
  const latestRunIdRef = useRef<string | null>(null);

  const commanders = useMemo(
    () =>
      commanderInput
        .split(/\r?\n|;/)
        .map((name) => name.trim())
        .filter(Boolean),
    [commanderInput],
  );
  const visibleParseIssues = useMemo(
    () => parseResult.issues.filter(
      (issue) => issue.code !== "commander-missing" || commanders.length === 0,
    ),
    [commanders.length, parseResult.issues],
  );
  const analysisInputFingerprint = useMemo(
    () => JSON.stringify({ deckText, commanders, options }),
    [commanders, deckText, options],
  );
  const reportIsStale = Boolean(
    report && analysisInputFingerprint !== lastAnalyzedFingerprint,
  );
  const canAnalyze =
    parsedDeckText === deckText &&
    parseResult.isCommanderSized &&
    commanders.length > 0 &&
    phase !== "analyzing" &&
    phase !== "importing";
  const openAnalysisSetup = useCallback(() => {
    if (canAnalyze) setAnalysisSetupOpen(true);
  }, [canAnalyze]);

  useEffect(() => {
    getDataStatus().then(setDataStatus).catch(() => {
      setDataStatus({
        state: "offline",
        cardCount: 0,
        source: "Local card data",
        message: "Card data status is unavailable.",
        schemaVersion: "unavailable",
      });
    });
    getComboDataStatus().then(setComboStatus).catch((reason) => {
      setComboError(`Combo catalog status unavailable: ${readError(reason)}`);
    });
    getComprehensiveRulesStatus().then(setComprehensiveRulesStatus).catch((reason) => {
      setComprehensiveRulesError(`Comprehensive Rules status unavailable: ${readError(reason)}`);
    });
    getPolicyPackageStatus().then(setPolicyStatus).catch((reason) => {
      setPolicyError(`Policy package status unavailable: ${readError(reason)}`);
    });
    getSemanticPackageStatus().then(setSemanticStatus).catch((reason) => {
      setSemanticError(`Semantic package status unavailable: ${readError(reason)}`);
    });
  }, []);

  useEffect(() => {
    let cancelled = false;
    const submittedDeckText = deckText;
    const handle = window.setTimeout(() => {
      parseDeck(submittedDeckText)
        .then((parsed) => {
          if (cancelled) return;
          setParseResult(parsed);
          setParsedDeckText(submittedDeckText);
          if (!commanderInput.trim() && parsed.commanders.length) {
            setCommanderInput(parsed.commanders.join("\n"));
          }
          if (phase !== "analyzing" && phase !== "importing" && phase !== "complete") {
            setPhase(submittedDeckText.trim() ? "ready" : "empty");
          }
        })
        .catch((reason) => {
          if (!cancelled) setError(readError(reason));
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [deckText, commanderInput, phase]);

  const openDeckFile = useCallback(async () => {
    setError(null);
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: "Decklists", extensions: ["txt", "dec", "dek", "csv"] },
          { name: "All files", extensions: ["*"] },
        ],
      });
      if (!path) return;
      const text = await readDeckFile(path);
      setSource(null);
      setDeckName(fileStem(path));
      setSourceDirty(false);
      setDeckText(text);
      setPhase("ready");
      requestAnimationFrame(() => textareaRef.current?.focus());
    } catch (reason) {
      setError(readError(reason));
      setPhase("error");
    }
  }, []);

  const pasteDeck = useCallback(async () => {
    setError(null);
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) throw new Error("The clipboard does not contain a decklist.");
      setSource(null);
      setDeckName("");
      setSourceDirty(false);
      setDeckText(text);
      setPhase("ready");
    } catch (reason) {
      setError(readError(reason));
    }
  }, []);

  const importFromUrl = useCallback(async () => {
    if (!deckUrl.trim()) {
      setError("Enter a public deck URL first.");
      return;
    }
    setError(null);
    setPhase("importing");
    try {
      const imported = await importDeckUrl(deckUrl);
      setSource(imported);
      setDeckName(imported.deckName ?? "");
      setDeckText(imported.deckText);
      setCommanderInput(imported.commanders.join("\n"));
      setSourceDirty(false);
      setPhase("ready");
      if (imported.warnings.length) {
        setError(imported.warnings.join(" "));
      }
    } catch (reason) {
      setError(readError(reason));
      setPhase(deckText ? "ready" : "error");
    }
  }, [deckText, deckUrl]);

  const runAnalysis = useCallback(async (submittedOptions: AnalysisOptions) => {
    if (!canAnalyze) return;
    const runId = crypto.randomUUID();
    const submittedFingerprint = JSON.stringify({
      deckText,
      commanders,
      options: submittedOptions,
    });
    activeRunIdRef.current = runId;
    latestRunIdRef.current = runId;
    setActiveRunId(runId);
    setError(null);
    setProgress(null);
    setStageProgress({});
    setPhase("analyzing");
    setActiveTab("overview");
    try {
      const result = await analyzeDeck(
        {
          runId,
          deckText,
          commanderNames: commanders,
          options: submittedOptions,
        },
        (snapshot) => {
          if (
            snapshot.runId !== runId ||
            activeRunIdRef.current !== runId
          ) {
            return;
          }
          setProgress(snapshot);
          setStageProgress((current) => ({ ...current, [snapshot.stage]: snapshot }));
        },
        parseResult.canonicalText,
      );
      if (activeRunIdRef.current !== runId) return;
      setReport(result);
      setLastAnalyzedFingerprint(submittedFingerprint);
      setPhase("complete");
    } catch (reason) {
      if (activeRunIdRef.current !== runId) return;
      const message = readError(reason);
      if (/cancelled/i.test(message)) {
        setPhase("ready");
      } else {
        setError(message);
        setPhase("error");
      }
    } finally {
      if (activeRunIdRef.current === runId) {
        activeRunIdRef.current = null;
        setActiveRunId(null);
      }
    }
  }, [canAnalyze, commanders, deckText, parseResult.canonicalText]);

  const startConfiguredAnalysis = useCallback((settings: AnalysisRunSettings) => {
    const submittedOptions: AnalysisOptions = {
      ...DEFAULT_OPTIONS,
      openingHandSimulations: settings.trials,
      gameSimulations: settings.trials,
      maximumTurn: settings.maximumTurn,
      allowOnlineCardResolution,
    };
    setAnalysisSettings(settings);
    setAnalysisSetupOpen(false);
    void runAnalysis(submittedOptions);
  }, [allowOnlineCardResolution, runAnalysis]);

  const stopAnalysis = useCallback(() => {
    const runId = activeRunIdRef.current;
    if (!runId) return;

    activeRunIdRef.current = null;
    setActiveRunId(null);
    setProgress(null);
    setStageProgress({});
    setPhase(deckText.trim() ? "ready" : "empty");

    void cancelAnalysis(runId)
      .then((cancelled) => {
        if (
          !cancelled &&
          latestRunIdRef.current === runId &&
          activeRunIdRef.current === null
        ) {
          setError(
            "Analysis stopped locally. The engine did not confirm cancellation; any late result will be ignored.",
          );
        }
      })
      .catch((reason) => {
        if (
          latestRunIdRef.current === runId &&
          activeRunIdRef.current === null
        ) {
          setError(
            `Analysis stopped locally. Engine cancellation could not be confirmed: ${readError(reason)}`,
          );
        }
      });
  }, [deckText]);

  useEffect(() => () => {
    activeRunIdRef.current = null;
    latestRunIdRef.current = null;
  }, []);

  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.ctrlKey && event.key === "Enter") {
        event.preventDefault();
        openAnalysisSetup();
      } else if (event.ctrlKey && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void openDeckFile();
      } else if (event.key === "Escape" && updateConfirmationOpen) {
        event.preventDefault();
        setUpdateConfirmationOpen(false);
      } else if (event.key === "Escape" && analysisSetupOpen) {
        event.preventDefault();
        setAnalysisSetupOpen(false);
      } else if (event.key === "Escape" && privacyOpen) {
        event.preventDefault();
        setPrivacyOpen(false);
      } else if (event.key === "Escape" && dataOpen) {
        event.preventDefault();
        setDataOpen(false);
      } else if (event.key === "Escape" && creditsOpen) {
        event.preventDefault();
        setCreditsOpen(false);
      } else if (event.key === "Escape" && activeRunId) {
        event.preventDefault();
        void stopAnalysis();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    activeRunId,
    analysisSetupOpen,
    creditsOpen,
    dataOpen,
    openAnalysisSetup,
    openDeckFile,
    privacyOpen,
    stopAnalysis,
    updateConfirmationOpen,
  ]);

  const clearReportAfterKnowledgeGeneration = () => {
    setReport(null);
    setLastAnalyzedFingerprint("");
    setPhase(deckText.trim() ? "ready" : "empty");
  };

  const installCardData = async () => {
    setUpdatingData(true);
    setUpdateProgress(null);
    setError(null);
    try {
      const status = await updateCardDatabase(setUpdateProgress);
      setDataStatus(status);
      clearReportAfterKnowledgeGeneration();
      return true;
    } catch (reason) {
      setError(readError(reason));
      return false;
    } finally {
      setUpdatingData(false);
    }
  };

  const installComboData = async () => {
    setUpdatingCombos(true);
    setComboUpdateProgress(null);
    setComboError(null);
    setComboNotice(null);
    try {
      const outcome = await updateComboDatabase(setComboUpdateProgress);
      setComboStatus(outcome.status);
      setComboNotice(
        outcome.outcome === "notModified"
          ? "The installed Commander Spellbook catalog is already current."
          : `Installed ${outcome.status.variantCount.toLocaleString()} documented variants.`,
      );
      if (outcome.outcome === "installed") {
        clearReportAfterKnowledgeGeneration();
      }
      return true;
    } catch (reason) {
      setComboError(readError(reason));
      return false;
    } finally {
      setUpdatingCombos(false);
    }
  };

  const installComprehensiveRules = async () => {
    setUpdatingComprehensiveRules(true);
    setComprehensiveRulesProgress(null);
    setComprehensiveRulesError(null);
    setComprehensiveRulesNotice(null);
    try {
      const outcome = await updateComprehensiveRules(setComprehensiveRulesProgress);
      setComprehensiveRulesStatus(outcome.status);
      setComprehensiveRulesNotice(
        outcome.outcome === "notModified"
          ? "The installed Comprehensive Rules are already current."
          : `Installed ${outcome.status.ruleCount.toLocaleString()} numbered rules from the official document.`,
      );
      if (outcome.outcome === "installed") {
        clearReportAfterKnowledgeGeneration();
      }
      return true;
    } catch (reason) {
      setComprehensiveRulesError(readError(reason));
      return false;
    } finally {
      setUpdatingComprehensiveRules(false);
    }
  };

  const checkKnowledgeUpdates = async () => {
    setCheckingKnowledgeUpdates(true);
    setKnowledgeUpdateCheck(null);
    setKnowledgeUpdateNotice(null);
    setKnowledgeUpdateError(null);
    try {
      const result = await checkForKnowledgeUpdates();
      setKnowledgeUpdateCheck(result);
      const failures = result.items.filter((item) => item.error);
      if (result.updateAvailable) {
        setUpdateConfirmationOpen(true);
      } else if (failures.length === result.items.length) {
        setKnowledgeUpdateError("None of the update sources could be checked.");
      } else if (failures.length > 0) {
        setKnowledgeUpdateError(
          `Available sources are current, but ${failures.map((item) => item.label).join(", ")} could not be checked.`,
        );
      } else {
        setKnowledgeUpdateNotice("Oracle cards, Commander Spellbook, and the Comprehensive Rules are current.");
      }
    } catch (reason) {
      setKnowledgeUpdateError(readError(reason));
    } finally {
      setCheckingKnowledgeUpdates(false);
    }
  };

  const installConfirmedKnowledgeUpdates = async () => {
    const updateIds = new Set(
      knowledgeUpdateCheck?.items
        .filter((item) => item.updateAvailable && !item.error)
        .map((item) => item.id) ?? [],
    );
    setUpdateConfirmationOpen(false);
    setKnowledgeUpdateNotice(null);
    setKnowledgeUpdateError(null);
    let succeeded = true;
    if (updateIds.has("cardData")) {
      succeeded = await installCardData() && succeeded;
    }
    if (updateIds.has("comboData")) {
      succeeded = await installComboData() && succeeded;
    }
    if (updateIds.has("comprehensiveRules")) {
      succeeded = await installComprehensiveRules() && succeeded;
    }
    setKnowledgeUpdateCheck(null);
    if (succeeded) {
      setKnowledgeUpdateNotice("Selected knowledge updates finished.");
    } else {
      setKnowledgeUpdateError("One or more updates failed; previously installed snapshots were preserved.");
    }
  };

  const choosePolicyPackage = async () => {
    setPolicyError(null);
    setPolicyNotice(null);
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Commander policy package", extensions: ["json"] }],
      });
      if (!path) return;
      setImportingPolicy(true);
      const outcome = await importPolicyPackage(path);
      setPolicyStatus(outcome.status);
      setPolicyNotice(outcome.message);
      if (outcome.activated) {
        clearReportAfterKnowledgeGeneration();
      }
    } catch (reason) {
      setPolicyError(readError(reason));
    } finally {
      setImportingPolicy(false);
    }
  };

  const chooseSemanticPackage = async () => {
    setSemanticError(null);
    setSemanticNotice(null);
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Semantic annotation package", extensions: ["json"] }],
      });
      if (!path) return;
      setImportingSemantics(true);
      const outcome = await importSemanticPackage(path);
      setSemanticStatus(outcome.status);
      setSemanticNotice(outcome.message);
      if (outcome.activated) {
        clearReportAfterKnowledgeGeneration();
      }
    } catch (reason) {
      setSemanticError(readError(reason));
    } finally {
      setImportingSemantics(false);
    }
  };

  const resetPolicyToBundled = async () => {
    setResettingPolicy(true);
    setPolicyError(null);
    setPolicyNotice(null);
    try {
      const outcome = await resetPolicyPackage();
      setPolicyStatus(outcome.status);
      setPolicyNotice(outcome.message);
      clearReportAfterKnowledgeGeneration();
    } catch (reason) {
      setPolicyError(readError(reason));
    } finally {
      setResettingPolicy(false);
    }
  };

  const resetSemanticsToBundled = async () => {
    setResettingSemantics(true);
    setSemanticError(null);
    setSemanticNotice(null);
    try {
      const outcome = await resetSemanticPackage();
      setSemanticStatus(outcome.status);
      setSemanticNotice(outcome.message);
      clearReportAfterKnowledgeGeneration();
    } catch (reason) {
      setSemanticError(readError(reason));
    } finally {
      setResettingSemantics(false);
    }
  };

  const handleDeckChange = (value: string) => {
    setDeckText(value);
    if (source) setSourceDirty(true);
    if (phase === "error") setPhase("ready");
  };

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">
            <Layers3 size={22} />
          </div>
          <div>
            <h1>Commander Deck Analyzer</h1>
            <p>Local consistency, synergy, speed, and resilience modeling</p>
          </div>
        </div>
        <div className="header-actions">
          <button className={`data-pill ${dataStatus?.state ?? "empty"}`} onClick={() => setDataOpen(true)} type="button">
            {dataStatus?.state === "offline" ? <WifiOff size={15} /> : <Database size={15} />}
            <span>{dataStatusLabel(dataStatus)}</span>
            <ChevronRight size={14} />
          </button>
          <button className="icon-button" onClick={() => setPrivacyOpen(true)} title="Network privacy" type="button">
            <ShieldCheck size={19} />
          </button>
          <button className="icon-button mobile-menu" title="Menu" type="button"><Menu size={19} /></button>
        </div>
      </header>

      <main className="workspace">
        <aside className="deck-workspace" aria-label="Deck workspace">
          <section className="workspace-section source-section">
            <div className="section-title">
              <div><Globe2 size={17} /><span>Import from a public URL</span></div>
              <span className="supported-label">Archidekt · Deckstats · Scryfall</span>
            </div>
            <div className="url-row">
              <input
                value={deckUrl}
                onChange={(event) => setDeckUrl(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void importFromUrl();
                }}
                placeholder="https://archidekt.com/decks/…"
                aria-label="Public deck URL"
                disabled={phase === "importing" || phase === "analyzing"}
              />
              <button className="compact-primary" onClick={() => void importFromUrl()} disabled={phase === "importing" || phase === "analyzing"} type="button">
                {phase === "importing" ? <LoaderCircle className="spin" size={17} /> : <Import size={17} />}
                Import
              </button>
            </div>
            <div className="source-actions">
              <button onClick={() => void pasteDeck()} disabled={phase === "analyzing"} type="button"><ClipboardPaste size={16} /> Paste decklist</button>
              <button onClick={() => void openDeckFile()} disabled={phase === "analyzing"} type="button"><FileText size={16} /> Open file</button>
              {source && <button onClick={() => void importFromUrl()} disabled={phase === "analyzing"} type="button"><RefreshCw size={16} /> Refresh</button>}
            </div>
          </section>

          {(deckText || source) && (
            <section className="deck-identity">
              <div className="identity-heading">
                <div>
                  <span className="eyebrow">Deck</span>
                  <input
                    className="deck-name-input"
                    value={deckName}
                    onChange={(event) => setDeckName(event.target.value)}
                    placeholder="Untitled Commander deck"
                    aria-label="Deck name"
                  />
                </div>
                {source && <span className="source-badge">{source.provider}{sourceDirty ? " · edited" : ""}</span>}
              </div>
              <label className="commander-field">
                <span>Commander <small>one per line for partners</small></span>
                <textarea
                  value={commanderInput}
                  onChange={(event) => setCommanderInput(event.target.value)}
                  rows={Math.min(Math.max(commanders.length, 1), 2)}
                  placeholder="Select or enter the commander"
                  disabled={phase === "analyzing"}
                />
              </label>
            </section>
          )}

          <section className="editor-section">
            <div className="editor-heading">
              <div>
                <span className="eyebrow">Decklist</span>
                <h2>{parseResult.cardCount ? `${parseResult.cardCount} cards` : "Paste or import a deck"}</h2>
              </div>
              <span className={`count-badge ${parseResult.isCommanderSized ? "valid" : parseResult.cardCount ? "invalid" : ""}`}>
                {parseResult.cardCount}/100
              </span>
            </div>
            <textarea
              ref={textareaRef}
              className="deck-editor"
              value={deckText}
              onChange={(event) => handleDeckChange(event.target.value)}
              placeholder={"Commander\n1 Alela, Artful Provocateur\n\nDeck\n1 Sol Ring\n1 Command Tower\n…"}
              spellCheck={false}
              disabled={phase === "analyzing"}
              aria-label="Commander decklist"
            />
          </section>

          <section className="preflight-section" aria-label="Deck preflight">
            <div className="validation-chips">
              <ValidationChip ok={parseResult.isCommanderSized} label={`${parseResult.cardCount} cards`} />
              <ValidationChip ok={commanders.length > 0 && commanders.length <= 2} label={commanders.length ? `${commanders.length} commander${commanders.length === 1 ? "" : "s"}` : "Commander needed"} />
              <ValidationChip neutral label={`${parseResult.uniqueCardCount} unique`} />
            </div>
            {visibleParseIssues.length > 0 && (
              <div className="issue-summary">
                <Info size={15} />
                <span>{visibleParseIssues[0].message}</span>
              </div>
            )}
            {error && (
              <div className="error-summary" role="alert">
                <Info size={15} />
                <span>{error}</span>
                <button onClick={() => setError(null)} aria-label="Dismiss message" type="button"><X size={14} /></button>
              </div>
            )}
            <div className="analysis-actions">
              <div className="fixed-policy-summary">
                <SlidersHorizontal size={16} />
                <span><strong>Analysis setup</strong><small>Aggressive mulligans · {options.gameSimulations.toLocaleString()} trials · turns 1 to {options.maximumTurn}</small></span>
              </div>
              {phase === "analyzing" ? (
                <button className="cancel-button" onClick={() => void stopAnalysis()} type="button"><Square size={15} /> Cancel</button>
              ) : (
                <button className="analyze-button" onClick={openAnalysisSetup} disabled={!canAnalyze} type="button">
                  <Play size={17} fill="currentColor" />
                  Analyze deck
                  <kbd>Ctrl ↵</kbd>
                </button>
              )}
            </div>
          </section>
        </aside>

        <section className="results-host">
          {phase === "analyzing" ? (
            <AnalysisProgressView progress={progress} stageProgress={stageProgress} onCancel={() => void stopAnalysis()} />
          ) : report ? (
            <ReportView report={report} activeTab={activeTab} onTabChange={setActiveTab} stale={reportIsStale} />
          ) : (
            <EmptyResults />
          )}
        </section>
      </main>

      <footer className="status-bar">
        <div><ShieldCheck size={14} /><span>Parsing, simulation, and scoring run locally</span></div>
        <div>
          <span>{dataStatus?.cardCount ? `${dataStatus.cardCount.toLocaleString()} cards cached` : "Bootstrap card data"}</span>
          <span aria-hidden="true">•</span>
          <button
            className="status-credits-button"
            onClick={() => setCreditsOpen(true)}
            type="button"
          >
            Credits & data sources
          </button>
          <span aria-hidden="true">•</span>
          <span>Local engine</span>
        </div>
      </footer>

      {analysisSetupOpen && (
        <AnalysisSetupDialog
          initialSettings={analysisSettings}
          onConfirm={startConfiguredAnalysis}
          onClose={() => setAnalysisSetupOpen(false)}
        />
      )}
      {privacyOpen && (
        <PrivacyPanel
          allowOnlineCardResolution={allowOnlineCardResolution}
          onAllowOnlineCardResolutionChange={setAllowOnlineCardResolution}
          onClose={() => setPrivacyOpen(false)}
        />
      )}
      {dataOpen && (
        <DataPanel
          status={dataStatus}
          updating={updatingData}
          progress={updateProgress}
          comboStatus={comboStatus}
          updatingCombos={updatingCombos}
          comboProgress={comboUpdateProgress}
          comboError={comboError}
          comboNotice={comboNotice}
          comprehensiveRulesStatus={comprehensiveRulesStatus}
          updatingComprehensiveRules={updatingComprehensiveRules}
          comprehensiveRulesProgress={comprehensiveRulesProgress}
          comprehensiveRulesError={comprehensiveRulesError}
          comprehensiveRulesNotice={comprehensiveRulesNotice}
          policyStatus={policyStatus}
          importingPolicy={importingPolicy}
          resettingPolicy={resettingPolicy}
          policyError={policyError}
          policyNotice={policyNotice}
          onImportPolicy={() => void choosePolicyPackage()}
          onResetPolicy={() => void resetPolicyToBundled()}
          semanticStatus={semanticStatus}
          importingSemantics={importingSemantics}
          resettingSemantics={resettingSemantics}
          semanticError={semanticError}
          semanticNotice={semanticNotice}
          onImportSemantics={() => void chooseSemanticPackage()}
          onResetSemantics={() => void resetSemanticsToBundled()}
          checkingUpdates={checkingKnowledgeUpdates}
          updateCheck={knowledgeUpdateCheck}
          updateNotice={knowledgeUpdateNotice}
          updateError={knowledgeUpdateError}
          onCheckUpdates={() => void checkKnowledgeUpdates()}
          onClose={() => setDataOpen(false)}
        />
      )}
      {updateConfirmationOpen && knowledgeUpdateCheck && (
        <UpdateConfirmationDialog
          check={knowledgeUpdateCheck}
          onConfirm={() => void installConfirmedKnowledgeUpdates()}
          onClose={() => setUpdateConfirmationOpen(false)}
        />
      )}
      {creditsOpen && <CreditsPanel onClose={() => setCreditsOpen(false)} />}
    </div>
  );
}

function EmptyResults() {
  return (
    <div className="empty-results">
      <div className="instrument-visual" aria-hidden="true">
        <div className="orbit orbit-one" />
        <div className="orbit orbit-two" />
        <div className="instrument-core"><Gauge size={34} /></div>
        <span className="instrument-node node-one" />
        <span className="instrument-node node-two" />
        <span className="instrument-node node-three" />
      </div>
      <div className="eyebrow">Local analysis workspace</div>
      <h2>See how your deck actually behaves.</h2>
      <p>Import a 100-card list to model opening hands, plan access, credible threat speed, disruption sensitivity, and the evidence behind a likely bracket range.</p>
      <div className="empty-feature-grid">
        <div><BarChart3 size={18} /><strong>Consistency</strong><span>London mulligans and mana development</span></div>
        <div><Target size={18} /><strong>Plan speed</strong><span>Bounded, reproducible turn trajectories</span></div>
        <div><ShieldCheck size={18} /><strong>Resilience</strong><span>Standardized interaction and recovery model</span></div>
        <div><BookOpenCheck size={18} /><strong>Explainability</strong><span>Coverage, assumptions, and evidence</span></div>
      </div>
      <div className="privacy-note"><Database size={15} /> APIs update card identities and import public lists; analysis remains on this device.</div>
    </div>
  );
}

function AnalysisProgressView({
  progress,
  stageProgress,
  onCancel,
}: {
  progress: AnalysisProgress | null;
  stageProgress: Partial<Record<AnalysisStage, AnalysisProgress>>;
  onCancel: () => void;
}) {
  const activeIndex = progress ? stageOrder.findIndex((stage) => stage.id === progress.stage) : 0;
  return (
    <div className="analysis-progress-view">
      <div className="progress-header">
        <div>
          <div className="eyebrow">Analysis in progress</div>
          <h2>{progress?.stageLabel ?? "Preparing analysis"}</h2>
          <p>{progress?.detail ?? "Snapshotting the deck and local data versions…"}</p>
        </div>
        <button className="cancel-button" onClick={onCancel} type="button"><Square size={15} /> Cancel analysis</button>
      </div>
      <div className="overall-progress">
        <div>
          <span>Overall progress</span>
          <strong>{Math.round((progress?.overallProgress ?? 0) * 100)}%</strong>
        </div>
        <div className="bar-track huge"><div className="bar-fill primary" style={{ width: `${(progress?.overallProgress ?? 0) * 100}%` }} /></div>
      </div>
      <div className="stage-list">
        {stageOrder.map((stage, index) => {
          const snapshot = stageProgress[stage.id];
          const done = index < activeIndex || progress?.stage === "complete";
          const active = index === activeIndex && progress?.stage !== "complete";
          return (
            <div className={`stage-row ${done ? "done" : ""} ${active ? "active" : ""}`} key={stage.id}>
              <div className="stage-status">
                {done ? <CheckCircle2 size={20} /> : active ? <LoaderCircle className="spin" size={20} /> : <Circle size={20} />}
              </div>
              <div className="stage-copy">
                <strong>{stage.label}</strong>
                <span>{active && snapshot ? snapshot.detail : stage.description}</span>
              </div>
              <div className="stage-units">
                {snapshot && snapshot.totalUnits > 1 ? `${snapshot.completedUnits.toLocaleString()} / ${snapshot.totalUnits.toLocaleString()}` : done ? "Complete" : ""}
              </div>
            </div>
          );
        })}
      </div>
      <div className="analysis-footnote"><Sparkles size={16} /> Every progress update represents completed work; there is no artificial delay.</div>
    </div>
  );
}

function ValidationChip({ ok, neutral = false, label }: { ok?: boolean; neutral?: boolean; label: string }) {
  return <span className={`validation-chip ${neutral ? "neutral" : ok ? "ok" : "needs-attention"}`}>{ok && !neutral ? <Check size={13} /> : <Circle size={10} fill="currentColor" />}{label}</span>;
}

function AnalysisSetupDialog({
  initialSettings,
  onConfirm,
  onClose,
}: {
  initialSettings: AnalysisRunSettings;
  onConfirm: (settings: AnalysisRunSettings) => void;
  onClose: () => void;
}) {
  const [maximumTurn, setMaximumTurn] = useState(initialSettings.maximumTurn);
  const [trials, setTrials] = useState<AnalysisTrialCount>(initialSettings.trials);
  const dialogRef = useRef<HTMLElement>(null);
  const turnSliderRef = useRef<HTMLInputElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );

  useEffect(() => {
    turnSliderRef.current?.focus();
    return () => returnFocusRef.current?.focus();
  }, []);

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    trapDialogTab(event, dialogRef.current);
  };

  return (
    <div
      className="modal-backdrop centered-dialog-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        ref={dialogRef}
        className="confirmation-dialog analysis-setup-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="analysis-setup-title"
        tabIndex={-1}
        onKeyDown={handleKeyDown}
      >
        <div className="confirmation-dialog-header">
          <div>
            <span className="eyebrow">Analysis setup</span>
            <h2 id="analysis-setup-title">Choose this run’s workload</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close analysis setup" type="button"><X size={18} /></button>
        </div>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            onConfirm({ trials, maximumTurn });
          }}
        >
          <div className="analysis-setting">
            <div className="analysis-setting-heading">
              <label htmlFor="analysis-turn-horizon">Turns analyzed</label>
              <output htmlFor="analysis-turn-horizon">Turns 1 to {maximumTurn}</output>
            </div>
            <input
              ref={turnSliderRef}
              id="analysis-turn-horizon"
              type="range"
              min={MINIMUM_ANALYSIS_TURN}
              max={MAXIMUM_ANALYSIS_TURN}
              step={1}
              value={maximumTurn}
              onChange={(event) => setMaximumTurn(Number(event.target.value))}
            />
            <div className="range-boundaries" aria-hidden="true">
              <span>{MINIMUM_ANALYSIS_TURN}</span>
              <span>{MAXIMUM_ANALYSIS_TURN}</span>
            </div>
            <p>The selected horizon controls timing evidence and can change the estimated bracket.</p>
          </div>

          <fieldset className="analysis-setting">
            <legend>Trials</legend>
            <div className="trial-options">
              {ANALYSIS_TRIAL_OPTIONS.map((value) => (
                <label className={trials === value ? "selected" : ""} key={value}>
                  <input
                    type="radio"
                    name="analysis-trials"
                    value={value}
                    checked={trials === value}
                    onChange={() => setTrials(value)}
                  />
                  <strong>{value.toLocaleString()}</strong>
                  <span>{value === 1000 ? "Standard" : value === 5000 ? "Detailed" : "Maximum"}</span>
                </label>
              ))}
            </div>
            <p>The same count is used for opening hands and paired baseline/interaction trajectories.</p>
          </fieldset>

          <div className="confirmation-dialog-actions">
            <button className="secondary-button" onClick={onClose} type="button">Cancel</button>
            <button className="compact-primary" type="submit"><Play size={16} fill="currentColor" /> Analyze</button>
          </div>
        </form>
      </section>
    </div>
  );
}

function UpdateConfirmationDialog({
  check,
  onConfirm,
  onClose,
}: {
  check: KnowledgeUpdateCheck;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const noButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  const updates = check.items.filter((item) => item.updateAvailable && !item.error);
  const failures = check.items.filter((item) => item.error);

  useEffect(() => {
    noButtonRef.current?.focus();
    return () => returnFocusRef.current?.focus();
  }, []);

  const handleKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    trapDialogTab(event, dialogRef.current);
  };

  return (
    <div className="modal-backdrop centered-dialog-backdrop update-confirmation-backdrop">
      <section
        ref={dialogRef}
        className="confirmation-dialog update-confirmation-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-confirmation-title"
        tabIndex={-1}
        onKeyDown={handleKeyDown}
      >
        <div className="confirmation-dialog-header">
          <div>
            <span className="eyebrow">Updates available</span>
            <h2 id="update-confirmation-title">Download these updates?</h2>
          </div>
        </div>
        <div className="update-confirmation-body">
          <ul>
            {updates.map((item) => (
              <li key={item.id}>
                <CheckCircle2 size={16} />
                <div><strong>{item.label}</strong><span>{item.detail}</span></div>
              </li>
            ))}
          </ul>
          {failures.length > 0 && (
            <p className="update-check-warning">
              Could not check: {failures.map((item) => item.label).join(", ")}. Those items will not be downloaded.
            </p>
          )}
          <p className="update-confirmation-note">
            The check downloaded metadata only. Choosing Yes starts the bounded downloads and preserves each installed snapshot if validation fails.
          </p>
        </div>
        <div className="confirmation-dialog-actions">
          <button ref={noButtonRef} className="secondary-button" onClick={onClose} type="button">No</button>
          <button className="compact-primary" onClick={onConfirm} type="button"><CloudDownload size={16} /> Yes, download</button>
        </div>
      </section>
    </div>
  );
}

function trapDialogTab(
  event: ReactKeyboardEvent<HTMLElement>,
  dialog: HTMLElement | null,
) {
  if (event.key !== "Tab") return;
  const focusable = [
    ...(dialog?.querySelectorAll<HTMLElement>(DIALOG_FOCUSABLE_SELECTOR) ?? []),
  ].filter(
    (element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true",
  );
  if (!focusable.length) {
    event.preventDefault();
    dialog?.focus();
    return;
  }
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  } else if (!dialog?.contains(document.activeElement)) {
    event.preventDefault();
    first.focus();
  }
}

function PrivacyPanel({
  allowOnlineCardResolution,
  onAllowOnlineCardResolutionChange,
  onClose,
}: {
  allowOnlineCardResolution: boolean;
  onAllowOnlineCardResolutionChange: (allowed: boolean) => void;
  onClose: () => void;
}) {
  const panelRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );

  useEffect(() => {
    closeButtonRef.current?.focus();
    return () => {
      returnFocusRef.current?.focus();
    };
  }, []);

  const handleDialogKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;

    const focusable = [
      ...(panelRef.current?.querySelectorAll<HTMLElement>(DIALOG_FOCUSABLE_SELECTOR) ?? []),
    ].filter(
      (element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true",
    );
    if (!focusable.length) {
      event.preventDefault();
      panelRef.current?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    } else if (!panelRef.current?.contains(document.activeElement)) {
      event.preventDefault();
      first.focus();
    }
  };
  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <aside
        ref={panelRef}
        className="side-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="network-privacy-title"
        tabIndex={-1}
        onKeyDown={handleDialogKeyDown}
      >
        <div className="panel-header"><div><span className="eyebrow">Operational preference</span><h2 id="network-privacy-title">Network privacy</h2></div><button ref={closeButtonRef} className="icon-button" onClick={onClose} aria-label="Close network privacy" type="button"><X size={19} /></button></div>
        <div className="panel-body">
          <section className="fixed-policy-card" aria-label="Fixed analyzer policy">
            <Target size={18} />
            <div>
              <strong>One objective policy for every deck</strong>
              <p>Aggressive London mulligans, proactive route search, and standardized high-power interaction remain fixed. Trial count and turn horizon are chosen when analysis starts. Player-declared intent is not used.</p>
            </div>
          </section>
          <label className="privacy-toggle">
            <input type="checkbox" checked={allowOnlineCardResolution} onChange={(event) => onAllowOnlineCardResolutionChange(event.target.checked)} />
            <span><strong>Resolve missing cards through Scryfall</strong><small>Off by default. Enabling this sends only unresolved card names to Scryfall and stores the returned identities locally. Install the full snapshot in Local knowledge to keep analysis offline.</small></span>
          </label>
          <div className="panel-note"><Info size={17} /><p>This privacy choice affects only how unresolved card identities are fetched. It does not change mulligans, sequencing, interaction, timing, or scoring.</p></div>
        </div>
        <div className="panel-footer"><button className="compact-primary full" onClick={onClose} type="button">Done</button></div>
      </aside>
    </div>
  );
}

function DataPanel({
  status,
  updating,
  progress,
  comboStatus,
  updatingCombos,
  comboProgress,
  comboError,
  comboNotice,
  comprehensiveRulesStatus,
  updatingComprehensiveRules,
  comprehensiveRulesProgress,
  comprehensiveRulesError,
  comprehensiveRulesNotice,
  policyStatus,
  importingPolicy,
  resettingPolicy,
  policyError,
  policyNotice,
  onImportPolicy,
  onResetPolicy,
  semanticStatus,
  importingSemantics,
  resettingSemantics,
  semanticError,
  semanticNotice,
  onImportSemantics,
  onResetSemantics,
  checkingUpdates,
  updateCheck,
  updateNotice,
  updateError,
  onCheckUpdates,
  onClose,
}: {
  status: DataStatus | null;
  updating: boolean;
  progress: DataUpdateProgress | null;
  comboStatus: ComboStoreStatus | null;
  updatingCombos: boolean;
  comboProgress: ComboUpdateProgress | null;
  comboError: string | null;
  comboNotice: string | null;
  comprehensiveRulesStatus: ComprehensiveRulesStatus | null;
  updatingComprehensiveRules: boolean;
  comprehensiveRulesProgress: ComprehensiveRulesUpdateProgress | null;
  comprehensiveRulesError: string | null;
  comprehensiveRulesNotice: string | null;
  policyStatus: PolicyPackageStatus | null;
  importingPolicy: boolean;
  resettingPolicy: boolean;
  policyError: string | null;
  policyNotice: string | null;
  onImportPolicy: () => void;
  onResetPolicy: () => void;
  semanticStatus: SemanticPackageStatus | null;
  importingSemantics: boolean;
  resettingSemantics: boolean;
  semanticError: string | null;
  semanticNotice: string | null;
  onImportSemantics: () => void;
  onResetSemantics: () => void;
  checkingUpdates: boolean;
  updateCheck: KnowledgeUpdateCheck | null;
  updateNotice: string | null;
  updateError: string | null;
  onCheckUpdates: () => void;
  onClose: () => void;
}) {
  const panelRef = useRef<HTMLElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(
    typeof document !== "undefined" && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );

  useEffect(() => {
    closeButtonRef.current?.focus();
    return () => {
      returnFocusRef.current?.focus();
    };
  }, []);

  const handleDialogKeyDown = (event: ReactKeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;

    const focusable = [
      ...(panelRef.current?.querySelectorAll<HTMLElement>(DIALOG_FOCUSABLE_SELECTOR) ?? []),
    ].filter(
      (element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true",
    );
    if (!focusable.length) {
      event.preventDefault();
      panelRef.current?.focus();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    } else if (!panelRef.current?.contains(document.activeElement)) {
      event.preventDefault();
      first.focus();
    }
  };
  const installingKnowledge =
    updating || updatingCombos || updatingComprehensiveRules;

  return (
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <aside
        ref={panelRef}
        className="side-panel data-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="local-knowledge-title"
        tabIndex={-1}
        onKeyDown={handleDialogKeyDown}
      >
        <div className="panel-header"><div><span className="eyebrow">Update center</span><h2 id="local-knowledge-title">Local knowledge</h2></div><button ref={closeButtonRef} className="icon-button" onClick={onClose} aria-label="Close local knowledge" type="button"><X size={19} /></button></div>
        <div className="panel-body">
          <div className={`data-status-card ${status?.state ?? "empty"}`}>
            <div className="data-status-icon"><Database size={22} /></div>
            <div><span>{dataStatusLabel(status)}</span><strong>{status?.cardCount.toLocaleString() ?? "0"} cards</strong><p>{status?.message ?? "Checking the local database…"}</p></div>
          </div>
          {updating && progress && (
            <div className="update-progress-card" role="status" aria-live="polite">
              <div><span>{capitalize(progress.phase)}</span><strong>{Math.round(progress.progress * 100)}%</strong></div>
              <div className="bar-track large" role="progressbar" aria-label="Card database update" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progress.progress * 100)}><div className="bar-fill primary" style={{ width: `${progress.progress * 100}%` }} /></div>
              <p>{progress.detail}</p>
            </div>
          )}
          <div className="data-package-list">
            <div><div className="package-icon"><CloudDownload size={17} /></div><div><strong>Oracle card definitions</strong><span>Scryfall names, text, types, costs, colors, and legality</span></div><span className={`package-state ${status?.state}`}>{status?.state === "ready" ? "Installed" : "Partial"}</span></div>
            <div><div className="package-icon"><FileText size={17} /></div><div><strong>Comprehensive Rules</strong><span>Official game rules, indexed locally · {formatPolicyDate(comprehensiveRulesStatus?.effectiveDate)}</span></div><span className={`package-state ${comprehensiveRulesStatus?.ready ? "ready" : ""} ${comprehensiveRulesStatus?.compatibility === "changed" || comprehensiveRulesStatus?.compatibility === "referenceOnly" ? "warning" : ""}`}>{comprehensiveRulesCompatibilityLabel(comprehensiveRulesStatus)}</span></div>
            <div><div className="package-icon"><BookOpenCheck size={17} /></div><div><strong>Commander policy snapshot</strong><span>Legality, bans, Game Changers, and bracket guidance · {policyStatus?.packageVersion ?? "checking"}</span></div><span className={`package-state ${policyStatus?.ready ? "ready" : ""}`}>{policyOriginLabel(policyStatus)}</span></div>
            <div><div className="package-icon"><Sparkles size={17} /></div><div><strong>Semantic annotations</strong><span>Package-supplied card overrides · {semanticStatus?.packageVersion ?? "checking"}</span></div><span className={`package-state ${semanticStatus?.ready ? "ready" : ""}`}>{semanticOriginLabel(semanticStatus)}</span></div>
            <div><div className="package-icon"><Gauge size={17} /></div><div><strong>Simulation & bracket model</strong><span>Versioned with application releases</span></div><span className="package-state ready">Bundled</span></div>
          </div>
          {status?.snapshotSha256 && <p className="catalog-authenticity"><strong>Card snapshot SHA-256:</strong> {status.snapshotSha256}</p>}

          <section className={`combo-catalog-card comprehensive-rules-card ${comprehensiveRulesStatus?.ready ? "ready" : ""}`} aria-labelledby="comprehensive-rules-title">
            <div className="combo-catalog-heading">
              <div className="data-status-icon"><FileText size={21} /></div>
              <div>
                <span className="eyebrow">Official game reference</span>
                <h3 id="comprehensive-rules-title">Comprehensive Rules</h3>
                <p>{comprehensiveRulesStatus?.message ?? "Checking the locally indexed official rules document…"}</p>
              </div>
              <span className={`package-state ${comprehensiveRulesStatus?.ready ? "ready" : ""} ${comprehensiveRulesStatus?.compatibility === "changed" || comprehensiveRulesStatus?.compatibility === "referenceOnly" ? "warning" : ""}`}>
                {comprehensiveRulesCompatibilityLabel(comprehensiveRulesStatus)}
              </span>
            </div>

            {comprehensiveRulesStatus?.ready && (
              <dl className="combo-catalog-details comprehensive-rules-details">
                <div><dt>Effective</dt><dd>{formatPolicyDate(comprehensiveRulesStatus.effectiveDate)}</dd></div>
                <div><dt>Installed</dt><dd>{formatTimestamp(comprehensiveRulesStatus.installedAt)}</dd></div>
                <div><dt>Numbered rules</dt><dd>{comprehensiveRulesStatus.ruleCount.toLocaleString()}</dd></div>
                <div><dt>Sections</dt><dd>{comprehensiveRulesStatus.sectionCount.toLocaleString()}</dd></div>
                <div><dt>Glossary entries</dt><dd>{comprehensiveRulesStatus.glossaryCount.toLocaleString()}</dd></div>
                <div><dt>Commander rules</dt><dd>{comprehensiveRulesStatus.commanderRuleCount.toLocaleString()}</dd></div>
                <div><dt>Examples</dt><dd>{comprehensiveRulesStatus.exampleCount.toLocaleString()}</dd></div>
                <div><dt>Parser</dt><dd title={comprehensiveRulesStatus.parserVersion}>{comprehensiveRulesStatus.parserVersion}</dd></div>
              </dl>
            )}

            <div className="catalog-digest">
              <span>Official discovery page</span>
              <code title={comprehensiveRulesStatus?.sourcePageUrl ?? "https://magic.wizards.com/en/rules"}>
                magic.wizards.com/en/rules
              </code>
            </div>

            {comprehensiveRulesStatus?.snapshotSha256 && (
              <div className="catalog-digest">
                <span>Local document SHA-256</span>
                <code title={comprehensiveRulesStatus.snapshotSha256}>{shortHash(comprehensiveRulesStatus.snapshotSha256)}</code>
              </div>
            )}

            {updatingComprehensiveRules && comprehensiveRulesProgress && (
              <div className="update-progress-card combo-progress" role="status" aria-live="polite">
                <div><span>{capitalize(comprehensiveRulesProgress.phase)}</span><strong>{Math.round(comprehensiveRulesProgress.progress * 100)}%</strong></div>
                <div className="bar-track large" role="progressbar" aria-label="Comprehensive Rules update" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(comprehensiveRulesProgress.progress * 100)}><div className="bar-fill primary" style={{ width: `${comprehensiveRulesProgress.progress * 100}%` }} /></div>
                <p>{comprehensiveRulesProgress.detail}</p>
              </div>
            )}

            {(comprehensiveRulesStatus?.compatibility === "changed" || comprehensiveRulesStatus?.compatibility === "referenceOnly") && (
              <div className="catalog-message warning" role="status">
                <Info size={14} />
                <span>
                  One or more reviewed rule IDs or headings are missing from this snapshot. Affected mechanics remain reference-only until reviewed
                  {comprehensiveRulesStatus.changedCapabilityRuleIds.length > 0
                    ? ` (${comprehensiveRulesStatus.changedCapabilityRuleIds.slice(0, 8).join(", ")}${comprehensiveRulesStatus.changedCapabilityRuleIds.length > 8 ? ", …" : ""}).`
                    : "."}
                </span>
              </div>
            )}
            {comprehensiveRulesError && <div className="catalog-message error" role="alert"><Info size={14} /><span>{comprehensiveRulesError}</span></div>}
            {comprehensiveRulesNotice && !comprehensiveRulesError && <div className="catalog-message success" role="status"><CheckCircle2 size={14} /><span>{comprehensiveRulesNotice}</span></div>}

            <div className="catalog-disclosure">
              <ShieldCheck size={15} />
              <p>
                This downloads the official Comprehensive Rules TXT discovered through Wizards’ exact allowlisted HTTPS rules page and builds a local index. No decklist or card names are sent. The app records transport metadata and its own SHA-256; Wizards does not provide a signed manifest for this file. Downloaded rule text never executes; only reviewed, versioned capabilities may contribute strategic evidence.
              </p>
            </div>
            {comprehensiveRulesStatus && <p className="catalog-authenticity"><strong>Recorded provenance:</strong> {comprehensiveRulesStatus.authenticityBasis}</p>}
          </section>

          <section className={`combo-catalog-card policy-package-card ${policyStatus?.ready ? "ready" : ""}`} aria-labelledby="policy-package-title">
            <div className="combo-catalog-heading">
              <div className="data-status-icon"><BookOpenCheck size={21} /></div>
              <div>
                <span className="eyebrow">Versioned rules</span>
                <h3 id="policy-package-title">Commander policy package</h3>
                <p>{policyStatus?.message ?? "Checking the active legality and bracket policy snapshot…"}</p>
              </div>
              <span className={`package-state ${policyStatus?.ready ? "ready" : ""}`}>{policyOriginLabel(policyStatus)}</span>
            </div>

            {policyStatus && (
              <dl className="combo-catalog-details policy-package-details">
                <div><dt>Package</dt><dd>{policyStatus.packageVersion}</dd></div>
                <div><dt>Policy status</dt><dd>{capitalize(policyStatus.policyStatus)}</dd></div>
                <div><dt>Effective</dt><dd>{formatPolicyDate(policyStatus.effectiveDate)}</dd></div>
                <div><dt>Verified</dt><dd>{formatPolicyDate(policyStatus.verifiedAt)}</dd></div>
                <div><dt>Sources</dt><dd>{policyStatus.sourceCount.toLocaleString()}</dd></div>
                <div><dt>Imported</dt><dd>{formatTimestamp(policyStatus.importedAt)}</dd></div>
              </dl>
            )}

            {policyStatus?.snapshotSha256 && (
              <div className="catalog-digest">
                <span>Active policy SHA-256</span>
                <code title={policyStatus.snapshotSha256}>{shortHash(policyStatus.snapshotSha256)}</code>
              </div>
            )}

            {policyError && <div className="catalog-message error" role="alert"><Info size={14} /><span>{policyError}</span></div>}
            {policyNotice && !policyError && <div className="catalog-message success" role="status"><CheckCircle2 size={14} /><span>{policyNotice}</span></div>}

            <div className="catalog-action-row">
              <button className="catalog-update-button policy-import-button" onClick={onImportPolicy} disabled={importingPolicy || resettingPolicy} type="button">
                {importingPolicy ? <LoaderCircle className="spin" size={16} /> : <Import size={16} />}
                {importingPolicy ? "Validating and activating…" : "Import newer policy package"}
              </button>
              <button
                className="catalog-update-button catalog-reset-button"
                onClick={onResetPolicy}
                disabled={importingPolicy || resettingPolicy || !policyStatus || policyStatus.origin === "bundled"}
                aria-label="Reset Commander policy package to bundled"
                type="button"
              >
                {resettingPolicy ? <LoaderCircle className="spin" size={16} /> : <RotateCcw size={16} />}
                {resettingPolicy ? "Resetting policy…" : "Reset to bundled policy"}
              </button>
            </div>

            <div className="catalog-disclosure">
              <ShieldCheck size={15} />
              <p>
                Select a local JSON package from a source you trust. The app validates its schema, dates, limits, and exact SHA-256, and checks that each declared citation is a well-formed HTTPS URL. It does not fetch or authenticate cited content. Downgrades and same-version conflicts are rejected before atomic activation. No publisher signature is currently available, so a valid package is not proof of authorship.
              </p>
            </div>
            {policyStatus && <p className="catalog-authenticity"><strong>Recorded provenance:</strong> {policyStatus.authenticityBasis}</p>}
          </section>

          <section className={`combo-catalog-card semantic-package-card ${semanticStatus?.ready ? "ready" : ""}`} aria-labelledby="semantic-package-title">
            <div className="combo-catalog-heading">
              <div className="data-status-icon"><Sparkles size={21} /></div>
              <div>
                <span className="eyebrow">Versioned semantics</span>
                <h3 id="semantic-package-title">Semantic annotation package</h3>
                <p>{semanticStatus?.message ?? "Checking the active card-annotation snapshot…"}</p>
              </div>
              <span className={`package-state ${semanticStatus?.ready ? "ready" : ""}`}>{semanticOriginLabel(semanticStatus)}</span>
            </div>

            {semanticStatus && (
              <dl className="combo-catalog-details semantic-package-details">
                <div><dt>Package</dt><dd>{semanticStatus.packageVersion}</dd></div>
                <div><dt>Origin</dt><dd>{semanticOriginLabel(semanticStatus)}</dd></div>
                <div><dt>Declared effective</dt><dd>{formatPolicyDate(semanticStatus.effectiveDate)}</dd></div>
                <div><dt>Declared verified</dt><dd>{formatPolicyDate(semanticStatus.verifiedAt)}</dd></div>
                <div><dt>Sources</dt><dd>{semanticStatus.sourceCount.toLocaleString()}</dd></div>
                <div><dt>Overrides</dt><dd>{semanticStatus.overrideCount.toLocaleString()}</dd></div>
                <div><dt>Schema</dt><dd>{semanticStatus.schemaVersion}</dd></div>
                <div><dt>Imported</dt><dd>{formatTimestamp(semanticStatus.importedAt)}</dd></div>
              </dl>
            )}

            {semanticStatus?.snapshotSha256 && (
              <div className="catalog-digest">
                <span>Active semantics SHA-256</span>
                <code title={semanticStatus.snapshotSha256}>{shortHash(semanticStatus.snapshotSha256)}</code>
              </div>
            )}

            {semanticError && <div className="catalog-message error" role="alert"><Info size={14} /><span>{semanticError}</span></div>}
            {semanticNotice && !semanticError && <div className="catalog-message success" role="status"><CheckCircle2 size={14} /><span>{semanticNotice}</span></div>}

            <div className="catalog-action-row">
              <button className="catalog-update-button semantic-import-button" onClick={onImportSemantics} disabled={importingSemantics || resettingSemantics} type="button">
                {importingSemantics ? <LoaderCircle className="spin" size={16} /> : <Import size={16} />}
                {importingSemantics ? "Validating and activating…" : "Import semantic annotation package"}
              </button>
              <button
                className="catalog-update-button catalog-reset-button"
                onClick={onResetSemantics}
                disabled={importingSemantics || resettingSemantics || !semanticStatus || semanticStatus.origin === "bundled"}
                aria-label="Reset semantic annotation package to bundled"
                type="button"
              >
                {resettingSemantics ? <LoaderCircle className="spin" size={16} /> : <RotateCcw size={16} />}
                {resettingSemantics ? "Resetting semantics…" : "Reset to bundled semantics"}
              </button>
            </div>

            <div className="catalog-disclosure">
              <ShieldCheck size={15} />
              <p>
                Select a local JSON package from a source you trust. The app validates its schema, dates, card identifiers, limits, and exact SHA-256, and checks that each declared citation is a well-formed HTTPS URL. It does not fetch or authenticate cited content. Before atomic activation it validates any supplied Oracle-text SHA-256 guard; unguarded overrides are allowed and reported in analysis. Packages are not publisher-signed, so their declared verification date and authorship are not independently authenticated.
              </p>
            </div>
            {semanticStatus && <p className="catalog-authenticity"><strong>Recorded provenance:</strong> {semanticStatus.authenticityBasis}</p>}
          </section>

          <section className={`combo-catalog-card ${comboStatus?.ready ? "ready" : ""}`} aria-labelledby="combo-catalog-title">
            <div className="combo-catalog-heading">
              <div className="data-status-icon"><Braces size={21} /></div>
              <div>
                <span className="eyebrow">Optional catalog</span>
                <h3 id="combo-catalog-title">Commander Spellbook combinations</h3>
                <p>{comboStatus?.ready ? "A searchable catalog is installed locally." : "Install a local index of documented combo variants."}</p>
              </div>
              <span className={`package-state ${comboStatus?.ready ? "ready" : ""}`}>{comboStatus ? (comboStatus.ready ? "Installed" : "Not installed") : "Checking"}</span>
            </div>

            {comboStatus?.ready && (
              <dl className="combo-catalog-details">
                <div><dt>Variants</dt><dd>{comboStatus.variantCount.toLocaleString()}</dd></div>
                <div><dt>Catalog version</dt><dd>{comboStatus.upstreamVersion ?? "Not reported"}</dd></div>
                <div><dt>Published</dt><dd>{formatTimestamp(comboStatus.upstreamTimestamp)}</dd></div>
                <div><dt>Installed</dt><dd>{formatTimestamp(comboStatus.installedAt)}</dd></div>
                <div><dt>Downloaded</dt><dd>{formatBytes(comboStatus.compressedBytes)}</dd></div>
                <div><dt>Decoded snapshot</dt><dd>{formatBytes(comboStatus.decompressedBytes)}</dd></div>
              </dl>
            )}

            {comboStatus?.snapshotSha256 && (
              <div className="catalog-digest">
                <span>Local snapshot SHA-256</span>
                <code title={comboStatus.snapshotSha256}>{shortHash(comboStatus.snapshotSha256)}</code>
              </div>
            )}

            {updatingCombos && comboProgress && (
              <div className="update-progress-card combo-progress" role="status" aria-live="polite">
                <div><span>{capitalize(comboProgress.phase)}</span><strong>{Math.round(comboProgress.progress * 100)}%</strong></div>
                <div className="bar-track large" role="progressbar" aria-label="Commander Spellbook catalog update" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(comboProgress.progress * 100)}><div className="bar-fill primary" style={{ width: `${comboProgress.progress * 100}%` }} /></div>
                <p>{comboProgress.detail}</p>
              </div>
            )}

            {comboError && <div className="catalog-message error" role="alert"><Info size={14} /><span>{comboError}</span></div>}
            {comboNotice && !comboError && <div className="catalog-message success" role="status"><CheckCircle2 size={14} /><span>{comboNotice}</span></div>}

            <div className="catalog-disclosure">
              <ShieldCheck size={15} />
              <p>
                This downloads Commander Spellbook’s public bulk snapshot from its exact allowlisted HTTPS host and builds a local SQLite index. No decklist or card names are sent. The publisher does not currently provide a signed manifest or checksum, so authenticity relies on HTTPS transport; the app records source metadata and its own SHA-256 for reproducibility. The expanded source is streamed and not retained, but the local index can use substantial disk space.
              </p>
            </div>
            {comboStatus?.ready && <p className="catalog-authenticity"><strong>Recorded provenance:</strong> {comboStatus.authenticityBasis}</p>}
          </section>

          {updateError && <div className="catalog-message error" role="alert"><Info size={14} /><span>{updateError}</span></div>}
          {updateNotice && !updateError && <div className="catalog-message success" role="status"><CheckCircle2 size={14} /><span>{updateNotice}</span></div>}
          {updateCheck && (
            <p className="catalog-authenticity">
              <strong>Last checked:</strong> {formatTimestamp(updateCheck.checkedAt)} · Scryfall Oracle cards,
              Commander Spellbook, and official Comprehensive Rules
            </p>
          )}
          <div className="panel-note"><ShieldCheck size={17} /><p>Checks only the three remotely updateable knowledge sources above. Policy and semantic packages remain explicit local imports; simulation and bracket-model updates ship with the app. Downloads begin only after you confirm, and each update is activated only after validation.</p></div>
        </div>
        <div className="panel-footer">
          <button
            className="compact-primary full"
            onClick={onCheckUpdates}
            disabled={checkingUpdates || installingKnowledge}
            type="button"
          >
            {checkingUpdates || installingKnowledge
              ? <LoaderCircle className="spin" size={17} />
              : <RefreshCw size={17} />}
            {checkingUpdates
              ? "Checking for updates…"
              : installingKnowledge
                ? "Installing confirmed updates…"
                : "Check for updates"}
          </button>
        </div>
      </aside>
    </div>
  );
}

function dataStatusLabel(status: DataStatus | null) {
  if (!status) return "Checking card data";
  switch (status.state) {
    case "ready": return "Full card snapshot installed";
    case "partial": return "Card data partial";
    case "offline": return "Offline · local data";
    case "updating": return "Updating card data";
    default: return "Card data setup";
  }
}

function comprehensiveRulesCompatibilityLabel(status: ComprehensiveRulesStatus | null) {
  if (!status) return "Checking";
  if (!status.ready || status.compatibility === "notInstalled") return "Not installed";
  switch (status.compatibility) {
    case "compatible": return "Compatible";
    case "changed": return "Review needed";
    case "referenceOnly": return "Reference only";
    default: return "Installed";
  }
}

function policyOriginLabel(status: PolicyPackageStatus | null) {
  if (!status) return "Checking";
  switch (status.origin) {
    case "localImport": return "Local import";
    case "bundledFallback": return "Safe fallback";
    default: return "Bundled";
  }
}

function semanticOriginLabel(status: SemanticPackageStatus | null) {
  if (!status) return "Checking";
  switch (status.origin) {
    case "localImport": return "Local import";
    case "bundledFallback": return "Safe fallback";
    default: return "Bundled";
  }
}

function formatPolicyDate(value?: string | null) {
  if (!value) return "Not reported";
  const parsed = new Date(`${value}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(parsed);
}

function fileStem(path: string) {
  const name = path.split(/[\\/]/).pop() ?? "";
  return name.replace(/\.[^.]+$/, "");
}

function formatTimestamp(value?: string | null) {
  if (!value) return "Not reported";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsed);
}

function formatBytes(value?: number | null) {
  if (value === undefined || value === null) return "Not reported";
  const units = ["B", "KB", "MB", "GB"];
  let amount = value;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount.toLocaleString(undefined, { maximumFractionDigits: unit === 0 ? 0 : 1 })} ${units[unit]}`;
}

function shortHash(value: string) {
  return value.length > 24 ? `${value.slice(0, 12)}…${value.slice(-12)}` : value;
}

function readError(reason: unknown) {
  if (typeof reason === "string") return reason;
  if (reason instanceof Error) return reason.message;
  return "Something went wrong. Try again or inspect the decklist.";
}

const capitalize = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);

export default App;
