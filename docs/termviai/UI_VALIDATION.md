# TermViAI UI 验证


## 2026-10-23 Ctrl + 单击打开终端链接

TermViAI 默认鼠标绑定中，普通左键、Shift 左键和 Alt+Shift 左键释放只完成文本选择，不再在空选区时打开链接；Ctrl 左键按下建立新选区，拖动扩展选区，释放后有选区则复制、无选区才打开链接。覆盖自动识别 URL 和 OSC 超链接。手形链接光标要求 Ctrl；原有显式自定义绑定、禁用默认绑定、非 TermViAI UI 及鼠标报告模式保留原语义。

Windows GUI 140 项测试通过，新增真实 InputMap 查找回归覆盖主/备用屏幕、各选择修饰键、Ctrl 按下/拖动/释放、鼠标报告模式。试用包为 `releases/ctrl-click-links-20261023.zip`，包含此前主题、线条及广播顺序修复。浏览器真实拉起仍需原生人工验收。

## 2026-10-23 广播成员按屏幕顺序排列

截图中的四路终端显示为 209 / 212（上排）、216 / 217（下排），底栏此前跟随分屏树逐列遍历，得到 209、216、212、217。绘制前按实际 `(top, left)` 坐标排序，底栏现在为 209、212、216、217；每帧使用最新布局，拖动重排后同步更新。标签与点击目标使用同一排序后的 pane 列表，保持成员身份；不修改 Mux 拓扑或广播路由顺序。

Windows GUI 原有 139 项回归测试通过。修正版试用包为 `releases/broadcast-order-20261023.zip`，包含上一轮线条与图标修正；真实原生拖动后的视觉仍待试用验收。

## 2026-10-23 主题试用版截图对齐修正

用户截图显示 pane 外框与 header 不对齐、横向分屏和广播条有多层线、Tab 三点过细。根因是 frame 已针对字符宽高补偿分隔区域，但 header 仍使用原始字符网格坐标；每个 pane 单独描边，叠加标题激活线与底栏额外分隔线。

header 改为锚定 frame，标题视觉高度不超过 35 px，保留独立的实际内容起点用于动画裁切；PTY 网格和输入坐标未改动。工作区只画一个整体圆角外框，内部共享 1 px 分隔线；标题使用 Terminal 图标。广播条与紧凑状态栏统一画布背景，取消重复横线和成员常驻底色。More 图标圆点直径从 2 增至 4 个矢量单位；Tab 菜单对齐标签左侧，使用卡片背景和细边框。

Windows GUI 139 项测试通过；新增回归覆盖 100% / 125% / 150% / 200% DPI、不同字格宽高下 header/frame 对齐、内容边界、四分屏单线和单终端无内部线。原生截图对照已人工读取；修改后的 Windows 窗口视觉仍需用户试用验收。

试用包 `releases/themes-chrome-alignment-20261023.zip`，构建日志和校验记录在同级 `verify-themes-chrome-alignment-20261023/`，保留 beta.5 版本字符串。

## 2026-10-23 三套主题与紧凑终端工作区

按照已确认的 terminal-space UX 接入 Ink / Navy / Light 三套语义配色，覆盖所有原生库页面、表单、菜单、确认界面与终端默认色/ANSI 16 色。Settings 加入主题缩略卡片：窗口内预览、保存到现有 settings.json、取消恢复；保存通知其他窗口，其他窗口未提交的预览保持独立。缺失/未知主题回退 Ink，不覆盖损坏文件，保留其他字段与显式终端调色板覆盖。

广播启动时默认收起为 34 px 状态栏；展开增加 52 px 成员条，Select all / Clear all、单成员开关和窄宽度翻页。无独立暂停开关，灰色成员被选中时只启用该成员，不恢复暂停前的其他目标。单终端显示连接摘要，不占用展开条高度。Tab 新增 ⋯ 菜单，提供重命名、保存工作区、按需重连离线成员、关闭；离线时保留广播与独立重连按钮。侧栏 176 px，pane 间距 2 px，激活项顶部细线和主题化嵌套悬浮；原有 PTY/render/mouse/IME 同步及远程布局兼容路径保留。

最终 Windows Release GUI **137 项测试通过**，Release 编译成功；nightly rustfmt、git diff --check 通过。新增验证覆盖主题持久化/兼容、取消预览、480–1080 高度 Settings 可达性、三主题文字对比度、OSC 与扩展色保留、单/组广播条高度以及关闭广播后的精确成员选择。保留此前 Recent connections、Keychain 编辑和删除布局修复。

试用包 `releases/themes-workspace-20261023.zip`，保留 beta.5 版本字符串，不修改公开 Release。构建日志、源码摘要和包摘要保存在同级 `verify-themes-workspace-20261023/`；包内附 THEMES.md。Windows 原生视觉、DPI、鼠标/IME 与真实 SSH 验收尚未执行，步骤见 [THEMES.md](THEMES.md)。

## 2026-10-23 Recent connections、New Tab 与 Keychain 编辑

Recent connections 删除 X 纳入父行的几何包含悬浮逻辑，仅实际所属行保持高亮。New Tab 与邻接加号拆为 NewTab / AddTab 两个 Action；点击入口行为相同，但按下反馈和释放匹配互不联动。Identity 卡片按钮取消自动居中图标，仅保留既有左侧图标。

Key 卡片按原始索引（先枚举后过滤）打开编辑页，可修改名称和路径，删除采用与 Identity 相同的二次点击确认和 Host 引用阻止策略。保存/删除重读文件，拒绝过期快照；删除不动真实私钥文件。Key 改名更新保存的 Host，旧名称保留为同一 Key 的别名供最近连接、保存工作区和其他窗口引用；历史名称不能被另一把 Key 占用。原有无别名字段的库正常加载。两个编辑页为删除按钮预留字段之外的区域，按钮底部距页脚分隔线 18 px，不再交叠。

Windows GUI 131 项测试通过（新增 3 项，扩展既有 tab 测试），含旧引用解析、更改路径、名字冲突、过期编辑/删除、引用阻止、私钥文件保留和 400–900 逻辑像素高度的布局。Release 构建成功，rustfmt 与 diff 检查通过，两份修改的 Rust 文件与 Windows 副本逐字节一致。独立试用包 `releases/keychain-interaction-20261023.zip` 的 ZIP CRC、逐文件哈希与 x64 PE 校验通过；版本仍为 beta.5，未更新已发布附件。构建证据保存在同级 `keychain-interaction-20261023-verification`。原生鼠标反馈、DPI 视觉和真实 SSH 仍待试用确认。

## 2026-10-23 tab 与卡片内部按钮悬浮

旧逻辑仅按单一 Action 相等高亮，进入子按钮就让父容器淡出；New Tab 页签和加号又共用 Action，造成跨控件联动。动画缓存改为 Action 加实际控件矩形，使用当前指针、有效顶层命中和几何包含关系计算父子悬浮。Tab 的关闭/广播/重连、New Tab 的关闭、Group 与 Host 卡片的菜单保留父容器高亮；其他同 ID 控件不继承。只有实际命中的图标高亮，嵌套菜单/广播/重连使用 #5a5c76 的独立悬浮面；关闭 X 保持透明底色和红色悬浮。点击分发、拖动及广播状态未改变。

新增 tab 子控件、新页签/加号隔离、容器外同动作和父悬浮不反向点亮子按钮回归；现有分组卡片测试补充菜单与容器关系。Windows GUI 128 项通过，Release 构建成功；格式/补丁检查、修改源码与 Windows 副本哈希一致。试用包 `releases/nested-hover-20261023.zip`。原生鼠标动画与 DPI 视觉仍待试用确认。

## 2026-10-23 首页 Add Host 不再误移除分组

All Hosts 复用了分组添加菜单，空目标 ID 在存储层表示未分组，导致 Add Existing Hosts 将选择的主机移出原分组。首页现直接打开 New Host，不显示下拉箭头；具体分组保留两种添加方式。选择器打开与保存均要求非空目标，防止过期或错误状态取消分组；底层 move_hosts_to_group 仍保留合法的取消分组能力。新增回归覆盖首页直接打开表单、空目标无法打开选择器、错误空目标保存不改变文件；原有有效目标及并发删除测试保留。Windows GUI 127 项通过，Release 构建及格式/补丁检查通过，源码与构建副本一致。试用包 `releases/home-add-host-20261023.zip`。既有误移动不自动猜测恢复。

## 2026-10-23 All Hosts 面包屑背景贴合

截图中的右侧留白过大来自固定字符宽度估算。改用 UI 实际字体 shaping advance 计算背景与点击宽度，左右各 6 px，文字起点、字号、其他面包屑位置均保持不变。纯视觉改动未新增测试；格式/补丁检查及 Windows Release 编译通过，修改源码与 Windows 副本哈希一致。试用包 `releases/breadcrumb-fit-20261023.zip`，实际视觉待试用确认。

## 2026-10-23 删除分组确认按钮焦点框

截图底部横线来自 draw_group_delete 的焦点下划线，不是边框裁切。改为与其他确认弹窗相同的完整 2 px 圆角焦点框，先绘制扩展底层再绘制原按钮，默认 Cancel，随键盘焦点移至 Delete Group。按钮命中区域、背景和确认逻辑不变。相关弹窗动画/命中回归通过（1 项，125 项过滤）；Windows Release 构建通过，源码副本、格式及补丁检查通过。试用包 `releases/group-focus-20261023.zip`；实际 DPI 视觉仍待试用确认。

## 2026-10-23 分组卡片、面包屑与菜单反馈修正

根据两张实际截图移除分组卡片右箭头，将省略号移到卡片右侧并保留独立命中区域。面包屑标签内边距由 12 px 缩为 6 px，All Hosts 按文字估算宽度排列，分隔箭头由 12 px 改为 18 px 并加深至 #b4b8cd；多层路径分页仍保留。Host 菜单 Edit / Duplicate 补充同款 18 px Edit / Copy 图标，三项统一图标文字对齐，Delete 保留危险操作红色。

所谓 Add Host 点击穿透来自 NewGroupHost 分支显式调用 navigate_group，并非底层命中区域泄漏。改为 open_group_host 捕获目标 ID/路径并打开表单，页面分组保持不变。回归覆盖 All Hosts 和其他分组作为入口、取消、保存后的原位置、主机实际归属及菜单隔离底层卡片。Windows GUI 126 项通过，格式与补丁检查通过，10 个已修改 Rust 文件与 Windows 构建副本逐字节一致。Windows Release 编译通过，试用包为 `releases/group-polish-20261023.zip`。原生鼠标/DPI 视觉效果仍待试用确认。


## 2026-10-23 分组管理操作（审核 UX）

实现审核稿的分组箭头、省略号菜单、当前分组菜单、Add Host 下拉、新建分组后进入和空分组添加入口。New Group 与 Add Existing Hosts 使用居中原生弹窗；New Host 复用现有认证编辑表单并明确显示目标分组。Save/Add Hosts 默认与 Cancel 同背景，悬浮低饱和紫色；删除确认默认中性背景、悬浮红色，X 默认灰色、悬浮红色。选择弹窗支持滚轮、方向键、Space/Enter、Tab 和 Escape，并隔离底层点击。

删除只移除容器，直属 Hosts/子分组提升一层，深层路径更新，旧文本分组不会重新出现；同名提升冲突和过期条目均拒绝保存。批量加入已有 Hosts 为移动而非复制，整批校验后原子保存，保留所有认证字段。新增主机对捕获的目标 group ID 校验，目标删除后不会通过旧路径重新创建。

Windows 原生 GUI 125 项测试通过，含 7 项新增回归与更新后的分组流程回归。源码与 Windows 构建副本逐字节核对，格式/补丁检查通过；试用包 `releases/group-actions-20261023.zip`。保留上次输出覆盖修复、最近连接移除与认证 UX 改动。原生鼠标视觉、DPI 与真实 SSH 网络验收仍需在目标机器试用。

## 2026-10-23 启动输出覆盖、最近连接移除与 Key 按钮

已在 Windows 原生测试复现截图中的 `SSH-2.0-OpenSSH_8.8connect...` 覆盖：`Screen::rewrap_lines` 把普通行首（x=0）误认为自动折行边界，将光标退回上一行。首次连接的布局宽度变化可与 CR/LF 分批到达交错，导致后续文字覆盖既有输出。修复仅让逻辑行内真正的折行边界回到上一行；普通行首保持原位置。新增核心回归覆盖变宽/变窄、CR 与 LF 之间/完整换行之后的调整；GUI 回归使用实际更新横幅，遍历启动文字每个字节边界与三种分块方式。该测试修复前失败、修复后通过。

New Tab 的 Recent connections 行右侧新增透明背景 X，复用应用关闭图标，默认灰色、悬浮红色。只移除选中的历史快照，不删除 Host、凭据、保存的工作区或当前连接；并发重连产生的新记录保留，旧窗口缓存保存不会恢复已移除条目。Add Key 使用密钥图标和与 Add Identity 相同的 CARD 背景。

Windows Release 终端核心 54、Mux 42、GUI 118 项测试通过，共 214 项；Release 编译通过。试用包为 `releases/startup-output-recents-20261023.zip`，未发布新的 beta。真实 SSH 网络和原生鼠标视觉仍需人工验收。

## 2026-10-22 嵌套 Host 分组与面包屑

按确认 UX 实现独立的嵌套分组：All Hosts 展示全部主机；分组卡片进入下一层；蓝色面包屑返回祖先，当前节点为普通文字。父分组与搜索包含全部后代主机。Groups 区只展示直属子分组；输入搜索时收起分组卡片。工具栏保留 Add Host，新增 New Group，并将搜索放到标题下方。Host 卡片补充地址，采用现有圆角、原生矢量图标及低饱和配色。深层面包屑溢出时提供翻页，全部分组与主机共用可滚动内容区。

New Group 使用紧凑卡片，Name 加只读 Parent group，创建于打开时捕获的层级；空分组持久化。同级名称忽略大小写判重，不同父分组可同名。Add Host 默认填写当前位置，保存后保留导航位置。旧文本分组加载为确定 ID 的一级分组，含斜杠的旧名字不自动拆层；Host 快照及认证保持兼容。详见 [HOST_GROUPS.md](HOST_GROUPS.md)。

Windows 原生 GUI **116 项通过**，Release 构建成功。新增 6 项回归覆盖层级汇总、相似名称隔离、旧数据/空组重载、坏图保护、当前层级建组和 Host 默认归属、并发重名及失效父级、子树搜索、长列表滚动、面包屑边界及新建卡片命中区域。测试后仅调整计数文案的单复数，最终程序已重新编译。nightly rustfmt 和 `git diff --check` 通过，6 个改动 Rust 源文件与 Windows 构建快照逐字节一致。

试用包：`releases/host-groups-20261022.zip`；SHA-256：`f62b023d724e0fdc9232c1f63a87573b63404a8c1f5ffd7e23a6c7432a243944`。5 个 PE 文件均为 AMD64，ZIP CRC 和包内逐文件摘要校验通过。测试、最终构建与源码清单位于 `releases/verify-host-groups-20261022/`。保留 beta.4 版本字符串，未发布新 Beta。真实鼠标、DPI 与最终视觉仍待人工验收；包内包含上一轮认证与 Host 删除修复。

## 2026-10-22 Host 删除入口

Host 三点菜单新增红色 Delete Host，与 Edit / Duplicate 之间使用细分隔线。删除复用现有原生确认卡片、动画和输入隔离，显示主机名及地址、垃圾桶图标与 Delete Host 按钮，默认焦点为 Cancel；Esc、外部点击均取消。确认时重新读取主机库，以完整条目匹配被确认的主机，防止排序、并发新增或过期编辑导致误删；写入成功才刷新内存列表。

删除仅移除 Hosts 条目，不断开现有连接，不删除 Keychain、保存的终端组或最近连接；这些对象及复制的 Host 可能仍引用同一密码，因此保留凭据引用。嵌套分组导航仍处于 UX 审核阶段，没有加入本次应用代码。

Windows 完整 GUI **110 项通过**，Release 构建成功，nightly rustfmt 与 `git diff --check` 通过。新增回归覆盖取消/仅确认一次、目标捕获、并发新增和重排、共享引用保留、重复删除/过期条目拒绝以及损坏文件保护。6 个修改 Rust 文件与 Windows 快照一致。试用包 `releases/host-delete-20261022.zip`，SHA-256 `b693f7d4c01e062216b1562607c3c9b6cd6e3626e79772b17f6f9359912ba823`；ZIP CRC、逐文件摘要和 5 个 AMD64 PE 检查通过。日志及清单位于 `releases/verify-host-delete-20261022/`。真实原生鼠标和视觉验收尚未执行。

## 2026-10-22 Host 互斥认证设计落地

按审核后的 UX 改为 Password / SSH Key / Identity 三个互斥模式，默认 Password；Key 内通过 File path / Keychain 单选切换。Address / Port 同行，Group 位于认证下方，Identity 使用只读用户名及密码摘要。取消旧的可同时选择的折叠列表。模式切换仅在表单内保留草稿，保存只写入当前模式和来源。Save 与 Cancel 默认同一中性背景，Save 悬浮变低饱和紫色。列表方向键选择，Enter / Tab / Esc 收起列表，不提交后方表单。

显式认证方式同时约束 libssh / ssh2：Password、Identity 不尝试 agent/公钥；Key 只尝试选定的私钥，含空格路径保留完整，完成密钥认证因子后仍允许交互式后续挑战。旧配置未编辑时保留既有 SSH 行为；编辑时推断模式，保存后使用明确的互斥模式。连接缓存区分模式，工作区和最近连接保留原始引用。

Windows 原生 **GUI 108 项、SSH 19 项，共 127 项通过**；Release 编译成功。覆盖模式草稿、焦点、仅保存当前来源、旧配置与密码迁移、Identity 共享密码保护、失效选择、缓存区分、480/720/1080 高度滚动及下拉边界。nightly rustfmt 与 `git diff --check` 通过。5 个修改 Rust 文件与 Windows 构建输入逐字节一致。

验证包：`releases/host-auth-exclusive-20261022.zip`；SHA-256：`0ab1639b059ec07eed6cc4007c9f834d1f2ff521630546943bd2b7868e129b0d`。ZIP CRC、包内逐文件摘要及 5 个 PE 的 AMD64 架构检查通过，CLI 版本仍为 `0.1.0-beta.4`。证据目录为 `releases/verify-host-auth-exclusive-20261022/`。这是独立本地验证包；真实 GUI 视觉、鼠标、DPI 与真实 SSH 密码/密钥/MFA 登录尚未验收。历史折叠列表方案及其验证记录保留在下节，当前实现已由本节替代。

## 2026-10-22 Host 认证折叠列表与密码保存

移除 Remember password 勾选框、独立认证选择页和 Manual credentials。输入的 Host 密码随保存写入 Windows 凭据管理器，未修改时固定显示 `••••••••`；占位符不写入输入值，Show/Hide 仅作用于新输入。Keys / Identities 在字段下方分别折叠，选中项使用低饱和底色、勾号和标题摘要，再次点击取消选择。空列表与失效引用有对应提示。沿用现有矢量图标、圆角、配色和抽屉动画。

上方用户名、密码与密钥路径不随列表回填、清空或锁定。连接时所选 Identity 覆盖用户名/密码，所选 Key 覆盖路径；手填值独立保留。工作区与最近连接保存原始配置，避免将连接时解析出的凭据回写为手填值。

Windows 原生完整 GUI 测试 **105 项通过**，Release 编译、nightly rustfmt 和 `git diff --check` 通过。覆盖默认持久保存、未改保留、替换、旧临时密码迁移/过期、Identity 选中时保留手填密码及取消选择、选择优先级、旧格式兼容，以及 480/720/1080 高度下长列表全部项目可滚动到且不覆盖页脚。3 个改动 Rust 文件与最终 Windows 构建输入逐字节一致。

验证包：`releases/host-auth-accordion-20261022.zip`；SHA-256：`32208fc62d46209c5d6d3c2250dc32bf1208f02be22b01938774edce9605abb8`。包内 5 个 PE 文件为 AMD64，ZIP CRC 与逐文件摘要校验通过。构建证据保存于 `releases/verify-host-auth-accordion-20261022/`。沿用 beta.4 版本字符串，属于独立本地验证包；真实 GUI 视觉、鼠标和 SSH 登录仍待人工验收。

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

## 2026-10-23 beta.6 发布验证

Windows 原生 SSH 19、window 5、终端核心 54、mux 42、广播 12、GUI 140，共 272 项通过，Release 编译成功。CLI/PE 版本为 beta.6，数值版本 0.1.0.6，10 个嵌入图标与源文件一致；557 个构建输入核对通过。归档内容、校验值、审查范围与人工验收边界见 [beta.6 验证记录](RELEASE_V0.1.0_BETA.6_VALIDATION.md)。历史试用记录保留，不代表正式版本字符串。
