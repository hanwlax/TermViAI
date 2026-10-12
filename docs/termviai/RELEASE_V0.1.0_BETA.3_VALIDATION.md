# TermViAI v0.1.0-beta.3 验证记录

日期：2026-10-12（Asia/Shanghai）。Windows 10/11 x86_64 便携 beta。

## 构建与测试

Windows 原生 MSVC Release：`termviai-beta3.cmd`，日志 `/tmp/termviai-beta3.log`。

| 检查 | 结果 |
| --- | --- |
| wezterm-ssh 库测试 | 13 通过，含会话生命周期回归 |
| window 库测试 | 5 通过，含最大化客户区、光标映射和实时缩放队列 |
| mux | 41 通过，含 tab 排序与原 tab 对象保留 |
| 广播策略 | 12 通过（11 单元测试 + 1 集成测试） |
| TermViAI GUI | 62 通过，含拖动插入位置与原位置不提示 |
| 工作区存储 | 7 通过 |
| 合计 | 140 项通过 |
| Windows Release 编译和链接 | 成功 |
| CLI | `TermViAI 0.1.0-beta.3` |
| PE 产品字符串 | `TermViAI` / `0.1.0-beta.3` |
| PE FileVersion / ProductVersion 数值 | `0.1.0.3` |
| 新图标 | ID 0x101；10 个尺寸的图像数据与新 ICO 逐字节一致 |
| 构建快照 | 本轮 Rust/Cargo/版本/图标输入与当前工作树一致 |

版本源同步修改 `VERSION`、GUI Cargo 版本、`Cargo.lock` 和 Windows PE 数值资源。源码没有因图标修改引入新运行依赖。已存在的上游弃用、dead code 和 OpenSSL PDB 警告未阻止构建。

## 发布包

- ZIP：`TermViAI-v0.1.0-beta.3-windows-x86_64.zip`。
- 大小：29,767,844 字节。
- SHA-256：`1f60d24c1622af1cf7b9cbc1f08c582e4ad07d6da4a9629785fa15fd7279a9a9`。
- EXE SHA-256：`a110242abec058fae7e7e38eb6c061d4a2dc419eff661ef8a6bb8ac0f9fccff8`。
- EXE、OpenConsole.exe、conpty.dll、libEGL.dll、libGLESv2.dll 的 PE 架构均为 AMD64。
- ZIP CRC 和包内 8 个文件 SHA-256 已核验，另含 `SHA256SUMS.txt`。
- ZIP 外部 `.sha256` 与便携包一起上传到 prerelease；原 beta.1/beta.2 不覆盖。

发布后从 GitHub 回传 ZIP 和 `.sha256`，核对上述摘要、CRC 与包内清单。Release 地址：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.3>。

## 验证边界

真实四 pane 同主机整组重连、鼠标拖动、小手视觉、最大化顶栏偶发事件、跨屏 DPI、网络断开/睡眠唤醒、GPU/IME 及 vim/tmux 未自动验收。源码修复和单元测试不是上述交互场景已经通过的证据。客户端数据格式未改变，重连仍创建新的远端 shell。
