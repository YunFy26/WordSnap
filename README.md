<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" height="96" alt="WordSnap 图标">
</p>

<h1 align="center">WordSnap</h1>

<p align="center">选中文本，按下快捷键，在原地获得 AI 翻译。</p>

<p align="center">
  <strong>简体中文</strong> · <a href="README_EN.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

WordSnap 是一个轻量的桌面划词 AI 翻译工具。在任意应用中选中可复制文本后按 `Option+T`（macOS）或 `Alt+T`，译文会在选区附近弹出。外语默认翻译为简体中文；简体中文输入会生成自然的英文表达。

设置和单词记录保存在本机。翻译时，选中的文本会发送给你配置的 OpenAI 兼容 API。

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/releases/latest">下载最新版本</a> ·
  <a href="CONTRIBUTING.md">参与贡献</a> ·
  <a href="docs/PRIVACY.md">隐私说明</a> ·
  <a href="SECURITY.md">安全策略</a>
</p>

## 演示

### 中文转英文

![WordSnap 将选中的中文快速翻译为英文](docs/assets/demo-zh-to-en.gif)

### 英文转中文

![WordSnap 将选中的英文快速翻译为中文](docs/assets/demo-en-to-zh.gif)

## 功能

- 在任意应用中通过 `Option+T` / `Alt+T` 翻译可复制的选中文本。
- 外语翻译为所选目标语言，简体中文转换为自然英文表达。
- 翻译卡片显示在选区附近，完成后点击卡片外区域即可关闭。
- macOS 下支持原生全屏和多 Space，浮窗显示时不会抢走当前应用焦点。
- 仅将单个 ASCII 英文单词（可含连字符）记录到本地 SQLite 词表。
- 重复查询同一单词时更新译文、查询次数和最近使用时间。
- 通过菜单栏或系统托盘访问词表、设置和退出操作。
- 支持自定义 OpenAI 兼容 Base URL、模型 ID 和目标语言。
- 不包含账号、云同步、广告或遥测。

## 下载与运行

从 [GitHub Releases](https://github.com/YunFy26/WordSnap/releases/latest) 下载对应平台的便携版或安装包。

| 平台 | 推荐下载 | 其他格式 |
| --- | --- | --- |
| macOS Apple Silicon | `WordSnap_<版本>_macos-aarch64_portable.zip` | `.dmg` |
| macOS Intel | `WordSnap_<版本>_macos-x64_portable.zip` | `.dmg` |
| Windows x64 | `WordSnap_<版本>_windows-x64_portable.zip` | `.msi`、NSIS `.exe` |
| Linux x64 | `.AppImage` | `.deb`、`.rpm` |

便携版无需安装：解压后直接运行 `WordSnap.app` 或 `WordSnap.exe`；Linux AppImage 首次运行前需要执行 `chmod +x WordSnap_*.AppImage`。

> [!NOTE]
> 当前发布包未签名。macOS 首次打开时如果被 Gatekeeper 拦截，请右键点击 `WordSnap.app` 并选择“打开”，或执行 `xattr -cr WordSnap.app`。Windows 也可能显示 SmartScreen 提示。

## 首次使用

1. 启动 WordSnap，点击菜单栏或系统托盘中的图标。
2. 打开“设置…”，填写 API Key。
3. 按需修改 Base URL、模型 ID 和目标语言。
4. 在任意应用中选中可复制的文本。
5. 按 `Option+T`（macOS）或 `Alt+T` 查看翻译。

默认配置：

| 设置 | 默认值 |
| --- | --- |
| Base URL | `https://api.openai.com/v1` |
| 模型 | `gpt-4o-mini` |
| 目标语言 | 简体中文 |

macOS 首次读取选区时可能需要辅助功能权限。如果已经选中文本但应用无法读取，请在“系统设置 → 隐私与安全性 → 辅助功能”中允许 WordSnap。

## 配置与数据

Base URL 可以填写完整地址，也可以省略协议。WordSnap 会规范化地址，并在需要时追加 `/chat/completions`；已经以该路径结尾的地址会直接使用。远程服务应使用 HTTPS，HTTP 仅适合明确的本机开发接口。

本地数据位于 Tauri 为当前系统分配的应用数据目录：

- `settings.json`：API Key、Base URL、模型和目标语言。
- `wordsnap.sqlite3`：英文单词、译文、查询次数和时间。

> [!IMPORTANT]
> 选中文本会发送到你配置的 API 服务。API Key 当前以明文保存在本机 `settings.json`，不使用系统钥匙串。使用前请阅读[隐私与数据处理](docs/PRIVACY.md)。

## 从源码运行

### 环境要求

- Node.js 20 或更高版本及 npm
- Rust stable（含 `rustfmt` 和 Clippy）
- 当前平台所需的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)
- 一个 OpenAI 兼容 API Key

### 启动桌面应用

```bash
git clone https://github.com/YunFy26/WordSnap.git
cd WordSnap
npm ci
npm run tauri dev
```

### 检查与打包

```bash
npm run check
npm run audit
npm run tauri build
```

| 命令 | 用途 |
| --- | --- |
| `npm run build` | TypeScript 检查并构建 Vite 前端 |
| `npm run format:check` | 检查 Rust 格式 |
| `npm run lint` | 对全部 Rust target 运行严格 Clippy |
| `npm test` | 运行 Rust 单元测试 |
| `npm run check` | 运行构建、格式检查、Clippy 和测试 |
| `npm run audit` | 通过官方 npm registry 检查高风险漏洞 |
| `npm run tauri dev` | 启动完整桌面开发模式 |
| `npm run tauri build` | 构建当前平台的应用包 |
| `npm run icons` | 从源文件重新生成应用和托盘图标 |

依赖变更需要同时运行 `npm run audit`。本地打包产物位于 `src-tauri/target/release/bundle/`。

## 工作原理

1. 快捷键触发时记录当前鼠标位置。
2. 暂存剪贴板并模拟复制，读取当前选区后尽力恢复原内容。
3. 根据输入和设置构造 OpenAI 兼容的 `/chat/completions` 请求。
4. 在选区附近显示加载状态和翻译结果。
5. 翻译完成后，点击卡片之外的区域会隐藏浮窗。
6. 如果输入是单个 ASCII 英文单词，则将结果写入或更新本地词表。

短语、句子、中文文本和失败的翻译不会写入词表。

## 项目结构

```text
.
├── docs/                    # 隐私、发布说明和演示素材
├── scripts/                 # 图标生成脚本
├── src/                     # TypeScript 窗口视图与样式
├── src-tauri/               # Tauri 配置、Rust 后端和应用资源
│   ├── capabilities/        # 前端可调用的 Tauri 权限
│   ├── icons/               # 应用与托盘图标
│   └── src/lib.rs           # 快捷键、选区、翻译、词表和窗口逻辑
├── CONTRIBUTING.md          # 贡献流程
├── SECURITY.md              # 安全报告与支持范围
└── package.json             # npm 命令和前端依赖
```

主要入口：

- `src-tauri/src/lib.rs`：应用状态、全局快捷键、选区读取、翻译请求、SQLite、窗口与托盘。
- `src/main.ts`：浮窗、词表、设置和菜单视图。
- `src/styles.css`：全部生产 UI 样式及明暗主题。
- `src-tauri/tauri.conf.json`：窗口、打包和 Tauri 安全配置。

## CI 与发布

- `CI` 在 push 或 PR 指向 `main` 时运行构建、Rust 格式检查、Clippy、测试和 npm 审计。
- `Release` 在每次 push 到 `main` 时构建 macOS Apple Silicon、macOS Intel、Windows x64 和 Linux x64 包，并发布 GitHub Release。

发布格式、便携包和未签名构建说明见 [docs/RELEASE.md](docs/RELEASE.md)。

## 常见问题

### 快捷键没有反应

- 确认 WordSnap 正在运行，且没有其他应用占用 `Option+T` / `Alt+T`。
- macOS 上检查辅助功能权限。
- 确认当前应用中的文本可以正常复制。

### 翻译失败

- 检查 API Key、Base URL 和网络连接。
- 确认所选模型可由当前 API Key 调用。
- 不要在 Issue、日志或截图中公开 API Key 和私人文本。

问题反馈请使用 [GitHub Issues](https://github.com/YunFy26/WordSnap/issues)。安全漏洞请按照[安全策略](SECURITY.md)私密报告。
