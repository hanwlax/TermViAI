# 多 SSH 与广播验证（2026-09-17）

**Windows x64 Release 构建退出码 0，21 项 Windows 测试通过；另有 12 项 Linux 策略与真实 shell 管道测试通过。实际窗口点击、SSH 认证/ConPTY、IME 和 vim/tmux 仍需人工验收。**

## 新版

[TermViAI 程序](/mnt/c/Users/h00893113/Documents/Codex/termviai-win/releases/broadcast-20260917/wezterm-gui.exe)

Windows 路径：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-20260917\wezterm-gui.exe`。

大小：72,367,616 字节。SHA256：`a8456da7d9bdf6d1c0809f0c786d6b1bb9995b60cef954bbd50a62721271d7eb`。

同时附带 conpty.dll、OpenConsole.exe、libEGL.dll、libGLESv2.dll 和 mesa/opengl32.dll，全部逐字节校验且为 x64 PE。常规 `src/target/release/wezterm-gui.exe` 本轮也已更新。没有关闭用户窗口或启动真实 SSH 连接。

## 最终验证

| 范围 | 结果 |
| --- | --- |
| GUI Release | `cargo build --locked --release -p wezterm-gui` 通过，45.85s |
| Mux 用户输入集成 | 3 项通过：真实终端 Ctrl+C/方向键/中文括号粘贴、直接写入捕获、协议/跨线程隔离、错误/panic 清理、8 MiB 上限、后台短写入补齐 |
| GUI 状态与交互逻辑 | 4 项通过：Tab 隔离与关闭成员清理，加上抽屉和文本编辑回归 |
| 主机库 | 2 项通过：地址校验、保存重读与损坏文件保护 |
| Windows 策略核心 | 11 项通过：成员/独立/开关/生命周期/断线/失败隔离/字节一致性 |
| Windows 真实进程 | 1 项通过：四个 cmd.exe 管道，三成员同步、第四个独立、OFF 后独立输入 |
| Linux 策略与 shell | 11 + 1 项通过；四个 /bin/sh 管道测试，不构建 WSL GUI |
| 格式和源码 | rustfmt、git diff --check 通过；19 个本轮源文件与 Windows 快照一致 |

本轮集成测试发现并修复了“捕获位于 ThreadedWriter 之后导致普通按键/粘贴漏广播”的问题。最终捕获位于终端用户编码输出进入异步通道之前；广播成员复用后台队列，不重新编码。后台 writer 使用 write_all 处理短写入。

## 日志与限制

[终端及 GUI 测试日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/tests-verified.log) 包含最后一项 Windows shell 的早期断言失败（提示符与输出在同一行）；[最终核心回归及 Release 日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/release.log) 记录该断言修正后的通过结果与 Release 构建。按测试名合并后的 21 项最终结果、产物校验见 [result.json](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/result.json)。[Linux 日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/core-linux.log)、[源文件清单](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/source-files.json)、[修改前备份](/mnt/c/Users/h00893113/Documents/Codex/2026-09-17/outputs/termviai-broadcast/before.tar.gz) 一并保留。

单测不等同于真实 SSH/ConPTY 或图形运行验收。Computer Use 先前对该应用返回 `product policy blocks this app`，本轮没有用替代 UI 自动化绕过限制，也未尝试连接用户主机。仍有上游生命周期及 OpenSSL PDB 缺失警告，未阻止构建。

使用方法、输入边界、运行时成员状态和现有队列限制见 [BROADCAST.md](BROADCAST.md)。主机库和 SSH 认证实现沿用原有路径；没有提交或推送。
