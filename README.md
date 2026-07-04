# WordSnap

<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" height="96" alt="WordSnap icon">
</p>

WordSnap 是一个轻量的桌面划词 AI 翻译工具。选中英文文本后按 `Option+T` / `Alt+T`，应用会在选区附近弹出翻译浮窗；如果选中的是单个英文单词，会自动写入本地词表。

这个项目更适合作为自用工具或小型开源工具维护：功能边界很窄，界面尽量贴近 macOS 菜单栏小工具，数据默认只保存在本机。

## Features

- 全局快捷键触发划词翻译，当前实现为 `Alt+T`，在 macOS 上对应 `Option+T`。
- 通过 OpenAI 兼容的 `/chat/completions` 接口请求翻译。
- 单词、句子、加载中、错误四种浮窗状态。
- 单个英文单词会写入 SQLite 词表；短语和句子只翻译，不记录。
- 重复翻译同一个单词时更新次数和最近时间，不新增重复记录。
- 词表窗口支持手动移除不需要的单词记录。
- 菜单栏/托盘入口提供词表、设置和退出。
- 设置窗口支持配置 API Key、Base URL、任意模型 ID 和常用目标语言。
- 词表窗口按最近翻译时间展示本地单词记录。

## Current Scope

WordSnap 的第一版只做“划词翻译 + 自动记单词”。当前没有 OCR、截图翻译、实时窗口翻译、例句、词性、生词复习、搜索、标签、导出、编辑或多翻译源。

当前实现主要按 macOS 桌面体验设计和调试。Tauri 依赖本身具备跨平台能力，但 Windows/Linux 体验没有在本仓库中作为已验证目标说明。

## Tech Stack

- Tauri 2
- Rust
- TypeScript
- Vite
- SQLite via `rusqlite`
- OpenAI-compatible chat completions API via `reqwest`

## Prerequisites

- Node.js and npm
- Rust toolchain
- Tauri 2 所需的系统依赖
- 一个 OpenAI 兼容接口的 API Key

macOS 首次使用时，WordSnap 可能需要辅助功能权限。它会通过模拟复制当前选区来读取文本，因此如果浮窗提示无法读取选中文本，请在系统设置中允许 WordSnap 使用辅助功能。

## 免安装使用 / Portable

不想从源码构建的用户，可以直接从 [GitHub Releases](../../releases) 下载对应平台的构建产物，无需安装即可运行。推荐优先选择「免安装 / 绿色版」：

- macOS (Apple Silicon)：`WordSnap_<版本>_macos-aarch64_portable.zip`
- macOS (Intel)：`WordSnap_<版本>_macos-x64_portable.zip`
  解压后直接双击 `WordSnap.app`。应用未签名，首次打开若被 Gatekeeper 拦截，请右键点击 App → 选择「打开」，或在终端执行 `xattr -cr WordSnap.app` 后再打开。
- Windows x64：`WordSnap_<版本>_windows-x64_portable.zip`
  解压即用，双击 `WordSnap.exe`。需要系统自带的 WebView2 运行时（Win10/11 一般已内置）。
- Linux x64：`*.AppImage`
  下载后执行 `chmod +x WordSnap_*.AppImage`，然后直接运行。

偏好安装包的用户，也可以在同一 Release 页面下载 `.dmg` / `.msi` / `.exe (NSIS)` / `.deb` 安装器。

## Getting Started

如果需要从源码构建，先安装依赖：

```bash
npm install
```

启动完整桌面应用：

```bash
npm run tauri dev
```

只启动前端预览：

```bash
npm run dev
```

前端在浏览器里缺少 Tauri bridge 时会使用 mock 数据，适合快速查看窗口样式。可以打开这些视图：

- `http://127.0.0.1:1420/?view=float`
- `http://127.0.0.1:1420/?view=words`
- `http://127.0.0.1:1420/?view=settings`
- `http://127.0.0.1:1420/?view=menu`

## First Use

1. 启动应用后，点击菜单栏/托盘里的 WordSnap 图标。
2. 打开“设置…”，填写 API Key。
3. 按需修改 Base URL、模型 ID 和目标语言，默认值分别是 `https://api.openai.com/v1`、`gpt-4o-mini` 和简体中文。
4. 在任意应用中选中英文文本。
5. 按 `Option+T` / `Alt+T` 查看翻译结果。
6. 如果选中的是单个英文单词，结果会自动记入词表。

## Configuration

设置项会保存在 Tauri 的应用数据目录中：

- `settings.json`：API Key、Base URL、模型、快捷键和目标语言。
- `wordsnap.sqlite3`：本地词表数据库。

Base URL 可以填写完整地址，也可以省略协议。例如 `api.openai.com/v1` 会被规范化为 `https://api.openai.com/v1`。只需要填到 `/v1` 这一层即可；如果填写的地址已经以 `/chat/completions` 结尾，应用会直接使用它，否则会自动拼接 `/chat/completions`。模型字段是自由输入，不再被预设列表限制，适配用户自己的 OpenAI 兼容服务。目标语言下拉目前提供简体中文、繁体中文、英语、日语、韩语、法语、德语、西班牙语、葡萄牙语、意大利语、俄语、荷兰语、阿拉伯语、印地语、越南语、泰语、印度尼西亚语和土耳其语。

## Commands

| Command | Purpose |
| --- | --- |
| `npm run dev` | 启动 Vite 前端预览服务 |
| `npm run build` | 运行 TypeScript 检查并构建前端 |
| `npm run check` | 构建前端并运行 Rust `cargo check` |
| `npm run tauri dev` | 启动 Tauri 桌面开发模式 |
| `npm run tauri build` | 构建桌面应用安装包 |
| `npm run desktop:build` | 同 `npm run tauri build`，用于本地打包 |
| `npm run icons` | 从 `src-tauri/icons/icon.ico` 生成应用图标资源 |

`npm run icons` 使用 macOS 的 `sips` 和 `iconutil`，因此主要面向 macOS 开发环境。

## CI/CD

仓库内置 GitHub Actions：

- `.github/workflows/ci.yml`：push/PR 到 `main` 时运行前端构建和 Rust 检查。
- `.github/workflows/release.yml`：push 到 `main` 或手动触发时构建 Windows、Linux、macOS Intel 和 macOS Apple Silicon 包，并自动发布 GitHub Release。

更完整的发布说明见 `docs/RELEASE.md`。

## Project Structure

```text
.
├── design/                  # 高保真视觉参考稿
├── scripts/                 # 图标生成脚本
├── src/                     # TypeScript 前端入口与样式
├── src-tauri/               # Tauri/Rust 桌面端
│   ├── capabilities/        # Tauri 权限配置
│   ├── icons/               # 应用图标与托盘图标
│   └── src/                 # Rust 应用逻辑
├── index.html               # Vite 入口 HTML
├── package.json             # npm scripts 与前端依赖
└── vite.config.ts           # Vite 配置
```

核心逻辑集中在：

- `src-tauri/src/lib.rs`：全局快捷键、选区读取、翻译请求、SQLite 写入、窗口与托盘控制。
- `src/main.ts`：不同窗口视图的渲染、设置表单、词表刷新和浮窗交互。
- `src/styles.css`：浮窗、词表、设置和菜单样式。
- `src-tauri/tauri.conf.json`：窗口配置、vibrancy/window effects、bundle 配置。

## How It Works

触发翻译时，WordSnap 会：

1. 记录当前鼠标位置，用于定位浮窗。
2. 暂存剪贴板内容。
3. 模拟 `Cmd+C` 或 `Ctrl+C` 读取当前选区文本。
4. 还原原剪贴板内容。
5. 判断文本是否为单个英文单词。
6. 调用 OpenAI 兼容接口翻译。
7. 展示浮窗结果。
8. 如果是单个英文单词，将结果 upsert 到 SQLite 词表。

单词判断规则在 `src-tauri/src/lib.rs` 的 `is_single_english_word` 中：去掉首尾空白后，文本不能包含空白，只允许 ASCII 英文字母和连字符，并且至少包含一个英文字母。

## Data Model

词表数据表：

```sql
CREATE TABLE IF NOT EXISTS words (
  id            INTEGER PRIMARY KEY,
  word          TEXT UNIQUE,
  translation   TEXT NOT NULL,
  count         INTEGER NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at  TEXT NOT NULL
);
```

重复单词会执行 upsert：`count + 1`，刷新 `last_seen_at`，并用最新翻译覆盖 `translation`。

## Design Reference

`design/WordSnap UI.dc.html` 是项目早期的高保真视觉参考稿，可以直接在浏览器中打开查看。它不是生产代码，也不需要被移植到应用里；当前生产实现已经拆分到 `src/` 和 `src-tauri/`。

## Troubleshooting

如果按快捷键没有反应：

- 确认应用正在运行。
- 确认没有其他应用占用了 `Option+T` / `Alt+T`。
- macOS 上检查辅助功能权限。

如果浮窗提示无法读取文本：

- 先确认已经选中可复制的英文文本。
- 在 macOS 系统设置中允许 WordSnap 使用辅助功能。
- 有些应用或特殊文本区域可能不支持通过复制读取选区。

如果翻译失败：

- 检查 API Key 是否已填写。
- 检查 Base URL 是否可访问。
- 确认所选模型在当前 API Key 下可用。

## License

本仓库目前还没有提供 `LICENSE` 文件。公开发布前请补充明确的开源许可证。
