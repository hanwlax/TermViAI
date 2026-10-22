# TermViAI v0.1.0-beta.4 验证记录

日期：2026-10-22（Asia/Shanghai）。Windows 10/11 x86_64 便携 Beta。

## 构建与测试

Windows 原生 MSVC Release，Rust 1.98.1。脚本 `termviai-beta4.cmd`，日志 `termviai-beta4.log`；证据归档在本地 Windows 构建目录的 `releases/verify-v0.1.0-beta.4`。

| 检查 | 结果 |
| --- | --- |
| wezterm-ssh 库测试 | 18 通过，包括 Windows 凭据管理器存取/替换/删除、临时密码、密码挑战选择和空闲轮询 |
| window 库测试 | 5 通过 |
| mux | 42 通过，包括组内终端交换、焦点、pane ID 和布局保留 |
| 广播策略 | 12 通过（11 单元 + 1 集成） |
| 完整 GUI 测试 | 101 通过，包括 Host/Identity 存储、认证选择、保存回滚、表单、广播、拖动、字体及工作区 |
| 合计 | 178 项通过 |
| Windows Release 编译和链接 | 成功 |
| CLI | `TermViAI 0.1.0-beta.4` |
| PE 产品字符串 | `TermViAI` / `0.1.0-beta.4` |
| PE FileVersion / ProductVersion 数值 | `0.1.0.4` |
| 嵌入应用图标 | ID 0x101；10 个尺寸与 `assets/windows/terminal.ico` 逐字节一致 |
| 构建快照 | 本轮 Rust/Cargo/版本输入与最终工作树一致 |
| Rust 格式 | `cargo +nightly fmt --all -- --check` 通过（2026-10-22 nightly） |
| 差异空白检查 | `git diff --check` 通过 |

Windows CI 增加 VERSION 触发路径，并执行完整 SSH 库及 GUI 测试。本地验证结果不等同于远端 CI 已完成。上游已有的弃用、dead code 和 PDB 等警告未阻止构建。

## 发布前审查

范围为 beta.3 后全部本地修改及新增文件，重点检查凭据持久化/回滚、认证挑战、旧配置兼容、组内交换和广播/重连边界。

修复一个已确认问题：重启后临时密码已丢失，再勾选“记住密码”时，原逻辑可能静默保留空凭据。现明确要求重新输入密码，保持磁盘配置不变；回归场景纳入 GUI 测试。另将 6 个既有 Rust 文件的格式调整为当前 nightly rustfmt 输出，没有行为变更。审查未发现其他需要阻止本次发布的问题；这不代表真实 GUI/SSH 场景已完成验收。

## 发布包

- ZIP：`TermViAI-v0.1.0-beta.4-windows-x86_64.zip`。
- 大小：29,821,824 字节。
- ZIP SHA-256：`fd25b4603e8a9d0af65b43e422baf65a73a6b45feacf47908b1d6aaf53b2a60d`。
- EXE SHA-256：`5d951f113fee4ca378b7b63a39b00242d8c5907778ae016164761b264732da33`。

包内包括 `TermViAI.exe`、`OpenConsole.exe`、`conpty.dll`、`libEGL.dll`、`libGLESv2.dll`、README、发布说明、MIT 许可证，以及 `SHA256SUMS.txt`。5 个 PE 文件均为 AMD64。ZIP CRC 与清单内 8 个文件的 SHA-256 全部校验通过。

对外附件为 ZIP 与 `.sha256`。发布流程要求从 GitHub Release 回传附件，复核 SHA-256、CRC 及逐文件清单；旧 Beta 标签和附件保留。Release：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.4>。

## 验证边界

凭据测试只使用唯一命名的合成测试凭据并清理，不使用真实 Host 密码。旧 JSON 的兼容、凭据引用及回滚已测试；密码不写入 Host/Keychain JSON。记住的密码属于当前 Windows 用户，不随便携 ZIP 或普通配置迁移；未记住的密码仅在当前进程有效。

未完成真实 SSH 密码/MFA 与四路长期空闲/睡眠唤醒验收，也未自动操作实际 GUI 验证拖放、DPI/GPU/IME 和 vim/tmux。SSH poll 修复有代码及回归测试依据，但不能据此断定它是所有断连的唯一原因。

默认禁用 `liga/clig/calt` 同时影响 `]#` 与 `>=`、`<=` 等编程连字，显式字体配置可覆盖；本次没有声称已实现仅针对 `]#` 的选择性连字处理。重连仍创建新的远端 shell，不恢复已结束的远端进程。
