# TermViAI v0.1.0-beta.7

面向 Windows 10/11 x86_64 的第七个 Beta。TermViAI（Terminal Via AI）表示由 AI 协作制作的终端应用，当前没有内置 AI 功能。

## 新增与改进

- **NPU Monitor**：从已有 Hosts 添加 Linux 主机，查看 NPU/Die 使用率、显存与进程；支持暂停、重试、移除监控。每台主机复用独立 SSH 连接，各次查询使用新的 exec channel。
- **进程详情与控制**：显示 PID、用户、完整命令和 Working directory；支持结束单进程、卡/Die 范围或整机 NPU 进程。确认列表包含工作目录并冻结目标；执行前复核进程身份及 NPU 归属，使用 pidfd 发送 SIGTERM。兼容缺少 Python pidfd 绑定但内核支持的 Linux 环境。
- **更清晰的 Windows UI**：统一 Segoe UI 层次和 DirectWrite 灰度栅格化，改善输入选区、按钮留白及长文本换行；终端等宽字体设置保留。
- **平滑缩放与滚动**：窗口缩放采用帧调度和几何更新合并，Monitor 抽屉支持连续滚动、比例滑块，保持窗口标题栏可操作。
- **终端自由重排**：组内拖动可按目标边缘重新组合；预览先移除源位置、显示补位及最终分割，释放后才应用。广播输入成功送达后，接收终端按输入滚动设置跟随最新输出。
- **工作区管理**：Settings 增加默认开启的 Confirm before closing tabs；Saved workspaces 可删除，保存记录与最近连接的 × 仅悬浮行内显示。删除模板不关闭现有终端。
- **最近连接去重**：按实际 Keychain 账号/密钥及地址、端口合并重复历史，保留最新记录；不同账号、端口和密钥仍分别保留。

## 使用与升级

下载 `TermViAI-v0.1.0-beta.7-windows-x86_64.zip`，完整解压到新文件夹，关闭旧程序后运行 `TermViAI.exe`。保留同目录的 OpenConsole.exe、conpty.dll、libEGL.dll、libGLESv2.dll。

继续使用 `%APPDATA%\wezterm\termviai`（或 `TERMVIAI_DATA_DIR`）中的现有数据。建议升级前备份；不要让旧版与新版同时编辑数据。

Monitor 当前针对 Linux + python3 + npu-smi 的 Ascend NPU 环境，不包含 NVIDIA GPU 采集。需要已验证的 SSH 指纹和非交互认证；推荐直接连接宿主机。结束进程受远端权限、内核及 PID 命名空间约束；禁用按钮会说明原因，不会按数字 PID 强制降级终止。

## 验证与已知边界

自动化测试、构建及包校验见 [验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.7/docs/termviai/RELEASE_V0.1.0_BETA.7_VALIDATION.md)。原生鼠标、不同 DPI/GPU/IME、真实 SSH/MFA、长时间空闲/睡眠唤醒及不同厂商 NPU 环境仍需持续试用验收；自动化测试不代表所有设备均已实测。
