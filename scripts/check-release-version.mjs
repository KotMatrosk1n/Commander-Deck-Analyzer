import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const requested = process.argv[2];
if (!requested) {
  console.error("Usage: node scripts/check-release-version.mjs vX.Y.Z");
  process.exit(1);
}

const expected = requested.startsWith("v") ? requested.slice(1) : requested;
if (!/^\d+\.\d+\.\d+$/.test(expected)) {
  console.error("Release version must use X.Y.Z.");
  process.exit(1);
}

const root = process.cwd();
const read = (relativePath) =>
  fs.readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

function check(label, actual) {
  if (actual !== expected) {
    failures.push(`${label}: expected ${expected}, found ${actual ?? "nothing"}`);
  }
}

function matchVersion(label, text, pattern) {
  const match = text.match(pattern);
  if (!match) {
    failures.push(`${label}: version field was not found`);
    return;
  }
  check(label, match[1]);
}

const packageJson = JSON.parse(read("apps/desktop/package.json"));
const packageLock = JSON.parse(read("apps/desktop/package-lock.json"));
const tauriConfig = JSON.parse(
  read("apps/desktop/src-tauri/tauri.conf.json"),
);

check("apps/desktop/package.json", packageJson.version);
check("apps/desktop/package-lock.json root", packageLock.version);
check(
  "apps/desktop/package-lock.json workspace",
  packageLock.packages?.[""]?.version,
);
check("apps/desktop/src-tauri/tauri.conf.json", tauriConfig.version);

matchVersion(
  "apps/desktop/src-tauri/Cargo.toml",
  read("apps/desktop/src-tauri/Cargo.toml"),
  /^\[package\]\r?\n(?:[^\r\n]*\r?\n)*?version\s*=\s*"([^"]+)"/m,
);
matchVersion(
  "apps/desktop/src-tauri/Cargo.lock",
  read("apps/desktop/src-tauri/Cargo.lock"),
  /\[\[package\]\]\r?\nname = "commanderdeckanalyzer"\r?\nversion = "([^"]+)"/,
);
matchVersion(
  ".github/ISSUE_TEMPLATE/bug_report.yml",
  read(".github/ISSUE_TEMPLATE/bug_report.yml"),
  /placeholder:\s*(\d+\.\d+\.\d+)/,
);

const notesPath = `docs/releases/${expected}.md`;
if (!fs.existsSync(path.join(root, notesPath))) {
  failures.push(`${notesPath}: release notes were not found`);
} else {
  const notes = read(notesPath);
  const repository =
    "https://github.com/KotMatrosk1n/Commander-Deck-Analyzer";
  const firstReleaseUrl = `${repository}/commits/v${expected}`;
  const comparisonUrl = new RegExp(
    `${repository.replaceAll("/", "\\/")}\\/compare\\/v\\d+\\.\\d+\\.\\d+\\.\\.\\.v${expected.replaceAll(".", "\\.")}`,
  );
  const validChangelog =
    expected === "0.0.1"
      ? notes.includes(firstReleaseUrl)
      : comparisonUrl.test(notes);
  if (!validChangelog) {
    failures.push(
      `${notesPath}: full changelog link does not match the release history`,
    );
  }
}

if (failures.length > 0) {
  console.error("Release version check failed:");
  for (const failure of failures) {
    console.error(`* ${failure}`);
  }
  process.exit(1);
}

console.log(`Release version ${expected} is consistent.`);
