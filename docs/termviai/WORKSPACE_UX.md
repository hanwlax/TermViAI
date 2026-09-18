# TermViAI 工作区 UX（2026-09-17）

2026-09-18 更新：New Tab 已移除自动历史组，保留手动保存组与最近连接。本次工作区存储 7 项、TermViAI GUI 61 项回归通过，Windows x64 Release 构建成功。新版目录为 `new-tab-simplified-20260918`；下文初版验证记录保留供追溯。

本轮将 Hosts、SSH Tab、New Tab 和广播控制整合为同一原生工作区流程，保留 Catppuccin Mocha 配色和 JetBrainsMono NFM 字体。用户提供的界面图片是布局与交互参考，不作为执行终端命令、导入主机或登录服务器的指令。

## 本轮 8 项变化

| 用户操作 | 当前行为 |
| --- | --- |
| 查看 Tab 和窗格名称 | 使用保存的 Host label；未填写 label 时使用主机地址。终端当前命令、目录或输出不会替换该名称。成组 Tab 默认组合主机 label，用户可自行重命名。 |
| 切换独立/组广播 | 每个直接 SSH 窗格的标题右侧提供成员按钮；成组 Tab 提供全组广播开关：开启时全选所有窗格并广播，关闭时停止广播并取消所有成员。当前 Tab 的底部面板同步显示广播状态及成员。 |
| 使用侧栏 | 保留 Hosts、Keychain 等实际功能入口，删除 Terminal 导航项；已有终端通过顶栏 Tab 访问。 |
| 启动与关闭最后一个终端 | 默认进入 Hosts，创建真实无 pane 的应用窗口，不启动本地 shell。通过 Tab 关闭按钮关闭最后一个终端后返回 Hosts；其他页面的后台会话自行退出时保留当前页面。应用窗口继续存在，显式窗口关闭按钮仍关闭窗口。 |
| 点击顶栏 `+` | 打开 New Tab 页面，展示 Saved workspaces 和 Recent connections，不创建本地终端。 |
| 在 SSH 组中按 `Ctrl+S` | 将当前组的名称、连接元数据、分屏方向及比例保存到 New Tab；同一保存组再次保存时更新原条目。至少需要两个由 TermViAI 打开的 SSH 窗格。 |
| 右键单会话/成组 Tab | 菜单提供 Rename tab，通过右侧表单输入新名称；该操作修改 Tab 名称，保留原 Host label。已关联保存项的组会同步更新保存名称。 |
| 收起或展开广播面板 | 面板位于终端区域底部，箭头控制收放；显示和隐藏均使用滑动过渡。隐藏后终端收回面板占用的空间，仅保留可重新展开的箭头。 |

## New Tab 与保存组

New Tab 的搜索框支持 SSH 地址快速连接，也可筛选已有条目。连接新主机后进入终端 Tab；已打开的会话继续保留在顶栏，打开 New Tab 本身不会产生 SSH 连接。

- **Saved workspaces**：通过 `Ctrl+S` 或组的保存按钮创建。保存后关闭 Tab 或退出应用，条目仍保留。
- **Recent connections**：记录已打开的 SSH 主机，按连接身份去重并按时间排列。

点击保存项时，TermViAI 根据模板重新建立 SSH 连接并按布局分屏。认证仍由原有 SSH 流程完成，可能需要输入密码或确认主机。它不会恢复已退出的远端程序、终端滚屏或曾输入的命令。恢复过程中某个连接失败时显示错误，并保留已经打开的会话。

2026-09-18 起移除 Recent tab groups，停止在组合变化、重命名和关闭时自动记录布局。需要重复使用的组通过 `Ctrl+S` 保存。

保存组重新打开后广播默认关闭。保存模板不包含广播开关或成员选择，避免把连接恢复理解为立即开始广播。

单会话与成组 Tab 都可右键重命名。组保存之后再重命名，会同步保存项的名称；单会话的 Tab 名称不修改 Hosts 库中的主机 label。

## 字体与终端焦点

终端标题不再显示选中/悬浮背景条，焦点通过圆角边框及名称/图标颜色表示。拖动窗口大小时，所有本地/直接 SSH 窗格保持现有分屏比例。

- Ctrl+鼠标滚轮：仅调整当前选中终端的字号，每次 1 pt，范围 6–48 pt；本次会话有效，不保存到磁盘。
- 左侧 Settings：修改 Global font size 并 Save，保存全局终端字号并在下次启动恢复；已有会话独立字号仍保留。
- 全局设置保存在 hosts.json 同目录的 settings.json。字体系列继续使用 JetBrainsMono NFM。

详情与验证见 [SESSION_POLISH.md](SESSION_POLISH.md)。

## 广播与终端区域

窗格标题上的广播按钮决定该窗格是否属于当前组的广播成员。成组 Tab 上的按钮执行全组操作：关闭状态下点击，会把所有窗格（包括此前独立输入的窗格）加入并开启广播；开启状态下点击，会关闭广播并取消全部成员。该按钮只作用于对应 Tab。

底部开关仍用于对已选成员暂停/恢复广播，成员按钮用于选择性广播。顶部全关后，需要先选择成员（或 All）再通过底部开关开启；也可以直接再次点击顶部按钮全组开启。收起面板不改变这些状态。

| 状态 | 输入目的地 |
| --- | --- |
| 顶部 Tab 按钮全关 | 全部窗格独立输入，同时取消全部成员选择 |
| 顶部 Tab 按钮全开 | 全部窗格加入广播，任意窗格输入发送到全组 |
| 底部开关 OFF | 仅当前输入窗格；保留已有成员选择 |
| 组广播 ON，从成员输入 | 当前 Tab 的全部成员，源窗格只发送一次 |
| 组广播 ON，从独立窗格输入 | 仅该独立窗格 |
| 清空成员 | 关闭组广播 |

窗格标题和底栏的紫色仅表示当前已开启的广播成员。组广播关闭时全部灰显；底栏中性色勾选仅表示预选，计数标为 selected，开启后才标为 active。

底栏采用统一圆角工具栏：左侧显示广播及成员计数，中间按 Host label 长度显示紧凑标签，右侧集中 All/None 分段选择、带滑动过渡的 On/Off 开关及收起箭头。长名称显示省略号；成员过多时支持左右翻页与滚轮，窄窗口优先保留主开关和收起按钮。详细改版及验证见 [BROADCAST_TOOLBAR.md](BROADCAST_TOOLBAR.md)。

收起底部面板只改变显示，不改变组广播开关或成员选择；成组 Tab 和窗格标题仍提供可见控制。面板与终端可用区域同步过渡，对单元格取整增量增加连续像素位移，正文、标题、圆角边框、鼠标与 IME 共用位置；只有 PTY 网格变化才调整终端尺寸。

直接 SSH/本地 pane 的 header 预留真实终端行，绘制、鼠标和 IME 共用内容区域坐标。远端 Mux 镜像或混合 Tab 不应用此 header inset，避免远端尺寸同步反复扣除行数。顶栏组广播及底部面板仍沿用 Tab 广播状态。

## 本地数据

工作区库位于 `config::DATA_DIR/termviai/workspaces.json`，Windows 默认通常为 `%APPDATA%\wezterm\termviai\workspaces.json`。设置 `TERMVIAI_DATA_DIR` 时改为该目录下的 `workspaces.json`；Hosts 库仍在同目录的 `hosts.json`。

保存内容包括组名称、SSH 地址/端口/用户名、Host label、私钥文件路径，以及嵌套分屏方向和比例。库中不保存认证密码、私钥内容、终端输出或命令输入。

最近连接最多保留 30 条，手动保存组最多 100 条，不再记录自动历史组。旧文件的 `history` 字段在读取时忽略，下次正常写入时清理。保存使用同目录临时文件替换；读取损坏文件或不支持的版本会报错并保留原文件。当前实现不提供远端进程恢复或自动重连。

## 验证状态

Windows 验证共 51 项通过：mux 16 项、UI 26 项、工作区存储 7 项、Hosts 2 项。Windows x64 GUI Release 构建通过，新包已生成。

新版入口为：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\workspace-ux-20260917\wezterm-gui.exe
```

最终构建结果、源文件同步、测试次数和包校验值以 [WORKSPACE_UX_VALIDATION.md](WORKSPACE_UX_VALIDATION.md) 为准。之前的 Mocha/现代界面包保留为历史产物。

此前 Computer Use 对该应用明确返回 `product policy blocks this app`。本轮没有借助其他自动化绕过限制，因此逻辑测试和 Release 构建不能替代真实 Windows 窗口验收。以下操作仍需人工确认：

1. 启动后直接进入 Hosts，没有默认本地终端；首次 SSH、关闭最后一个 Tab、显式窗口关闭均符合预期。
2. `+` 打开 New Tab；保存、重开、重命名及应用重启后列表持久化正常。
3. 四窗格 A/B/C 为成员、D 独立，核对组按钮、pane header 和底部面板状态同步与实际输入隔离。
4. 展开/收起面板及快速反向操作动画连续；隐藏后不遮挡终端；分屏调整、缩放和 DPI 改变后鼠标与 IME 位置正确。
5. 真实 SSH/ConPTY、Ctrl+C、方向键、粘贴、IME、vim/tmux、认证失败和断线时没有输入重放或连接丢失。

## 实现入口

- `wezterm-gui/src/termwindow/termviai_ui.rs`：导航、New Tab、重命名表单、header/Tab 广播按钮及底部动画面板。
- `wezterm-gui/src/termwindow/termviai_workspace.rs`：稳定 label、SSH 组快照、保存/重命名与重新连接。
- `wezterm-gui/src/workspaces.rs`：工作区存储、去重、限额、版本校验与文件写入。
- `wezterm-gui/src/termwindow/termviai_layout.rs`、`mux/src/tab.rs`：header 行预留、终端内容坐标与面板空间。
- `wezterm-gui/src/main.rs`、`frontend.rs`、`mux/src/window.rs`、`mux/src/lib.rs`：无终端启动、空窗口保留及首次 SSH 创建。

Mux 仍是 Tab、pane 与分屏布局的唯一生命周期来源。新增磁盘模板用于重新连接，不是另一套正在运行的 pane 树。
