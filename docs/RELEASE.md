# Release workflow

WordSnap uses GitHub Actions for two layers of automation:

- `CI`: runs on pushes and pull requests targeting `main`; it installs frontend dependencies, builds the Vite frontend, and runs `cargo check` for the Tauri backend.
- `Release`: runs on every push to `main` and can also be started manually from the Actions tab. It builds Windows x64, Linux x64, macOS Intel, and macOS Apple Silicon packages, then publishes a GitHub Release.

The release tag format is:

```text
wordsnap-v<app-version>-build-<github-run-number>
```

The app version comes from `src-tauri/tauri.conf.json`. The build number keeps tags unique, so every `main` commit can publish a release even when the app version has not changed.

## Release artifacts

Every release publishes both regular installers and **portable / no-install** artifacts. Portable builds are the recommended download for most users, since they run without an installer.

| Platform | Portable (no install) | Installer |
| --- | --- | --- |
| macOS (Apple Silicon) | `WordSnap_<version>_macos-aarch64_portable.zip` | `.dmg` |
| macOS (Intel) | `WordSnap_<version>_macos-x64_portable.zip` | `.dmg` |
| Windows x64 | `WordSnap_<version>_windows-x64_portable.zip` | `.msi` / NSIS `.exe` |
| Linux x64 | `*.AppImage` | `.deb` |

The macOS portable zips are produced with `ditto -c -k --sequesterRsrc --keepParent` so the `WordSnap.app` bundle stays intact. The Windows portable zip contains the standalone `WordSnap.exe` (the Tauri main binary, named after `productName`) built at `src-tauri/target/release/WordSnap.exe`. The Linux AppImage is emitted by the `"targets": "all"` bundle config and needs no extra build steps.

### 免安装使用 / Portable

- macOS 免安装 zip：解压后直接双击 `WordSnap.app`。应用未签名，首次打开若被 Gatekeeper 拦截，请右键点击 App → 选择「打开」，或在终端执行 `xattr -cr WordSnap.app` 后再打开。
- Windows 绿色版 zip：解压即用，双击 `WordSnap.exe`。需要系统自带的 WebView2 运行时（Win10/11 一般已内置）。
- Linux AppImage：下载后执行 `chmod +x WordSnap_*.AppImage`，然后直接运行。

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
