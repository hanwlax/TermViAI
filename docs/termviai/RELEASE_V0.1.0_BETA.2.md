# TermViAI v0.1.0-beta.2

本次 Beta 集中改进 SSH 断线重连，修复重连后历史文字重叠及广播输入失效，并精简 New Tab。

## SSH 重连

- 断线后保留终端、组合 Tab、分屏布局及主屏滚动历史，终端名称旁提供小号刷新按钮。
- 组内终端全部断线时，Tab 提供一键重连整个终端组。
- 显式重连建立新 SSH 传输；缓存连接失效时，新 Tab 也能重新建立连接，无需先关闭旧组。
- 重连错误限制在对应终端，不再蔓延到 Hosts 或 New Tab。
- 重连输出从历史末尾继续，不添加分隔线；修复连续重连失败、网络恢复后首行覆盖旧文字的问题。
- 重建断网后失效的后台输入线程，修复广播时其他终端收到输入、源终端却写入失败的问题。旧的未发送输入直接丢弃，不重放到新会话。

## New Tab

- 移除 Recent tab groups 及其自动记录，保留 Saved workspaces 和 Recent connections。
- 需要复用的终端组通过 `Ctrl+S` 手动保存。已有手动保存组与最近连接保留；旧自动历史在下次正常保存数据时清理。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.2-windows-x86_64.zip`，完整解压到新目录，关闭旧程序后运行 `TermViAI.exe`。请保留同目录的 `OpenConsole.exe`、`conpty.dll`、`libEGL.dll` 和 `libGLESv2.dll`。

支持 Windows 10/11 x86_64，沿用 `%APPDATA%\wezterm\termviai` 数据目录及 `TERMVIAI_DATA_DIR` 覆盖设置。升级不会删除 Hosts、Keychain、设置或手动保存的工作区。

附件提供 SHA-256 校验文件，可使用 PowerShell 核验：

```powershell
Get-FileHash .\TermViAI-v0.1.0-beta.2-windows-x86_64.zip -Algorithm SHA256
```

## 验证与边界

已覆盖输入线程写入/flush 失败后的恢复、旧输入队列丢弃、历史保留、失效连接恢复、广播路由、工作区存储和界面逻辑回归，并完成 Windows x64 Release 构建。

重连会启动新的远端 shell，不能恢复已结束的远端进程；关闭应用后滚动历史不会持久化。真实 SSH 网络、睡眠唤醒、认证、GPU/IME 及 vim/tmux 交互仍需持续验证。

TermViAI 基于 WezTerm，保留上游归属和 MIT 许可证。

完整改动：[v0.1.0-beta.1...v0.1.0-beta.2](https://github.com/hanwlax/TermViAI/compare/v0.1.0-beta.1...v0.1.0-beta.2)。
