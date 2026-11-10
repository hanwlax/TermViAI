# 当前维护者的 Windows / WSL 环境

以下为 2026-11-10 使用过的配置，属于环境线索，不是每台机器的必需路径。使用前核对文件、工具链及 Cargo 配置。

| 用途 | 路径 |
| --- | --- |
| WSL 工作树 | `/home/hanwlax/workspace/TermViAI` |
| Windows 构建根目录 | `C:\Users\h00893113\Documents\Codex\termviai-win` |
| WSL 访问构建根目录 | `/mnt/c/Users/h00893113/Documents/Codex/termviai-win` |
| 完整源码镜像 | `<构建根目录>\src` |
| 原生 crate | `<构建根目录>\src\termviai-gpui` |
| Cargo 缓存与可执行文件 | `<构建根目录>\cargo`、`cargo\bin` |
| Rustup 工具链 | `<构建根目录>\rustup` |
| CARGO_TARGET_DIR | `<构建根目录>\gpui-probe\target` |
| 当前 EXE | `<构建根目录>\gpui-probe\target\release\TermViAI.exe` |
| 试用 / 发布文件 | `<构建根目录>\releases` |
| MSVC 初始化 | `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat` |
| Perl | `<构建根目录>\perl\perl\bin` |
| CMake | `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin` |

## 已存在的后台入口

构建根目录中有 `gpui-native-build.cmd`、`gpui-native-tests.cmd`，日志分别为 `gpui-native-build.log`、`gpui-native-tests.log`；名称保留 GPUI 不代表公开产物也带此后缀。

复用前阅读脚本。历史 build 脚本包含手写 rustfmt 文件列表，可能遗漏新模块；有的 test 脚本未使用 `--locked`。按当前任务纠正命令后使用，不把旧脚本内容视为规范。

从 WSL 调用的形式：

```bash
/mnt/c/Windows/System32/cmd.exe /d /c 'C:\Users\h00893113\Documents\Codex\termviai-win\gpui-native-build.cmd'
```

使用后台执行会话继续观察日志。建议命令工作目录设为 `/mnt/c` 或 `/tmp`，脚本内部 `cd /d` 到镜像，避免 CMD 在 WSL UNC 工作目录下的警告。

沙盒可能允许读取 Windows 文件但不允许写镜像、执行 Windows 工具或写项目技能目录。使用执行工具提供的权限升级机制完成用户已授权的动作；不改动其他目录权限绕过限制。

## 重建入口时的环境骨架

下面是 Windows `.cmd` 内容示例，不是 PowerShell：路径确认后再写入独立任务脚本。不要直接用 WSL 的 rustc 生成 Windows 发布包。

```bat
@echo off
setlocal
set "PATHEXT=.COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC"
set "TVAI_ROOT=C:\Users\h00893113\Documents\Codex\termviai-win"
call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 exit /b %errorlevel%
set "RUSTUP_HOME=%TVAI_ROOT%\rustup"
set "CARGO_HOME=%TVAI_ROOT%\cargo"
set "CARGO_TARGET_DIR=%TVAI_ROOT%\gpui-probe\target"
set "CARGO_BUILD_JOBS=6"
set "PATH=%TVAI_ROOT%\cargo\bin;%TVAI_ROOT%\perl\perl\bin;C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;%PATH%;%TVAI_ROOT%\perl\c\bin"
cd /d "%TVAI_ROOT%\src"
if errorlevel 1 exit /b %errorlevel%
cargo build --locked --release --manifest-path termviai-gpui/Cargo.toml > "%TVAI_ROOT%\gpui-native-build.log" 2>&1
if errorlevel 1 exit /b %errorlevel%
cargo test --locked --release --manifest-path termviai-gpui/Cargo.toml --lib -- --test-threads=1 > "%TVAI_ROOT%\gpui-native-tests.log" 2>&1
exit /b %errorlevel%
```

编译并发数 `6` 是该机器已用值，可按实际内存调整。已有格式、构建和测试成功日志可作为复用线索，但必须核对对应源码，不能凭旧日志宣称新改动通过。
