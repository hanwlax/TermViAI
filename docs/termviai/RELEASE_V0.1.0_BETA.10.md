# TermViAI v0.1.0-beta.10

适用于 Windows 10/11 x86_64。

## SSH 主机信任与自动恢复

- TermViAI 在自己的数据目录中按主机和端口保存 SHA256 指纹。升级后首次连接原有主机时，请从可信渠道核对指纹，再选择 **Trust & Connect**。
- 主机密钥变化时，终端会同时显示已保存和本次收到的指纹。确认服务器确已更换密钥后，可选择 **Replace key & Connect**。取消会保留原记录。
- 在 **Port Forwarding** 下方打开 **Known Hosts** 可查看信任记录。将鼠标移到卡片上，点击 × 即可移除，下次连接时会重新询问。
- 确认指纹后，等待该端点和指纹的端口转发会按自动重连设置重新检查网络、排队连接。未暂停的 Monitor 会重新采样。手动停止的转发和暂停的监控保持原状态。

## 桌面布局与终端

- 启动后进入 **New Tab**，搜索框自动获得焦点。Hosts、Keychain、Port Forwarding、Monitor 和 Settings 的内容与工具栏会随窗口宽度调整。添加操作位于左侧，使用低强调配色。
- 转发服务和 Monitor 的 NPU 卡片按可用宽度自动换列。进程详情面板延伸至窗口右缘。
- 修复广播分屏中一次拖动丢失鼠标释放事件后，右侧窗格的关闭按钮或滚动条无法正常响应的问题。
- SSH 连接诊断日志记录传输错误、保活发送结果和终端结束原因，保存在应用数据目录的 `logs` 文件夹中，便于排查断线。

## 安装与升级

退出旧版，将 `TermViAI-v0.1.0-beta.10-windows-x86_64.zip` 解压到新目录，运行 `TermViAI.exe`。现有 Hosts、Keychain、工作区和设置会继续使用。可在 PowerShell 运行以下命令，将结果与发布页的同名 `.sha256` 文件核对。

```powershell
Get-FileHash .\TermViAI-v0.1.0-beta.10-windows-x86_64.zip -Algorithm SHA256
```
