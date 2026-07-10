# Contributing to WordSnap

感谢你愿意改进 WordSnap。项目当前专注于“选中文本、快捷翻译、自动记录英文单词”，小而可靠比快速扩张功能更重要。

## 开始之前

- Bug、兼容性问题和小型改进可以直接创建 Issue。
- 较大的功能或会改变数据格式、快捷键、网络请求、发布流程的改动，请先创建 Feature Request 讨论边界。
- 安全漏洞不要公开披露，请按照 [SECURITY.md](SECURITY.md) 报告。

## 本地开发

需要 Node.js 20 或更高版本、npm、Rust stable，以及当前平台所需的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/YunFy26/WordSnap.git
cd WordSnap
npm ci
npm run tauri dev
```

只查看前端界面时可以运行 `npm run dev`。完整检查命令为：

```bash
npm run check
```

它会依次执行 TypeScript/Vite 构建、Rust 格式检查、Clippy 和 Rust 单元测试。

提交依赖相关改动时，另行运行 `npm run audit`；该命令需要访问官方 npm registry。

## 提交改动

1. 从最新 `main` 创建一个短生命周期分支。
2. 保持改动单一、可审查，不在同一个 PR 中混入无关重构。
3. 为新增的纯逻辑或修复的回归补测试；涉及窗口、快捷键、剪贴板时，写清手动验证平台和步骤。
4. 运行 `npm run check`。
5. 填写 PR 模板，关联对应 Issue，并说明用户可见变化。

提交信息推荐使用简洁的 Conventional Commit 风格，例如：

```text
fix(clipboard): preserve copied content after translation
feat(settings): support a custom model id
docs: clarify local data storage
```

## 隐私与安全要求

- 不要提交 API Key、真实选中文本、个人数据库、日志或应用数据目录。
- 截图和报错信息必须移除密钥、接口地址中的凭据以及敏感文本。
- 不要在日志中记录完整 API Key 或默认记录用户选中的内容。
- 对网络请求、本地存储、剪贴板和 Tauri capability 的改动，需要在 PR 中说明数据流和风险变化。
- 除明确面向本机开发服务外，不应把远程 API 的 HTTPS 地址降级为 HTTP。

## 代码与产品边界

- Rust 由 `rustfmt` 格式化并通过严格 Clippy。
- TypeScript 保持 `strict` 模式通过。
- 新增 UI 应同时考虑键盘操作、焦点可见性、浅色/深色模式和长文本。
- 保持第一版边界：不默认扩展为 OCR、截图翻译、词典管理或学习系统。

参与本项目即表示你同意遵守 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。
