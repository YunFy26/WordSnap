# AGENTS.md

This file applies to the entire repository. It is the working contract for coding agents; user instructions always take precedence.

## Read first

- `README.md` — current product behavior, setup, commands, and architecture map.
- `CONTRIBUTING.md` — contribution workflow, code expectations, and product scope.
- `SECURITY.md` and `docs/PRIVACY.md` — security reporting and data-flow constraints.
- `docs/RELEASE.md` — packaging, versioning, and unsigned-release behavior.

Treat the implementation in `src/` and `src-tauri/` as the source of truth. Files under `design/` are visual references, not production code or an authoritative specification.

## Product invariants

- WordSnap is a small Tauri 2 selection-translation utility, not a general dictionary or learning platform.
- `Option+T` on macOS and `Alt+T` elsewhere capture copyable selected text and show a nearby translation popup.
- Foreign-language input is translated to the configured target language. Simplified Chinese input is translated into English for quick expression drafting.
- Only a single ASCII English word, with an optional hyphen, may be written to the SQLite word list. Phrases, sentences, Chinese text, and failed translations must not be recorded.
- Repeated word lookups update the existing row, increment `count`, refresh `last_seen_at`, and replace the stored translation.
- Preserve and restore the user's clipboard as faithfully as the platform permits. Clipboard changes, selection capture, and simulated keyboard input require platform-specific verification.
- On macOS the app behaves as a menu-bar utility and must not gain a normal Dock presence.
- Keep the v1 scope narrow. Do not add OCR, screenshot translation, search, tags, export, editing, review systems, or additional translation backends without explicit approval.

## Code map

- `src-tauri/src/lib.rs` — application state, global shortcut, selection/clipboard capture, translation request, SQLite storage, windows, tray, and Tauri commands.
- `src/main.ts` — view routing, float/word/settings/menu rendering, frontend events, and browser mock data.
- `src/styles.css` — all production UI styling and light/dark behavior.
- `src-tauri/tauri.conf.json` — window definitions, bundle metadata, and Tauri security configuration.
- `src-tauri/capabilities/default.json` — allowed frontend capabilities.
- `.github/workflows/ci.yml` and `.github/workflows/release.yml` — validation and packaging automation.

Most backend behavior is intentionally concentrated in `src-tauri/src/lib.rs`. Prefer small focused helpers and tests over introducing a new abstraction layer for a one-off change.

## Working rules

1. Start with `git status --short --branch`. Preserve all pre-existing edits and never reformat, revert, stage, or commit unrelated work.
2. Inspect the actual call path before changing behavior. Keep Rust payload fields and TypeScript camelCase interfaces synchronized.
3. Add or update tests for pure logic and regressions. For shortcut, window, clipboard, tray, and positioning work, also document manual verification and the tested OS.
4. Never commit API keys, real selected text, personal databases, app-data files, or unredacted logs/screenshots.
5. Do not log full API keys or selected text. Remote services should use HTTPS; plain HTTP is only appropriate for an explicitly local development endpoint.
6. If storage, network requests, clipboard behavior, or Tauri permissions change, update `docs/PRIVACY.md` or `SECURITY.md` in the same change.
7. If user-visible behavior, setup, commands, or limitations change, update `README.md`. Keep durable process detail in the dedicated docs rather than duplicating it here.
8. Keep `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` versions aligned for a named release.
9. Do not change release triggers, artifact naming, signing expectations, or supported-platform claims without checking `docs/RELEASE.md` and updating it together.

## Commands

```bash
npm ci                 # install the locked frontend/Tauri toolchain
npm run tauri dev      # run the complete desktop app
npm run dev            # browser-only UI preview with mock data
npm run check          # TypeScript/Vite build + rustfmt + Clippy + Rust tests
npm run audit          # npm vulnerability audit via the official registry
npm run tauri build    # package the current platform; run only when needed
```

`npm run check` is the required baseline before handoff. Dependency changes also require `npm run audit`. Do not claim desktop behavior is verified from the browser mock alone.

## Definition of done

- The requested behavior is implemented without expanding the agreed product scope.
- `npm run check` passes.
- Relevant manual platform checks are recorded, or the lack of platform verification is stated clearly.
- Documentation and privacy/security disclosures match the final code.
- `git diff --check` passes, and the final diff contains no unrelated or generated files.
