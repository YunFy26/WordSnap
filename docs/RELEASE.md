# 发布流程

WordSnap 使用 GitHub Actions 执行持续集成和发布自动化：

- `CI`：在推送到 `main` 分支或向 `main` 分支提交 Pull Request 时运行。该流程安装前端依赖，构建 Vite 前端，并执行 Rust 格式检查、Clippy、Rust 测试和 npm 依赖审计。
- `Release`：在每次推送到 `main` 分支时运行，也可以在 Actions 页面手动启动。该流程构建 Windows x64、Linux x64、macOS Intel 和 macOS Apple Silicon 安装包，并发布 GitHub Release。

发布标签采用以下格式：

```text
wordsnap-v<app-version>-build-<github-run-number>
```

应用版本来自 `src-tauri/tauri.conf.json`。GitHub Actions 运行编号用于保证标签唯一，因此，即使应用版本未变化，每个进入 `main` 分支的提交仍可生成独立发布版本。

## 发布产物

每个发布版本同时提供常规安装包和免安装产物。对于多数用户，建议优先使用免安装产物。

| 平台 | 免安装产物 | 安装包 |
| --- | --- | --- |
| macOS Apple Silicon | `WordSnap_<version>_macos-aarch64_portable.zip` | `.dmg` |
| macOS Intel | `WordSnap_<version>_macos-x64_portable.zip` | `.dmg` |
| Windows x64 | `WordSnap_<version>_windows-x64_portable.zip` | `.msi`、NSIS `.exe` |
| Linux x64 | `*.AppImage` | `.deb`、`.rpm` |

macOS 免安装压缩包通过 `ditto -c -k --sequesterRsrc --keepParent` 生成，以保持 `WordSnap.app` 包结构完整。Windows 免安装压缩包包含独立的 `WordSnap.exe`，该文件是根据 `productName` 命名的 Tauri 主程序，构建路径为 `src-tauri/target/release/WordSnap.exe`。Linux AppImage 由配置项 `"targets": "all"` 直接生成，无需附加构建步骤。

### 使用免安装产物

- macOS：解压后运行 `WordSnap.app`。当前应用未签名。如果首次启动被 Gatekeeper 阻止，可以右键单击应用并选择打开，或在终端执行 `xattr -cr WordSnap.app` 后重新启动。
- Windows：解压后运行 `WordSnap.exe`。应用依赖系统中的 WebView2 运行时，Windows 10 和 Windows 11 通常已经包含该运行时。
- Linux：下载 AppImage 后执行 `chmod +x WordSnap_*.AppImage`，随后直接运行该文件。

## 本地打包

使用以下命令为当前平台生成生产包：

```bash
npm ci
npm run build
npm run tauri build
```

安装包和其他产物位于以下目录：

```text
src-tauri/target/release/bundle/
```

## GitHub 配置

发布工作流使用 GitHub 内置的 `GITHUB_TOKEN`。生成未签名构建时，无需配置额外密钥。

如果发布任务显示 `Resource not accessible by integration`，应在 GitHub 仓库中打开以下设置，并将工作流权限设为可读写：

```text
Settings > Actions > General > Workflow permissions > Read and write permissions
```

## 版本管理

准备用户可见的发布版本时，应在合并或推送到 `main` 分支前更新 `src-tauri/tauri.conf.json` 中的 `version`。

`package.json` 和 `src-tauri/Cargo.toml` 也包含项目版本。准备具名发布版本时，必须保持这三个文件中的版本一致。

## 代码签名

当前工作流生成未签名安装包，适用于内部测试和通过 GitHub 分发产物。macOS Gatekeeper 和 Windows SmartScreen 仍可能显示安全提示。

面向公众正式发布前，应增加相应平台的签名流程：

- macOS：使用 Apple Developer ID 签名并完成公证。
- Windows：使用 Authenticode 代码签名证书。
