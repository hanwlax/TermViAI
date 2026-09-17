# 工作区 UX 验证（2026-09-17）

本轮实现详情见 [WORKSPACE_UX.md](WORKSPACE_UX.md)。Windows x64 GUI Release 已构建，原生界面的布局、动画与真实 SSH 行为仍需人工验收。

## 已完成的检查

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked --release -p mux --lib -- --test-threads=1` | 16 项通过 |
| `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_ -- --test-threads=1` | 26 项通过 |
| `cargo test --locked --release -p wezterm-gui --bin wezterm-gui workspaces::tests -- --test-threads=1` | 7 项通过 |
| `cargo test --locked --release -p wezterm-gui --bin wezterm-gui hosts::tests -- --test-threads=1` | 2 项通过 |
| `cargo build --locked --release -p wezterm-gui` | Release 成功，退出码 0 |
| Windows PE Machine | `0x8664`，x64 |
| WSL 与 Windows 构建快照 | 47 个新增/修改源码与清单文件逐字节一致，含既有 Mocha 配置和广播实现 |
| 格式检查 | 修改文件 rustfmt 与 `git diff --check` 通过 |

共 51 项 Windows 测试通过。首轮测试为 mux 16、UI 19、工作区 7；最终增加 UI 回归并补跑 Hosts。中间一次测试编译发现新增断言要求 `Page: Debug`，已改用布尔断言，随后所有测试通过。最终测试后仅移除了未使用的 import 和 header 数据字段，重新编译 Release；未更改行为。

覆盖内容包括：无 pane 窗口保留与显式关闭、常规空窗口清理、header 行预留/分屏尺寸/移动失败回滚、默认标题随 pane 变化与手工名称保留、右侧重命名输入、窄窗口广播按钮不重叠、抽屉优先命中、广播栏退场预留释放及动画末帧、广播 Tab 隔离、嵌套工作区文件往返/去重/限额/多窗口缓存合并，以及损坏或未来版本数据不被覆盖。

## 产物

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\workspace-ux-20260917\wezterm-gui.exe
```

- exe 大小：72,733,184 字节。
- SHA256：`a2b302a74832320818da2b967409c64c6f8dbd61bd34f9adcf349f26e2630894`。
- 同目录包含 `OpenConsole.exe`、ConPTY、ANGLE 和 Mesa 运行依赖及使用说明。
- 历史版本保留在原目录。未停止用户进程，未覆盖历史版本。

日志、构建脚本、18 文件本轮同步清单、47 文件完整改动源码校验清单和发布文件校验清单归档在：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\workspace-ux-20260917
```

保留上游 OpenSSL PDB 缺失的 LNK4099 提示；预留图标以及供复用/测试的工作区保存接口仍有 dead-code 提示，不影响 Release 生成。

## 运行边界

没有自动操作 TermViAI 窗口，也没有连接用户的真实 SSH 主机。此前 Computer Use 明确返回 `product policy blocks this app`，没有改用其他自动化绕过。构建与逻辑测试不代表已完成实际窗口点击、动画观感、ConPTY、SSH、IME、vim/tmux、认证失败及断线恢复验收。

按 [功能说明中的人工步骤](WORKSPACE_UX.md#验证状态) 验收。保存的工作区用于按元数据重新建立连接和布局，不保存密码、私钥内容、远端程序状态、滚屏或输入命令；恢复广播默认关闭。历史 HTML 是旧版示例设计，不是本轮原生运行截图。
