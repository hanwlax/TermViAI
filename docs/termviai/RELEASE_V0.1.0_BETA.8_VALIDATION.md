# TermViAI v0.1.0-beta.8 验证记录

日期：2026-11-10（Asia/Shanghai）。Windows x86_64 MSVC Release。

## 首次发布验证（替换前）

| 检查 | 结果 |
| --- | --- |
| 原生界面及共享后端库测试 | 88 通过、0 失败、3 项外部环境测试忽略 |
| Linux Python 采集器与转发测试 | 34 通过、0 失败 |
| 根 workspace nightly rustfmt | 通过 |
| 原生界面 edition 2024 rustfmt | 通过 |
| git diff --check | 通过 |
| Windows Release 构建 | 通过 |
| 构建输入一致性 | 285 个源码、清单、版本等输入逐字节一致 |
| PE 结构与依赖 | AMD64、Windows GUI；导入项均为 Windows 系统库 |
| Windows 产品名与原文件名 | TermViAI / TermViAI.exe |
| 字符串 / 数值版本 | 0.1.0-beta.8 / 0.1.0.8 |
| --version | TermViAI 0.1.0-beta.8；创建窗口前退出 |
| 压缩包 | 三个文件、CRC 通过、附 SHA-256 校验文件 |

本轮新增测试覆盖标签溢出几何、自动定位、拖动插入、键盘作用域、组名与拖放规则、主机多选、Monitor 重复选择与替换失败、输入法取消/正常提交/快速重新组字。原生输入法消息测试创建不可见窗口，不改变用户输入法，不激活窗口。以前版本的 SSH、window、mux 和终端核心测试统计不作为本轮重新执行的结果。

## 首次发布审查与范围

使用 review-code-changes 技能审查 main 上未提交改动及新增源码，重点检查输入焦点、原生消息钩子生命周期、排序与实际放置的一致性、主机引用变化、Monitor 替换时先持久化再取消旧任务、停止进程期间的拒绝条件和存储兼容。No actionable findings：未发现仍需阻止发布的已证实问题。

公开应用、窗口标题和 EXE 统一为 TermViAI；内部 crate/目录继续使用 termviai-gpui。Windows CI 的构建、原生界面测试和上传路径改为当前前端，保留共享后端检查；补充原生界面格式检查。整理了两处已有模块顺序以通过根格式检查，移除了复制源文件带来的意外权限变化。

## Beta 8 覆盖更新 · 2026-11-10

按用户要求保留 `v0.1.0-beta.8`，将标签和发布附件更新为本次代码及产物，未创建新的版本号。

- 恢复终端选区背景、左键拖选自动复制、双击整行（不含尾部空白）和滚动扩展选区。
- 新增保守识别的 SSH 系统广播通知：蓝色铃铛、悬浮预览、点击固定详情、文本选择复制，无复制按钮；未知或不完整输出原样保留。
- 新增项目内 Windows 构建技能 `.agents/skills/termviai-windows-build/`，结构校验通过。
- Windows 原生 Release 构建成功；库测试 **108 通过、0 失败、3 项外部环境测试忽略**。新增覆盖选区、广播边界/回退、PTY 换行和 Session 集成。最终仅 UI 的调整再次通过 Release 构建。
- 核对本次构建镜像与仓库的 **288** 个源码/清单输入一致，包含所有原生 crate Rust 文件；仅文档及技能改动不影响已验证二进制。
- 首次发布的 Python 及其他结果保留在上表作历史记录，本次覆盖未重新执行未修改模块的全部检查。

## 当前发布包

- 文件：TermViAI-v0.1.0-beta.8-windows-x86_64.zip
- 大小：14225695 字节。
- ZIP SHA-256：`b0dc4a5c27e888e35b48815f068b380806162ea0a534eb3c65c51ad6fbfe3606`。
- EXE SHA-256：`c3e69ca6d5f034905b610926b90d7af022c841617353f1573f235dba1582e00c`。
- 内容：TermViAI.exe、README.txt、LICENSES.txt。许可证保留应用及第三方全文，依赖清单未变。
- ZIP CRC、成员列表、归档内 EXE 与已验证构建的逐字节比较通过，附 SHA-256 校验文件。
- 发布目标：[TermViAI v0.1.0-beta.8](https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.8)，保持 prerelease；替换此版本原有标签与附件。

## 验证边界

本轮未使用 computer-use 或启动可见应用窗口，未对真实 SSH/NPU 主机执行进程操作。真实 IME、DPI/GPU、鼠标拖动、长时间连接和硬件采集仍需持续验收。终端图片绘制、旧 Lua GUI 自定义和完整无障碍尚未迁移。

本地验证不等于远端 CI 已完成，远端状态以本次提交的 GitHub Actions 为准。发布流程会回读 GitHub 附件并核对 SHA-256 与 ZIP CRC。
