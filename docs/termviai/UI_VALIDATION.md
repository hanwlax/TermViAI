# TermViAI UI 验证

## 2026-10-22 Identities 与 Host 密码认证

Keychain 增加 Keys / Identities 分区、身份卡片和新增/编辑抽屉；Host 增加遮蔽密码、Show/Hide、Remember password 和可滚动认证选择器。沿用现有矢量图标、卡片间距、圆角、低饱和紫色和抽屉动画。密码存储与兼容约定见 [IDENTITIES.md](IDENTITIES.md)。


Windows 原生回归：SSH 18 项、GUI 101 项，共 119 项通过；Release 构建通过。验证了系统凭据的中文密码读写/替换/删除、进程内密码、旧库兼容、Identity 引用与留空保留密码、Host 记住开关双向迁移及 JSON 无明文密码。验证包 `releases/identities-20261022.zip`，SHA-256 `7b17a8db3762c167bce6108436e90a1117d53cbafb0906e97fd5ae3b9639ebf5`；日志与源码校验位于 `releases/verify-identities-20261022/`。16 个源文件/清单与 Windows 构建输入一致。真实 SSH 密码/MFA 认证和鼠标视觉未进行自动端到端验收。

## 2026-10-22 移除低频侧栏区域与 Grid view 标识

移除终端页侧栏 WORKSPACE 标题、Add SSH right、Add SSH below 与 Save group 按钮及其点击区域。清理只服务于该添加入口的待选主机状态和返回提示；拖动组合终端、标签右键保存组及 Ctrl+S 保留。移除 Hosts/Keychain 工具栏右侧静态 Grid view 图标文字，保留 Add Host / Add Key、搜索与主机网格布局。


Windows GUI 97 项测试与 Release 构建通过；修改的 11 个 Rust 文件与构建源码一致。验证包 `releases/ui-trim-20261022.zip`，SHA-256 `5c75e7800cdb7d7aa169c4191cdd41b6a6514661b844d5fc2309da8023b52c63`。构建记录位于 `releases/verify-ui-trim-20261022/`；实际窗口视觉待人工验收。

## 2026-10-22 精简提示与广播悬浮隔离

移除 Host 三点按钮、广播总开关、广播底栏展开/收起按钮的悬浮提示。终端标题广播按钮使用独立的交互标识，底栏成员按钮继续使用原标识；hover 动画、按下反馈和提示锚点因此分别归属各自按钮。两处点击仍调用同一成员切换逻辑，实际广播选择和启用状态保持同步。


Windows GUI 97 项测试与 Release 构建通过，11 个修改的 Rust 文件与 Windows 构建源码一致。验证包 `releases/hover-isolation-20261022.zip`，SHA-256 `db74858701de3c1344c8cf31cb15559ef5b52d8096a991119a7aff03cc448d6e`；日志与校验记录在 `releases/verify-hover-isolation-20261022/`。本次不新增镜像实现的 UI 单元测试；实际鼠标交互视觉仍需人工验收。

## 2026-10-22 Host 菜单入口与终端标点连字

Host 卡片省略号单击打开 Edit Host / Duplicate Host 菜单，菜单锚定按钮下方并限制在窗口内。单击菜单项执行操作，点击外部或 Esc 关闭；Host 卡片右键不再打开菜单。省略号悬浮提示改为 Host actions，任意菜单打开时不绘制悬浮提示，避免和菜单重叠。

` ]# ` 粘连已用本机安装的 `JetBrainsMonoNerdFontMono-Medium.ttf` 实测定位为字体连字：HarfBuzz 默认特性产生 `SPC` + `bracketright_numbersign.liga`；关闭 `liga/clig/calt` 后产生独立 `bracketright` 和 `numbersign`，每个字符 advance 仍为 600 字体单位。产品默认特性改为 `kern, liga=0, clig=0, calt=0`，保留字偶距；显式用户配置可重新启用连字。这里不插入空格，也不修改远端 shell 提示符。实验输出保留于验证目录的 `termviai-prompt-shaping.jsonl`。


Windows GUI 97 项测试通过，包含默认 `]#` 与单字符绘制结果一致、显式开启连字的旧快照、Host 菜单锚点及不直接打开编辑表单。Windows Release 构建通过，11 个修改的 Rust 源文件与构建目录逐字节一致。验证包 `releases/host-menu-20261022.zip`，SHA-256 `f8660cf895b55318d73fdf9a6c58832de2dece0e219bf00cb049c69c14c8019f`；测试、构建、字体实验及源文件校验保存在 `releases/verify-host-menu-20261022/`。未自动验收实际窗口鼠标交互视觉。

## 2026-10-22 字体与 Host 编辑交互

- 默认终端字体继续使用 Windows 的 JetBrainsMono NFM，默认字重改为 Medium (500)，增强笔画；显式用户字体配置仍优先。本机已确认存在该字体的 Medium 文件。截图不能证明渲染器与 VS Code 完全一致，实际字重视觉仍需对照验收。
- 输入框不再将 `│` 插入字符串；使用与标题字体相同的 shaping advances 绘制独立、像素对齐的 1 px 光标。输入、搜索、选择高亮、鼠标定位和 IME 位置使用同一套文字度量，超长内容随光标横向滚动。
- 添加/编辑 Host、Key、设置和重命名输入框支持鼠标拖动选区、Shift+方向键/Home/End、Ctrl+A/C/X/V。复制只取选区，输入/删除替换当前选区，支持中文；剪切无选区时不改变剪贴板或文本。
- 短悬停提示按真实字体宽度分配空间，`Edit Host` 不再被估算字符列宽硬拆成两行。
- Hosts 卡片（含省略号区域）右键打开 Edit Host / Duplicate Host 菜单。Duplicate 重新读取库，复制全部字段，label 使用 `xxx-copy`；重名追加 `-2`、`-3`。保存采用现有原子写入方式；原主机已被其他窗口修改时拒绝复制过期快照。菜单点击外部或 Esc 关闭。

Windows GUI 95 项测试通过，包含选区复制/替换/删除、比例字宽命中、重复复制及全部配置字段/持久化/原主机保留等回归。最终 Windows Release 构建通过；本地验证包 `releases/host-editor-20261022.zip`，SHA-256 `07afc3261d82f592797d97ce8b40490bc1fcec0a93bf984466bab9d6521a6f0f`。构建与测试记录位于 Windows 构建目录 `releases/verify-host-editor-20261022/`。真实四路 SSH 与鼠标视觉没有自动端到端验收。


## 2026-10-22 组内重排与错误提示

广播全选修正：tab-bc 颜色与点击共用 `group_broadcasting`，只在运行且全部选中时紫色；部分选中为灰色，点击会全选并启用，全部启用时再次点击会全部关闭。排除的 pane 保持独立输入，其他成员继续广播，底部暂停/恢复开关语义保留。新增回归覆盖四成员中排除一个、重新全选、单独补齐、暂停后重新全选、全部关闭与其他组隔离。顶栏广播/关闭图标从 16 px 改为与 pane 标题一致的 18 px（100% DPI 笔画为 2 px）。Windows GUI 65 项及 Release 构建通过，验证包 `releases/broadcast-all-20261022.zip`，SHA-256 `92210eef7e759ad55a278af94cd93903f6d5f4fc391d928f9c7ee72abbfd08a8`；日志保存在 `releases/verify-broadcast-all-20261022/`。

同日图标交互细化：静态无底色、广播紫/灰区分状态、悬停关闭 X 变红和广播背景淡入。GUI 64 项回归和 Release 构建通过，新验证包 `releases/icon-hover-20261022.zip`（包含组内重排等前述功能），SHA-256：`bf630d4874044771b48fd16a41ebb8514e5a84ce17e0a0a765d227e15672f11c`。日志保存在 Windows 构建目录 `releases/verify-icon-hover-20261022/`。

- 从组内终端标题栏拖起，越过 6 个逻辑像素后显示小手；放到另一终端的整块区域交换位置。来源自身、组外、Esc、失焦、已关闭来源/目标或切换到其他组不会执行交换。放大单个 pane 时需先恢复组合视图。
- 交换原有 pane 对象及活动位置索引，保留 SSH 连接、滚屏、广播成员、每终端字号、原焦点与布局比例；不临时切换焦点，不触发远端焦点报告。拖动后 Ctrl+S 可保存新布局。
- 标签组网格和广播图标采用简化矢量线条；标签栏及终端标题栏的广播/关闭图标静态均无底色；广播开启为低饱和紫色、关闭为灰色，悬停时才淡入灰紫色圆角背景。关闭 X 悬停变红且保持透明背景。
- 组内有任一可重连的 SSH 终端离线时，标签提供重连入口，只处理离线成员，在线与正在重连的成员不会被重启。
- 广播失败/跳过离线终端、重连错误绑定 pane ID；对应终端标题栏右侧显示红色圆形叹号，悬停 400ms 查看折行详情、点击消除，关闭 pane 时清理。开始重连或下次广播投递成功会清理该 pane 的广播错误；未投递输入不会重放。其他终端操作错误绑定所属标签，不再跨标签悬浮整条横幅。
- 窄终端优先保留错误与重连/广播按钮；宽度不足三个按钮时暂隐藏移除按钮。错误提示限制在窗口内。

Windows 回归：mux 42 项、TermViAI GUI 64 项、广播 12 项，共 118 项通过；最后的窄栏布局修改另跑 GUI 64 项。Windows x64 Release 构建通过；最终构建源文件与本地 8 个已修改 Rust 文件逐字节一致。测试覆盖交换布局与对象/焦点/字号保留、关闭目标与放大拒绝、部分离线重连入口、错误随 pane 迁移及清理、Unicode 折行和窄栏按钮边界。实际四路 SSH 下的鼠标拖放、小手与红色圆点视觉仍需人工验收。

验证包 SHA-256：`c972986317b285a5fc6be85b97ee9546d84c3fbe08c575e54b9bbaefa1f64094`。构建和测试日志保存在 Windows 构建目录 `releases/verify-group-ui-20261022/`。

本地验证包为 `releases/group-ui-20261022.zip`，基于 beta.3，包含待提交的 SSH 空闲保活修复，不代表新发布版本。解压后运行其中的 TermViAI.exe；现有安装与已发布 beta.3 不被替换。

## 2026-10-12 应用图标重绘

图标改为深色圆角底、紫色终端提示符、青色光标和小星芒。TermViAI = Terminal Via AI，表示通过 AI 制作的终端应用；星芒表达制作来源，当前产品没有内置 AI 功能。

透明 PNG 源图为 `assets/icon/termviai-icon.png`，GUI 使用的 `terminal.png` 为 128 px，Windows `terminal.ico` 包含 16/20/24/32/40/48/64/96/128/256 px。`assets/icon/export.py` 从同一源图生成资源，`update.sh` 调用该脚本，避免后续重新导出旧 WezTerm 图标。图标与生成提示词说明见 [assets/icon/README.md](../../assets/icon/README.md)。

Windows Release 构建成功，直接解析 EXE 的 PE 图标资源，确认 ID 0x101 包含上述全部 10 个尺寸，每个图像资源与新 ICO 内容逐字节一致。未重新运行不受图标变更影响的行为测试。最新本地验证包 `releases/termviai-icon-20261012.zip` 包含之前的修复和 tab 拖动改进；任务栏和资源管理器视觉仍需人工查看。构建日志 `/tmp/termviai-icon.log`。

## 2026-10-12 拖动 tab 排序

按住顶栏真实 tab 的名称区域，越过拖动阈值后左右移动，紫色插入线指示松手位置。支持当前选中 tab、未选中 tab、整个多终端组、已断线 tab，以及 Hosts/New Tab 页面中的真实 tab。单纯点击仍切换 tab；关闭、广播与重连按钮保持原行为，虚拟 New Tab 页固定在真实 tab 之后。

拖动反馈细化：指针在原 tab 的左右两半内不显示插入线，任何松手后顺序不变的位置也不显示；只有实际改变顺序的目标才提示插入线。拖动时使用 `Grabbing` 光标（Windows 原生小手），松手、Esc 或取消时恢复普通光标。既有顶栏命中测试补充了原 tab 两半、末尾 tab 和有效跨 tab 目标的断言。

本轮反馈调整的 Windows GUI 62 项回归测试通过，Release 构建成功。最新本地包为 `releases/tab-feedback-20261012.zip`，包含之前的排序、SSH 和最大化修复；本轮未重复运行未受影响的 mux/SSH/广播/存储测试。日志 `/tmp/termviai-tab-feedback.log`，实际鼠标视觉仍待人工复测。

排序直接移动 Mux 窗口中的原 `Arc<Tab>`，按稳定 tab ID 查找来源和目标，并重新定位原选中 tab 的索引，不触发 SSH 重连、pane 移动或选中历史更新。离开顶栏后不再显示旧插入线；Esc、失去焦点或无效区域松手取消排序。原有“未选中单终端 tab 拖进当前终端组”的合并操作保留，整组 tab 只支持排序。

顶栏溢出时，可在按住拖动期间用滚轮滚动顶栏，再松手放置；最后一个可见 tab 的右侧会插入到下一个隐藏 tab 之前，而非直接跳到整个列表末尾。新增回归覆盖双向移动、选中与上一选中 tab 保留、原对象保留、目标/来源关闭、滚动顶栏插入锚点以及无效区域。

Windows x64 验证：mux 41、GUI 62、广播 12、存储 7、SSH 生命周期 2、最大化边界 3，共 127 项通过，Release 构建成功。本地包 `releases/tab-reorder-20261012.zip` 包含本轮及前两轮修复，仍为 beta.2 版本字符串。日志 `/tmp/termviai-tab-reorder.log`；实际鼠标拖动未自动验收，请覆盖多终端组左右移动、当前/非当前 tab、Esc 取消、溢出顶栏滚轮、按钮点击、单终端拖入组以及最大化窗口。

## 2026-10-12 最大化窗口 New Tab 顶栏越界

用户反馈右上角最大化后打开 New Tab，顶栏偶尔高出屏幕；任务栏最小化后恢复正常。New Tab 只切换库页面，没有主动扩展窗口高度，之前 `TabAddedToWindow` 将新 tab 网格适配当前窗口的保护仍保留。

本轮修改 Windows 自绘边框路径：`SetWindowTextW` 在主线程取得 HWND 后释放 `WindowInner` 借用，再调用同步窗口 API，避免重入回调因借用冲突放弃自绘客户区规则。`WM_NCCALCSIZE` 使用 `IsZoomed` 判断最大化，并将计算后的客户区限制在最近显示器的 `rcWork` 中，使自绘顶栏不落入最大化外框的屏外部分。普通窗口沿用原边框规则；Alt+Enter 全屏仍走已有的 `saved_placement` 分支。

证据边界：上述是源码中确认的风险路径和边界修复，尚未在真实 GUI 上复现这次偶发事件，不能认定已经确认其唯一原因。Windows API 行为参考 [SetWindowTextW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowtextw)、[IsZoomed](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iszoomed) 和 [WM_NCCALCSIZE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-nccalcsize)。

Windows x64 验证：新边界回归 3（超出工作区外框、负坐标副屏与各边缘任务栏及多种 DPI、普通窗口跨屏）、SSH 生命周期 2、mux 39、广播 12、GUI 61、存储 7，共 124 项通过，Release 构建成功。Windows CI 已加入新边界测试。

本地验证包：`releases/maximized-tabs-20261012.zip`，包括上一轮同主机整组重连修复，仍使用 beta.2 版本字符串，未发布新 GitHub Release。人工验收请覆盖：最大化后反复打开/关闭 New Tab、连接已有多终端组、切换 Hosts 与 Terminal、最小化恢复、还原后重新最大化、跨屏和 DPI 切换；同时观察窗口是否向下变长。日志为 `/tmp/termviai-maximized-tabs.log`。

## 2026-09-08 右侧抽屉更新

**Windows 原生 5 项测试通过，最终 GUI 编译和链接成功。旧程序正在运行，Cargo 最后替换 exe 失败；已将本轮链接产物和依赖复制到独立目录。原生拖动及最终视觉、点击仍需人工验收。**

## 本次修改

Add Host、Edit Host 和 Add Key 共用右侧抽屉，外部点击取消且不穿透，内部空白保持打开；独立渲染层修复原先背景文字叠在表单上的问题。小窗口支持滚动字段和键盘焦点跟随，底部固定保存按钮。顶栏空白与 TermViAI 标题在鼠标移动时设置 Windows caption 命中，同时保留已有窗口拖动回退路径。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| `hosts::tests` | 2 项通过：SSH 地址解析、主机库保存与损坏文件保护 |
| `termwindow::termviai_ui::tests` | 3 项通过：外部点击隔离/内部命中/顶栏命中、布局与焦点滚动、调色板与 Unicode 编辑 |
| Windows Release 编译和链接 | 成功生成新的 `target/release/deps/wezterm_gui.exe` |
| Cargo 最终复制 | 退出码 101；旧 `target/release/wezterm-gui.exe` 被运行进程占用，删除时报 os error 5，旧文件未替换 |
| 新版打包 | 从最终测试之后生成的链接产物复制，附带 5 个依赖文件，逐字节一致且 PE 架构均为 x64 |
| 源码核对 | 修改的 `termviai_ui.rs` 与 Windows 快照一致 |
| 格式检查 | rustfmt 和 `git diff --check` 通过 |
| 最终视觉、原生鼠标拖动、resize、IME、SSH | 未执行；单元测试不能替代原生事件验收 |

仍有上游生命周期和 OpenSSL 缺少 PDB 的警告。没有为本次改动关闭已有窗口或结束用户会话。

## 新版产物

[打开新版所在目录中的程序](/mnt/c/Users/h00893113/Documents/Codex/termviai-win/releases/drawer-20260908/wezterm-gui.exe)

Windows 路径：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\drawer-20260908\wezterm-gui.exe`。

大小：72,258,048 字节。SHA256：`2da18189e4daafdb301767edbe13fc97ad97e087afb2c1b53eff95bfefbb0e30`。

同目录包括 `conpty.dll`、`OpenConsole.exe`、`libEGL.dll`、`libGLESv2.dll`、`mesa/opengl32.dll`。这是独立验收目录，原 `src/target/release/wezterm-gui.exe` 仍是旧版本。停止旧程序后应打开上述新路径；后续常规构建可在旧 exe 不被占用时恢复覆盖。

## 验证限制与人工检查

Computer Use 对该应用返回 `product policy blocks this app`，没有绕过限制执行自动点击或截图。建议在新程序依次检查 Hosts 添加/编辑、Keychain 添加、外部空白收起、内部空白保留、小窗口滚轮与 Tab、普通及最大化窗口顶栏拖动、窗口按钮与 tab 点击。

主机数据格式和位置未改变，本轮未写入正式主机库。原有广播代码及未提交工作保留，没有提交或推送。

## 证据

- [最终构建日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/build-final.log)：编译链接完成，最后复制拒绝访问。
- [最终测试日志](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/tests-final.log)：5 项通过。
- [产物与源码清单](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/result.json)：路径、大小、SHA256、x64 架构。
- [修改前文件备份](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-drawer/before.tar.gz)。

首版 UI 的历史构建结果、原生窗口响应检查及旧产物记录保留在 [首版证据目录](/mnt/c/Users/h00893113/Documents/Codex/2026-09-08/outputs/termviai-ui/REPORT.md)。本轮不沿用旧产物的运行检查作为验收依据。
