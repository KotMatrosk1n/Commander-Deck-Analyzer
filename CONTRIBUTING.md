# Contributing

Thanks for helping improve Commander Deck Analyzer.

## Before opening a pull request

1. Keep the change focused on one clear outcome.
2. Complete the relevant local validation.
3. Run the public repository checks from `apps/desktop`.
4. Confirm the report and data boundaries remain accurate.
5. Update public documentation when behavior or user expectations change.

## Pull requests

Use a short imperative title such as `Improve report provenance` or
`Correct Commander policy fallback`. Do not use conventional commit prefixes.

The description should explain:

- what changed for users or maintainers
- why the change was needed
- which safeguards and compatibility boundaries remain
- which compile and static checks completed
- any effect on data, network access, or report accuracy
- known limitations

Keep claims narrow. A bracket, score, or timing estimate should not be described
as accurate without suitable independent evidence.

## Public checks

```powershell
node scripts\check-workspace-paths.mjs
cd apps\desktop
npm ci
npm run check
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri\Cargo.toml -- -D warnings
cargo check --locked --manifest-path src-tauri\Cargo.toml
```

## Code style

- Prefer clear names and explicit failure states.
- Keep analysis claims separate from data provenance.
- Preserve local-first behavior unless a network action is clearly disclosed.
- Fail closed when required data or execution coverage is missing.
- Avoid unrelated cleanup in focused changes.
