# Release workflow

WordSnap uses GitHub Actions for two layers of automation:

- `CI`: runs on pushes and pull requests targeting `main`; it installs frontend dependencies, builds the Vite frontend, and runs `cargo check` for the Tauri backend.
- `Release`: runs on every push to `main` and can also be started manually from the Actions tab. It builds Windows x64, Linux x64, macOS Intel, and macOS Apple Silicon packages, then publishes a GitHub Release.

The release tag format is:

```text
wordsnap-v<app-version>-build-<github-run-number>
```

The app version comes from `src-tauri/tauri.conf.json`. The build number keeps tags unique, so every `main` commit can publish a release even when the app version has not changed.

## Local packaging

Run a production package build for the current platform:

```bash
npm ci
npm run build
npm run tauri build
```

Generated installers and bundles are written under:

```text
src-tauri/target/release/bundle/
```

## GitHub setup

The release workflow uses GitHub's built-in `GITHUB_TOKEN`; no extra secret is required for unsigned builds.

If the release job fails with `Resource not accessible by integration`, open the repository on GitHub and set:

```text
Settings -> Actions -> General -> Workflow permissions -> Read and write permissions
```

## Versioning

For user-visible releases, update `version` in `src-tauri/tauri.conf.json` before merging or pushing to `main`.

`package.json` and `src-tauri/Cargo.toml` also carry project versions. Keep them aligned when preparing a named release.

## Code signing

The current workflow produces unsigned packages. That is enough for internal testing and GitHub asset distribution, but macOS Gatekeeper and Windows SmartScreen can still warn users.

Before a public production launch, add platform signing:

- macOS: Apple Developer ID signing and notarization.
- Windows: Authenticode code-signing certificate.
