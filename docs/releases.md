# Release process

Commander Deck Analyzer releases use a reviewed source commit, an exact version
tag, a draft Windows installer, and a final handwritten changelog.

## Prepare the version

1. Merge the product changes intended for the release.
2. Open a dedicated pull request named `Prepare Commander Deck Analyzer X.Y.Z`.
3. Update every version surface and add `docs/releases/X.Y.Z.md`.
4. Run the source checks documented in [Development](development.md).
5. Merge the preparation pull request after review.

Run the version check from the repository root:

```powershell
node scripts\check-release-version.mjs vX.Y.Z
```

## Create the draft

1. Create the exact tag `vX.Y.Z` from the merged preparation commit.
2. Push the tag.
3. Run the Release workflow with that tag.
4. Wait for the workflow to create the draft release.

The workflow checks the tag, source version, public paths, frontend build, Rust
formatting, locked compile, and strict lint. It then builds one NSIS installer
and adds these assets to the draft:

```text
Commander.Deck.Analyzer_X.Y.Z_x64-setup.exe
SHA256SUMS.txt
```

The workflow never publishes the draft.

## Review and publish

Before publication, confirm:

1. The draft tag points to the intended merged commit.
2. The title is `Commander Deck Analyzer vX.Y.Z`.
3. The installer name and embedded version match the tag.
4. `SHA256SUMS.txt` matches the uploaded installer.
5. The installer signing status is recorded accurately.
6. The draft contains only the intended release assets.

Publish the reviewed draft. Immediately replace the generated notes with the
complete body from `docs/releases/X.Y.Z.md`, then verify the title, changelog
link, and assets on the published release.

The first release uses a link to the tag history. Later releases use a
comparison link from the previous tag.

## Signing status

The current workflow produces an unsigned installer. Windows may show a
SmartScreen or publisher warning. Do not describe the installer as signed.
Signing credentials must be supplied through protected release configuration
and must never be committed to the repository.
