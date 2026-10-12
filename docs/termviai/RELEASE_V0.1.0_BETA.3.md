# TermViAI v0.1.0-beta.3

第三个 Beta 新增顶栏 tab 拖动排序和 TermViAI 应用图标，修复同主机整组 SSH 重连，并改进最大化窗口的顶栏边界。

## Tab 排序

- 按住 tab 名称左右拖动，紫色插入线指示放置位置；支持整个多终端组和当前选中 tab。
- 排序保留 SSH 连接、组内分屏布局、广播状态和当前选中 tab。
- 指针仍在原 tab 内或放置后顺序不变时不显示插入线；拖动时显示小手。
- Esc 可取消；tab 较多时，拖动期间可用滚轮滚动顶栏。原有单终端 tab 拖入终端组的操作保留。

## SSH 与窗口修复

- 修复三个 pane 连接同一主机时，一键重连可能只有最后一个成功、前面的连接报 `SSH session is dead`：替换缓存不会再提前关闭仍在认证的会话。
- 保留 beta.2 后的建连响应修复：PTY 请求等待不持有终端尺寸锁，认证后的远端终端创建等待上限为 30 秒。
- 最大化自绘客户区限制到显示器工作区，更新窗口标题时释放窗口对象借用，减少 New Tab 顶栏越界的风险；偶发 GUI 场景仍需实际使用验证。
- 保留实时缩放期间限制事件队列处理、延后非当前 tab 尺寸更新，以及新 tab 适配现有窗口尺寸的改进。

## 应用图标与 CI

- 替换旧图标，使用深色圆角底、终端提示符、光标和制作来源星芒，Windows 图标覆盖 16–256 px。
- TermViAI = Terminal Via AI，表示应用通过 AI 制作；当前没有内置 AI 功能。
- CI 聚焦 Windows 编译和核心回归，保留 Rust 格式检查；上游多平台定时构建和发布工作流已归档。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.3-windows-x86_64.zip`，完整解压到新目录，关闭旧程序后运行 `TermViAI.exe`。保留同目录的 `OpenConsole.exe`、`conpty.dll`、`libEGL.dll` 和 `libGLESv2.dll`。

支持 Windows 10/11 x86_64，继续使用 `%APPDATA%\wezterm\termviai` 和 `TERMVIAI_DATA_DIR`。升级不删除 Hosts、Keychain、设置或手动保存工作区。

附件提供 ZIP 的 `.sha256`，包内 `SHA256SUMS.txt` 列出各文件的校验值：

```powershell
Get-FileHash .\TermViAI-v0.1.0-beta.3-windows-x86_64.zip -Algorithm SHA256
```

## 验证与边界

Windows 原生 Release 构建及回归结果见 [验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.3/docs/termviai/RELEASE_V0.1.0_BETA.3_VALIDATION.md)。真实 SSH 网络、鼠标拖动、最大化/DPI、GPU/IME 和 vim/tmux 场景仍需持续人工验证。

重连创建新的远端 shell，不能恢复已结束的远端进程；关闭应用后滚动历史不持久化。TermViAI 基于 WezTerm，保留上游归属和 MIT 许可证。

完整改动：[v0.1.0-beta.2...v0.1.0-beta.3](https://github.com/hanwlax/TermViAI/compare/v0.1.0-beta.2...v0.1.0-beta.3)。
