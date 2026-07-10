# Privacy and data handling

WordSnap 不包含账号系统、云同步、广告或分析 SDK。它仍然需要把选中文本发送给你配置的翻译接口，因此“本地工具”不等于“所有数据都只在本机处理”。

## Data flow

| Data | Where it goes | Why |
| --- | --- | --- |
| 选中的文本 | 内存、用户配置的 OpenAI 兼容接口 | 生成翻译结果 |
| API Key、Base URL、模型和目标语言 | 本机应用数据目录中的 `settings.json` | 保存用户配置 |
| 单个英文单词、译文、次数和时间 | 本机 `wordsnap.sqlite3` | 展示本地词表 |
| 短语、句子和中文输入 | 不写入词表数据库 | 只用于当前翻译请求 |
| 剪贴板内容 | 本机临时读取并尽力恢复 | 通过模拟复制获取当前选区 |

当前代码没有遥测或使用分析。网络翻译请求由用户选择的 API 提供方处理；请在使用前确认该提供方的隐私政策、日志和数据保留规则。

## Local storage

WordSnap 使用 Tauri 为当前操作系统分配的应用数据目录：

- `settings.json` 以明文保存设置和 API Key。它不使用 macOS Keychain、Windows Credential Manager 或其他系统密钥库。
- `wordsnap.sqlite3` 保存词表。重复单词会更新翻译、次数和最近使用时间。
- Unix 平台会尝试把 `settings.json` 权限设置为 `0600`；这不能防止已经以当前用户身份运行的其他进程读取该文件。

不要在共享电脑或不受信任的用户账户中保存高权限 API Key。推荐使用额度受限、可轮换的独立 Key。

## Network transport

远程翻译接口应使用 `https://`。WordSnap 允许 `http://`，主要用于本机开发服务；对远程主机使用 HTTP 会暴露 API Key 和选中文本。

Base URL 会被规范化，并在需要时自动追加 `/chat/completions`。请求使用 Bearer Token 发送 API Key。

## Clipboard behavior

快捷键触发时，WordSnap 会暂存剪贴板、模拟 `Cmd+C` 或 `Ctrl+C`、读取选区，再尝试恢复原内容。macOS 和其他平台的剪贴板格式支持不同；在非 macOS 平台，复杂的富文本、图片或自定义剪贴板格式可能无法被完整保留。请不要把剪贴板当作唯一副本保存重要内容。

## Removing data

- 在词表窗口中可以逐条删除保存的单词。
- 要删除全部本地数据和已保存 API Key，请先退出 WordSnap，再删除操作系统中 `com.yuntsy.wordsnap` 对应的应用数据目录。
- 要让旧 Key 立即失效，还应在 API 提供方控制台中撤销或轮换它。

报告问题或分享截图前，请移除 API Key、私人文本、数据库内容和带凭据的接口地址。安全问题请按照 [SECURITY.md](../SECURITY.md) 私密报告。
