---
name: termviai-windows-build
description: 在 TermViAI 项目中编译、验证和打包 Windows x86_64 原生产物。适用于 Windows 构建、试用包和发布包准备；不因使用本技能自动执行提交、推送或 GitHub 发布。
---

# TermViAI Windows 构建

使用现有 Rust 终端 / SSH 内核与 GPUI 原生界面，输出 `TermViAI.exe` 和可追溯的 Windows x86_64 ZIP。应用及公开产物名称不带 `-GPUI`，内部 crate / 目录名 `termviai-gpui` 保留。

## 构建前确认

- 从当前仓库定位 `termviai-gpui/Cargo.toml`、`build.rs`、根目录 `VERSION`、`.github/workflows/windows.yml`。该 crate 是独立 Cargo workspace，但依赖根仓库中的共享源码；不能只复制 `termviai-gpui/` 到独立目录编译。
- 检查分支、工作树、暂存区、新文件、删除文件和子模块状态。保留用户修改；构建需要包含当前任务的新文件，不能仅同步 `git diff --name-only`。
- 明确本次是试用包还是已获授权的发布包。编译请求不包含提交、推送、打标签、改写既有 Release 或上传产物；用户已明确授权时直接执行相应步骤，不反复确认。
- 全程使用后台命令。除非用户另行要求，不使用 computer-use、不启动可见应用、不操作用户桌面或真实 SSH/NPU 主机。自动化测试成功不等于屏幕效果已验收。

## 环境选择

Windows 本机从 MSVC x64 开发者命令行构建；WSL 工作树使用 Windows 原生编译器和 Windows 磁盘上的构建镜像，避免把 Linux 构建误当成 Windows 验证。

使用当前维护者的 Windows / WSL 环境时，读取 [references/local-environment.md](references/local-environment.md)。其中路径是已用环境记录，先检查存在性；换机器时按实际路径调整，不修改用户 HOME 或覆盖无关工具链。

需要 Rust MSVC x64 工具链、Visual Studio C++ Build Tools / Windows SDK，以及 native 依赖使用的 Perl、CMake 等工具。优先复用已有 Cargo / Rustup / target 缓存；不为普通增量构建清空 target 或升级依赖。

## 源码同步与格式

在 WSL 构建镜像流程中：

1. 首次镜像包含完整依赖源码和已初始化的子模块。增量同步包含已跟踪改动、暂存改动、任务相关新文件及共享模块；镜像里已删除的构建输入也要同步删除。只删除已确认属于该源码镜像的对应文件，不对构建根目录执行全量删除。
2. 不复制用户凭据、私有配置或开发环境数据。对构建源文件记录相对路径和 SHA256，构建完成前后核对工作树与镜像一致，避免打包旧源码或编译中变化的文件。记录当前 commit 和 dirty 状态；子模块单独核对。
3. 格式检查以仓库 CI 为准。原生 crate 自有 `.rs` 使用 edition 2024、`skip_children=true,imports_granularity=Preserve`；共享文件按所属 workspace 的格式规则处理，避免从 `#[path]` 递归格式化整个旧仓库。
4. 优先在工作树格式化后再同步。若现有 Windows 脚本会格式化镜像，检查差异，只回传本次相关格式化结果，并保留文件原有 Git 模式；Windows 文件权限不能直接当成 Linux 可执行位。

不要依赖 `/tmp` 中历史任务脚本、手写的新文件白名单或旧构建包的源码清单作为完整性依据。现有脚本可复用，但先确认它没有漏掉新模块、删除文件或新的共享依赖。

## 构建与测试

在已配置 MSVC 的 Windows 命令行，从源码仓库根目录执行：

```powershell
cargo build --locked --release --manifest-path termviai-gpui/Cargo.toml
cargo test --locked --release --manifest-path termviai-gpui/Cargo.toml --lib -- --test-threads=1
```

- 输出分别保存到本次构建的日志，检查进程退出码。出现编译或测试失败先定位首个实际错误，不使用旧 EXE 继续交付。
- 不固定“应该通过多少项测试”。从本次日志记录 passed / failed / ignored；ignored 的外部环境测试不能计为通过。
- 结合修改范围选择额外检查：终端核心、SSH、window、mux、广播策略对应 CI 中的测试命令；远端采集器改动运行 `python3 -B -m unittest discover -s wezterm-gui/tests -p 'test_*.py' -v`。未涉及的模块不必为了试用包全部重测。
- 同一源码已有适当且成功的验证时可复用，不重复耗时测试；后续代码改变了已测逻辑则重新验证相关范围。
- 长命令用后台会话与日志轮询，不阻塞反馈。不要同时启动争用同一 target 的多个构建 / 打包操作。

默认产物为 `termviai-gpui/target/release/TermViAI.exe`；设置了 `CARGO_TARGET_DIR` 时，使用其 `release/TermViAI.exe`。若显式指定 `--target`，路径还包含 target triple。以本次命令和 Cargo 输出为准，不拿历史 `termviai-gpui.exe` 充数。

## 打包与一致性

- 试用包示例：`TermViAI-trial-YYYYMMDD-feature-windows-x86_64.zip`。发布包示例：`TermViAI-v<version>-windows-x86_64.zip`。版本从当前发布任务和源码读取，不硬编码历史 beta 编号；试用构建不自动增加正式版本。
- 常规便携包只放 `TermViAI.exe`、`README.txt`、`LICENSES.txt`。README 简述本次变更、启动方法和实际验证范围；许可证保留全文。若引入额外非系统 DLL 或资源，必须包含运行必需文件，不能为凑“三文件”交付坏包。
- 许可证优先从现有受维护的清单生成。只有确认依赖与许可证集合仍适用时，才能复用旧发布包的完整 `LICENSES.txt`；不要只附项目许可证而遗漏字体、GPUI 和其他第三方依赖。
- 日志、源码清单、哈希及需要交付的源码放独立 developer 包或构建证据目录，不塞进用户运行目录。developer 包不是“完整可重建源码”时不要这样宣称。
- 用明确输入的 EXE 计算 SHA256，打包后验证 ZIP 完整性、成员列表，并比较 ZIP 内 EXE 与本次构建 EXE 的哈希。写入 `.zip.sha256`；更新说明后也要重新计算 ZIP 校验值。
- Python 构建文件名使用 `path.with_name(path.name + '.zip')`；不要对带 `0.1.0-beta.8` 等点号的包名用 `with_suffix('.zip')`，它会截掉版本后缀。
- 有必要验证命名和版本时检查 PE ProductName / OriginalFilename / ProductVersion；仅执行已确认会在创建界面前退出的 `TermViAI.exe --version`，并隐藏启动窗口。不要直接启动 EXE 进行“冒烟测试”。
- 普通试用包使用新目录，避免覆盖正在使用或已发布的包。用户明确要求覆盖既有版本时，先完成构建、校验和具体发布说明，再执行已经授权的替换；覆盖后重新核对远端标签、Release 和下载包。

## 交付

返回试用 ZIP 的绝对路径链接、构建结果、实际测试数量及必要限制。明确说明是否做过真实 SSH / 屏幕交互验证。仅构建时不擅自提交或发布；发布已获授权时返回核验后的 Release 链接，并说明实际提交 / 标签。
