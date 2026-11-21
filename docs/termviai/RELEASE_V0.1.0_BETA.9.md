# TermViAI v0.1.0-beta.9

本版面向 Windows 10/11 x86_64，更新了端口转发的网络恢复机制、标签组管理和 NPU Monitor 的暂停行为。

## 本次更新

- Settings 中可以查看、搜索和编辑快捷键，保存后重启仍生效。终端内支持复制选区和右键粘贴，应用内其他文本框沿用 Ctrl+C 复制。
- 四窗格布局支持按行或按列独立拖动分隔线。拖中央交点可同时调整宽高，错开的分隔线拖近时会吸附对齐。
- Settings 的快捷键入口与分区标题左对齐。设置项和保存按钮都能放入窗口时，页面不会因底部留白产生滚动条。

## Port Forwarding

- Windows 确认目标无可用网络路径后，规则显示 **Waiting for network** 并暂停 SSH 建连。路径恢复后，开启自动重连的规则会排队连接，远端确认监听成功后才显示 **Tunnel established**。
- 目标仍有路由但服务器持续不可达时，重试间隔逐步增加至 30 秒。等待期间可从卡片菜单选择 **Retry now**。手动停止的规则在网络恢复和应用重启后均保持停止。
- 多条规则同时恢复时，最多并行建立四条连接，同一 SSH 端点依次建连。网络切换和电脑唤醒后，已有隧道会核对对端应答。

## 标签组与终端

- 侧栏标签组支持拖动排序和右键保存。New Tab 可恢复保存时的标签组合，标签组和工作区改名后会同步更新。只有启动页或 Hosts 标签的组会禁用保存。
- 当前输入窗格以完整的圆角细边框标识，广播接收成员以广播图标和空框光标标识。超长标签名在悬停时循环滚动。首次 SSH 连接的主机指纹可在对应窗格内核验。

## 主机与监控

- Hosts 支持单击连接、Ctrl 加单击多选、Enter 在同一标签内分屏打开。编辑主机时可搜索已有分组、输入新组名或留空。New Tab 与 Hosts 各自保留编辑期间的搜索条件。
- Keychain 合并了新增入口，凭据可在卡片内编辑。Hosts、Keychain、Monitor、Port Forwarding 和 Settings 采用统一的页面边距与布局。
- Settings 可开关 NPU Monitor，默认开启。已暂停的主机在重新开启 Monitor 后保持暂停。主机总览、NPU 与 Die 资源、悬浮进程预览和进程操作继续可用。

## 安装与校验

下载 `TermViAI-v0.1.0-beta.9-windows-x86_64.zip`，解压到新目录后运行 `TermViAI.exe`。升级时先退出旧版，应用会继续读取原有主机、工作区和设置。

可在 PowerShell 使用 `Get-FileHash .\TermViAI-v0.1.0-beta.9-windows-x86_64.zip -Algorithm SHA256`，将结果与同名 `.sha256` 附件核对。网络恢复策略详见[设计说明](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.9/docs/termviai/PORT_FORWARDING_RECOVERY.md)。
