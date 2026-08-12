<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" height="96" alt="WordSnap icon">
</p>

<h1 align="center">WordSnap</h1>

<p align="center">Select text, press the shortcut, and view an AI translation near the selection.</p>

<p align="center">
  <a href="README.md">简体中文</a> · <strong>English</strong>
</p>

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

WordSnap is a lightweight desktop selection-translation utility. Select copyable text in any application, then press `Option+T` on macOS or `Alt+T` on other platforms to display the translation near the selection. Foreign-language input is translated into the configured target language. Simplified Chinese input is translated into natural English.

Application settings and saved words remain on the local device. When a translation is requested, the selected text is sent to the OpenAI-compatible API configured by the user.

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/releases/latest">Download the latest release</a> ·
  <a href="CONTRIBUTING.md">Contributing</a> ·
  <a href="docs/PRIVACY.md">Privacy</a> ·
  <a href="SECURITY.md">Security</a>
</p>

## Demonstration

### Chinese to English

![WordSnap translating selected Chinese text into English](docs/assets/demo-zh-to-en.gif)

### English to Chinese

![WordSnap translating selected English text into Chinese](docs/assets/demo-en-to-zh.gif)

## Features

- Translate copyable selected text in any application by pressing `Option+T` or `Alt+T`.
- Translate foreign-language text into the selected target language and Simplified Chinese into natural English.
- Display the translation near the selection and close a completed card by clicking outside it.
- Remain visible over native full-screen applications and multiple Spaces on macOS without taking focus from the current application.
- Save only a single ASCII English word, optionally containing a hyphen, to the local SQLite word list.
- Update the translation, lookup count, and most recent lookup time when a word is queried again.
- Provide access to the word list, settings, and quit action from the menu bar or system tray.
- Support a custom OpenAI-compatible Base URL, model ID, and target language.
- Include no account system, cloud synchronization, advertising, or telemetry.

## Download and run

Download the portable build or installer for the current platform from [GitHub Releases](https://github.com/YunFy26/WordSnap/releases/latest).

| Platform | Recommended download | Other formats |
| --- | --- | --- |
| macOS Apple Silicon | `WordSnap_<version>_macos-aarch64_portable.zip` | `.dmg` |
| macOS Intel | `WordSnap_<version>_macos-x64_portable.zip` | `.dmg` |
| Windows x64 | `WordSnap_<version>_windows-x64_portable.zip` | `.msi`, NSIS `.exe` |
| Linux x64 | `.AppImage` | `.deb`, `.rpm` |

Portable builds do not require an installer. On macOS and Windows, extract the archive and run `WordSnap.app` or `WordSnap.exe`. Before running an AppImage for the first time, execute `chmod +x WordSnap_*.AppImage`.

> [!NOTE]
> Current releases are not code-signed. If macOS Gatekeeper prevents the first launch, right-click `WordSnap.app` and select Open, or run `xattr -cr WordSnap.app`. Windows SmartScreen may also display a security warning.

## Initial configuration

1. Start WordSnap and select its icon in the menu bar or system tray.
2. Open Settings and enter the API key.
3. Modify the Base URL, model ID, and target language if required.
4. Select copyable text in any application.
5. Press `Option+T` on macOS or `Alt+T` on another platform, then view the translation.

The default configuration is as follows:

| Setting | Default value |
| --- | --- |
| Base URL | `https://api.openai.com/v1` |
| Model | `gpt-4o-mini` |
| Target language | Simplified Chinese |

On macOS, selection capture may require Accessibility permission. If WordSnap cannot read text that is already selected, allow WordSnap under System Settings, Privacy & Security, Accessibility.

## Configuration and local data

The Base URL may include or omit its scheme. WordSnap normalizes the address and appends `/chat/completions` when required. An address that already ends with this path is used without modification. Remote services should use HTTPS. Plain HTTP is appropriate only for an explicitly local development endpoint.

Tauri stores local data in the application data directory assigned by the operating system:

- `settings.json`: stores the API key, Base URL, model, and target language.
- `wordsnap.sqlite3`: stores English words, translations, lookup counts, and timestamps.

Runtime logs are stored in the project root `logs/` directory. Every application start creates a
separate `YYYY-MM-DD_HH-mm-ss.SSS_pid-<process-id>.log` file. All `DEBUG`, `INFO`, and `ERROR`
entries for that application session are written to the same file until the application exits. Logs
cover startup, initialization, hotkeys, clipboard handling, translation requests, database
operations, windows, settings, and shutdown. They contain diagnostic metadata such as events,
outcomes, durations, character counts, and HTTP status codes. They do not contain API keys, selected
text, translations, clipboard contents, complete service responses, or configured endpoint addresses.

> [!IMPORTANT]
> Selected text is sent to the API service configured by the user. The API key is currently stored as plain text in the local `settings.json` file, and the application does not use the system credential store. Read [Privacy and data handling](docs/PRIVACY.md) before use.

## Run from source

### Requirements

- Node.js 20 or later and npm.
- Rust stable with `rustfmt` and Clippy.
- The [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for the current platform.
- An OpenAI-compatible API key.

### Start the desktop application

```bash
git clone https://github.com/YunFy26/WordSnap.git
cd WordSnap
npm ci
npm run tauri dev
```

### Check and package

```bash
npm run check
npm run audit
npm run tauri build
```

| Command | Purpose |
| --- | --- |
| `npm run build` | Type-check and build the Vite frontend |
| `npm run format:check` | Check Rust formatting |
| `npm run lint` | Run strict Clippy checks for all Rust targets |
| `npm test` | Run the Rust unit tests |
| `npm run check` | Run the frontend build, formatting check, Clippy, and tests |
| `npm run audit` | Check high-severity dependency advisories through the official npm registry |
| `npm run tauri dev` | Start the complete desktop development environment |
| `npm run tauri build` | Build application packages for the current platform |
| `npm run icons` | Regenerate application and tray icons from the source files |

Run `npm run audit` whenever dependencies change. Local packages are written to `src-tauri/target/release/bundle/`.

## Processing sequence

1. When the shortcut is triggered, the application records the current pointer position.
2. The application saves the clipboard, simulates a copy operation to read the selection, and attempts to restore the original clipboard content.
3. The application constructs an OpenAI-compatible `/chat/completions` request from the input and user settings.
4. The loading state and translation result are displayed near the selection.
5. After translation completes, clicking outside the card hides the popup.
6. If the input is a single ASCII English word, the application inserts or updates the corresponding local word-list record.

Phrases, sentences, Chinese text, and failed translations are not written to the word list.

## Project structure

```text
.
├── docs/                    # Privacy information, release instructions, and demonstrations
├── scripts/                 # Icon generation scripts
├── src/                     # TypeScript window views and styles
├── src-tauri/               # Tauri configuration, Rust backend, and application resources
│   ├── capabilities/        # Tauri permissions available to the frontend
│   ├── icons/               # Application and tray icons
│   └── src/lib.rs           # Shortcut, selection, translation, word-list, and window logic
├── CONTRIBUTING.md          # Contribution workflow
├── SECURITY.md              # Vulnerability reporting process and support scope
└── package.json             # npm commands and frontend dependencies
```

The primary entry points are:

- `src-tauri/src/lib.rs`: application state, global shortcut, selection capture, translation requests, SQLite, windows, and tray.
- `src/main.ts`: translation popup, word list, settings, and menu views.
- `src/styles.css`: all production interface styles and light or dark theme behavior.
- `src-tauri/tauri.conf.json`: window, package, and Tauri security configuration.

## Continuous integration and releases

- `CI` runs the frontend build, Rust formatting check, Clippy, tests, and npm audit for pushes and pull requests targeting `main`.
- `Release` builds macOS Apple Silicon, macOS Intel, Windows x64, and Linux x64 packages for each push to `main`, then publishes a GitHub Release.

See [Release workflow](docs/RELEASE.md) for package formats, portable builds, and unsigned-release behavior.

## Troubleshooting

### The shortcut does not respond

- Confirm that WordSnap is running and that no other application has registered `Option+T` or `Alt+T`.
- Check Accessibility permission on macOS.
- Confirm that the selected text can be copied normally.

### The translation request fails

- Check the API key, Base URL, and network connection.
- Confirm that the current API key is authorized to use the selected model.
- Do not disclose API keys or private text in issues, logs, or screenshots.

General problems can be submitted through [GitHub Issues](https://github.com/YunFy26/WordSnap/issues). Vulnerabilities must be reported privately according to the [Security Policy](SECURITY.md).
