# WordSnap 贡献指南

WordSnap 当前专注于选中文本、快捷翻译和自动记录英文单词。项目优先保证既有功能的可靠性，不以扩大功能范围为主要目标。

## 提交建议前

- 对于可复现的缺陷、兼容性问题和范围较小的改进，可以直接创建 Issue。
- 对于较大的功能，或可能改变数据格式、快捷键、网络请求和发布流程的变更，应先创建 Feature Request，讨论实施范围。
- 不得公开披露安全漏洞。请按照 [安全策略](SECURITY.md) 中的流程提交报告。

## 本地开发

开发环境需要 Node.js 20 或更高版本、npm、Rust stable，以及当前平台所需的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。

```bash
git clone https://github.com/YunFy26/WordSnap.git
cd WordSnap
npm ci
npm run tauri dev
```

如果仅需查看前端界面，可以运行 `npm run dev`。完整检查命令如下：

```bash
npm run check
```

该命令依次执行 TypeScript 和 Vite 构建、Rust 格式检查、Clippy 检查和 Rust 单元测试。

提交依赖相关变更时，还应运行 `npm run audit`。该命令需要访问 npm 官方软件源。

## 提交变更

1. 基于最新的 `main` 分支创建短期开发分支。
2. 保持变更范围单一且便于审查，不得在同一个 Pull Request 中混入无关重构。
3. 为新增纯逻辑或回归修复补充测试。涉及窗口、快捷键和剪贴板时，应说明手动验证平台和步骤。
4. 运行 `npm run check`。
5. 填写 Pull Request 模板，关联相应 Issue，并说明用户可见的变化。

提交信息建议采用简洁的 Conventional Commits 格式，例如：

```text
fix(clipboard): preserve copied content after translation
feat(settings): support a custom model id
docs: clarify local data storage
```

## 隐私与安全要求

- 不得提交 API Key、真实选中文本、个人数据库、日志或应用数据目录。
- 截图和错误信息必须移除密钥、接口地址中的凭据及其他敏感文本。
- 不得在日志中记录完整 API Key，也不应默认记录用户选中的内容。
- 对网络请求、本地存储、剪贴板和 Tauri 权限的变更，必须在 Pull Request 中说明数据流及风险变化。
- 除明确用于本机开发服务外，不得将远程 API 地址从 HTTPS 降级为 HTTP。

## 代码与产品范围

- Rust 代码必须通过 `rustfmt` 和严格的 Clippy 检查。
- TypeScript 代码必须通过 `strict` 模式检查。
- 新增界面应同时考虑键盘操作、焦点可见性、浅色与深色主题，以及长文本显示。
- 第一版不应扩展为 OCR、截图翻译、词典管理或语言学习系统。

参与本项目即表示同意遵守 [行为准则](CODE_OF_CONDUCT.md)。
