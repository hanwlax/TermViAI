# 构建与开发

## 广播策略模块

在项目根目录执行：

```sh
cargo test --offline --locked --manifest-path termviai/Cargo.toml
```

无第三方依赖。Windows 上可运行策略单元测试；四 shell 管道测试仅在 Unix 上运行。
测试不会连接用户的 SSH 主机。

## Windows 原生 GUI 基线

以仓库中的 `docs/install/source.md` 和 `.github/workflows/gen_windows.yml` 为基准。
需要 Rust MSVC x64 工具链、Visual Studio C++ build tools / Windows SDK，
以及排在 PATH 前面的 Strawberry Perl。WSL 的 GNU 工具链不能替代 Windows/MSVC 验收。

建议将当前工作树复制到 Windows 本地磁盘进行构建，避免原生构建工具的 UNC 路径问题。
如果 TermViAI 改动尚未提交，仅执行 git clone 不会包含这些改动，必须复制工作树或先提交。
主要开发目录仍为 `/home/hanwlax/workspace/termviai`。

在 Windows Developer PowerShell 的项目目录中执行：

```powershell
$env:Path = "C:\Strawberry\perl\bin;" + $env:Path
git submodule update --init --recursive
if ($LASTEXITCODE -ne 0) { throw 'Submodule initialization failed' }
cargo build --locked --release -p wezterm-gui
if ($LASTEXITCODE -ne 0) { throw 'Windows baseline build failed' }
.\target\release\wezterm-gui.exe --skip-config start --always-new-process
```

目前二进制名称和 UI 仍是 WezTerm。没有把包重命名为 TermViAI，以避免在建立基线前改动打包链。

基线验收记录应包含：本地 PowerShell/cmd、SSH、split、resize、clipboard、CJK/IME、
vim/tmux/top。完成这些之后再接入 GUI 广播。完整 MVP 要进一步验证广播和恢复行为。

## 验证范围

按用户要求，仅构建 Windows x64 GUI。不构建 WSL GUI、Windows CLI 或 mux-server。
最新原生构建结果见 [Windows 构建验证](WINDOWS_BUILD_VALIDATION.md)。

## 本次工具链

本次测试使用任务目录 `work/termviai-bootstrap/` 内的隔离 Rust 工具链；没有修改用户 PATH。
后续正常开发需在选定平台配置自己的 Rust 工具链。

Windows 原生隔离工具链位于 `C:\Users\h00893113\Documents\Codex\termviai-win`。
2026-09-07 首次原生构建被 Smart App Control 拦截；用户手动关闭后，GUI Release 构建成功（退出码 0）。Defender 杀毒与实时保护仍开启。图形窗口及 SSH 等运行验收待完成，详见验证记录。

## TermViAI 界面（2026-09-08）

现有 GUI 构建目标已包含默认开启的 TermViAI 原生界面。运行同一 `target/release/wezterm-gui.exe` 即可进入 Hosts 页面，无需账户登录。使用 `--skip-config` 可以排除已有 Lua 配置影响。主机数据和操作说明见 [UI_IMPLEMENTATION.md](UI_IMPLEMENTATION.md)，最新构建结果见 [UI_VALIDATION.md](UI_VALIDATION.md)。
