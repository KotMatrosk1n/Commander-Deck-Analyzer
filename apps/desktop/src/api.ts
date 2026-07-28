import { Channel, invoke } from "@tauri-apps/api/core";
import { assertCurrentReportTimingSemantics } from "./reportCompatibility";
import type {
  AnalysisOptions,
  AnalysisProgress,
  AnalysisReport,
  ComboStoreStatus,
  ComboUpdateOutcome,
  ComboUpdateProgress,
  ComprehensiveRulesStatus,
  ComprehensiveRulesUpdateOutcome,
  ComprehensiveRulesUpdateProgress,
  DataStatus,
  DataUpdateProgress,
  DeckParseResult,
  ImportResult,
  KnowledgeUpdateCheck,
  PolicyImportOutcome,
  PolicyPackageStatus,
  SemanticImportOutcome,
  SemanticPackageStatus,
} from "./types";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

export async function openExternalCreditUrl(url: string): Promise<void> {
  if (!isTauri()) return;
  return invoke<void>("open_external_credit_url", { url });
}

export async function parseDeck(deckText: string): Promise<DeckParseResult> {
  if (isTauri()) {
    return invoke<DeckParseResult>("parse_decklist", { deckText });
  }
  return parseDeckFallback(deckText);
}

export async function importDeckUrl(url: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck_url", { url });
}

export async function readDeckFile(path: string): Promise<string> {
  return invoke<string>("read_deck_file", { path });
}

export async function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke<void>("write_text_file", { path, contents });
}

export async function getDataStatus(): Promise<DataStatus> {
  if (!isTauri()) {
    return {
      state: "partial",
      cardCount: 1847,
      source: "Scryfall API cache",
      message: "Browser preview: native card data is unavailable.",
    };
  }
  return invoke<DataStatus>("get_data_status");
}

export async function checkForKnowledgeUpdates(): Promise<KnowledgeUpdateCheck> {
  if (!isTauri()) {
    return {
      checkedAt: new Date().toISOString(),
      updateAvailable: false,
      items: [
        {
          id: "cardData",
          label: "Oracle card definitions",
          updateAvailable: false,
          installedVersion: null,
          availableVersion: null,
          detail: "Update checks are available in the installed desktop app.",
          error: "Native update checking is unavailable in browser preview.",
        },
        {
          id: "comboData",
          label: "Commander Spellbook combinations",
          updateAvailable: false,
          installedVersion: null,
          availableVersion: null,
          detail: "Update checks are available in the installed desktop app.",
          error: "Native update checking is unavailable in browser preview.",
        },
        {
          id: "comprehensiveRules",
          label: "Comprehensive Rules",
          updateAvailable: false,
          installedVersion: null,
          availableVersion: null,
          detail: "Update checks are available in the installed desktop app.",
          error: "Native update checking is unavailable in browser preview.",
        },
      ],
    };
  }
  return invoke<KnowledgeUpdateCheck>("check_for_knowledge_updates");
}

export async function updateCardDatabase(
  onProgress: (progress: DataUpdateProgress) => void,
): Promise<DataStatus> {
  const channel = new Channel<DataUpdateProgress>();
  channel.onmessage = onProgress;
  return invoke<DataStatus>("update_card_database", { onProgress: channel });
}

export async function getComboDataStatus(): Promise<ComboStoreStatus> {
  if (!isTauri()) {
    return {
      ready: false,
      schemaVersion: "1",
      upstreamVersion: null,
      upstreamTimestamp: null,
      installedAt: null,
      etag: null,
      lastModified: null,
      snapshotSha256: null,
      compressedBytes: null,
      decompressedBytes: null,
      variantCount: 0,
      aliasCount: 0,
      authenticityBasis: "Browser preview: native combo data is unavailable.",
    };
  }
  return invoke<ComboStoreStatus>("get_combo_data_status");
}

export async function updateComboDatabase(
  onProgress: (progress: ComboUpdateProgress) => void,
): Promise<ComboUpdateOutcome> {
  if (!isTauri()) {
    throw new Error("Combo catalog updates are available in the installed desktop app.");
  }
  const channel = new Channel<ComboUpdateProgress>();
  channel.onmessage = onProgress;
  return invoke<ComboUpdateOutcome>("update_combo_database", { onProgress: channel });
}

export async function getComprehensiveRulesStatus(): Promise<ComprehensiveRulesStatus> {
  if (!isTauri()) {
    return {
      ready: false,
      schemaVersion: "1",
      parserVersion: "comprehensive-rules-parser-1",
      effectiveDate: null,
      installedAt: null,
      sourcePageUrl: "https://magic.wizards.com/en/rules",
      documentUrl: null,
      etag: null,
      lastModified: null,
      snapshotSha256: null,
      documentBytes: null,
      ruleCount: 0,
      sectionCount: 0,
      exampleCount: 0,
      glossaryCount: 0,
      commanderRuleCount: 0,
      compatibility: "notInstalled",
      changedCapabilityRuleIds: [],
      authenticityBasis: "Browser preview: native Comprehensive Rules data is unavailable.",
      message: "Install the official Comprehensive Rules in the desktop app.",
    };
  }
  return invoke<ComprehensiveRulesStatus>("get_comprehensive_rules_status");
}

export async function updateComprehensiveRules(
  onProgress: (progress: ComprehensiveRulesUpdateProgress) => void,
): Promise<ComprehensiveRulesUpdateOutcome> {
  if (!isTauri()) {
    throw new Error("Comprehensive Rules updates are available in the installed desktop app.");
  }
  const channel = new Channel<ComprehensiveRulesUpdateProgress>();
  channel.onmessage = onProgress;
  return invoke<ComprehensiveRulesUpdateOutcome>("update_comprehensive_rules", {
    onProgress: channel,
  });
}

export async function getPolicyPackageStatus(): Promise<PolicyPackageStatus> {
  if (!isTauri()) {
    return {
      ready: true,
      origin: "bundled",
      schemaVersion: 1,
      packageVersion: "2026.02.09-r2",
      effectiveDate: "2026-02-09",
      verifiedAt: "2026-07-23",
      policyStatus: "beta",
      snapshotSha256: "browser-preview",
      sourceCount: 9,
      bracketNoteCount: 7,
      authenticityBasis: "Browser preview: native policy provenance is unavailable.",
      message: "Browser preview of the policy package bundled with the desktop app.",
    };
  }
  return invoke<PolicyPackageStatus>("get_policy_package_status");
}

export async function importPolicyPackage(path: string): Promise<PolicyImportOutcome> {
  if (!isTauri()) {
    throw new Error("Policy package imports are available in the installed desktop app.");
  }
  return invoke<PolicyImportOutcome>("import_policy_package", { path });
}

export async function resetPolicyPackage(): Promise<PolicyImportOutcome> {
  if (!isTauri()) {
    throw new Error("Policy package reset is available in the installed desktop app.");
  }
  return invoke<PolicyImportOutcome>("reset_policy_package");
}

export async function getSemanticPackageStatus(): Promise<SemanticPackageStatus> {
  if (!isTauri()) {
    return {
      ready: true,
      origin: "bundled",
      schemaVersion: 1,
      packageVersion: "semantic-overrides-2026.07.23-r0",
      effectiveDate: "2026-07-23",
      verifiedAt: "2026-07-23",
      snapshotSha256: "browser-preview",
      sourceCount: 0,
      overrideCount: 0,
      authenticityBasis: "Browser preview: native semantic-package provenance is unavailable.",
      message: "Browser preview of the empty semantic-annotation fallback bundled with the desktop app.",
    };
  }
  return invoke<SemanticPackageStatus>("get_semantic_package_status");
}

export async function importSemanticPackage(path: string): Promise<SemanticImportOutcome> {
  if (!isTauri()) {
    throw new Error("Semantic package imports are available in the installed desktop app.");
  }
  return invoke<SemanticImportOutcome>("import_semantic_package", { path });
}

export async function resetSemanticPackage(): Promise<SemanticImportOutcome> {
  if (!isTauri()) {
    throw new Error("Semantic package reset is available in the installed desktop app.");
  }
  return invoke<SemanticImportOutcome>("reset_semantic_package");
}

export async function analyzeDeck(
  request: {
    runId: string;
    deckText: string;
    commanderNames: string[];
    options: AnalysisOptions;
  },
  onProgress: (progress: AnalysisProgress) => void,
  expectedCanonicalDeck = request.deckText,
): Promise<AnalysisReport> {
  const channel = new Channel<AnalysisProgress>();
  channel.onmessage = onProgress;
  const report = await invoke<AnalysisReport>("analyze_deck", {
    request,
    onProgress: channel,
  });
  assertCurrentReportTimingSemantics(report, {
    runId: request.runId,
    options: request.options,
    canonicalDeck: expectedCanonicalDeck,
    commanderNames: request.commanderNames,
  });
  return report;
}

export async function cancelAnalysis(runId: string): Promise<boolean> {
  return invoke<boolean>("cancel_analysis", { runId });
}

function parseDeckFallback(deckText: string): DeckParseResult {
  const ignored = new Set([
    "sideboard",
    "maybeboard",
    "considering",
    "tokens",
    "companion",
  ]);
  const included = new Set([
    "commander",
    "commanders",
    "deck",
    "decklist",
    "mainboard",
    "main deck",
    "creatures",
    "artifacts",
    "enchantments",
    "instants",
    "sorceries",
    "lands",
    "planeswalkers",
    "other",
  ]);
  let include = true;
  let commanderSection = false;
  let ignoredLineCount = 0;
  const entries: DeckParseResult["entries"] = [];

  deckText
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .forEach((raw, index) => {
      const line = raw.trim();
      if (!line) return;
      const heading = line
        .replace(/^(?:#{1,6}|\/\/)\s*/, "")
        .replace(/\s*[\[(]\s*\d+\s*[\])]\s*$/, "")
        .replace(/:$/, "")
        .trim()
        .toLowerCase();
      if (ignored.has(heading)) {
        include = false;
        commanderSection = false;
        ignoredLineCount += 1;
        return;
      }
      if (included.has(heading)) {
        include = true;
        commanderSection = heading.startsWith("commander");
        ignoredLineCount += 1;
        return;
      }
      if (!include || /^(#|\/\/|;|format:|cards:|total:)/i.test(line)) {
        ignoredLineCount += 1;
        return;
      }
      const match = line.match(/^(\d+)(?:\s*x\s*|\s+)(\S.*)$/i);
      const quantity = match ? Number(match[1]) : 1;
      const name = (match ? match[2] : line)
        .replace(/\s+(?:\([A-Z0-9]{2,8}\)|\[[A-Z0-9]{2,8}\])\s+[A-Z0-9-]+.*$/i, "")
        .trim();
      if (quantity > 0 && name) {
        entries.push({
          quantity,
          name,
          lineNumber: index + 1,
          isCommander: commanderSection,
        });
      }
    });

  const cardCount = entries.reduce((sum, entry) => sum + entry.quantity, 0);
  const commanders = entries
    .filter((entry) => entry.isCommander)
    .flatMap((entry) => Array(entry.quantity).fill(entry.name) as string[]);
  const issues: DeckParseResult["issues"] = [];
  if (cardCount !== 100 && cardCount > 0) {
    issues.push({
      severity: "error",
      code: "deck-size",
      message: `Commander decks normally contain exactly 100 cards; this list contains ${cardCount}.`,
    });
  }
  if (!commanders.length && cardCount > 0) {
    issues.push({
      severity: "warning",
      code: "commander-missing",
      message: "No Commander section was found. Select the commander before analysis.",
    });
  }
  return {
    entries,
    cardCount,
    uniqueCardCount: new Set(entries.map((entry) => entry.name.toLowerCase())).size,
    ignoredLineCount,
    commanders,
    issues,
    canonicalText: entries.map((entry) => `${entry.quantity} ${entry.name}`).join("\n"),
    isCommanderSized: cardCount === 100,
  };
}
