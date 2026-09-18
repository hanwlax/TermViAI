# TermViAI v0.1.0-beta.2 发布验证

日期：2026-09-18（Asia/Shanghai）。正式源码由 `v0.1.0-beta.2` 标签标识，开发分支为 `main`。

## 提交范围

保留已推送的功能提交，追加统一版本和发布文档提交：

- `8189da02d`：SSH 断线 pane 保留与原位重连。
- `279d7c714`：失效 SSH 缓存恢复、整组重连及错误隔离。
- `d66899c4d`：重连失败及恢复后输出不覆盖历史。
- `82d2e6558`：New Tab 移除自动历史组。
- `50a6e05ea`：重建 SSH 重连输入线程和队列，修复源终端广播写入失败。

## Windows x64 Release 验证

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked --release -p mux -- --test-threads=1` | 36 通过 |
| `cargo test --offline --locked --release --manifest-path termviai/Cargo.toml` | 12 通过 |
| GUI `termviai_` 测试 | 61 通过 |
| GUI `workspaces::tests` 测试 | 7 通过 |
| `cargo build --locked --release -p wezterm-gui` | 成功 |
| `TermViAI.exe --version` | `TermViAI 0.1.0-beta.2` |
| Windows ProductName | `TermViAI` |
| Windows FileVersion / ProductVersion | `0.1.0-beta.2` |
| Windows 数值文件 / 产品版本 | `0.1.0.2` |
| ZIP CRC 与包内文件清单 | 通过 |

共 116 项不同测试通过。Windows shell 路由测试中的旧品牌测试标记已更新，并单独重跑广播包的 12 项测试通过，不重复计数。构建中仍有上游弃用、dead code 及 OpenSSL PDB 警告。

## 产物

- ZIP：`TermViAI-v0.1.0-beta.2-windows-x86_64.zip`。
- 大小：29,599,156 字节。
- ZIP SHA-256：`2dbbaf31f10acbfa2bfbe07b94765ae22ea8b8cb24f6f0c29490365c36ec97e8`。
- `TermViAI.exe` SHA-256：`066de249097122947c6d64ce68125d5b10e46240a0a3cdbb77b28fc1d061313e`。

压缩包内包含 `TermViAI.exe`、`OpenConsole.exe`、`conpty.dll`、`libEGL.dll`、`libGLESv2.dll`、MIT 许可证、README、发布说明和各文件的 `SHA256SUMS.txt`。外部 `.zip.sha256` 与 ZIP 一起作为 Release 附件。

发布操作最后须重新下载 GitHub 附件，验证下载 ZIP 与上述 SHA-256 相同，并检查包内校验清单。

## 验证边界

已验证原生 Windows 编译及逻辑回归，未自动操作真实应用窗口和用户 SSH 主机。真实断网/唤醒后重连、认证、广播、GPU、IME 与 vim/tmux 仍需人工验收。旧连接中未发送的输入不会自动重放到新 shell。
