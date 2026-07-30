# Commander Deck Analyzer

[![Latest release](https://img.shields.io/github/v/release/KotMatrosk1n/Commander-Deck-Analyzer?display_name=tag&sort=semver)](https://github.com/KotMatrosk1n/Commander-Deck-Analyzer/releases/latest)
![Windows](https://img.shields.io/badge/platform-Windows-0078D4)
[![License](https://img.shields.io/github/license/KotMatrosk1n/Commander-Deck-Analyzer)](LICENSE)

Commander Deck Analyzer is a Windows desktop app for understanding how a
Magic: The Gathering Commander deck is likely to play. It reviews opening
hands, mana development, card roles, synergy, interaction, resilience, known
combo lines, and modeled threat timing. The report explains what supports its
bracket estimate and where the analysis still has limits.

The estimate is not an official ruling and is not yet empirically calibrated.
When an important interaction is outside what the analyzer can execute, the
report marks that gap instead of presenting the result as fully supported.

[Download the latest release](https://github.com/KotMatrosk1n/Commander-Deck-Analyzer/releases/latest) | [Read about supported analysis](docs/supported-analysis.md) | [Report an issue](https://github.com/KotMatrosk1n/Commander-Deck-Analyzer/issues/new/choose)

## Analyze a Deck

1. Install and open Commander Deck Analyzer.
2. Download the current card data from the status button when the app asks for
   it.
3. Paste a deck list, open a supported text file, or enter a supported public
   deck URL.
4. Select the commander and review any unresolved cards.
5. Choose the trial count, turn horizon, and interaction profile.
6. Run the analysis, review the evidence, and export the report when needed.

## Supported Analysis

| Area | What the report covers |
| --- | --- |
| Opening hands | Composition, keep rate, and a fixed aggressive multiplayer London mulligan policy |
| Mana and development | Mana production, color access, early development, and commander access |
| Threat timing | Known routes, attempt timing, blockers, and recovery |
| Interaction | No Interaction, Mild Interaction, Moderate Interaction, and cEDH Interaction profiles |
| Rules coverage | Card functions the analyzer can execute and any remaining gaps |
| Bracket estimate | A recommendation with supporting reasons, confidence limits, and visible gaps |

Threat timing follows the routes the analyzer knows how to execute. It is not a
complete Magic game engine or a claim about every possible line. The bracket
estimate is not calibrated against a large independent set of real games. See
[Supported analysis](docs/supported-analysis.md) for details.

## Data and Privacy

Analysis runs on your computer. Network access occurs only when you request a
deck import, card lookup, or data update. Analysis data and caches stay on your
computer. Reports are written only to the location you choose.

The application does not bundle card artwork, a complete card database, a
Commander Spellbook catalog, tournament records, or the complete Comprehensive
Rules. See [Data sources](docs/data-sources.md) and
[third party notices](THIRD_PARTY_NOTICES.md) for details.

## Documentation

| Guide | Contents |
| --- | --- |
| [Supported analysis](docs/supported-analysis.md) | What the analyzer calculates and what it does not |
| [Data sources](docs/data-sources.md) | Card, policy, combo, and import data |
| [Architecture](docs/architecture.md) | Desktop structure and analysis flow |
| [Development](docs/development.md) | Local setup and source checks |
| [Release process](docs/releases.md) | Versioning, draft releases, and publication |
| [Security](SECURITY.md) | Vulnerability reporting and supported versions |

## Developing

Requirements:

- Node.js 22 or newer
- Rust stable with `rustfmt` and `clippy`
- Windows development tools required by Tauri
- Microsoft Edge WebView2

Build the public desktop source:

```powershell
cd apps\desktop
npm ci
npm run check
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --locked --manifest-path src-tauri\Cargo.toml
cargo clippy --locked --manifest-path src-tauri\Cargo.toml -- -D warnings
npm run notices -- --target x86_64-pc-windows-msvc
```

The Compile workflow checks pull requests and the default branch. The Release
workflow creates a draft installer only for an existing version tag. Complete
the relevant local checks before opening a pull request.

## Repository Map

```text
.github/        Collaboration, compile checks, and draft release automation
apps/desktop/   React, TypeScript, Tauri, and Rust desktop source
docs/           Public architecture, development, and data documentation
scripts/        Repository integrity checks
```

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before proposing a change. Security
reports follow [SECURITY.md](SECURITY.md).

Commander Deck Analyzer is available under the
[GNU General Public License v3.0](LICENSE). Third party material remains subject
to its own terms.
