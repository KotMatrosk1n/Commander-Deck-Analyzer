# Commander Deck Analyzer

Commander Deck Analyzer is a local-first Windows desktop app for understanding
how a Magic: The Gathering Commander deck is likely to behave. It reviews
opening hands, mana development, card roles, synergy, interaction, resilience,
known combo lines, and modeled threat timing, then explains the evidence behind
its bracket estimate.

The estimate is not an official ruling and is not yet empirically calibrated.
When an important interaction is outside the supported model, the report marks
that gap instead of presenting the result as fully supported.

## How it works

1. Paste a decklist, open a supported text file, or submit a supported public
   deck URL.
2. Select the commander and review unresolved cards.
3. Choose the trial count, turn horizon, and interaction profile, then run the
   local analysis.
4. Review the estimate, assumptions, coverage, and supporting evidence.
5. Export the report as Markdown when you want a shareable record.

## What the report covers

- Commander legality and current policy context
- Opening-hand composition and mulligan behavior
- Mana production, color access, and early development
- Card roles, synergy resources, and known combo lines
- Modeled threat, win-attempt, and resilience timing
- Selected interaction pressure and recovery behavior
- Rules and execution coverage for the analyzed list
- A bracket estimate with reasons and confidence limits

See [Supported analysis](docs/supported-analysis.md) for the current claim
boundary.

## Privacy and data

Analysis runs on the device. Network access occurs only for a user-requested
deck import, missing-card lookup, or data update. Reports, downloaded snapshots,
and cached analysis remain in local application data.

The application does not bundle card artwork, a complete card database, a
Commander Spellbook catalog, tournament records, or the complete Comprehensive
Rules. See [Data sources](docs/data-sources.md) and
[Third-party notices](THIRD_PARTY_NOTICES.md) for details.

## Development

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
cargo clippy --locked --manifest-path src-tauri\Cargo.toml -- -D warnings
cargo check --locked --manifest-path src-tauri\Cargo.toml
```

The hosted workflow compiles and statically checks the public product source.
Complete the relevant local validation before opening a pull request.

More detail is available in [Development](docs/development.md) and
[Architecture](docs/architecture.md).

## Repository map

```text
.github/        Collaboration and compile checks
apps/desktop/   React, TypeScript, Tauri, and Rust desktop source
docs/           Public architecture, development, and data documentation
scripts/        Repository integrity checks
```

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before proposing a change. Security
reports follow [SECURITY.md](SECURITY.md).

Commander Deck Analyzer is available under the
[GNU General Public License v3.0](LICENSE). Third-party material remains subject
to its own terms.
