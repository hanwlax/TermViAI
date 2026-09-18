# TermViAI 状态

## 2026-09-18 New Tab 精简

移除占用页面空间的 Recent tab groups 及自动记录链路；New Tab 只显示手动保存组与最近主机连接。旧自动历史在下次正常保存库时清理，保留已保存组、连接记录和兼容扩展字段。退出应用不再等待各窗口写入自动历史。

## 2026-09-18 重连输出不再覆盖历史

重连 reader 不再重放带绝对光标定位的启动横幅，旧输出解析完毕才允许替换连接。新输出从历史末尾继续，重置残留光标模式和滚动区域，屏幕写满时滚入历史；保留 SSH 具体错误，省略底层进程退出/Hold 提示。覆盖连续失败后成功、光标停在历史中间和窄屏折行场景；验证详情见 [SSH_RECONNECT.md](SSH_RECONNECT.md)。

## 2026-09-18 SSH 重连修正版

修复失效缓存会话阻塞原位重连和新 Tab 连接的问题，保留旧组即可新建连接。错误提示限制在对应 pane，刷新按钮改成小号单箭头圆弧；整组断线时在 Tab 显示一键重连。移除误解参考图而添加的蓝色分隔线，历史内容继续保留。当前行为以 [SSH_RECONNECT.md](SSH_RECONNECT.md) 为准。

## 2026-09-18 SSH 原位重连

SSH 断线、远端 shell 结束或 PTY 读取失败后，原 pane 与组合布局会保留，并在名称右侧提供蓝色刷新按钮。重连复用同一 Terminal，在蓝色分隔线下继续输出；主屏滚动历史、焦点、临时字号与广播成员关系不丢失。断线 pane 不接受输入且暂不参与实际广播。Windows x64 已通过 94 项测试及 Release 构建，详情见 [SSH_RECONNECT.md](SSH_RECONNECT.md)。

## 2026-09-18 发布后项目记忆

`v0.1.0-beta.1` 已推送至 GitHub 并作为 prerelease 发布，远端 `main`、标签和发布提交一致，Windows 附件已回传下载并通过 SHA-256 校验。当前真实状态、关键路径、开发约束和按优先级排列的后续工作统一记录在 [MEMORY.md](MEMORY.md)。

## 2026-09-17 v0.1.0-beta.1 品牌与发布

产品名已统一为 TermViAI（Terminal Via AI），用户可见界面、Windows 产品元数据和便携包入口均使用该名称。作为首个发布版本，内部模块、配置项、环境变量和数据目录也统一为 `termviai`/`TERMVIAI`，不保留旧品牌兼容层。版本固定为 `0.1.0-beta.1`，目标公开仓库和 Release 为 [hanwlax/TermViAI](https://github.com/hanwlax/TermViAI)。

## 2026-09-17 SSH Keepalive Interval

Settings 可配置 SSH keepalive 间隔，默认 30 秒，`0` 关闭，最大 86400 秒。值持久化到 `settings.json`，通过已有 `serveraliveinterval` 接入 SSH 会话；修改后对新连接生效。Windows mux 20 项、GUI 60 项测试及 x64 Release 构建通过；新版见 [SSH_KEEPALIVE.md](SSH_KEEPALIVE.md)。

## 2026-09-17 终端背景一致性

活动与非活动 TermViAI pane 使用一致的基础背景，修复标题/留白与文字区域因非活动背景压暗产生的色差。焦点边框、名称和图标保持原设计，普通 WezTerm 模式不变。Windows mux 20 项、GUI 58 项测试及 x64 Release 构建通过；新版见 [PANE_BACKGROUND.md](PANE_BACKGROUND.md)。

## 2026-09-17 响应式布局与窗口缩放

Broadcast 标题和 `x/x selected` 已从固定坐标改为相对内容区布局；Windows 窗口增加 720×480 逻辑像素最小尺寸。TermViAI 实时缩放仍逐帧更新界面，但只在终端网格变化时触发 Mux/PTY 重排。Windows mux 20 项、GUI 57 项测试及 x64 Release 构建通过；新版见 [RESPONSIVE_RESIZE.md](RESPONSIVE_RESIZE.md)。

## 2026-09-17 终端与 Broadcast 四边等距

下排终端和 Broadcast 面板之间改为固定 12 个逻辑像素，与顶部及左右间距一致。整行取整产生的剩余像素由终端可视外框吸收，不改变 PTY 行列、分屏比例和输入坐标。Windows mux 20 项、GUI 56 项测试及 x64 Release 构建通过；新版见 [FOUR_SIDED_GUTTER.md](FOUR_SIDED_GUTTER.md)。

## 2026-09-17 左侧栏动画切换

顶栏菜单按钮现在只切换左侧栏，不再打开 Hosts。收起与显示使用 220 毫秒双向动画，保持当前页面和会话；页面内容、终端尺寸、底部 Broadcast 工具栏及鼠标命中区同步更新。Windows mux 20 项、GUI 55 项测试及 x64 Release 构建通过；新版见 [SIDEBAR_TOGGLE.md](SIDEBAR_TOGGLE.md)。

## 2026-09-17 组合 Tab 单终端移除

每个组内终端标题的广播按钮右侧新增关闭按钮，可精确断开并移除对应 pane；剩余布局和广播成员自动同步，只剩一个终端时关闭按钮隐藏。Windows mux 20 项、GUI 54 项测试及 x64 Release 构建通过；新版见 [GROUP_PANE_REMOVE.md](GROUP_PANE_REMOVE.md)。

## 2026-09-17 组合终端间隔统一

组合终端的上下、左右视觉间隔统一为 12 个逻辑像素：缩小上下间隔并加宽左右间隔。49 项 Windows 界面测试及 x64 Release 构建通过；新版与验证见 [EQUAL_PANE_GUTTERS.md](EQUAL_PANE_GUTTERS.md)。

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

## 2026-09-17 工作区 UX 当前状态

- Tab 和 pane header 使用稳定 Host label，支持对单会话/成组 Tab 右键重命名。
- pane header 提供独立/广播成员切换，成组 Tab 自带组广播开关；底部广播面板提供成员和状态控制，并可通过箭头收起/展开，两个方向均有动画。
- 移除侧栏 Terminal 导航。默认启动为真实空 Hosts 页面；从终端页关闭最后一个终端后回到 Hosts，窗口继续保留，显式窗口关闭仍生效。
- 顶栏 `+` 打开 New Tab，展示 Saved workspaces、Recent tab groups、Recent connections；`Ctrl+S` 保存成组 SSH 的连接元数据和分屏布局，重复保存更新原条目。
- 点击保存项/历史项重新创建 SSH 组。保存不包含命令内容、滚屏、认证密码或广播开关；认证继续沿用 SSH 流程。
- 保留 Catppuccin Mocha、JetBrainsMono NFM、矢量图标和原生过渡效果。

Windows 测试 51 项通过（mux 16、UI 26、工作区存储 7、Hosts 2），Windows x64 GUI Release 构建成功。新版目录为 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\workspace-ux-20260917`；最终结果以 [WORKSPACE_UX_VALIDATION.md](WORKSPACE_UX_VALIDATION.md) 为准。

功能说明和数据边界见 [WORKSPACE_UX.md](WORKSPACE_UX.md)。真实窗口的布局、动画、SSH、ConPTY、IME 与广播操作仍待人工验收；此前自动操作工具对该应用的限制保持有效，未通过替代自动化绕过。后续章节保留历史结果。

## 2026-09-17 终端 Mocha 配色与 Nerd Font Mono

终端默认配色改为内置 `Catppuccin Mocha`，字体使用本机 Windows 注册名 `JetBrainsMono NFM`（即 JetBrainsMono Nerd Font Mono）。四种常见字重/样式已核验，无需安装字体。配置库现有 8 项 Windows 测试及 GUI Release 构建通过。新版位于 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\mocha-font-20260917\wezterm-gui.exe`，详细记录见 [TERMINAL_STYLE.md](TERMINAL_STYLE.md)。

## 2026-09-17 终端内容四边内边距

终端网格现在在圆角窗格内保留 10 个逻辑像素的对称边距，PTY 行列、文字渲染、鼠标映射、输入法光标和动画裁剪使用同一尺寸。Windows mux 20 项、GUI 53 项测试及 x64 Release 构建通过；新版见 [PANE_CONTENT_PADDING.md](PANE_CONTENT_PADDING.md)。

## 2026-09-17 矢量图标、过渡动画与 Tab 拖放

参考本次 Termius 录屏，原生界面已换用统一矢量图标，修正标题/按钮留白、窄窗口工具栏和细滚动条圆角，加入抽屉滑动、背景遮罩、悬停及页面过渡。单会话 Tab 可拖入当前工作区，在四个方向显示半窗格预览；移动现有连接，支持 Esc/失焦/区域外松手取消。Mux 移动失败会保留源和目标布局，增加 resize 故障回滚测试。

Windows x64 GUI Release 构建通过；本轮 23 项 Windows 测试通过。新版入口：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\modern-ui-20260917\wezterm-gui.exe`。使用说明见 [MODERN_UI.md](MODERN_UI.md)，证据和未验证边界见 [MODERN_UI_VALIDATION.md](MODERN_UI_VALIDATION.md)。[HTML 设计预览](modern-ui-preview.html) 使用示例数据，不是原生截图。真实窗口视觉、SSH 拖放、IME 与广播回归仍需人工验收。

## 2026-09-17 多 SSH 与广播已接入

同 Tab 向右/向下添加 SSH 窗格、按 Tab 广播开关、All/None、逐窗格成员/独立切换已接入真实用户输入。广播使用源终端编码后的字节，经现有异步通道提交到成员；协议回复、鼠标与 overlay 输入不广播。新 pane 默认加入，关闭 pane/Tab 清理状态，OFF 保留成员。

Windows x64 GUI Release 构建通过；21 项 Windows 测试和 12 项 Linux 策略/shell 测试通过。新版位于 `C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-20260917\wezterm-gui.exe`。使用方式见 [BROADCAST.md](BROADCAST.md)，证据和运行验收边界见 [BROADCAST_VALIDATION.md](BROADCAST_VALIDATION.md)。真实 SSH/ConPTY/IME/vim/tmux 的窗口交互仍需人工验收。

下文保留初始化和前两次 UI 迭代历史，其中“广播未接入”不再代表当前代码状态。

# 初始化状态

日期：2026-09-07。

## 完成

- 从本地 WezTerm 创建独立 Git 仓库；首个公开版本整理到 `main` 分支。
- 上游代码基线和许可证完整保留；源仓库没有修改。
- 新增无依赖 `termviai-broadcast` 包及策略/子进程测试。
- 完成键盘、IME、粘贴、writer、Mux 生命周期源码定位。
- 保存原始设计参考和构建说明。
- 已按锁定提交递归初始化全部 Git 子模块（包括 FreeType 的 dlg 子模块）。

## 验证结果

- Rust 1.98.1 隔离工具链。
- `cargo test --offline`：11 项策略测试通过。
- 四个真实 `/bin/sh` 子进程管道测试通过，验证三成员广播和第四个独立输入。
- 最终 WSL 项目路径下 `cargo test --offline --locked` 再次通过，共 12 项测试。
- `cargo clippy --all-targets -- -D warnings` 通过。
- `cargo fmt -- --check` 和 `git diff --check` 通过；stable rustfmt 提示忽略上游 nightly-only 的 imports_granularity 配置。
- 这些测试不覆盖 PTY、GUI、SSH 或真实键盘编码。

## 未完成项

- GUI 输入路由接入、开关与成员 UI。
- Windows 图形窗口运行、ConPTY、SSH 和终端程序验收（GUI Release 构建已通过）。
- Host 管理、Pane Header、Broadcast Bar、Workspace 持久化、重连。
- 广播延迟、CPU/RSS、16/32 pane 性能测试。

初始检查中，WSL 与 Windows PATH 均未找到 cargo/rustc；上游四个字体相关
Git 子模块未初始化，现已全部补齐。本次采用隔离工具链验证广播包，没有把策略测试当作完整 GUI 构建。

## 下一验收门槛

Windows 上游基线通过后，完成实际 GUI 输入接入。四 pane 设置 A/B/C 成员、D 独立，
验证任一成员输入只进 A/B/C，D 输入只进 D；关闭和重新开启保留成员。
覆盖 Ctrl+C、Tab、箭头、粘贴、IME、vim/tmux，确认终端协议响应不被广播。

## Windows GUI 原生构建验证（2026-09-07）

- 范围仅 Windows x64 GUI；完整记录见 [WINDOWS_BUILD_VALIDATION.md](WINDOWS_BUILD_VALIDATION.md)。
- 已定位现有 MSVC 14.50.35717 和 Windows SDK 10.0.26100.0。
- 已配置隔离的 Windows Rust 1.98.1 和便携 Perl。
- Windows 原生广播策略 11 项测试全部通过。
- `cargo build --locked --release -p wezterm-gui` 失败：系统应用程序控制策略拒绝执行 getrandom 构建脚本（os error 4551），同时拒绝加载 Perl Encode.xs.dll。
- 未生成 GUI exe；GUI 启动、ConPTY 和 SSH 未验收。未更改系统安全策略。
- 继续构建需要组织批准的 Windows 构建环境或管理员依据策略批准构建程序执行。

## 构建阻塞解除（2026-09-07 后续更新）

- 原阻塞确认为 Smart App Control；用户授权后手动关闭。系统查询确认 Off，Defender 杀毒/实时保护均保持开启。
- Perl Encode 复测通过；同一源码快照及构建脚本的 GUI Release 构建成功，退出码 0，耗时 12m 14s。
- 已生成 72,128,000 字节的 x64 wezterm-gui.exe，ConPTY、ANGLE、Mesa 附带文件齐全。
- `--version` 退出码 0，但返回 `someone forgot to call assign_version_info` 占位信息，待排查。
- 未修改生产源码或清理已有工作树；图形窗口、ConPTY、SSH 等尚未验收，广播仍未接入 GUI。
- 上一节的失败描述为历史结果，当前验证和证据见 [WINDOWS_BUILD_VALIDATION.md](WINDOWS_BUILD_VALIDATION.md)。

## 原生 TermViAI 界面（2026-09-08）

- 已实现工作区顶栏、Hosts/Keychain 侧栏、搜索/分组卡片、Add/Edit Host 与本机密钥引用。
- 排除用户指定的 SFTP、Vaults、Port Forwarding、Snippets、Known Hosts、Logs、账号登录和 Telnet 入口。
- Windows GUI Release 构建通过；新增 3 项 Windows 原生测试通过，格式检查通过。
- 自动操作工具禁止操作该 exe，最终显示与真实 GUI/SSH 交互仍待人工验收；广播未接入。
- 详细实现及当前验证见 [UI_IMPLEMENTATION.md](UI_IMPLEMENTATION.md)、[UI_VALIDATION.md](UI_VALIDATION.md)。
