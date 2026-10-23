# TermViAI v0.1.0-beta.5 验证记录

日期：2026-10-23（Asia/Shanghai）。Windows 10/11 x86_64 便携 Beta。

## 构建与测试

Windows 原生 MSVC Release。发布测试脚本 `termviai-beta5.cmd`，日志 `termviai-beta5.log`；最终资源重编译日志 `termviai-beta5-final-build.log`。证据归档于本地构建目录 `releases/verify-v0.1.0-beta.5`。

| 检查 | 结果 |
| --- | --- |
| SSH 库 | 19 通过，含显式认证模式隔离、密钥第二因子与合成凭据存取 |
| window 库 | 5 通过 |
| 终端核心 | 54 通过，含行首 resize/CRLF 与已有折行测试 |
| mux | 42 通过 |
| 广播策略 | 12 通过（11 单元 + 1 集成） |
| 完整 GUI | 128 通过，含认证、分组、删除、移动、历史和父子悬浮回归 |
| 合计 | 260 项通过 |
| 最终 Windows Release 编译 | 成功 |
| CLI | `TermViAI 0.1.0-beta.5` |
| PE 产品字符串 | `TermViAI` / `0.1.0-beta.5` |
| PE FileVersion / ProductVersion 数值 | `0.1.0.5` |
| 嵌入图标 | ID 0x101，10 个尺寸与源 ICO 逐字节一致 |
| 构建源码 | 556 个 Rust/TOML/lock/版本/图标输入与工作树哈希一致 |
| 格式与差异 | nightly rustfmt 与 git diff --check 通过 |

检查发现第一轮产物仍含旧数值资源版本；同步资源并重编译后，最终 EXE 上述检查全部通过。260 项行为测试基于同一 beta.5 Rust 行为代码，资源数值修正后未重复无关测试。

Windows CI 新增终端核心测试；本地结果不等同于远端 CI 已完成。上游已有弃用、dead code、PDB 等警告未阻止构建。

## 发布前审查

范围为 beta.4 后的全部本地变更，包括新文档。重点核对互斥认证与旧配置兼容、凭据持久化/回滚、分组删除提升及旧文本迁移、批量移动的过期条目检查、最近连接合并、行首重排与父子悬浮隔离。No actionable findings：未发现需要阻止本次发布的已证实问题。此前用户反馈的首页误取消分组、菜单主动跳转、焦点横线、面包屑宽度与悬浮闪烁均已纳入最终实现。

## 发布包

- ZIP：`TermViAI-v0.1.0-beta.5-windows-x86_64.zip`。
- 大小：29,919,205 字节。
- ZIP SHA-256：`ca4e680038fe7be0e6a1af454337a3a79ef733634f24313077aba13bbeac656e`。
- EXE SHA-256：`99c5a58f490ca427f2224cd622ec7dc6b24c2f4f79f47789d4e9aa668a1c3d2f`。

包含 TermViAI.exe、四个运行依赖、README、发布说明、认证/分组说明、MIT 许可证和 SHA256SUMS.txt。5 个 PE 文件均为 AMD64。ZIP CRC 及清单内 10 个文件的 SHA-256 校验通过。

发布附件为 ZIP 与 `.sha256`，保留旧 Beta。发布流程还需从 GitHub Release 下载附件，核对 ZIP SHA-256、CRC 及逐文件清单，并将回传验证记录留在本地证据目录。Release：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.5>。

## 边界

所有凭据测试使用唯一命名的合成测试数据并清理，不读取真实 Host 密码。密码由当前 Windows 用户凭据管理器保存，不随便携包或 JSON 迁移。升级前可备份数据目录；旧版本不了解新认证/分组字段，不建议回退后编辑已升级数据。

真实 SSH 密码/私钥/MFA、四路长期空闲与睡眠唤醒、原生鼠标动画、DPI/GPU/IME、vim/tmux 仍需人工验收。重连创建新的远端 shell，不恢复远端进程。当前没有内置 AI 功能。
