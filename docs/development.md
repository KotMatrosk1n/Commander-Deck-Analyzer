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
cd apps\desktop
npm run check
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri\Cargo.toml -- -D warnings
cargo check --locked --manifest-path src-tauri\Cargo.toml
```

Run the relevant local validation before opening a pull request. The hosted
workflow checks the public product source and does not package a release.

## Pull request scope

Keep one user-visible or repository outcome per pull request. Describe the
behavior first, then safeguards, validation, data effects, and known
limitations. Use plain imperative titles without prefixes.
