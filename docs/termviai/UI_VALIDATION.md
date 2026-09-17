# TermViAI UI 验证（2026-09-08，右侧抽屉更新）

**Windows 原生 5 项测试通过，最终 GUI 编译和链接成功。旧程序正在运行，Cargo 最后替换 exe 失败；已将本轮链接产物和依赖复制到独立目录。原生拖动及最终视觉、点击仍需人工验收。**

## 本次修改

Add Host、Edit Host 和 Add Key 共用右侧抽屉，外部点击取消且不穿透，内部空白保持打开；独立渲染层修复原先背景文字叠在表单上的问题。小窗口支持滚动字段和键盘焦点跟随，底部固定保存按钮。顶栏空白与 TermViAI 标题在鼠标移动时设置 Windows caption 命中，同时保留已有窗口拖动回退路径。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| `hosts::tests` | 2 项通过：SSH 地址解析、主机库保存与损坏文件保护 |
| `termwindow::termviai_ui::tests` | 3 项通过：外部点击隔离/内部命中/顶栏命中、布局与焦点滚动、调色板与 Unicode 编辑 |
| Windows Release 编译和链接 | 成功生成新的 `target/release/deps/wezterm_gui.exe` |
| Cargo 最终复制 | 退出码 101；旧 `target/release/wezterm-gui.exe` 被运行进程占用，删除时报 os error 5，旧文件未替换 |
| 新版打包 | 从最终测试之后生成的链接产物复制，附带 5 个依赖文件，逐字节一致且 PE 架构均为 x64 |
| 源码核对 | 修改的 `termviai_ui.rs` 与 Windows 快照一致 |
| 格式检查 | rustfmt 和 `git diff --check` 通过 |
| 最终视觉、原生鼠标拖动、resize、IME、SSH | 未执行；单元测试不能替代原生事件验收 |

仍有上游生命周期和 OpenSSL 缺少 PDB 的警告。没有为本次改动关闭已有窗口或结束用户会话。

## 新版产物

[打开新版所在目录中的程序](/mnt/c/Users/h00893113/Documents/Codex/termviai-win/releases/drawer-20260908/wezterm-gui.exe)

Windows 路径：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\drawer-20260908\wezterm-gui.exe`。

大小：72,258,048 字节。SHA256：`2da18189e4daafdb301767edbe13fc97ad97e087afb2c1b53eff95bfefbb0e30`。

同目录包括 `conpty.dll`、`OpenConsole.exe`、`libEGL.dll`、`libGLESv2.dll`、`mesa/opengl32.dll`。这是独立验收目录，原 `src/target/release/wezterm-gui.exe` 仍是旧版本。停止旧程序后应打开上述新路径；后续常规构建可在旧 exe 不被占用时恢复覆盖。

## 验证限制与人工检查

Computer Use 对该应用返回 `product policy blocks this app`，没有绕过限制执行自动点击或截图。建议在新程序依次检查 Hosts 添加/编辑、Keychain 添加、外部空白收起、内部空白保留、小窗口滚轮与 Tab、普通及最大化窗口顶栏拖动、窗口按钮与 tab 点击。

主机数据格式和位置未改变，本轮未写入正式主机库。原有广播代码及未提交工作保留，没有提交或推送。

## 证据

- [最终构建日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/build-final.log)：编译链接完成，最后复制拒绝访问。
- [最终测试日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/tests-final.log)：5 项通过。
- [产物与源码清单](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/result.json)：路径、大小、SHA256、x64 架构。
- [修改前文件备份](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/before.tar.gz)。

首版 UI 的历史构建结果、原生窗口响应检查及旧产物记录保留在 [首版证据目录](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-ui/REPORT.md)。本轮不沿用旧产物的运行检查作为验收依据。
