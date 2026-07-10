# Security Policy

## Supported versions

WordSnap 仍处于早期阶段。安全修复只会进入最新发布版本和 `main` 分支，旧版本不单独维护。

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| `main` | Yes |
| Older releases | No |

## Reporting a vulnerability

请不要在公开 Issue、讨论区或 Pull Request 中披露漏洞细节、API Key、真实选中文本或可直接利用的复现数据。

1. 在仓库的 **Security** 页面使用 **Report a vulnerability** 私密报告功能。
2. 如果该功能暂不可用，请创建一个不包含漏洞细节的普通 Issue，只说明需要与维护者建立私密联系。
3. 提供受影响版本、平台、影响范围、最小复现步骤和可能的缓解措施；所有密钥与个人数据都应脱敏。

维护者会先确认收到报告，再评估影响、准备修复并协调披露时间。早期项目无法承诺固定响应时限，但会优先处理可能泄露 API Key、选中文本、剪贴板或本地词表的数据问题。

## Important security notes

- 选中文本会发送给用户配置的 OpenAI 兼容服务。该服务的隐私政策和数据保留规则由其提供方决定。
- API Key 当前保存在应用数据目录的 `settings.json` 中，不使用系统钥匙串。Unix 平台会尝试把文件权限设为仅当前用户可读写。
- 使用远程服务时应配置 HTTPS。HTTP 地址可能让 API Key 和选中文本以明文经过网络。
- 应用会通过模拟复制读取选区并尝试还原剪贴板；涉及剪贴板恢复失败或数据丢失的问题应按安全/隐私问题处理。

完整数据流和本地清理方式见 [docs/PRIVACY.md](docs/PRIVACY.md)。
