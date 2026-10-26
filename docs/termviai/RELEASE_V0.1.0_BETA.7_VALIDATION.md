# TermViAI v0.1.0-beta.7 验证记录

日期：2026-10-26（Asia/Shanghai）。Windows 10/11 x86_64 便携 Beta。

## 构建与测试

Windows 原生 MSVC Release，Rust 1.98.1；发布脚本 `termviai-beta7.cmd`，证据保存在本地构建目录 `releases/verify-v0.1.0-beta.7`。

| 检查 | 结果 |
| --- | --- |
| SSH 库 | 19 通过 |
| window 库及帧调度 | 9 通过 |
| 终端核心 | 54 通过 |
| mux | 46 通过 |
| 广播策略 | 12 通过（11 单元 + 1 集成） |
| 完整 GUI | 185 通过，1 项手动性能基准忽略 |
| Windows 测试合计 | 325 通过，0 失败 |
| Linux Python 远程采集器 | 22 通过，0 失败 |
| nightly rustfmt / git diff --check | 通过 |
| 构建输入一致性 | 579 个源码/清单/版本/图标/脚本/fixture 文件逐字节一致 |

新增及扩展回归覆盖 NPU 数据解析、SSH exec channel 生命周期、pidfd 原生/libc/syscall 兼容、过期 PID 和命名空间拒绝、工作目录缺失/长路径、抽屉滚动和标题栏命中、实际字体测宽/像素对齐、窗口帧调度、96 组拖动预览/提交几何对照、广播输入跟随、关闭确认偏好、模板删除和最近连接 Identity 去重。进程控制测试不连接真实主机、不发送终止信号；真实兼容探测仅对测试自身发送信号 0。

## 发布前审查

使用 review-code-changes 技能审查 beta.6 之后的全部本地改动和未跟踪源码。重点检查远程命令封装与 SSH 指纹验证、连接复用和取消、pidfd/进程身份及 NPU 归属复核、预览树与真实 PTY 隔离、Windows 原生字体回退、模态 resize 队列、存储并发和旧配置兼容。

修复发布验证发现的版本遗漏：Windows FILEVERSION / PRODUCTVERSION 数值字段仍固定为 0.1.0.6，现与文字版本同步为 0.1.0.7，并重新编译和检查 PE 资源。其余审查 No actionable findings：未发现仍需阻止发布的已证实问题。整理时排除 Python 缓存并统一 nightly 格式；Windows CI 补充完整窗口库测试，新增轻量 Linux Python 采集器测试，不构建 Linux GUI。源码中未发现私钥或常见访问令牌模式。

## 验证边界

用户已有分轮试用反馈；本轮发布自动化不连接真实 SSH/NPU 主机。不同驱动/内核、SSH/MFA、长时间空闲/睡眠唤醒、不同 DPI/GPU/IME、原生拖动和渲染效果仍需持续验收。NPU Monitor 当前只覆盖 Linux + python3 + npu-smi，进程控制失败不降级为按数字 PID 杀进程。

## 发布包

- Windows Release 编译成功，命令行 `TermViAI 0.1.0-beta.7`。
- PE 为 AMD64；产品字符串 TermViAI / 0.1.0-beta.7，数值 FileVersion/ProductVersion 为 0.1.0.7。
- 图标资源 ID 0x101，10 个尺寸与源 ICO 逐字节一致。
- ZIP：`TermViAI-v0.1.0-beta.7-windows-x86_64.zip`，30,175,489 字节。
- ZIP SHA-256：`142ff8f5aa205a9a4a9785222a78bf8186faca5db46e671f5a687dd29f9d8ed1`。
- EXE SHA-256：`4d54ff809d29c695c3cfa63302009443ff79fd03fc443e082f7e8dee8c9fb175`。

包含 EXE、四个运行依赖、README、发布说明、认证/分组/主题/Monitor/缩放诊断说明、诊断启动脚本、MIT 许可证与 SHA256SUMS.txt。5 个 PE 文件均为 AMD64，ZIP CRC 与 14 个清单文件摘要通过。

附件为 ZIP 与 `.sha256`，旧 Beta 不覆盖。发布流程从 GitHub Release 回传附件，核对 SHA-256、CRC 与逐文件清单，证据归档到本地 verify-v0.1.0-beta.7。Release：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.7>，标记 prerelease。本地验证不等于远端 CI 已完成，远端状态以 GitHub Actions 为准。
