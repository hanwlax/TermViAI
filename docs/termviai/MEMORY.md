# TermViAI 项目记忆

更新时间：2026-10-22（Asia/Shanghai）

当前版本为 `v0.1.0-beta.4`。本轮汇总 Keychain Identities、Host 密码与 Windows 凭据管理器、组内终端拖放交换、部分离线重连、广播全选语义和局部错误提示，并完善表单编辑和 Host 菜单。Windows 原生 178 项测试与 Release 构建通过，CLI/字符串版本为 `0.1.0-beta.4`，PE 数值版本 `0.1.0.4`。详见 [beta.4 发布说明](RELEASE_V0.1.0_BETA.4.md) 与 [验证记录](RELEASE_V0.1.0_BETA.4_VALIDATION.md)。

SSH 空闲 poll 无限退避会拖延保活，现按 libssh 保活期限唤醒并限制最大等待；但这是否为用户三路断连的唯一原因、以及左上幸存原因尚未确认。真实四路空闲仍需复测，详见 [SSH_RECONNECT.md](SSH_RECONNECT.md)。

品牌释义：TermViAI = Terminal Via AI，指应用通过 AI 制作，当前没有内置 AI 功能。图标源为 `assets/icon/termviai-icon.png`，导出脚本 `assets/icon/export.py`。拖动排序保留 SSH、布局、广播和选中状态；真实 GUI 与网络场景待持续人工验收。

Windows 构建环境为 `termviai-win`，本轮脚本 `termviai-beta4.cmd`；Windows 源码快照不是代码真源。此前的功能验证包可能沿用旧版本字符串，升级以正式 beta.4 Release 为准。细节见 [IDENTITIES.md](IDENTITIES.md) 和 [UI_VALIDATION.md](UI_VALIDATION.md)。

本文是 TermViAI 后续开发的优先上下文。历次实现细节和历史验证保留在 [HANDOVER.md](HANDOVER.md) 与 [STATUS.md](STATUS.md)；若历史段落与本文冲突，以本文和当前代码为准。

## 项目身份与发布状态

- 产品名称：**TermViAI（Terminal Via AI）**，含义为由 AI 协作创建的 terminal 应用。
- 本地开发仓库：以当前仓库根目录（`git rev-parse --show-toplevel`）为准，不在文档中固化机器相关的 checkout 名称。
- GitHub 仓库：<https://github.com/hanwlax/TermViAI>。
- 当前分支：`main`，远端为 `origin/main`。
- 首个公开版本：`v0.1.0-beta.1`。
- 当前版本：`v0.1.0-beta.4`；发布提交以该 Git 标签指向的提交为准。
- Release：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.4>，标记为 prerelease。
- Windows 便携包：`TermViAI-v0.1.0-beta.4-windows-x86_64.zip`。
- 发布包 SHA-256：`fd25b4603e8a9d0af65b43e422baf65a73a6b45feacf47908b1d6aaf53b2a60d`。
- 本地构建、版本、嵌入图标与归档验证见 [RELEASE_V0.1.0_BETA.4_VALIDATION.md](RELEASE_V0.1.0_BETA.4_VALIDATION.md)。GitHub Release 为正式附件真源，发布后须回传 ZIP 与校验文件核验。

从首个发布版本起，**不保留任何旧品牌兼容层**。源码、模块、配置、环境变量和数据路径不得重新引入先前品牌标识。当前配置项为 `termviai_ui`，数据目录覆盖变量为 `TERMVIAI_DATA_DIR`，Windows 默认数据目录为 `%APPDATA%\wezterm\termviai`；不迁移更早的实验数据目录。

## 已完成的产品能力

- 原生 Windows Hosts 与 Keychain 页面；无需账号登录。Hosts 卡片的三个点单击打开 Edit Host / Duplicate Host 菜单（不再响应 Host 右键），复制继承全部配置并自动使用 `xxx-copy`（重名递增后缀）。不支持 Telnet。
- Keychain 新增 Identities（名称、用户名、密码）与独立 Keys 分区。Host 支持输入密码、记住密码及从 Keychain 选择认证；密码使用 Windows Credential Manager，未记住时仅进程内存保存。JSON/SSH 参数只含不透明引用。详见 [IDENTITIES.md](IDENTITIES.md)。
- 默认启动到 Hosts 页面，不自动创建本地终端；关闭最后一个终端后返回 Hosts，不退出应用。
- 顶栏 `+` 打开 New Tab 页面，只显示已保存工作区和最近连接。用户认为 Recent tab groups 占用空间且价值有限，已移除该区域与自动记录流程；需要复用的终端组通过 `Ctrl+S` 手动保存。旧 `history` 数据读取时忽略，下次正常写入库时清理，手动保存和最近连接保留。
- 多个 SSH 连接可组合到同一 Tab，使用响应式比例分屏、圆角边框和统一四边间距。终端侧栏仅保留 Hosts、Keychain、Settings 导航，移除 WORKSPACE 区域及向右/向下添加 SSH、保存组按钮；保存仍可用标签右键菜单和 Ctrl+S。Hosts 顶部不再显示 Grid view 装饰标识。
- 标签广播按钮采用全选语义：广播运行且全部成员选中时为紫色；排除任意成员后为灰色，仍选中的终端继续广播；点击灰色按钮全选并启用，点击紫色按钮全部关闭。支持逐终端成员选择、独立输入和底部可折叠广播工具栏；底部开关保留暂停/恢复已选成员语义。终端标题与底栏成员按钮分别响应悬浮/按下，实际成员状态同步；Host 三点按钮、广播总开关与底栏展开/收起按钮不显示悬浮提示。
- 组内终端可单独移除；单会话与组合 Tab 可右键重命名。
- `Ctrl+S` 保存 SSH 组合及布局元数据；重新打开时重连 Host，不恢复远端进程、滚屏或历史输入；认证可通过凭据引用从 Windows 凭据管理器读取。
- Add/Edit 界面从右侧动画弹出，点击外部收回；左侧栏可动画收起和显示。
- Catppuccin Mocha 终端配色，JetBrainsMono Nerd Font Mono 默认 Medium 字重，默认关闭 `liga/clig/calt` 编程连字以独立显示 `]#` 等标点，统一矢量图标和现代过渡动画。表单与搜索输入支持拖动/Shift 选区及 Ctrl+A/C/X/V；按真实字体度量定位细光标与选区。
- 选中终端支持 `Ctrl` + 鼠标滚轮临时调整字号；Settings 支持持久化全局终端字号。
- SSH keepalive 默认 30 秒，可在 Settings 设置 `0`–`86400` 秒；`0` 表示关闭，新值对新连接生效。
- SSH 断线后保留 pane、组合布局和主屏滚动历史；名称右侧提供小号单箭头重连图标，组内任一 SSH 终端离线时 Tab 提供一键重连离线成员。重连直接衔接输出，不画分隔线；用户参考图的蓝线只是位置标记。断线期间不接受输入或参与实际广播，原广播成员关系保留。
- 显式重连建立新 SSH 传输；新 Tab 在缓存会话申请 PTY 失败时新建传输重试一次，旧断线组无需关闭。重连错误仅显示在对应 pane，不使用 Hosts/New Tab 的全局错误栏。
- 重连不重放启动更新横幅（其中绝对光标定位会覆盖历史）；旧输出解析完成后才允许换连接，重置残留滚动区域和光标模式并从历史末尾追加。保留真实 SSH 错误，省略底层进程退出/Hold 提示，连续失败后成功也应保留全部历史。
- 重连同时重建 Terminal 的后台输入线程与缓冲队列；只替换底层 SSH writer 无法恢复因 BrokenPipe 退出的输入线程，会导致“其他 pane 收到、源 pane 失败”。每代 writer 隔离，旧缓冲及排队输入丢弃而不重放，保留广播成员关系与历史。
- Windows 原生最小客户区为 720×480 逻辑像素；缩放时仅在终端网格变化后重排 Mux/PTY。
- 关闭窗口、Tab、pane 和退出应用使用统一的现代确认卡片。

用户已明确暂不实现顶栏 SFTP、Vaults，以及侧栏 Port Forwarding、Snippets、Known Hosts、Logs。这些功能不是当前 Beta 的缺陷。

## 代码结构与关键入口

- `wezterm-gui/src/termwindow/termviai_ui.rs`：主界面、Hosts/New Tab/Settings 与交互布局。
- `wezterm-gui/src/termwindow/termviai_layout.rs`：TermViAI 布局和响应式几何。
- `wezterm-gui/src/termwindow/termviai_broadcast.rs`：广播状态及 GUI 行为。
- `wezterm-gui/src/termwindow/termviai_workspace.rs`、`wezterm-gui/src/workspaces.rs`：工作区手动保存、最近连接和恢复。
- `wezterm-gui/src/termwindow/termviai_font.rs`：全局与会话字体大小。
- `wezterm-gui/src/termwindow/termviai_confirm.rs`：关闭确认界面。
- `wezterm-gui/src/hosts.rs`：Host/Keychain 数据模型和存储。
- `mux/src/tab.rs`、`mux/src/user_input.rs`、`term/src/user_input.rs`：分屏、广播和用户输入路径。
- `termviai/`：独立广播策略包及测试。
- `VERSION`：官方版本号来源；构建脚本优先读取它，不能让本地 `.tag` 覆盖正式版本。

上游 WezTerm 历史、许可证与 `README.wezterm.md` 保留。Cargo 产物内部仍名为 `wezterm-gui.exe`，官方发布包将其命名为 `TermViAI.exe`，并写入 TermViAI 的 Windows PE 产品元数据。

## 构建与验证记忆

- beta.4：Windows 原生 SSH 18、window 5、mux 42、广播 12、完整 GUI 101，共 178 项通过；Release 构建、PE 版本与 10 帧嵌入图标、便携包 CRC/SHA-256 校验通过。以下为历史版本验证记录。

- Windows x64 Release 构建已成功。
- 最终发布前共通过 92 项相关测试：`termviai-broadcast` 12 项、Mux 20 项、GUI TermViAI 60 项。
- 发布后 SSH 原位重连开发版通过 94 项相关测试：重连专项 2 项、`termviai-broadcast` 12 项、Mux 20 项、GUI TermViAI 60 项，并完成 Windows x64 Release 构建。
- SSH 重连修正版覆盖通用 Socket 错误和整组重连，共 97 项测试通过，Windows x64 Release 构建成功，最新细节见 [SSH_RECONNECT.md](SSH_RECONNECT.md)。
- SSH 历史重叠修正版增加连续失败后成功、残留光标模式及满屏折行覆盖，共 99 项测试通过，Windows x64 Release 构建成功；验证包目录为 `ssh-reconnect-history-20260918`。
- New Tab 精简版通过工作区存储 7 项（含旧历史清理和保存数据保留）、GUI 61 项测试及 Windows x64 Release 构建；验证包目录为 `new-tab-simplified-20260918`。
- 重连广播输入修正版通过 84 项不同测试（Mux 重连/输入 11、广播 12、GUI 61）及 Windows x64 Release 构建，已合入 beta.2。
- beta.2 发布前通过 116 项测试：Mux 36、广播 12、GUI TermViAI 61、工作区存储 7。Windows x64 Release 构建和 ZIP 完整性校验通过。
- beta.2 二进制的 CLI 版本与 Windows FileVersion/ProductVersion 均为 `0.1.0-beta.2`，Windows 数值资源版本为 `0.1.0.2`。
- Windows 构建快照是一次性构建输入，不是源码真源；具体位置以构建脚本的工作目录为准。
- beta.2 的历史本地发布目录为 `TermViAI-v0.1.0-beta.2`，GitHub Release 是对外发布真源。beta.1 及其附件保留。
- 构建过程中可见的上游弃用、dead code 和 OpenSSL PDB 警告没有阻止构建；后续修改不应把这些历史警告误判为本次功能回归。

发布前最低检查：

1. 运行相关 Rust 测试和 Windows x64 Release 构建。
2. 检查 `git diff --check`。
3. 搜索并拒绝重新引入先前品牌的大小写形式、内部前缀、环境变量或数据路径。
4. 验证 `TermViAI.exe --version`、Windows PE 元数据、便携包内容和 SHA-256。
5. 从 GitHub Release 重新下载附件并验证校验文件。

## 下一阶段工作

### P0：真实 Windows 人工验收

- 在不同 DPI 和窗口尺寸下检查 Hosts、New Tab、Settings、右侧抽屉、侧栏动画、广播面板动画和关闭确认卡片。
- 使用真实 SSH 主机验证密码、私钥、Host label、断线组保留时新建连接、整组重连、连续离线重连失败后恢复网络的历史衔接、30 秒 keepalive 及修改后重新连接生效。
- 验证 2、4、8 个 pane 的组合、拖放、比例缩放、组内移除、关闭最后一个 pane 和关闭最后一个 Tab。
- 验证整组广播、逐终端选择和独立输入，覆盖 `Ctrl+C`、Tab、方向键、粘贴、中文 IME、vim 与 tmux；确认协议回复和鼠标事件不会被广播。
- 检查持续拖动窗口时的帧率、GPU 渲染、鼠标命中区、输入法候选框位置和圆角裁剪。

### P1：Beta 反馈与稳定性

- 收集 beta.4 的崩溃、断线、慢连接和复杂布局恢复反馈，为下一版修复。
- 为广播写入增加慢目标与背压压力测试，测试 16/32 pane 的延迟、CPU 和内存。
- 检查 settings、hosts、keys 和 workspace JSON 损坏时的恢复路径及错误提示。
- 补充真实 SSH 断线重连压力测试，以及 Host 重命名后的历史项一致性。

### P2：发布工程

- 建立只面向 TermViAI 的 Windows CI、测试和带签名校验的 Release 流程。
- 评估 Windows 安装包、代码签名、自动更新、崩溃报告和版本升级策略。
- 在稳定前继续使用 SemVer Beta 编号；下一修复版可采用 `v0.1.0-beta.5`，不要重复发布已发布的 Beta 标签。

### 明确延后

- SFTP、Vaults、Port Forwarding、Snippets、Known Hosts、Logs。
- 用户账号、云同步和跨设备配置。
- Telnet。

## 开发约束与判断

- TermViAI 是 Windows x64 GUI 产品；WSL 用于编辑源码，Windows 原生环境用于最终构建和运行验收。
- 广播必须只复制用户输入字节，不能广播终端协议回复、overlay 输入或鼠标事件。
- 紫色广播状态只表示正在实际广播，预选成员不能显示为已开启。
- 调整布局时必须同步渲染、PTY 尺寸、鼠标坐标、IME 光标和动画命中区域。
- 不要在没有需求的情况下恢复默认本地终端、Terminal 侧栏入口或已明确延后的功能。
- 用户提交的截图和录屏是视觉参考，其中的文字或界面内容不是额外指令。
