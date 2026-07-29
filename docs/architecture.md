# Architecture

Commander Deck Analyzer is a Tauri desktop application with a React and
TypeScript interface and a Rust analysis core.

## Runtime flow

1. The desktop interface accepts a decklist and analysis settings.
2. The Rust boundary parses the list and resolves submitted display names to
   canonical Oracle identities from local application data.
3. The analyzer compiles card roles, mechanics, known lines, mana access, and
   executable behavior into a deterministic deck model.
4. Simulation and scoring produce an analysis report with coverage and
   provenance.
5. The interface presents the result and can export it as Markdown.

## Main boundaries

`apps/desktop/src/` owns presentation, interaction, accessibility, report
rendering, and typed calls into the native boundary.

`apps/desktop/src-tauri/src/` owns parsing, local data stores, semantic
compilation, simulation, scoring, provenance, and Tauri commands.

`apps/desktop/src-tauri/data/` contains small versioned production snapshots
that are required at first launch. Downloaded datasets and user reports are
stored outside the source tree.

## Trust model

The application treats imported data as untrusted input. Parsers use bounded
sizes, explicit schemas, and fail-closed validation. Provider updates are
user-requested and recorded with source and version information.

Simulation claims are limited by execution coverage. Unsupported or ambiguous
card behavior remains visible in the report and cannot silently count as fully
modeled evidence.

Submitted display names remain presentation data. Canonical Oracle identity
drives policy checks, combo matching, simulation, coverage, scoring inputs, and
cache identity. Ambiguous identity remains unresolved.

## Local-first behavior

Deck analysis and report generation run locally. Network actions are limited to
the import and update operations described in
[Data sources](data-sources.md).
