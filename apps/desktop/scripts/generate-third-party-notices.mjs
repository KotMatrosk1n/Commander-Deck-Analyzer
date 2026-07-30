import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const targetOptionIndex = process.argv.indexOf("--target");
const windowsTarget =
  targetOptionIndex >= 0
    ? process.argv[targetOptionIndex + 1]
    : "x86_64-pc-windows-msvc";

if (!/^(?:x86_64|aarch64)-pc-windows-msvc$/.test(windowsTarget ?? "")) {
  throw new Error(
    "Use --target x86_64-pc-windows-msvc or --target aarch64-pc-windows-msvc.",
  );
}

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const appDirectory = dirname(scriptDirectory);
const tauriDirectory = join(appDirectory, "src-tauri");
const cargoManifestPath = join(tauriDirectory, "Cargo.toml");
const cargoLockPath = join(tauriDirectory, "Cargo.lock");
const packageLockPath = join(appDirectory, "package-lock.json");
const outputPath = join(
  tauriDirectory,
  "resources",
  "OPEN_SOURCE_LICENSES.md",
);
const licenseFilePattern =
  /^(?:(?:licen[cs]e|copying|notices?|copyright)(?:s|[._-].*)?|unlicense)$/i;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function markdownCell(value) {
  return String(value ?? "not declared")
    .replaceAll("\\", "\\\\")
    .replaceAll("|", "\\|")
    .replaceAll("\r", " ")
    .replaceAll("\n", " ");
}

function packageKey(dependency) {
  return `${dependency.ecosystem}:${dependency.name}@${dependency.version}`;
}

function packageLicenseFiles(packageDirectory, explicitLicenseFile) {
  const candidates = [];
  if (existsSync(packageDirectory)) {
    for (const entry of readdirSync(packageDirectory)) {
      if (licenseFilePattern.test(entry)) {
        candidates.push(join(packageDirectory, entry));
      }
    }
  }

  if (explicitLicenseFile) {
    candidates.push(
      isAbsolute(explicitLicenseFile)
        ? explicitLicenseFile
        : resolve(packageDirectory, explicitLicenseFile),
    );
  }

  const packageRoot = resolve(packageDirectory);
  return [...new Set(candidates.map((candidate) => resolve(candidate)))]
    .filter((candidate) => {
      const contained =
        candidate === packageRoot ||
        candidate.startsWith(`${packageRoot}\\`) ||
        candidate.startsWith(`${packageRoot}/`);
      if (!contained) {
        throw new Error(
          `Refusing to read a license file outside its package: ${candidate}`,
        );
      }
      return existsSync(candidate) && statSync(candidate).isFile();
    })
    .sort((left, right) => left.localeCompare(right));
}

function cargoProductionDependencies() {
  const stdout = execFileSync(
    "cargo",
    [
      "metadata",
      "--manifest-path",
      cargoManifestPath,
      "--locked",
      "--offline",
      "--filter-platform",
      windowsTarget,
      "--format-version",
      "1",
    ],
    {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  const metadata = JSON.parse(stdout.replace(/^\uFEFF/, ""));
  const root = metadata.packages.find(
    (candidate) =>
      candidate.name === "commanderdeckanalyzer" &&
      resolve(candidate.manifest_path) === resolve(cargoManifestPath),
  );
  if (!root) {
    throw new Error("Could not identify the Commander Deck Analyzer package.");
  }

  const packagesById = new Map(
    metadata.packages.map((candidate) => [candidate.id, candidate]),
  );
  const nodesById = new Map(
    metadata.resolve.nodes.map((candidate) => [candidate.id, candidate]),
  );
  const reachable = new Set([root.id]);
  const pending = [root.id];

  while (pending.length > 0) {
    const packageId = pending.shift();
    const node = nodesById.get(packageId);
    if (!node) {
      throw new Error(`Cargo metadata omitted ${packageId}.`);
    }
    for (const dependency of node.deps) {
      const normalDependency = dependency.dep_kinds.some(
        (kind) => kind.kind === null,
      );
      if (normalDependency && !reachable.has(dependency.pkg)) {
        reachable.add(dependency.pkg);
        pending.push(dependency.pkg);
      }
    }
  }

  reachable.delete(root.id);
  return [...reachable].map((packageId) => {
    const dependency = packagesById.get(packageId);
    if (!dependency) {
      throw new Error(`Cargo metadata omitted package ${packageId}.`);
    }
    const packageDirectory = dirname(dependency.manifest_path);
    return {
      ecosystem: "Cargo",
      name: dependency.name,
      version: dependency.version,
      licenseExpression:
        dependency.license ??
        (dependency.license_file
          ? `license-file: ${dependency.license_file}`
          : null),
      files: packageLicenseFiles(packageDirectory, dependency.license_file),
    };
  });
}

function npmProductionDependencies() {
  const lock = JSON.parse(readFileSync(packageLockPath, "utf8"));
  if (!lock.packages || !lock.packages[""]) {
    throw new Error("package-lock.json does not contain a packages map.");
  }

  return Object.entries(lock.packages)
    .filter(
      ([packagePath, dependency]) =>
        packagePath !== "" &&
        dependency.dev !== true &&
        dependency.devOptional !== true,
    )
    .map(([packagePath, dependency]) => {
      const packageDirectory = resolve(appDirectory, packagePath);
      const manifestPath = join(packageDirectory, "package.json");
      if (!existsSync(manifestPath)) {
        throw new Error(
          `Run npm ci before generating notices. Missing ${relative(
            appDirectory,
            manifestPath,
          )}.`,
        );
      }
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      if (manifest.version !== dependency.version) {
        throw new Error(
          `${manifest.name}@${manifest.version} does not match the lockfile version ${dependency.version}.`,
        );
      }
      return {
        ecosystem: "npm",
        name: manifest.name,
        version: dependency.version,
        licenseExpression: manifest.license ?? dependency.license ?? null,
        files: packageLicenseFiles(packageDirectory, null),
      };
    });
}

function compareDependencies(left, right) {
  return (
    left.ecosystem.localeCompare(right.ecosystem) ||
    left.name.localeCompare(right.name) ||
    left.version.localeCompare(right.version)
  );
}

const cargoDependencies =
  cargoProductionDependencies().sort(compareDependencies);
const npmDependencies = npmProductionDependencies().sort(compareDependencies);
const dependencies = [...cargoDependencies, ...npmDependencies];
const textsByHash = new Map();

for (const dependency of dependencies) {
  dependency.licenseFiles = dependency.files.map((filePath) => {
    const bytes = readFileSync(filePath);
    const text = bytes.toString("utf8");
    if (text.includes("\uFFFD") || text.includes("\0")) {
      throw new Error(`License file is not plain UTF-8 text: ${filePath}`);
    }
    const hash = sha256(bytes);
    const file = {
      name: relative(dirname(filePath), filePath),
      hash,
    };
    const existing = textsByHash.get(hash);
    if (existing) {
      existing.packages.add(packageKey(dependency));
      existing.fileNames.add(file.name);
    } else {
      textsByHash.set(hash, {
        text,
        packages: new Set([packageKey(dependency)]),
        fileNames: new Set([file.name]),
      });
    }
    return file;
  });
}

const metadataOnly = dependencies.filter(
  (dependency) => dependency.licenseFiles.length === 0,
);
const missingLicenseMetadata = dependencies.filter(
  (dependency) => !dependency.licenseExpression,
);

const output = [];
output.push(
  "# Open Source Dependency Inventory and License Texts",
  "",
  "> Generated file. Regenerate it with `npm run notices` after a lockfile change.",
  "",
  "This inventory records package metadata and the license or notice files distributed with each included package. File hashes use the original package bytes. Unicode dash punctuation in the displayed copy is converted to ASCII so repository text remains consistent.",
  "",
  "## Generation Scope",
  "",
  `* Windows target: \`${windowsTarget}\`.`,
  `* Cargo.lock SHA 256: \`${sha256(readFileSync(cargoLockPath))}\`.`,
  `* package-lock.json SHA 256: \`${sha256(readFileSync(packageLockPath))}\`.`,
  `* Included ${cargoDependencies.length} Cargo packages and ${npmDependencies.length} npm packages, with ${textsByHash.size} distinct license or notice texts.`,
  "* The Cargo inventory follows normal dependency edges for the Windows target.",
  "* The npm inventory includes packages marked for production by the lockfile.",
  "",
  "## Items for Review",
  "",
);

if (missingLicenseMetadata.length === 0) {
  output.push("* Every included package declares license metadata.");
} else {
  output.push(
    `* Missing license metadata: ${missingLicenseMetadata
      .map(packageKey)
      .join(", ")}.`,
  );
}

if (metadataOnly.length === 0) {
  output.push("* Every included package provides a license or notice file.");
} else {
  output.push(
    `* ${metadataOnly.length} package entries provide metadata without a package license or notice file:`,
    "",
    ...metadataOnly.map(
      (dependency) =>
        `  * \`${packageKey(dependency)}\`: \`${dependency.licenseExpression ?? "not declared"}\``,
    ),
  );
}

output.push(
  "",
  "## Cargo Production Dependencies",
  "",
  "| Package | Version | Declared license | License or notice files |",
  "| --- | --- | --- | --- |",
);

for (const dependency of cargoDependencies) {
  output.push(
    `| \`${markdownCell(dependency.name)}\` | \`${markdownCell(
      dependency.version,
    )}\` | \`${markdownCell(
      dependency.licenseExpression,
    )}\` | ${markdownCell(
      dependency.licenseFiles.length > 0
        ? dependency.licenseFiles
            .map((file) => `${file.name} (sha256:${file.hash.slice(0, 16)}...)`)
            .join("; ")
        : "metadata only",
    )} |`,
  );
}

output.push(
  "",
  "## npm Production Dependencies",
  "",
  "| Package | Version | Declared license | License or notice files |",
  "| --- | --- | --- | --- |",
);

for (const dependency of npmDependencies) {
  output.push(
    `| \`${markdownCell(dependency.name)}\` | \`${markdownCell(
      dependency.version,
    )}\` | \`${markdownCell(
      dependency.licenseExpression,
    )}\` | ${markdownCell(
      dependency.licenseFiles.length > 0
        ? dependency.licenseFiles
            .map((file) => `${file.name} (sha256:${file.hash.slice(0, 16)}...)`)
            .join("; ")
        : "metadata only",
    )} |`,
  );
}

output.push(
  "",
  "## License and Notice Texts",
  "",
  "Each heading contains the SHA 256 of the original package file.",
  "",
);

for (const [hash, entry] of [...textsByHash.entries()].sort(([left], [right]) =>
  left.localeCompare(right),
)) {
  output.push(
    `### sha256:${hash}`,
    "",
    `Source filename: ${[...entry.fileNames]
      .sort()
      .map((name) => `\`${name}\``)
      .join(", ")}`,
    "",
    "Used by:",
    "",
    ...[...entry.packages]
      .sort()
      .map((dependency) => `* \`${dependency}\``),
    "",
  );
  const displayedText = entry.text
    .replaceAll("\r\n", "\n")
    .replaceAll("\r", "\n")
    .replaceAll("\u2013", "-")
    .replaceAll("\u2014", "--");
  output.push(
    ...displayedText
      .replace(/\n+$/, "")
      .split("\n")
      .map((line) => {
        const cleaned = line.replaceAll("\t", "    ").replace(/[ \t]+$/, "");
        return cleaned.length > 0 ? `    ${cleaned}` : "";
      }),
    "",
  );
}

writeFileSync(
  outputPath,
  `${output.join("\n").replace(/\n+$/, "")}\n`,
  "utf8",
);
console.log(
  `Wrote ${relative(appDirectory, outputPath)} with ${dependencies.length} packages and ${textsByHash.size} license or notice texts.`,
);
