# TermViAI 项目交接文档

> 最新、精简且可执行的项目上下文见 [MEMORY.md](MEMORY.md)。后续工作应先读取该文件；下文保留完整历史记录。

## 2026-09-18 SSH 原位重连与历史保留

SSH 断线后不再移除 pane 或终端组。断线 pane 名称右侧显示蓝色刷新图标；点击后在同一 pane、同一分屏位置重新建立 SSH，会先退出残留备用屏幕，再用蓝色 `Reconnected` 分隔线衔接新输出。Terminal 对象、主屏滚动历史、临时字号和广播成员关系保留；断线期间输入被消费且不参与广播。

Windows x64 已通过 94 项测试（重连专项 2、广播 12、Mux 20、GUI 60）及 Release 构建。行为、实现位置、校验值和人工验收边界见 [SSH_RECONNECT.md](SSH_RECONNECT.md)。

## 2026-09-17 TermViAI v0.1.0-beta.1

应用正式命名为 **TermViAI（Terminal Via AI）**。顶栏、窗口标题、退出确认、命令行说明、Windows AppUserModelID 和 PE 产品信息使用该名称；版本由根目录 `VERSION` 固定为 `0.1.0-beta.1`。官方便携包中的入口为 `TermViAI.exe`。

这是首个发布版本，不保留旧品牌兼容层。内部模块使用 `termviai_*`，配置项为 `termviai_ui`，环境变量为 `TERMVIAI_DATA_DIR`，默认数据路径为 `%APPDATA%\wezterm\termviai`。公开仓库为 [hanwlax/TermViAI](https://github.com/hanwlax/TermViAI)，发布说明见 [RELEASE_V0.1.0_BETA.1.md](RELEASE_V0.1.0_BETA.1.md)。

## 2026-09-17 SSH Keepalive Interval

Settings 新增 SSH keepalive interval，默认 30 秒，支持 `0`–`86400` 秒，`0` 表示关闭。设置保存在现有 `settings.json`，并以 `serveraliveinterval` 注入后续新建的 SSH Domain；间隔纳入 Domain 缓存标识，避免设置变更后复用旧参数。现有连接需重连后采用新值。

Windows mux 20 项、GUI 60 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\ssh-keepalive-20260917\wezterm-gui.exe`。详情见 [SSH_KEEPALIVE.md](SSH_KEEPALIVE.md)。

## 2026-09-17 终端背景一致性

TermViAI 不再对非活动 pane 的基础背景应用 `inactive_pane_hsb`，标题、内容留白和终端文字区域现在保持同一 Catppuccin Mocha 背景；焦点继续通过圆角边框、名称和图标颜色显示。普通 WezTerm 模式不受影响。

Windows mux 20 项、GUI 58 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\pane-background-20260917\wezterm-gui.exe`。详情见 [PANE_BACKGROUND.md](PANE_BACKGROUND.md)。

## 2026-09-17 响应式布局与窗口缩放

Broadcast 标题和选择状态改为跟随内容区左边界，侧栏展开、收起及动画时不再与成员区域重叠。Windows 窗口增加 720×480 逻辑像素的原生最小客户区约束，并按 DPI 换算；实时拖动窗口时，仅在终端网格实际变化后执行 Mux/PTY 重排，降低逐像素缩放的重复开销。

Windows mux 20 项、GUI 57 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\responsive-resize-20260917\wezterm-gui.exe`。说明、校验值及验证边界见 [RESPONSIVE_RESIZE.md](RESPONSIVE_RESIZE.md)。

## 2026-09-17 终端与 Broadcast 四边等距

下排终端外框会吸收窗口高度除以终端行高后的余量，使终端与 Broadcast 面板固定保持 12 个逻辑像素的视觉间距，并与顶部、左右边距一致。仅调整外框，PTY 行列、分屏比例和输入坐标保持不变；Broadcast 收起时底部也保留 12px。

Windows mux 20 项、GUI 56 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\four-sided-gutter-20260917\wezterm-gui.exe`。说明、校验值及验证边界见 [FOUR_SIDED_GUTTER.md](FOUR_SIDED_GUTTER.md)。

## 2026-09-17 左侧栏动画切换

顶栏菜单按钮已从 Hosts 导航中分离，只收起或显示左侧栏。切换保留当前页面、Tab、活动终端和输入状态；侧栏、库页面、终端网格及底部 Broadcast 区使用同一动画宽度，命中区域同步移动。

Windows mux 20 项、GUI 55 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\sidebar-toggle-20260917\wezterm-gui.exe`。说明、校验值及验证边界见 [SIDEBAR_TOGGLE.md](SIDEBAR_TOGGLE.md)。

## 2026-09-17 组合 Tab 单终端移除

多终端 Tab 的每个 pane header 现在按“广播、关闭”排列操作按钮。关闭按钮通过 pane ID 精确关闭目标，复用现代确认卡片；移除后同步清理广播状态并重排剩余窗格，剩一个终端时隐藏组内关闭按钮。

Windows mux 20 项、GUI 54 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\group-pane-remove-20260917\wezterm-gui.exe`。说明与验证边界见 [GROUP_PANE_REMOVE.md](GROUP_PANE_REMOVE.md)。

## 2026-09-17 终端内容四边内边距

组合终端的文字与圆角边框现在保留 10 个逻辑像素的对称内边距，并随 DPI 缩放。PTY 网格、渲染裁剪、鼠标映射、输入法光标和动画坐标同步更新；边距空白仍可激活对应窗格，远程 mux/tmux 不重复缩小。

Windows mux 20 项、GUI 53 项测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\pane-content-padding-20260917\wezterm-gui.exe`。说明与验证边界见 [PANE_CONTENT_PADDING.md](PANE_CONTENT_PADDING.md)。

## 2026-09-17 组合终端间隔统一

组合终端上下、左右的圆角边框间隔统一为 12 个逻辑像素，不再受终端单元格宽高比影响；上下间隔缩小，左右间隔加宽。PTY 行列、分屏比例及输入坐标保持不变。

49 项 Windows 界面测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\equal-pane-gutters-20260917\wezterm-gui.exe`。说明与验证边界见 [EQUAL_PANE_GUTTERS.md](EQUAL_PANE_GUTTERS.md)。

## 2026-09-17 终端交互、字体与渲染改进

修复圆角背景拼接暗线，明确 All/None 选中态，精简悬浮提示；终端焦点改由名称/图标和圆角边框表示，OFF 广播图标透明。分屏窗口缩放保持比例；Ctrl+滚轮可临时调整选中 pane 字号，Settings 可保存全局字号；广播收放时正文、标题、边框与输入坐标使用连续过渡。

71 项 Windows 测试（mux 19、GUI 52）和 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\session-polish-20260917\wezterm-gui.exe`。使用、校验值及验证边界见 [SESSION_POLISH.md](SESSION_POLISH.md)；实际 GPU/SSH/IME 仍需人工验收。

## 2026-09-17 底部广播工具栏重设计

广播底栏改为统一圆角工具栏：左侧状态、中部紧凑 Host 标签、右侧 All/None 分段选择与带过渡的滑动开关/收起按钮。增加长名称省略与成员分页边界处理，紫色仍仅表示实际广播，保留收放动画与原输入语义。

41 项 Windows 界面测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-toolbar-20260917\wezterm-gui.exe`。布局、校验值及验证边界见 [BROADCAST_TOOLBAR.md](BROADCAST_TOOLBAR.md)，真实窗口仍需人工验收。

## 2026-09-17 广播状态显示修正

窗格标题和底栏不再把预选成员显示成广播开启：紫色高亮同时要求组广播开启与该窗格为成员。首次打开/恢复组、拖入关闭的目标组时显示灰色；底栏区分 selected/active，悬停提示明确 ON/OFF。实际输入路由及整组开关语义保持。

5 项 Windows 广播测试及 x64 Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-status-20260917\wezterm-gui.exe`。校验值与验证范围见 [BROADCAST_STATUS.md](BROADCAST_STATUS.md)，真实窗口仍需人工验收。

## 2026-09-17 关闭确认界面统一

旧式终端文本关闭确认已替换为原生居中卡片：深色圆角、遮罩/阴影、矢量图标、对象及连接数量、Cancel/关闭按钮和双向过渡。窗口、Tab、pane 与应用退出统一；取消保留原页面及表单草稿。键鼠、IME、粘贴与拖入操作受确认屏蔽，长按 Enter/Esc 不会在退场后落入终端。

38 项 Windows 界面测试和 x64 GUI Release 构建通过。新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\close-dialog-20260917\wezterm-gui.exe`。说明、校验值及运行边界见 [CLOSE_DIALOG.md](CLOSE_DIALOG.md)，真实窗口和 SSH 仍需人工验收。

## 2026-09-17 Tab 广播按钮改为全组开关

顶部 Tab 广播按钮开启时全选该组所有窗格并广播，关闭时停止广播并取消全部成员选择；只影响对应 Tab。窗格按钮继续提供单独选择，底部开关继续暂停/恢复已选成员。3 项 Windows 广播测试和 x64 Release 构建通过。

新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\group-broadcast-20260917\wezterm-gui.exe`。详情及校验值见 [GROUP_BROADCAST.md](GROUP_BROADCAST.md)。真实窗口和 SSH 操作仍需人工验收。

## 2026-09-17 工作区、New Tab 与广播位置调整

当前代码已按本轮 8 项 UX 要求调整：Tab/窗格显示稳定 Host label；窗格标题右侧切换广播成员，成组 Tab 上切换组广播；移除侧栏 Terminal 导航；默认打开无终端的 Hosts，关闭最后一个终端仍保留应用窗口；顶栏 `+` 打开 New Tab，列出已保存工作区、历史成组 Tab 和最近连接；`Ctrl+S` 保存 SSH 组；单会话和成组 Tab 均可右键重命名；广播面板移动到终端区域底部，支持箭头收起/展开及双向动画。

空首页使用真实无 pane 的 Mux 窗口，不创建隐藏 shell。显式窗口关闭按钮仍关闭应用窗口。保存的是 SSH 连接元数据和分屏布局；点击保存项/历史项时重新连接，不恢复远端进程或命令输入。终端继续使用 Catppuccin Mocha / JetBrainsMono NFM。

本轮 Windows 验证通过 51 项测试（mux 16、UI 26、工作区存储 7、Hosts 2），Windows x64 GUI Release 构建成功。新版入口为 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\workspace-ux-20260917\wezterm-gui.exe`。最终构建结果、测试次数与校验值以 [WORKSPACE_UX_VALIDATION.md](WORKSPACE_UX_VALIDATION.md) 为准。功能、数据位置及人工验收步骤见 [WORKSPACE_UX.md](WORKSPACE_UX.md)。

真实 Windows 窗口、SSH/ConPTY、IME 和动画仍需人工验收。此前自动操作工具明确禁止操作该应用，本轮未通过其他自动化方式绕过。下文是历次实现与构建记录；其中旧侧栏位置、默认本地终端及“尚无持久化”等描述不代表当前代码。

## 2026-09-17 终端 Mocha 配色与 Nerd Font Mono

终端默认配色改为内置 `Catppuccin Mocha`，字体使用本机 Windows 注册名 `JetBrainsMono NFM`（即 JetBrainsMono Nerd Font Mono）。四种常见字重/样式已核验，无需安装字体。配置库现有 8 项 Windows 测试及 GUI Release 构建通过。新版位于 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\mocha-font-20260917\wezterm-gui.exe`，详细记录见 [TERMINAL_STYLE.md](TERMINAL_STYLE.md)。

## 2026-09-17 矢量图标、过渡动画与 Tab 拖放

参考本次 Termius 录屏，原生界面已换用统一矢量图标，修正标题/按钮留白、窄窗口工具栏和细滚动条圆角，加入抽屉滑动、背景遮罩、悬停及页面过渡。单会话 Tab 可拖入当前工作区，在四个方向显示半窗格预览；移动现有连接，支持 Esc/失焦/区域外松手取消。Mux 移动失败会保留源和目标布局，增加 resize 故障回滚测试。

Windows x64 GUI Release 构建通过；本轮 23 项 Windows 测试通过。新版入口：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\modern-ui-20260917\wezterm-gui.exe`。使用说明见 [MODERN_UI.md](MODERN_UI.md)，证据和未验证边界见 [MODERN_UI_VALIDATION.md](MODERN_UI_VALIDATION.md)。[HTML 设计预览](modern-ui-preview.html) 使用示例数据，不是原生截图。真实窗口视觉、SSH 拖放、IME 与广播回归仍需人工验收。

## 2026-09-17 多 SSH 与广播已接入

同 Tab 向右/向下添加 SSH 窗格、按 Tab 广播开关、All/None、逐窗格成员/独立切换已接入真实用户输入。广播使用源终端编码后的字节，经现有异步通道提交到成员；协议回复、鼠标与 overlay 输入不广播。新 pane 默认加入，关闭 pane/Tab 清理状态，OFF 保留成员。

Windows x64 GUI Release 构建通过；21 项 Windows 测试和 12 项 Linux 策略/shell 测试通过。新版位于 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-20260917\wezterm-gui.exe`。使用方式见 [BROADCAST.md](BROADCAST.md)，证据和运行验收边界见 [BROADCAST_VALIDATION.md](BROADCAST_VALIDATION.md)。真实 SSH/ConPTY/IME/vim/tmux 的窗口交互仍需人工验收。

下文保留初始化和前两次 UI 迭代历史，其中“广播未接入”不再代表当前代码状态。

## 2026-09-08 右侧抽屉与窗口拖动更新

Add Host、Edit Host 和 Add Key 已统一为右侧抽屉；外部点击仅收起表单，内部空白点击保留。抽屉使用独立渲染层修复背景文字穿透，小窗口支持字段滚动与 Tab 焦点跟随。顶栏空白与 TermViAI 标题可拖动窗口，按钮继续响应各自操作。

5 项 Windows 原生测试通过。最终 GUI 编译和链接成功，但旧 exe 正在运行，Cargo 最后替换文件失败（退出码 101 / os error 5）；从本轮链接产物打包的新版位于 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\drawer-20260908\wezterm-gui.exe`。未关闭用户当前窗口。源文件与 Windows 快照一致，详情见 [UI_VALIDATION.md](UI_VALIDATION.md)。原生点击和拖动仍需人工验收。

## 2026-09-08 首版界面记录

**已按用户参考图实现原生 TermViAI 工作区、Hosts/Keychain 和 Add/Edit Host，Windows GUI Release 构建及新增 3 项 Windows 测试通过。广播仍未接入 GUI。**

当前界面状态、数据位置和代码入口见 [UI_IMPLEMENTATION.md](UI_IMPLEMENTATION.md)；最新产物、测试及限制见 [UI_VALIDATION.md](UI_VALIDATION.md)。自动操作工具禁止操作该应用，最终视觉与交互、真实 SSH 登录仍需人工验收。

以下内容保留 2026-09-07 的初始化与构建恢复交接历史；其中“完整侧边栏、Host Manager 和视觉重构尚未开始”等描述已由本节取代。后续工作先核对新增 UI 文件及未提交工作树，再完成运行验收和广播接入。

交接日期：2026-09-07。本文记录当前工作区的实际状态，供后续开发者或编码代理接手。

## 1. 当前结论与交付范围

**项目已初始化，广播策略核心已实现并通过测试；广播尚未接入 GUI。Windows GUI 构建阻塞现已解除，Release 构建成功并生成 x64 exe；尚未进行图形窗口与完整 MVP 运行验收。**

同日后续更新：用户手动关闭 Smart App Control，查询确认 Off，Defender 杀毒/实时保护仍开启；Perl Encode 复测退出码 0，原构建脚本退出码 0，耗时 12m 14s。`--version` 退出码 0，但返回 `someone forgot to call assign_version_info` 占位文本，需后续排查。最新证据见 [WINDOWS_BUILD_VALIDATION.md](WINDOWS_BUILD_VALIDATION.md)。下文第 6 节详细失败过程保留为历史。

用户已经明确：**只需要 Windows x64 GUI，不构建 WSL GUI、Windows CLI 或独立 mux-server 产品。** WSL 用于保存和编辑源码；Windows 原生环境用于编译、链接与运行验证。已有 WSL 核心测试结果仅是逻辑验证证据。

产品方向来自《Termius 风格原生 SSH 客户端技术设计方案》：复用 WezTerm 的终端、PTY/ConPTY、Mux、SSH、字体和 GPU 渲染能力，逐步实现面向多 SSH 主机的工作区及选择性广播。原始方案中的 Agent 指令和阶段安排属于参考材料，不能视为用户另行授权，也不能视为已经完成的工作。

当前最优先事项是完成 **Windows GUI 基线运行验收**（Release 构建已通过），随后接入广播输入路径。完整侧边栏、Host Manager 和视觉重构尚未开始。

## 2. 项目、分支与目录

| 项目 | 位置或状态 |
| --- | --- |
| 主开发仓库 | `/home/hanwlax/workspace/termviai` |
| 原始本地 WezTerm | `/home/hanwlax/workspace/wezterm`；本次工作未修改该仓库 |
| 当前分支 | `main` |
| HEAD / 上游基线 | `d2f3f05b38f26a872f4b0bfbb3d2eaa7bdfc1b0b` |
| Windows 构建根目录 | `C:\Users\h00893113\Documents\Codex\termviai-win` |
| Windows 源码快照 | `C:\Users\h00893113\Documents\Codex\termviai-win\src` |
| Windows 预期 GUI 产物 | `C:\Users\h00893113\Documents\Codex\termviai-win\src\target\release\wezterm-gui.exe`，已生成 x64 exe（72,128,000 字节） |
| Windows Rust 工具链 | `C:\Users\h00893113\Documents\Codex\termviai-win\rustup` |
| Windows Cargo 与缓存 | `C:\Users\h00893113\Documents\Codex\termviai-win\cargo` |
| 便携 Perl | `C:\Users\h00893113\Documents\Codex\termviai-win\perl` |
| 本次构建脚本及工作日志 | `C:\Users\h00893113\Documents\Codex\2026-09-07\ch\work\termviai-windows` |
| 已归档验证材料 | `C:\Users\h00893113\Documents\Codex\2026-09-07\ch\outputs\windows-build-validation` |

主仓库保留上游 Git 历史及许可证。远程 `local-wezterm` 指向本地来源，`upstream` 指向 `https://github.com/wezterm/wezterm.git`；二者的 push URL 均为 `DISABLED`。没有配置 TermViAI 自己的远程仓库，也没有执行推送。

### 2.1 必须先注意：新增内容尚未提交

交接时 `git status --short` 为：

```text
 M Cargo.toml
 M README.md
?? README.wezterm.md
?? docs/termviai/
?? termviai/
```

因此当前 HEAD 仍是上游提交。**只克隆分支不会带走新增广播代码和交接资料。** 迁移时应保留完整工作树，或审阅后提交新增内容。不要执行会丢失这些文件的清理或重置操作。

所有上游子模块已经按锁定提交递归初始化，包含 FreeType、libpng、zlib、HarfBuzz，以及 FreeType 的 dlg 子模块。

### 2.2 Windows 目录是构建快照

Windows 源码通过复制工作树获得，复制时排除了 `.git` 和 `target`，不是另一个 Git 开发仓库，也没有自动同步机制。快照增加了 `.tag`，值为：

```text
20260906-101927-d2f3f05b3-termviai-bootstrap
```

截至本次交接，主仓库与 Windows 快照中的 `Cargo.toml`、`Cargo.lock`、`termviai/Cargo.toml` 和三个广播源文件均与已记录 SHA256 一致。校验清单只覆盖这六个文件，不代表全树一致；后续状态文档已在 WSL 主仓库更新。

后续源码变更应以 WSL 主仓库为准，构建前明确同步到 Windows 快照，避免编译旧代码。重新同步时保留或重新生成 `.tag`，不要把 Windows 构建缓存覆盖回源码。

## 3. 已完成的实现

### 3.1 包结构与集成状态

新增 Rust 包是 `/home/hanwlax/workspace/termviai/termviai`，包名为 `termviai-broadcast`，无第三方依赖。

该包暂时拥有独立 Cargo workspace，并在上游根 workspace 的 `exclude` 中声明。这样可以独立测试，而不解析整套 GUI 的依赖。**它尚未成为 `wezterm-gui` 的依赖，也未挂到实际终端输入路径。** 以后可通过 path dependency 接入。

| 文件（相对于主仓库） | 职责 |
| --- | --- |
| `termviai/src/lib.rs` | 导出广播状态、输入路由及报告类型 |
| `termviai/src/state.rs` | `BroadcastState<P>`、成员管理、pane 快照同步、目标选择、未知 pane 校验 |
| `termviai/src/input_router.rs` | 原样分发已编码字节，汇总成功、断线跳过和失败结果 |
| `termviai/tests/broadcast.rs` | 11 项策略测试 |
| `termviai/tests/shell_routing.rs` | Unix 下四个真实 shell 子进程的输入路由验证；使用管道，不是 PTY |
| `termviai/Cargo.toml`、`termviai/Cargo.lock` | 独立包及锁文件 |

根 README 已改为 TermViAI 入口，原上游说明保存在 `/home/hanwlax/workspace/termviai/README.wezterm.md`。当前没有修改生产环境中的 terminal parser、renderer、PTY 或 SSH 实现，也没有重命名上游可执行文件。

### 3.2 广播状态语义

`BroadcastState<P>` 包含开关、成员集合和最新 pane 集合快照。`P` 是泛型 ID，可使用上游 `PaneId = usize`。计划由应用层为每个 Tab 分别持有一个实例；当前包本身不维护 Tab 注册表。

| 操作或状态 | 已实现行为 |
| --- | --- |
| Broadcast OFF | 只选择输入源 pane，保留成员集合 |
| Broadcast ON，源 pane 是成员 | 选择该状态实例的全部成员，每个目标只选择一次 |
| Broadcast ON，源 pane 不是成员 | 只选择源 pane，保持独立输入 |
| `sync_panes` | 新 pane 默认加入；关闭的 pane 移除；已有非成员保持非成员 |
| Select All | 全部加入，不自动打开广播 |
| Select None / 移除最后成员 | 清空后关闭广播 |
| 未知源或未知成员 ID | 返回 `UnknownPane`，不进行发送 |

`sync_panes` 的数据必须来自 Mux，它不拥有 pane 生命周期，也不创建第二套 PTY 或布局树。

### 3.3 输入路由语义与边界

`route_input` 接收状态、源 pane、已编码的 `&[u8]`、连接状态回调和写入回调，返回 `DeliveryReport`。

- 对每个目标使用同一字节切片，不解析命令，不重新编码，也不保存输入内容。
- 跳过断线目标，保留其成员资格；连接恢复后可再次参与。
- 单目标写入失败后继续尝试其他目标，并记录错误。
- 写入回调应保证整段写入，例如使用 `write_all`；发生部分写入错误时不重放整段输入。
- 回调目前是同步接口，没有异步队列、背压或慢连接隔离；这些仍需在实际接入时处理。

**尚未实现磁盘持久化、真实重连、稳定会话 ID 映射或 GUI 状态显示。** 若底层断线时已销毁 pane，当前实现不能自动把旧成员状态转移到新 runtime ID。

## 4. 实际输入路径与后续接入设计

完整源码笔记位于 `/home/hanwlax/workspace/termviai/docs/termviai/ARCHITECTURE.md`。以下是已确认的主要调用链：

```text
GUI key event
  → TermWindow::key_event_impl
    → 快捷键 / modal 处理
    → 普通键：Pane::key_down / key_up
      → LocalPane → TerminalState::key_up_down → 编码 → writer
    → Kitty / Win32：GUI 编码 → Pane::writer
    → IME committed text：Pane::send_composed_text → Pane::writer

Clipboard（异步）
  → TermWindow::paste_from_clipboard
  → 重新查找 pane 或 overlay
  → Pane::send_paste → TerminalState::send_paste
  → bracketed paste / newline canonicalization / de-fang → writer
```

主要位置相对于主仓库：`wezterm-gui/src/termwindow/keyevent.rs`、`wezterm-gui/src/termwindow/clipboard.rs`、`mux/src/pane.rs`、`mux/src/localpane.rs`、`term/src/terminalstate/keyboard.rs`。拖入文本、URL 和路径还有独立的 `send_paste` 调用。

后续适合在“用户输入已经按源终端模式编码，尚未写入 transport”的边界接入路由。这里目前只有纯策略模块，**adapter 尚未实现，不能视为已经解决的接入方案**。

必须处理以下关键点：

1. `mux/src/domain.rs` 的 `WriterWrapper` 被 Pane 与 Terminal 共享，还承载终端协议响应。不能在该 writer 上无条件广播所有写入。
2. 只对用户键盘、IME 提交文本和粘贴输入路由；鼠标、resize、焦点报告及设备状态查询响应不应扇出。
3. 目标端使用原始 transport 写入，避免再次编码或递归进入 router。
4. 应用层应从 `iter_panes_ignoring_zoom()` 获取完整 Tab pane 快照，包含缩放模式下被隐藏的 pane；路由前核对成员归属。
5. Overlay、搜索框和命令面板不应参与终端广播；异步剪贴板回调还需重新确认目标生命周期。
6. 不同目标的应用光标、Kitty、bracketed paste 模式可能不同。“相同字节”不能保证目标应用产生相同语义，需在真实终端程序中验证。

上游 `PaneId`、`TabId` 都是 `usize`。Mux 已拥有二叉布局树。Pane 关闭会从映射移除、调用 `kill` 并发送 `PaneRemoved`；Tab 关闭会移除其 pane。未来应用元数据必须跟随这些生命周期清理。

## 5. 已执行的验证与覆盖限制

| 验证项 | 实际结果 | 能证明什么 |
| --- | --- | --- |
| WSL 广播策略测试 | 11 项通过 | 成员选择、输入路由和错误处理逻辑 |
| WSL 四 shell 管道测试 | 1 项通过 | 三成员接收广播，第四 shell 仅接收独立输入 |
| Windows/MSVC 广播策略测试 | 11 项通过 | 同一策略代码能在 Windows 编译、链接并运行测试 |
| Windows Unix shell 测试 | 按 `cfg(unix)` 跳过 | 不计为 Windows shell 或 ConPTY 验收 |
| 广播包 Clippy / fmt | 通过 | 已记录的检查在 WSL 工具链执行；stable rustfmt 提示忽略上游 nightly-only 配置 |
| `git diff --check` | 通过 | 已跟踪文件差异无空白格式错误 |
| Windows GUI Release 构建 | 后续重试通过，退出码 0 | 已完成 GUI 编译和链接并生成 x64 exe；不代表运行验收 |
| GUI 启动、ConPTY、SSH、IME、vim/tmux | 未执行验收 | 不应标为通过 |

现有字节测试包含控制字节、箭头序列、粘贴包围序列和 UTF-8；这验证的是字节原样路由，不是实际 GUI 键盘编码、输入法或剪贴板行为。

## 6. Windows 构建环境与首次阻塞历史

**当前状态：该节记录的拦截已解除，构建已成功。** 用户已手动关闭 Smart App Control，防护状态与构建证据见最新验证报告。以下失败日志和处置建议描述首次构建时的状态，不再是接手前置阻塞。

### 6.1 已确认的工具链

| 工具 | 版本 / 位置 |
| --- | --- |
| Rust | 1.98.1，host 为 `x86_64-pc-windows-msvc` |
| Visual Studio | Visual Studio 2026 Build Tools，MSVC 14.50.35717 |
| MSVC 环境入口 | `C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat` |
| Windows SDK | 10.0.26100.0 |
| CMake | 使用 Visual Studio 自带 CMake |
| Strawberry Perl | 5.42.2，便携 ZIP 发行版 5.42.2.1；已核对官方下载文件 SHA256 |

初次仅检查 PATH 时未检测到 MSVC，后续已找到现有安装，**不需要把“未安装 Visual Studio”当作当前阻塞**。Rust 和 Perl 使用独立目录，没有修改用户 PATH。

构建过程中发现进程 `PATHEXT` 只有 `.CPL`，导致 `rustc`、`findstr` 等命令无法解析。`build-gui.cmd` 已通过 `setlocal` 在本进程补齐扩展名；没有更改系统环境变量。构建使用 6 个并行任务。

### 6.2 阻塞证据

第一处是 Cargo 生成的 `getrandom v0.3.4` 构建脚本被禁止执行：

```text
could not execute process ...\getrandom-5e84762c2ed0350c\build-script-build (never executed)
应用程序控制策略已阻止此文件。 (os error 4551)
```

第二处是配置 OpenSSL 时，系统阻止加载：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\perl\perl\lib\auto\Encode\Encode.xs.dll
```

Git for Windows 自带 Perl 已验证可以加载 Encode，但它不能解决 `getrandom` 构建脚本被拒执行的问题，因此没有据此完成构建。

目前证据足以确认是操作系统应用程序控制阻塞；**尚未确认具体由哪一种应用控制产品或规则实施**。不要未经核实就归因于某个安全产品，也不要把它称为 Rust 源码编译错误。

没有关闭安全策略、绕过阻止或更改受控文件来规避检查。继续构建需要管理员按组织规则批准相关执行，或使用已有批准的 Windows 构建环境。解决该阻塞之后，后续编译仍可能出现尚未覆盖到的问题。

构建失败后已确认没有 cargo/rustc 编译进程；停止了仍等待的本次 PowerShell 包装进程。不要把包装进程的退出码当作 Cargo 原生返回码。日志中的明确错误是本次失败依据；`windows-gui-build.exit` 不是本次可靠的完成标志。

### 6.3 复现入口

批准的构建环境准备好后，先确保 Windows 快照对应所需源码，再在 PowerShell 中运行归档脚本：

```powershell
$build = 'C:\Users\h00893113\Documents\Codex\2026-09-07\ch\outputs\windows-build-validation\build-gui.cmd'
& "$env:SystemRoot\System32\cmd.exe" /d /c $build
```

该脚本使用固定的本机路径，调用 `vcvars64.bat`，配置进程内 Rust/Perl/CMake 路径，最终执行：

```text
cargo build --locked --release -p wezterm-gui
```

优先直接运行 `.cmd` 查看真实退出结果。归档的 `run-build.ps1` 用于输出日志，但本次出现了子任务退出后包装进程仍等待的情况，后续需检查这一行为，不能只等待完成标志文件。

Windows 原生核心测试可使用同目录的 `test-core.cmd`；其最终命令为：

```text
cargo test --offline --locked --manifest-path termviai\Cargo.toml
```

仅执行上述核心测试时，尚未接入 GUI 的广播包不会自动成为 GUI 编译依赖。不要将它通过测试理解为 GUI 已启用广播。

## 7. 接手后的工作顺序与验收门槛

| 顺序 | 工作 | 完成条件 |
| --- | --- | --- |
| 1 | 核对并保全工作树 | 确认未提交文件、分支、子模块、Windows 快照；审阅后提交或备份新增内容 |
| 2 | 解决受控环境阻塞（已完成） | 用户关闭 Smart App Control，Perl Encode 复测通过 |
| 3 | Windows GUI 基线构建（已完成） | 原命令退出码 0，生成实际 x64 exe；后续源码变更仍需重新构建 |
| 4 | 基线运行验收 | GUI 可启动；本地 shell、ConPTY、窗口/分屏/resize、clipboard、CJK 和 SSH 实测 |
| 5 | 广播生产接入 | 加入 GUI path dependency、每 Tab 元数据、输入 adapter、生命周期处理和明确可见的状态 |
| 6 | 四 pane 广播验收 | A/B/C 为成员、D 独立；成员输入只到 A/B/C，D 输入只到 D；OFF 后全部独立，再 ON 保留成员 |
| 7 | 终端与性能回归 | Ctrl+C、Tab、箭头、粘贴、IME、vim/tmux 等正常；协议响应不广播；失败和慢连接不拖死 GUI |
| 8 | 后续产品功能 | 在前述基础上实现 Pane Header、Broadcast Bar、Hosts、分组、恢复与视觉完善 |

运行验证可使用临时配置避免依赖当前用户的终端配置。基线 GUI 仍使用 WezTerm 名称和界面；并不存在已经完成的 Termius 风格 UI。

打包时不能只拿单个 exe。上游 Windows GUI 构建脚本涉及 `conpty.dll`、`OpenConsole.exe`、`libEGL.dll`、`libGLESv2.dll` 和 `mesa/opengl32.dll`，应核对实际依赖并验证可移植目录。`OpenConsole.exe` 属于 ConPTY 辅助组件，不是本次排除的独立 CLI 产品。是否需要任何其他辅助可执行文件，须基于实际运行结果确认，不能预先宣称单 exe 可独立运行。

## 8. 文档与证据索引

主仓库内的阅读顺序：

1. `/home/hanwlax/workspace/termviai/README.md`：项目入口和当前边界。
2. `/home/hanwlax/workspace/termviai/docs/termviai/HANDOVER.md`：本文仓库副本。
3. `/home/hanwlax/workspace/termviai/docs/termviai/ARCHITECTURE.md`：输入与生命周期源码定位。
4. `/home/hanwlax/workspace/termviai/docs/termviai/BUILD.md`：Windows GUI 构建说明。
5. `/home/hanwlax/workspace/termviai/docs/termviai/STATUS.md`：阶段记录。
6. `/home/hanwlax/workspace/termviai/docs/termviai/WINDOWS_BUILD_VALIDATION.md`：上次原生构建结果。
7. `/home/hanwlax/workspace/termviai/docs/termviai/design-reference.md`：原始设计参考。

已归档证据：

- [Windows 构建验证报告](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/REPORT.md)
- [GUI 构建错误日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/windows-gui-build.stderr.log)
- [工具链预检日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/windows-gui-build.log)
- [Windows 原生测试结果](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/windows-core-test.log)
- [源码快照校验清单](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/source-snapshot.json)
- [Windows GUI 构建脚本](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/build-gui.cmd)
- [Windows 核心测试脚本](/mnt/c/Users/h00893113/Documents/Codex/2026-09-07/ch/outputs/windows-build-validation/test-core.cmd)

## 9. 可直接用于下一次开发的任务说明

> 接手 `/home/hanwlax/workspace/termviai`，先阅读交接文档并保全未提交内容。用户仅需要 Windows x64 GUI，不构建 WSL GUI、Windows CLI 或独立 mux-server。当前 `termviai-broadcast` 仅是通过测试的独立策略包，未接入 GUI。Windows/MSVC 工具链已准备，原 Smart App Control 拦截经用户手动关闭后解除，GUI Release 构建成功并生成 x64 exe。先完成图形窗口、ConPTY、SSH 等基线运行验收，并排查 --version 的占位文本。运行基线通过后，按实际输入路径完成广播 adapter 及四 pane 验收。保留 Mux 作为布局与 pane 生命周期依据，不要直接广播底层 writer 的全部数据，也不要将核心测试通过写成 GUI 或 MVP 验收完成。
