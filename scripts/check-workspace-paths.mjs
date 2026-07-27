import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const tracked = execFileSync("git", ["ls-files", "-z"], {
  encoding: "utf8",
})
  .split("\0")
  .filter(Boolean);

const allowedRoots = new Set([
  ".github",
  "apps",
  "docs",
  "scripts",
]);

const allowedRootFiles = new Set([
  ".gitignore",
  "CODE_OF_CONDUCT.md",
  "CONTRIBUTING.md",
  "CONTRIBUTORS.md",
  "LICENSE",
  "README.md",
  "SECURITY.md",
  "THIRD_PARTY_NOTICES.md",
]);

const blockedPathPatterns = [
  /(^|\/)tests?(\/|$)/i,
  /(^|\/)fixtures?(\/|$)/i,
  /(^|\/)snapshots?(\/|$)/i,
  /(^|\/)(node_modules|dist|build|out|target|coverage)(\/|$)/i,
  /(^|\/)\.env(?:\.|$)/i,
  /\.(?:test|spec)\.[^/]+$/i,
  /\.(?:snap|sqlite|sqlite-shm|sqlite-wal|db)$/i,
  /\.(?:key|pem|pfx|p12)$/i,
  /\.(?:dll|dylib|exe|msi|pdb|so|app|deb|rpm)$/i,
  /\.(?:zip|7z|rar)$/i,
];

const violations = [];

for (const path of tracked) {
  const [root] = path.split("/");
  if (!allowedRoots.has(root) && !allowedRootFiles.has(path)) {
    violations.push(`${path}: unexpected repository root`);
  }
  if (blockedPathPatterns.some((pattern) => pattern.test(path))) {
    violations.push(`${path}: blocked tracked path`);
  }
}

const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".jsx",
  ".md",
  ".mjs",
  ".ps1",
  ".rs",
  ".svg",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yml",
  ".yaml",
]);
const blockedPunctuation = new Set([0x2013, 0x2014]);

for (const path of tracked) {
  const extension = path.includes(".") ? `.${path.split(".").pop().toLowerCase()}` : "";
  if (!textExtensions.has(extension) && !allowedRootFiles.has(path)) continue;
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    if ([...line].some((character) => blockedPunctuation.has(character.codePointAt(0)))) {
      violations.push(`${path}:${index + 1}: blocked punctuation`);
    }
  });
}

const rustSource = tracked.filter((path) => path.endsWith(".rs"));
if (rustSource.length > 0) {
  try {
    const rustMatches = execFileSync(
      "git",
      ["grep", "-I", "-n", "-E", "#\\[cfg\\(test\\)\\]|#\\[(tokio::)?test\\]", "--", ...rustSource],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
    if (rustMatches.trim()) {
      violations.push(...rustMatches.trim().split(/\r?\n/).map((line) => `${line}: blocked source marker`));
    }
  } catch (error) {
    if (error.status !== 1) throw error;
  }
}

const sensitivePatterns = [
  "[A-Z]:\\\\Users\\\\",
  ["/", "Users", "/"].join(""),
  ["/", "home", "/"].join(""),
  "-----BEGIN [A-Z ]*PRIVATE KEY-----",
  "github_pat_[A-Za-z0-9_]+",
  "gh[pousr]_[A-Za-z0-9]{20,}",
  "AKIA[0-9A-Z]{16}",
];

for (const pattern of sensitivePatterns) {
  try {
    const matches = execFileSync(
      "git",
      ["grep", "-I", "-n", "-E", "-e", pattern, "--", ...tracked],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
    if (matches.trim()) {
      violations.push(...matches.trim().split(/\r?\n/).map((line) => `${line}: blocked content`));
    }
  } catch (error) {
    if (error.status !== 1) throw error;
  }
}

if (violations.length > 0) {
  console.error("Repository path check failed:");
  for (const violation of violations) {
    console.error(`- ${violation}`);
  }
  process.exit(1);
}

console.log(`Repository path check passed for ${tracked.length} tracked files.`);
