# Windows GUI 构建恢复验证

日期：2026-09-07。范围仅 Windows x64 GUI。

## 结果

**系统应用控制阻塞已解除，Windows GUI Release 构建成功。尚未进行图形窗口、ConPTY、SSH 或完整 MVP 运行验收。广播策略仍未接入 GUI。**

用户明确授权关闭 Smart App Control，随后手动完成关闭。原生查询确认 `SmartAppControlState=Off`，`AntivirusEnabled=True`，`RealTimeProtectionEnabled=True`。

- Perl Encode 最小测试 `perl.exe -MEncode -e 1`：退出码 0（关闭前为 25）。
- 使用原有 `build-gui.cmd` 执行 `cargo build --locked --release -p wezterm-gui`：直接等待 cmd/Cargo 进程，退出码 0；Cargo 报告 `Finished release profile [optimized] target(s) in 12m 14s`。
- 生成 `C:\Users\h00893113\Documents\Codex\termviai-win\src\target\release\wezterm-gui.exe`，72,128,000 字节，PE Machine `0x8664`（x64）。
- GUI SHA256：`6e69995db9c1be97830ff3cda624cfc8cb5de2503d350e97504edef1a19a8510`。
- 同目录 `conpty.dll`、`OpenConsole.exe`、`libEGL.dll`、`libGLESv2.dll`、`mesa/opengl32.dll` 均存在，均为 x64；大小及 SHA256 见 `artifacts.json`。未据此声称单 exe 可移植。
- `wezterm-gui.exe --version` 可执行并以 0 退出，但输出为 `wezterm-gui someone forgot to call assign_version_info`。这只证明程序可加载到命令处理路径，版本信息仍异常，不能称为 GUI 窗口运行验收。

## 源码与工具链

源码主仓库仍为 `/home/hanwlax/workspace/termviai`，分支 `termviai/bootstrap`，上游基线 `d2f3f05b38f26a872f4b0bfbb3d2eaa7bdfc1b0b`。Windows 快照为 `C:\Users\h00893113\Documents\Codex\termviai-win\src`。

沿用 Rust 1.98.1 MSVC x64、Visual Studio 2026 Build Tools / MSVC 14.50.35717、Windows SDK 10.0.26100.0、便携 Strawberry Perl 5.42.2。仅构建 GUI 目标；其库依赖不代表构建了独立 CLI 或 mux-server 产品。

本次未修改生产源码、Cargo 配置、锁文件、构建参数或系统环境变量。主仓库及 Windows 快照中的 Cargo.toml、Cargo.lock、termviai/Cargo.toml、三个广播源文件，与之前六文件 SHA256 清单全部一致；不代表全树一致。已有未提交代码保留，本次仅更新交接文档。

## 警告与验收边界

编译有上游 Rust 生命周期警告、链接器标准输出警告，以及 OpenSSL `ossl_static.pdb` 缺失的 LNK4099 警告；链接器仍成功生成 exe。未为消除警告修改源码。

此前已完成的 11 项 Windows 广播策略测试仍为历史证据，本次没有重复执行。图形窗口启动、本地 shell/ConPTY、分屏、resize、clipboard、CJK/IME、SSH、vim/tmux 仍待验收；版本占位文本应在运行验收时排查。

## 原阻塞与恢复方式

CodeIntegrity/Operational 的 3077、3118 事件确认原阻塞是 Smart App Control 的 `VerifiedAndReputableDesktop` 策略，GUID `{0283ac0f-fff1-49ae-ada1-8a933130cad6}`，拒绝 getrandom 构建程序和 Perl Encode.xs.dll。详细初始诊断见 [WINDOWS_BUILD_BLOCKER_DIAGNOSIS.md](WINDOWS_BUILD_BLOCKER_DIAGNOSIS.md)。用户手动关闭该功能后，两项阻塞均已越过。

## 证据

新证据目录：`C:\Users\h00893113\Documents\Codex\2026-09-07\ch\outputs\windows-build-recovery`。

- [完整构建日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-recovery/build.log)
- [退出结果](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-recovery/result.json)
- [防护状态](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-recovery/security-state.json)
- [产物清单](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-recovery/artifacts.json)
- [版本输出](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-recovery/version.log)

此前失败日志和原构建脚本仍保留于相邻 `windows-build-validation` 目录，没有覆盖。
