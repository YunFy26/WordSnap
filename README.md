<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" height="96" alt="WordSnap 图标">
</p>

<h1 align="center">WordSnap</h1>

<p align="center">选中文本并按下快捷键，在选区附近查看 AI 翻译结果。</p>

<p align="center">
  <strong>简体中文</strong> · <a href="README_EN.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/YunFy26/WordSnap/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

WordSnap 是一款轻量桌面划词翻译工具。在任意应用中选中可复制文本后，按 `Option+T`（macOS）或 `Alt+T`，即可在选区附近查看译文。外语输入会翻译为用户配置的目标语言；简体中文输入会翻译为自然英文表达。

应用设置和单词记录保存在本机。执行翻译时，选中文本会发送至用户配置的 OpenAI 兼容 API。

<p align="center">
  <a href="https://github.com/YunFy26/WordSnap/releases/latest">下载最新版本</a> ·
  <a href="CONTRIBUTING.md">参与贡献</a> ·
  <a href="docs/PRIVACY.md">隐私说明</a> ·
  <a href="SECURITY.md">安全策略</a>
</p>

## 功能演示

### 中文翻译为英文

![WordSnap 将选中的中文翻译为英文](docs/assets/demo-zh-to-en.gif)

### 英文翻译为中文

![WordSnap 将选中的英文翻译为中文](docs/assets/demo-en-to-zh.gif)

## 主要功能

- 在任意应用中通过 `Option+T` 或 `Alt+T` 翻译可复制的选中文本。
- 将外语翻译为用户选择的目标语言，并将简体中文翻译为自然英文表达。
- 在选区附近显示翻译卡片；翻译完成后，点击卡片外部可以关闭卡片。
- 在 macOS 上支持原生全屏和多个 Space，显示浮窗时不会使当前应用失去焦点。
- 只将单个 ASCII 英文单词写入本地 SQLite 词表，单词中可以包含连字符。
- 重复查询同一单词时，更新译文、查询次数和最近使用时间。
- 通过菜单栏或系统托盘访问词表、设置和退出功能。
- 支持自定义 OpenAI 兼容 Base URL、模型 ID 和目标语言。
- 不包含账号系统、云同步、广告或遥测功能。

## 下载与运行

从 [GitHub Releases](https://github.com/YunFy26/WordSnap/releases/latest) 下载适用于当前平台的免安装版本或安装包。

| 平台 | 建议下载 | 其他格式 |
| --- | --- | --- |
| macOS Apple Silicon | `WordSnap_<版本>_macos-aarch64_portable.zip` | `.dmg` |
| macOS Intel | `WordSnap_<版本>_macos-x64_portable.zip` | `.dmg` |
| Windows x64 | `WordSnap_<版本>_windows-x64_portable.zip` | `.msi`、NSIS `.exe` |
| Linux x64 | `.AppImage` | `.deb`、`.rpm` |

免安装版本无需执行安装程序。macOS 和 Windows 用户解压后，可以直接运行 `WordSnap.app` 或 `WordSnap.exe`。Linux 用户首次运行 AppImage 前，需要执行 `chmod +x WordSnap_*.AppImage`。

> [!NOTE]
> 当前发布包未进行代码签名。macOS Gatekeeper 如果阻止首次启动，可以右键单击 `WordSnap.app` 并选择打开，或执行 `xattr -cr WordSnap.app`。Windows SmartScreen 也可能显示安全提示。

## 初次配置

1. 启动 WordSnap，单击菜单栏或系统托盘中的应用图标。
2. 打开设置并填写 API Key。
3. 根据需要修改 Base URL、模型 ID 和目标语言。
4. 在任意应用中选中可复制文本。
5. 在 macOS 上按 `Option+T`，在其他平台上按 `Alt+T`，随后查看翻译结果。

默认配置如下：

| 设置项 | 默认值 |
| --- | --- |
| Base URL | `https://api.openai.com/v1` |
| 模型 | `gpt-4o-mini` |
| 目标语言 | 简体中文 |

在 macOS 上，读取选区可能需要辅助功能权限。如果应用无法读取已经选中的文本，应在系统设置的隐私与安全性、辅助功能页面中允许 WordSnap。

## 配置与本地数据

Base URL 可以包含协议，也可以省略协议。WordSnap 会规范化地址，并在必要时追加 `/chat/completions`。如果地址已经以该路径结尾，应用会直接使用该地址。远程服务应使用 HTTPS，HTTP 仅适用于明确的本机开发接口。

本地数据位于 Tauri 为当前操作系统分配的应用数据目录：

- `settings.json`：保存 API Key、Base URL、模型和目标语言。
- `wordsnap.sqlite3`：保存英文单词、译文、查询次数和时间。

运行日志位于项目根目录的 `logs/` 文件夹。每次启动应用时都会创建一个独立日志文件，文件名
格式为 `YYYY-MM-DD_HH-mm-ss.SSS_pid-<进程ID>.log`。该次运行产生的 `DEBUG`、`INFO` 和
`ERROR` 日志写入同一个文件，直至应用退出。日志覆盖应用启动、初始化、快捷键、剪贴板处理、
翻译请求、数据库、窗口、设置和退出流程。日志只记录事件、结果、耗时、字符数量和 HTTP 状态码
等诊断元数据，不记录 API Key、选中文本、译文、剪贴板内容、完整服务响应或用户配置的接口地址。

> [!IMPORTANT]
> 选中文本会发送至用户配置的 API 服务。API Key 当前以明文形式保存在本机 `settings.json` 中，应用不使用系统密钥库。使用前应阅读[隐私与数据处理说明](docs/PRIVACY.md)。

## 从源代码运行

### 环境要求

- Node.js 20 或更高版本及 npm。
- Rust stable，并安装 `rustfmt` 和 Clippy。
- 当前平台所需的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。
- OpenAI 兼容 API Key。

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
| `npm run build` | 检查 TypeScript 并构建 Vite 前端 |
| `npm run format:check` | 检查 Rust 代码格式 |
| `npm run lint` | 对所有 Rust target 运行严格的 Clippy 检查 |
| `npm test` | 运行 Rust 单元测试 |
| `npm run check` | 运行前端构建、格式检查、Clippy 和测试 |
| `npm run audit` | 通过 npm 官方软件源检查高风险依赖漏洞 |
| `npm run tauri dev` | 启动完整桌面开发环境 |
| `npm run tauri build` | 为当前平台构建应用包 |
| `npm run icons` | 根据源文件重新生成应用图标和托盘图标 |

依赖发生变化时，应同时运行 `npm run audit`。本地打包产物位于 `src-tauri/target/release/bundle/`。

## 工作流程

1. 快捷键触发后，应用记录当前鼠标位置。
2. 应用暂存剪贴板，模拟复制操作以读取当前选区，并尝试恢复原剪贴板内容。
3. 应用根据输入文本和用户设置构造 OpenAI 兼容的 `/chat/completions` 请求。
4. 应用在选区附近显示加载状态和翻译结果。
5. 翻译完成后，点击卡片外部会隐藏浮窗。
6. 如果输入是单个 ASCII 英文单词，应用会在本地词表中新增或更新相应记录。

短语、句子、中文文本和翻译失败的内容不会写入词表。

## 项目结构

```text
.
├── docs/                    # 隐私说明、发布说明和演示素材
├── scripts/                 # 图标生成脚本
├── src/                     # TypeScript 窗口视图和样式
├── src-tauri/               # Tauri 配置、Rust 后端和应用资源
│   ├── capabilities/        # 前端可调用的 Tauri 权限
│   ├── icons/               # 应用图标和托盘图标
│   └── src/lib.rs           # 快捷键、选区、翻译、词表和窗口逻辑
├── CONTRIBUTING.md          # 贡献流程
├── SECURITY.md              # 安全报告流程和支持范围
└── package.json             # npm 命令和前端依赖
```

主要入口文件如下：

- `src-tauri/src/lib.rs`：应用状态、全局快捷键、选区读取、翻译请求、SQLite、窗口和托盘。
- `src/main.ts`：翻译浮窗、词表、设置和菜单视图。
- `src/styles.css`：生产界面的全部样式及浅色、深色主题行为。
- `src-tauri/tauri.conf.json`：窗口、打包和 Tauri 安全配置。

## 持续集成与发布

- `CI` 在推送到 `main` 分支或向 `main` 分支提交 Pull Request 时运行前端构建、Rust 格式检查、Clippy、测试和 npm 审计。
- `Release` 在每次推送到 `main` 分支时构建 macOS Apple Silicon、macOS Intel、Windows x64 和 Linux x64 应用包，并发布 GitHub Release。

发布格式、免安装产物和未签名构建说明见[发布流程](docs/RELEASE.md)。

## 常见问题

### 快捷键没有响应

- 确认 WordSnap 正在运行，并检查其他应用是否占用了 `Option+T` 或 `Alt+T`。
- 在 macOS 上检查辅助功能权限。
- 确认当前应用中的选中文本可以正常复制。

### 翻译请求失败

- 检查 API Key、Base URL 和网络连接。
- 确认当前 API Key 有权调用所选模型。
- 不得在 Issue、日志或截图中公开 API Key 和私人文本。

一般问题可以通过 [GitHub Issues](https://github.com/YunFy26/WordSnap/issues) 提交。安全漏洞应按照[安全策略](SECURITY.md)提交私密报告。
