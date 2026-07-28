export type IssueSeverity = "info" | "warning" | "error";

export interface DeckEntry {
  quantity: number;
  name: string;
  lineNumber: number;
  isCommander: boolean;
}

export interface DeckIssue {
  severity: IssueSeverity;
  code: string;
  message: string;
  lineNumber?: number;
  cardName?: string;
}

export interface DeckParseResult {
  entries: DeckEntry[];
  cardCount: number;
  uniqueCardCount: number;
  ignoredLineCount: number;
  commanders: string[];
  issues: DeckIssue[];
  canonicalText: string;
  isCommanderSized: boolean;
}

export type AnalysisTrialCount = 1000 | 5000 | 10000;

/**
 * Strategy remains fixed while the user selects the deterministic work budget
 * and timing horizon for this run.
 */
export interface AnalysisOptions {
  openingHandSimulations: AnalysisTrialCount;
  gameSimulations: AnalysisTrialCount;
  maximumTurn: number;
  mulliganPolicy: "aggressive";
  pilotPolicy: "race";
  interactionProfile: "highPower";
  declaredIntent: "unspecified";
  allowOnlineCardResolution: boolean;
  seed?: number;
}

export type AnalysisStage =
  | "validating"
  | "resolvingCards"
  | "compiling"
  | "openingHands"
  | "goldfish"
  | "interference"
  | "scoring"
  | "complete";

export interface AnalysisProgress {
  runId: string;
  stage: AnalysisStage;
  stageLabel: string;
  completedUnits: number;
  totalUnits: number;
  overallProgress: number;
  detail: string;
}

export type ConfidenceLevel = "low" | "medium" | "high";
export type CalibrationStatus = "uncalibrated" | "empiricallyCalibrated";
export type SimulationFidelity =
  | "strictExecutable"
  | "legacyHeuristic"
  | "blockedUnsupported";
export type LegalityStatus = "legal" | "illegal" | "unknown";

export interface AnalysisReport {
  runId: string;
  deck: {
    cardCount: number;
    uniqueCardCount: number;
    commanders: string[];
    resolvedCards: number;
    unresolvedCards: string[];
    canonicalDeck: string;
    canonicalDeckSha256: string;
  };
  recommendation: {
    likelyBracket: number;
    rangeLow: number;
    rangeHigh: number;
    confidence: ConfidenceLevel;
    rulesFloor?: number;
    probabilities: { bracket: number; probability: number }[];
    summary: string;
    calibrationStatus: CalibrationStatus;
  };
  overview: {
    manaScore: number;
    consistencyScore: number;
    speedScore: number;
    speedScoreBasis?:
      | "recognizedWinAttempt"
      | "genericConversionMilestone"
      | "proactiveDevelopment"
      | "credibleThreat"
      | "structuralPace";
    interactionScore: number;
    synergyScore: number;
    resilienceScore: number;
    commanderOnCurveRate: number;
    primaryPlanAccessRate: number;
  };
  openingHands: {
    simulations: number;
    candidateCohortVersion: string;
    candidateCohortSha256: string;
    keepableSevenRate: number;
    keepableAfterMulligansRate: number;
    averageMulligans: number;
    averageCardsKept: number;
    twoLandRate: number;
    threeLandByTurnThreeRate: number;
    rampAccessRate: number;
    engineAccessRate: number;
    confidenceMargin: number;
    policyFidelity: SimulationFidelity;
    mana: {
      reliabilityBand: "unknown" | "fragile" | "mixed" | "supported";
      reliabilityScore: number;
      modelConfidence: number;
      averageOpeningColorCoverage: number;
      averageTurnThreeColorCoverage: number;
      landSourceCount: number;
      nonlandSourceCount: number;
      conditionalSourceCount: number;
      unknownSourceCount: number;
      entersTappedLandCount: number;
      colors: {
        color: "W" | "U" | "B" | "R" | "G" | "C";
        exactSources: number;
        conditionalSources: number;
        tappedSources: number;
        weightedSourceEquivalents: number;
        demandPipAppearances: number;
      }[];
      notes: string[];
    };
  };
  winSpeed: {
    simulations: number;
    fidelity: SimulationFidelity;
    fidelityMessage: string;
    coverageManifestSha256?: string | null;
    /**
     * Exact endpoint contract. Reports without the current value are legacy
     * and must not have attempt timing relabeled as a resolved table win.
     */
    timingEndpointVersion?: string | null;
    /** First modeled turn the deck can present a credible threat. */
    baseline: TurnDistribution;
    interfered: TurnDistribution;
    /** First modeled turn the deck presents a win attempt if unanswered. */
    baselineWinAttempt: TurnDistribution;
    interferedWinAttempt: TurnDistribution;
    baselineModelPace?: TurnDistribution;
    interferedModelPace?: TurnDistribution;
    /** First turn a typed execution path proves a table-lethal sequence resolved. */
    baselineResolvedTableWin?: TurnDistribution | null;
    interferedResolvedTableWin?: TurnDistribution | null;
    medianDelay: number | null;
    winAttemptMedianDelay: number | null;
    resolvedTableWinMedianDelay?: number | null;
    firstAttemptStoppedRate: number;
    cumulativeThreatRate: { turn: number; rate: number }[];
    cumulativeInterferedThreatRate: { turn: number; rate: number }[];
    cumulativeWinAttemptRate: { turn: number; rate: number }[];
    cumulativeInterferedWinAttemptRate: { turn: number; rate: number }[];
    /** Broad engine/combat density milestone; explicitly not a win attempt. */
    baselineGenericConversionMilestone?: TurnDistribution;
    interferedGenericConversionMilestone?: TurnDistribution;
    cumulativeGenericConversionMilestoneRate?: { turn: number; rate: number }[];
    cumulativeInterferedGenericConversionMilestoneRate?: { turn: number; rate: number }[];
    attemptProvenance?: AttemptProvenanceReport;
    earlyTurnEvaluation?: EarlyTurnEvaluationReport | null;
    cumulativeResolvedTableWinRate?: { turn: number; rate: number }[] | null;
    cumulativeInterferedResolvedTableWinRate?: { turn: number; rate: number }[] | null;
    pairedThreatDelay: PairedTurnDelayReport;
    pairedWinAttemptDelay: PairedTurnDelayReport;
    pairedResolvedTableWinDelay?: PairedTurnDelayReport | null;
    interactionScenarios?: CompactInteractionScenarioReport[];
    firstAttemptOpportunities: number;
    stressTests: {
      name: string;
      outcome: string;
      severity: IssueSeverity;
    }[];
    recoveryOpportunities: number;
    recoveredAttempts: number;
    recoveryByMaxTurnRate?: number | null;
  };
  synergy: {
    detectedPlans: {
      name: string;
      confidence: number;
      supportingCards: string[];
    }[];
    knownLines: {
      name: string;
      cards: string[];
      compactness: number;
      isInfinite: boolean;
      tableLethalIfResolved: boolean;
      outcome: "tableWin" | "infiniteMana" | "infiniteEngine" | "engine";
      manaNeeded?: string;
      prerequisites: string[];
      modelConfidence: number;
    }[];
    roleCounts: { role: string; count: number }[];
    strategicProfile?: {
      modelVersion: string;
      usePolicy: {
        disposition: "reportOnly";
        affectsBracketRating: false;
        affectsSimulation: false;
        assertsPlayerIntent: false;
      };
      primaryPosture: "turbo" | "proactive" | "adaptive" | "reactive" | "attrition";
      postureRanking: {
        posture: "turbo" | "proactive" | "adaptive" | "reactive" | "attrition";
        score: number;
        evidence: string;
      }[];
      primaryArchetype:
        | "turboCombo"
        | "midrangeCombo"
        | "bigManaCombo"
        | "reactiveToolboxCombo"
        | "staxCombo"
        | "engineCombo"
        | "proactiveMidrange"
        | "reactiveControl";
      archetypeRanking: {
        archetype:
          | "turboCombo"
          | "midrangeCombo"
          | "bigManaCombo"
          | "reactiveToolboxCombo"
          | "staxCombo"
          | "engineCombo"
          | "proactiveMidrange"
          | "reactiveControl";
        score: number;
        evidence: string;
      }[];
      comboFamilyRanking: {
        family:
          | "compactTableWin"
          | "graveyardRecursion"
          | "spellChain"
          | "permanentEngine"
          | "infiniteResource"
          | "bigManaPayoff"
          | "combatConversion"
          | "tutorToolbox";
        score: number;
        supportingLineCount: number;
        evidence: string;
      }[];
      comboRouteClusters?: {
        routeId: string;
        rank: "primary" | "backup";
        score: number;
        lineCount: number;
        lineNames: string[];
        centralCards: {
          name: string;
          lineCount: number;
          appearsInEveryLine: boolean;
        }[];
        uniqueCards: string[];
        outcomes: ("tableWin" | "infiniteMana" | "infiniteEngine" | "engine")[];
        bestConfidence: number;
        tableLethalLineCount: number;
        conversionRequiredLineCount: number;
        conversion:
          | "tableLethal"
          | "mixedTableLethalAndConversion"
          | "requiresConversion";
        hasReportOnlyRequirements: boolean;
        reportOnlyLineCount: number;
      }[];
      evidence: {
        totalCards: number;
        commanderSlots: number;
        landSlots: number;
        nonlandSlots: number;
        fastManaSlots: number;
        rampSlots: number;
        tutorSlots: number;
        drawSlots: number;
        engineSlots: number;
        interactionSlots: number;
        protectionSlots: number;
        staxSlots: number;
        payoffSlots: number;
        comboPieceSlots: number;
        graveyardSlots: number;
        recursionSlots: number;
        commanderEngineSlots: number;
        commanderTutorSlots: number;
        knownLineCount: number;
        lethalLineCount: number;
        compactLethalLineCount: number;
        reportOnlyLineCount: number;
        averageNonlandManaValue: number;
        meanCardSemanticConfidence: number;
        semanticCoverage: number;
      };
      confidence: number;
      limitations: string[];
    } | null;
    graph?: {
      modelVersion: string;
      abilityModelVersion: string;
      nodeCount: number;
      connectedCardCount: number;
      edgeCount: number;
      displayedEdgeCount: number;
      graphCoverage: number;
      unsupportedClauseCount: number;
      resources: {
        resource: string;
        producerCount: number;
        consumerCount: number;
      }[];
      links: {
        sourceCard: string;
        targetCard: string;
        relation: "provides" | "triggers" | "knownCombination";
        resource: string;
        confidence: number;
        evidence: string;
      }[];
      commanderLinks: {
        sourceCard: string;
        targetCard: string;
        relation: "provides" | "triggers" | "knownCombination";
        resource: string;
        confidence: number;
        evidence: string;
      }[];
    };
    commanderDependence: number;
    cohesionScore: number;
    orphanedCards: string[];
  };
  coverage: {
    identityResolution: number;
    semanticCoverage: number;
    simulationCoverage: number;
    approximatedCards: string[];
    unresolvedCards: string[];
    notes: string[];
    executionManifest?: {
      schemaVersion: string;
      compilerVersion: string;
      provenance: {
        cardSnapshotSha256?: string | null;
        comprehensiveRulesSnapshotSha256?: string | null;
        comprehensiveRulesEffectiveDate?: string | null;
      };
      fingerprintSha256: string;
      projectionSha256: string;
      summary: {
        cardCount: number;
        faceCount: number;
        oracleSpanCount: number;
        leafCount: number;
      };
      gates: {
        metric:
          | "rawOpeningComposition"
          | "functionalMulligan"
          | "manaConsistency"
          | "goldfishTiming"
          | "interferenceTiming"
          | "synergyDescription"
          | "bracketRating";
        state: "executable" | "reportOnly" | "blocked";
        fullyExecutableLeafCount: number;
        safelyIrrelevantLeafCount: number;
        reportOnlyLeafCount: number;
        blockingLeafCount: number;
        blockers: {
          cardId: string;
          cardName: string;
          faceIndex?: number | null;
          leafId: string;
          blocker: {
            blockerCode: string;
            detail: string;
          };
        }[];
        /** The blocker array is a deterministic sample; counts above remain exact. */
        blockerSampleTruncated: boolean;
        blockerSampleLimit: number;
      }[];
    } | null;
  };
  evidence: {
    direction: "raises" | "lowers" | "neutral";
    title: string;
    detail: string;
    weight: number;
  }[];
  policy: {
    packageVersion: string;
    effectiveDate: string;
    legality: LegalityStatus;
    deckCardCount: number;
    formatViolations: {
      code: string;
      cardName?: string;
      message: string;
    }[];
    colorIdentityViolations: {
      cardName: string;
      cardColors: string[];
      commanderColors: string[];
    }[];
    duplicateViolations: {
      cardName: string;
      quantity: number;
    }[];
    commanderEligibility: {
      cardName: string;
      status: LegalityStatus;
      reason: string;
    }[];
    unresolvedCards: string[];
    gameChangerCount: number;
    gameChangers: string[];
    policyFloor?: number;
    policyFloorReason: string;
    bracketSignals: {
      code: string;
      kind: "deterministicFloor" | "modeledGuidance" | "manualReview";
      recommendedFloor?: number;
      title: string;
      detail: string;
      cards: string[];
      sourceUrls: string[];
    }[];
    intentAssessments: {
      bracket: number;
      status: "unknown";
      inferred: boolean;
      reason: string;
    }[];
    manualReviewReasons: string[];
  };
  assumptions: {
    openingHandSimulations: AnalysisTrialCount;
    gameSimulations: AnalysisTrialCount;
    maximumTurn: number;
    mulliganPolicy: "aggressive";
    pilotPolicy: "race";
    interactionProfile: "highPower";
    declaredIntent: "unspecified";
    allowOnlineCardResolution: boolean;
    seed: number;
    /** Exact decimal u64; prefer this over the compatibility number in displays/exports. */
    seedExact?: string | null;
  };
  versions: {
    cardData: string;
    cardSnapshotSha256?: string;
    rulesPackage: string;
    rulesSnapshotSha256?: string;
    rulesPackageOrigin?: string;
    semanticModel: string;
    semanticPackage?: string;
    semanticSnapshotSha256?: string;
    semanticPackageOrigin?: string;
    semanticImportedAt?: string;
    semanticAuthenticityBasis?: string;
    comprehensiveRulesEffectiveDate?: string;
    comprehensiveRulesSnapshotSha256?: string;
    comprehensiveRulesParserVersion?: string;
    ruleCapabilityModel?: string;
    strategicProfileModel?: string;
    simulationEngine: string;
    effectiveHandStrengthModel?: string;
    abilityProgram?: string;
    turnPlanner?: string;
    strictEngine?: string;
    executionCoverageCompiler?: string;
    bracketModel: string;
    comboCatalog?: string;
    comboSnapshotSha256?: string;
  };
  cache: {
    hit: boolean;
    createdAt?: string;
    keyVersion: string;
  };
  elapsedMs: number;
}

export interface TurnDistribution {
  /**
   * Population quantiles across every episode. A value is null when the
   * right-censored episodes prevent that quantile from being identified.
   */
  median?: number | null;
  p10?: number | null;
  p90?: number | null;
  /** Successful-episode-only diagnostics; never use these as expected timing. */
  conditionalMedian?: number | null;
  conditionalP10?: number | null;
  conditionalP90?: number | null;
  demonstratedRate: number;
  rightCensoredRate?: number;
}

export interface PairedTurnDelayReport {
  observedPairs: number;
  preventedByTurnCap: number;
  baselineNotDemonstrated: number;
  stressedOnly: number;
  median?: number | null;
  p10?: number | null;
  p90?: number | null;
}

export type GenericMilestoneKind = "engine" | "combat" | "engineAndCombat";
export type TimingSampleKind = "baseline" | "interfered";
export type ExplicitAttemptBlockerReason =
  | "noRecognizedExplicitRoute"
  | "missingNamedPieces"
  | "namedPiecesNotUsableTogether"
  | "insufficientNamedCardMana"
  | "unsupportedRequirement"
  | "unmetPrerequisite"
  | "unsupportedActivationCost"
  | "insufficientActivationMana"
  | "deferredAfterStoppedAttempt"
  | "readyButNotSelected";

export interface AttemptProvenanceReport {
  explicitRoutes: {
    routeId: string;
    name: string;
    cards: string[];
    prerequisites: string[];
    modelConfidence: number;
    baselineAttempts: number;
    interferedAttempts: number;
    baselineRate: number;
    interferedRate: number;
    baselineFirstAttempt: TurnDistribution;
    interferedFirstAttempt: TurnDistribution;
    cumulativeBaselineAttemptRate: { turn: number; rate: number }[];
    cumulativeInterferedAttemptRate: { turn: number; rate: number }[];
  }[];
  genericMilestoneKinds: {
    kind: GenericMilestoneKind;
    baselineEpisodes: number;
    interferedEpisodes: number;
    baselineRate: number;
    interferedRate: number;
  }[];
  earlyFailureHorizon: number;
  earlyTurnBlockers: {
    sample: TimingSampleKind;
    turn: number;
    routeId?: string | null;
    routeName?: string | null;
    blockedCard?: string | null;
    reason: ExplicitAttemptBlockerReason;
    episodes: number;
    rate: number;
  }[];
}

export type RouteModelingCeiling =
  | "unavailable"
  | "routeSkeletonOnly"
  | "routeSkeletonWithScalarResourceFloor";

export type EarlyTurnBlockerCategory =
  | "noExplicitTableWinRoute"
  | "routeTooLarge"
  | "missingRouteCard"
  | "insufficientRouteCopies"
  | "directPieceAccess"
  | "unresolvedPieceAccess"
  | "tutorPaymentOrTiming"
  | "tutorShapeUnsupported"
  | "manaDemandUnresolved"
  | "scalarManaCapacity"
  | "conditionalManaDependency"
  | "coloredPaymentUnverified"
  | "commandZoneCastUnverified"
  | "zoneOrSequenceUnverified"
  | "prerequisiteExecutionUnverified"
  | "unsupportedCardFunction"
  | "inconsistentRouteMetadata";

export interface EarlyTurnEvaluationReport {
  modelVersion: string;
  librarySize: number;
  knownLineCount: number;
  eligibleTableWinRouteCount: number;
  omittedNonTableWinLineCount: number;
  fixedPolicy: {
    openingHandSize: number;
    naturalDrawsBeforeTurnOne: number;
    naturalDrawsBeforeTurnTwo: number;
    aggressiveCandidateHands: number;
  };
  routes: {
    routeId: string;
    routeName: string;
    outcome: "tableWin" | "infiniteMana" | "infiniteEngine" | "engine";
    tableLethalIfResolved: boolean;
    modelConfidence: number;
    pieces: {
      card: string;
      normalizedCard: string;
      requiredCopies: number;
      commandZoneCopies: number;
      requiredLibraryCopies: number;
      availableLibraryCopies: number;
    }[];
    manaDemand?: {
      amount: number;
      includesColoredOrColorlessPips: boolean;
      basis: string;
      exactPrintedCostCoverage: boolean;
    } | null;
    modelingCeiling: RouteModelingCeiling;
    aggressiveMulligan: {
      candidateHands: number;
      directSkeletonInAtLeastOneCandidate: number;
      typedTutorSkeletonInAtLeastOneCandidate: number;
      scalarFloorSkeletonInAtLeastOneCandidate?: number | null;
      caveat: string;
    };
    turns: {
      turn: number;
      visibleLibraryCards: number;
      totalCombinations: string;
      directSkeletonCombinations: string;
      typedTutorSkeletonCombinations: string;
      strictScalarFloorCombinations?: string | null;
      conditionalScalarCeilingCombinations?: string | null;
      directSkeletonProbability: number;
      typedTutorSkeletonProbability: number;
      strictScalarFloorProbability?: number | null;
      conditionalScalarCeilingProbability?: number | null;
      executableConversionProbability?: number | null;
      blockers: EarlyTurnBlocker[];
    }[];
    blockers: EarlyTurnBlocker[];
  }[];
  blockers: EarlyTurnBlocker[];
  notes: string[];
}

export interface EarlyTurnBlocker {
  category: EarlyTurnBlockerCategory;
  detail: string;
  affectedCards: string[];
  probabilityMass?: number | null;
  preventsExecutableClaim: boolean;
}

export type InteractionScenario =
  | "targetedPermanentRemoval"
  | "commanderRemovalRecast"
  | "firstRelevantSpellCountered"
  | "creatureWipe"
  | "graveyardShutdown"
  | "genericTaxStax"
  | "ruleOfLawCap"
  | "firstWinAttemptStopped";

export interface CompactInteractionScenarioReport {
  schemaVersion: string;
  directive: {
    directiveVersion: string;
    checkpointVersion: string;
    scenario: InteractionScenario;
    scenarioId: string;
    checkpoint: { kind: string; [key: string]: unknown };
    intervention: { kind: string; [key: string]: unknown };
    recoveryCheckpoint: string;
    selection: {
      occurrence: "first";
      tieBreakers: ("eventSequence" | "stableObjectId" | "stablePlayerId")[];
    };
  };
  measurement: {
    label: "response-pressure" | "strict-legal-action";
    executionSource:
      | { kind: "responsePressure" }
      | {
          kind: "strictLegalActionEngine";
          engineId: string;
          engineVersion: string;
          legalActionSchemaVersion: string;
        };
    claimBoundary: string;
  };
  sampling: {
    masterSeed: number;
    /** Exact decimal u64; prefer this over the compatibility number. */
    masterSeedExact?: string | null;
    seedDerivationVersion: string;
    episodeCount: number;
    maximumTurn: number;
  };
  applicability: {
    applicableEpisodes: number;
    notApplicableEpisodes: number;
    undeterminedEpisodes: number;
    primaryNotApplicableReason?:
      | "noEligibleNoncommanderPermanent"
      | "noCommanderSubject"
      | "noRelevantSpellClass"
      | "noRelevantCreatureBoardPlan"
      | "noGraveyardDependency"
      | "noTaxableActionClass"
      | "noMultispellPlan"
      | "noRepresentableWinAttempt"
      | null;
    distinctNotApplicableReasons: number;
    primaryUndeterminedReason?: string | null;
    distinctUndeterminedReasons: number;
  };
  counters: {
    totalEpisodes: number;
    applicableEpisodes: number;
    notApplicableEpisodes: number;
    undeterminedEpisodes: number;
    applicableWithoutOpportunityEpisodes: number;
    opportunityEpisodes: number;
    checkpointEvents: number;
    opportunityEvents: number;
    directiveAttemptEvents: number;
    directiveAppliedEvents: number;
    directiveRejectedEvents: number;
    directiveNoOpEvents: number;
    affectedGameEvents: number;
    effectfulInterventionEpisodes: number;
  };
  credibleThreatDelay: CompactPairedDelayDistribution;
  firstWinAttemptDelay: CompactPairedDelayDistribution;
  resolvedTableWinDelay?: CompactPairedDelayDistribution | null;
  recovery: {
    opportunities: number;
    recovered: number;
    rightCensored: number;
    recoveredByTurnCapRate?: number | null;
    observedRecoveryP10Turn?: number | null;
    observedRecoveryMedianTurn?: number | null;
    observedRecoveryP90Turn?: number | null;
  };
}

export interface CompactPairedDelayDistribution {
  metric: "credibleThreat" | "firstWinAttempt" | "resolvedTableWin";
  totalEpisodePairs: number;
  applicablePairs: number;
  effectfulPairs: number;
  observedPairs: number;
  rightCensoredPairs: number;
  noOpInvariantPairs: number;
  nonEstimablePairs: number;
  excludedPairs: number;
  observedDelayP10Turns?: number | null;
  observedDelayMedianTurns?: number | null;
  observedDelayP90Turns?: number | null;
  censoredBoundMinTurns?: number | null;
  censoredBoundMedianTurns?: number | null;
  censoredBoundMaxTurns?: number | null;
}

export type DataState = "ready" | "partial" | "empty" | "offline" | "updating";

export interface DataStatus {
  state: DataState;
  cardCount: number;
  lastUpdated?: string;
  source: string;
  message: string;
  snapshotSha256?: string;
}

export interface DataUpdateProgress {
  phase: string;
  completedUnits: number;
  totalUnits?: number;
  progress: number;
  detail: string;
}

export type KnowledgeUpdateId =
  | "cardData"
  | "comboData"
  | "comprehensiveRules";

export interface KnowledgeUpdateCheckItem {
  id: KnowledgeUpdateId;
  label: string;
  updateAvailable: boolean;
  installedVersion: string | null;
  availableVersion: string | null;
  detail: string;
  error: string | null;
}

export interface KnowledgeUpdateCheck {
  checkedAt: string;
  updateAvailable: boolean;
  items: KnowledgeUpdateCheckItem[];
}

export interface ComboStoreStatus {
  ready: boolean;
  schemaVersion: string;
  upstreamVersion: string | null;
  upstreamTimestamp: string | null;
  installedAt: string | null;
  etag: string | null;
  lastModified: string | null;
  snapshotSha256: string | null;
  compressedBytes: number | null;
  decompressedBytes: number | null;
  variantCount: number;
  aliasCount: number;
  authenticityBasis: string;
}

export interface ComboUpdateProgress {
  phase: string;
  completedUnits: number;
  totalUnits: number | null;
  progress: number;
  detail: string;
}

export type ComboUpdateOutcome =
  | { outcome: "installed"; status: ComboStoreStatus }
  | { outcome: "notModified"; status: ComboStoreStatus };

export type RulesCompatibility =
  | "compatible"
  | "changed"
  | "referenceOnly"
  | "notInstalled";

export interface ComprehensiveRulesStatus {
  ready: boolean;
  schemaVersion: string;
  parserVersion: string;
  effectiveDate: string | null;
  installedAt: string | null;
  sourcePageUrl: string;
  documentUrl: string | null;
  etag: string | null;
  lastModified: string | null;
  snapshotSha256: string | null;
  documentBytes: number | null;
  ruleCount: number;
  sectionCount: number;
  exampleCount: number;
  glossaryCount: number;
  commanderRuleCount: number;
  compatibility: RulesCompatibility;
  changedCapabilityRuleIds: string[];
  authenticityBasis: string;
  message: string;
}

export interface ComprehensiveRulesUpdateProgress {
  phase: string;
  completedUnits: number;
  totalUnits: number | null;
  progress: number;
  detail: string;
}

export type ComprehensiveRulesUpdateOutcome =
  | { outcome: "installed"; status: ComprehensiveRulesStatus }
  | { outcome: "notModified"; status: ComprehensiveRulesStatus };

export type PolicyPackageOrigin = "bundled" | "localImport" | "bundledFallback";

export interface PolicyPackageStatus {
  ready: boolean;
  origin: PolicyPackageOrigin;
  schemaVersion: number;
  packageVersion: string;
  effectiveDate: string;
  verifiedAt: string;
  policyStatus: string;
  snapshotSha256: string;
  importedAt?: string;
  sourceCount: number;
  bracketNoteCount: number;
  authenticityBasis: string;
  message: string;
}

export interface PolicyImportOutcome {
  activated: boolean;
  status: PolicyPackageStatus;
  message: string;
}

export type SemanticPackageOrigin = "bundled" | "localImport" | "bundledFallback";

export interface SemanticPackageStatus {
  ready: boolean;
  origin: SemanticPackageOrigin;
  schemaVersion: number;
  packageVersion: string;
  effectiveDate: string;
  verifiedAt: string;
  snapshotSha256: string;
  importedAt?: string;
  sourceCount: number;
  overrideCount: number;
  authenticityBasis: string;
  message: string;
}

export interface SemanticImportOutcome {
  activated: boolean;
  status: SemanticPackageStatus;
  message: string;
}

export interface ImportResult {
  provider: string;
  deckName?: string;
  commanders: string[];
  deckText: string;
  sourceUrl: string;
  importedAt: string;
  warnings: string[];
}

export type ReportTab =
  | "overview"
  | "hands"
  | "speed"
  | "synergy"
  | "method";
