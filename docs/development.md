# Development

## Requirements

- Node.js 22 or newer
- Rust stable
- `rustfmt` and `clippy`
- Tauri's Windows prerequisites
- Microsoft Edge WebView2

## Desktop setup

```powershell
cd apps\desktop
npm ci
npm run dev
```

## Public validation

```powershell
node scripts\check-workspace-paths.mjs
node scripts\check-release-version.mjs v0.0.1
cd apps\desktop
npm run check
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --locked --manifest-path src-tauri\Cargo.toml
cargo clippy --locked --manifest-path src-tauri\Cargo.toml -- -D warnings
npm run notices -- --target x86_64-pc-windows-msvc
```

Run the relevant local validation before opening a pull request. The Compile
workflow checks the product source. The Release workflow accepts an existing
version tag and creates a draft NSIS installer. It does not publish the draft.
See [Release process](releases.md) for the complete checklist.

After changing either lockfile, regenerate the dependency notices and include
the updated inventory with the change.

## Pull request scope

Keep one user-visible or repository outcome per pull request. Describe the
behavior first, then safeguards, data effects, and known limitations. Use plain
imperative titles without prefixes.
