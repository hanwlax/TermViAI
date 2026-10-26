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

## 2026-10-23 输入框文字选区对比度修复

截图与代码确认文字选区复用主题 soft 底色，且输入框通用悬浮/按下叠色降低其辨识度。现在 Ink/Navy/Light 各有独立 input_selection 配色；聚焦输入框在悬浮和拖动时保留原底色。统一输入绘制路径覆盖表单与搜索，保留原有选区、光标与密码掩码行为。

扩展现有主题对比度回归：三套主题的选中文字/选区底色至少 4.5:1、选区/输入框至少 3:1。Windows 完整 GUI 140 项测试与 Release 编译通过；nightly rustfmt、git diff --check、两处 Rust 源码与构建副本一致性以及 ZIP CRC/逐文件 SHA-256 通过。构建日志 `termviai-input-selection.log`。未执行原生鼠标/DPI 实测，需在试用包确认悬浮、拖动、Ctrl+A 和密码框的实际效果。

独立试用包 `TermViAI-input-selection-20261023.zip`；仍使用 beta.6 版本字符串，未替换 GitHub 发布附件。ZIP SHA-256：`511663d177cd0a0fba682191303a728431f5b9331b9a1aeda0a8b805acaa4a7e`；EXE SHA-256：`f3b8ba214119198a7e5eaa9614e583b5f3398ad744fd29b5bcb052257dc76594`。

## 2026-10-23 组内终端四向停靠重排

组内标题栏拖动不再调用固定位置互换，改为复用单终端 Tab 拖入的 DockSide 命中、半区预览、preflight 与 Mux MovePane 路径。命中和预览统一使用当前显示的 pane frame；松手按左/右/上/下方向移入目标分屏，原位置自动合并。自身/有效区域外/切换组/已关闭目标不会移动；Esc 取消；单 pane 放大时需恢复组视图。LocalPane 与已连接 Domain 的安全限制保持，远程管理的布局不会被直接修改。

新增 mux 回归覆盖四终端所有不同来源/目标的四方向移动（48 组）、连续嵌套重排、原 pane 对象与独立字号保留、来源/目标关闭和自身落点。已有同组/跨组 resize 失败回滚、字体/标题留白、广播和 Tab 拖动回归保留。Windows mux 44 项、GUI 140 项共 184 项通过，Release 构建成功；nightly rustfmt、git diff --check、4 个修改过的 Rust 文件与 Windows 副本比对、ZIP CRC 和逐文件 SHA-256 通过。日志 `termviai-pane-docking.log`。真实鼠标拖动、SSH 会话和不同 DPI 仍需原生验收。

试用包 `TermViAI-pane-docking-20261023.zip` 包含上一轮输入选区修复，版本字符串仍为 beta.6，未替换公开发布附件。ZIP SHA-256：`06113d45c8ee6bf090389677351a6c72f5741a2bb099e92260e395eb33c650fa`；EXE SHA-256：`15e99ed96b0f57718510b943b965cff58bd36fb4cdc97f2309aaeac8c74f2a72`。

## 2026-10-24 NPU Monitor 试用

按已确认的紧凑监控 UX 新增原生 Monitor 页面、已有 Hosts 多选、独立 SSH 后台采样、主机汇总及 NPU/Die 卡片。整张 NPU 卡片统一悬浮预览和点击，内部指标没有重复入口；右侧进程抽屉提供 Die 筛选与确认后的单个/范围进程结束。复用三套主题、认证和 UI 样式。详见 [MONITOR.md](MONITOR.md)。

Windows 完整 GUI 149 项、Python 采集器 9 项测试通过，Release 编译成功。覆盖整卡单一命中、三种窗口尺寸下抽屉点击边界、去重、PID 重用、权限与排队操作取消。源码格式化、git diff --check、14 个变动构建输入与 Windows 副本哈希一致、ZIP CRC/逐文件 SHA-256 校验通过。打包 EXE 的 `--version` 成功，保留 `TermViAI 0.1.0-beta.6`。链接阶段存在依赖缺少调试 PDB 的既有警告，不影响 Release 构建成功。

试用包：`/mnt/c/Users/h00893113/Documents/Codex/termviai-win/releases/TermViAI-monitor-20261024-windows-x86_64.zip`（30,040,271 字节）。ZIP SHA-256：`5ba6b5376022be26eb324732c730bcde30431d945ecd1211ad06bb0ec06f86b3`；EXE SHA-256：`744939de4d1e65831f50557f4e507f6a3fa9de0b36f8a0c998cd190002c7e02b`。相同 releases 目录保留 `termviai-monitor-20261024-build.log`、`monitor-20261024-validation.json` 和 ZIP 校验文件。包含此前未发布的输入框选区与组内四向停靠改动。

未连接真实 SSH/NPU 主机，未结束任何真实进程；原生鼠标、DPI、远端设备表格版本与长时间运行仍待用户试用验证。本轮没有提交、推送或替换公开 Release 附件。

## 2026-10-24 Monitor 首次采样失败与表格兼容修复

用户反馈全部 Unavailable，详情 Remote collector failed。确认监控代码在 exec 启动后 drop stdin，而共享 SSH 桥把本地输入 EOF 处理为关闭整个通道；修复为保留 stdin 到输出及退出状态读完。状态文案改为等待首个样本/尚无成功样本，失败时保留退出码和远端 stderr，不再误报 Waiting for SSH。

参考用户提供的 npus-usage 项目，新增其 25.5.1 Chip/Phy-Id 与 25.6 单 Die 表格样例。旧解析器在两个样例上均复现 Unsupported device table，修复后 11 项采集测试通过；保留非连续编号、未知 HBM、进程身份与权限保护。Windows GUI 151 项通过，其中新增 socketpair 回归验证命令完成前 stdin 仍打开，以及 stderr/退出码保留。Release 编译、EXE --version、16 个源码/样例构建输入比对、ZIP CRC/逐文件 SHA-256、git diff --check 通过。

独立包 `TermViAI-monitor-fix-20261024-windows-x86_64.zip`，30,038,371 字节，位于此前相同 releases 目录。ZIP SHA-256：`ccd9ff515cec0f75166134ff5011467d4858ec95aee618c0908802e2639780f0`；EXE SHA-256：`4a716c64ae2581ee153fd394c9fd748a7b1152d4d4d9d9bae04864381fae9d69`。日志 `termviai-monitor-fix-20261024-build.log`，清单 `monitor-fix-20261024-validation.json`。保留 beta.6 版本字符串及旧试用包；没有连接真实远端或运行真实进程控制，需要用户更新后验证实际采集。

## 2026-10-24 Monitor UX 对照修正

用户提供的 UX/实现截图确认字号层级、主机图标、统计与设备信息、面包屑、错误空状态及操作入口存在偏差。按照既有 HTML 设计稿校正 Monitor 页面：独立 UI 字体尺寸和字重、卡片边框/指标布局、状态计数、采样间隔下拉框、错误时的重试与禁用操作、居中空状态及原因抽屉。卡片内 Retry/Resume 保留父卡高亮并有独立悬浮底色；设备拓扑未知时不填示例数值。详细规则见 MONITOR.md。

Windows 完整 GUI 154 项通过，Release 编译成功。新增对照回归覆盖 720×480、1024×768、1488×814 三种尺寸的概览/错误详情、字体层级、操作禁用、父子悬浮、间隔选择与原因抽屉返回；实际加载字体并验证缓存、DPI 变化及终端字号独立性。17 个构建输入与 Windows 副本一致；EXE --version、ZIP CRC/逐文件 SHA-256、git diff --check 通过。后台与采集逻辑沿用已验证的修复，11 项 Python 测试本轮未变更。

试用包 `TermViAI-monitor-ux-20261024-windows-x86_64.zip`，30,060,014 字节，位于 Windows 构建目录的 releases 中。ZIP SHA-256：`df0f6bebbfe1a9b4653d0c3337bac36c8c9378b912ac05a6605aa1801d99497a`；EXE SHA-256：`9cb244f153ee303ab8e50b081af340e36d12db554abf9b3bfbd9c9ee58006bc4`。日志 `termviai-monitor-ux-20261024-build.log`，清单 `monitor-ux-20261024-validation.json`，布局坐标 `monitor-ux-20261024-layout.json`。

离线 GDI 排版图来自原生绘制函数生成的 Tile 坐标，图标使用名称占位，仅用于排版核对，未冒充应用截图。真实 Windows GPU/鼠标/DPI 外观尚需用户试用确认；本轮未连接远端或执行真实进程操作。保留 beta.6 版本字符串，没有提交、推送或发布新 Beta。

## 2026-10-24 Monitor 进程工作目录

- Python 采集器 14 项测试通过；新增同 PID 跨 Die 复用、路径空格/Unicode/已删除目录后缀、权限/目录消失不影响其他元数据、PID 重用不发布路径。未连接真实主机或发送真实信号。
- Windows Release GUI 155 项测试通过；新增详情工作目录、Unavailable、长路径完整换行且不与启动命令重叠的原生布局验证。
- 实际远端工作目录及原生窗口视觉效果仍待用户试用验收。
- Windows Release 编译、EXE `--version`、17 个修改输入一致性及 ZIP 完整性/文件 SHA256 检查通过。试用包：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\TermViAI-monitor-cwd-20261024-windows-x86_64.zip`；ZIP SHA256：`eca9310b54b6dc9a6688dc4b7a63f4ba438487d6cc1c6cb4584adbb9030d3e1c`。版本号维持 beta.6，非正式新发布。

## 2026-10-24 NPU 悬浮预览与设计稿对齐

- 对照用户最新两张截图以及原稿 `termviai-monitor.html` 的 mn-peek 样式，确认原生实现错误使用了通用文字 tooltip。改为分层进程预览卡片、220 ms 延迟、右对齐指标及一致的边框/颜色。
- Windows GUI 159 项测试通过；新增覆盖：2 个跨 Die 进程的去重与当前卡显存合计；空/未知/暂停状态与模态遮挡；底部翻转、小窗口边界及超量提示；两种宽度的列数及指标对齐。Python 采集器 14 项测试通过。
- Windows Release 编译成功。原生布局坐标导出并使用 GDI 离线审查两列/四列布局、预览行间距与数值列；这不是原生运行截图，离线图使用矩形/图标名称占位，实际圆角、阴影、glyph 裁切及鼠标交互仍待试用验收。
- 新试用包为 `TermViAI-monitor-preview-20261024-windows-x86_64.zip`，版本号仍为 beta.6；保留此前工作目录、SSH 采样和解析修复。未连接真实主机或执行进程终止。

## 2026-10-24 文字与结束按钮悬浮优化

- Windows GUI 161 项测试通过，Release 编译成功。新增 smoothstep 动画首尾缓入缓出/反向连续性和重复目标不重启动画验证；glyph 原点测试覆盖 100%/125%/150%/200% DPI 与奇偶窗口尺寸；详情布局测试确认正文 13 px、名称较重字重且长路径不与命令重叠。
- 仅监控显式 UI 字体选择像素对齐入口，默认渲染入口保留原有浮点坐标。结束操作背景降低为淡红染色，文字/边框同步渐入，保持确认流程。
- 试用包 `TermViAI-monitor-polish-20261024-windows-x86_64.zip`。真实鼠标动画和文字清晰度尚未进行原生屏幕验收；未执行任何远端进程结束操作。采集器未更改，沿用此前 14 项测试结果。

## 2026-10-24 Monitor 进程抽屉滚动

- Windows GUI 164 项测试通过。新增长命令/三进程抽屉用例覆盖 720×480 与 1417×872 窗口：连续重绘和样本变化保留位置，最末命令及说明均可到达，操作命中不会溢出可见内容区。
- 背景 NPU 网格滚动与详情抽屉相互独立，关闭抽屉恢复原页位置；滚动条拖动到边界、松开、轨道点击、切换筛选清理拖动状态均通过回归。
- 试用包 `TermViAI-monitor-scroll-20261024-windows-x86_64.zip`；未连接远程主机或终止进程。实际鼠标操作仍需试用验收。
- Windows Release 编译通过，试用包压缩完整性及 19 个修改输入一致性检查通过；EXE 版本号维持 beta.6。

## 2026-10-24 Monitor 连续像素滚动

参考 Windows Scroll viewer 的鼠标/滑块交互以及可视区/内容比例模型，替换抽屉的整档滚动。实现连续像素位置、临界阻尼目标跟踪、直接拖动、比例滑块、悬浮加宽和固定标题/页脚遮罩。

测试覆盖 30/60/144 fps 下无超调收敛与静止后停止额外动画，连续输入积累、反向输入、拖动中止动画、内容缩短钳制；比例滑块与小数位置跟手；边缘部分文字保留在裁切层下而非整行移除；沿用长命令、多进程、背景页位置和命中边界回归。渲染动画接入正常帧调度，采集数据刷新仍保持独立周期。

试用包名 `TermViAI-monitor-smooth-20261024-windows-x86_64.zip`。本次测试不连接真实主机或结束进程；原生 GPU 裁切、实际鼠标/触控板手感仍需用户试用，不将时间模型测试当作实机流畅度测量。
- 最终源码 Windows GUI 169 项测试通过，包括不足 1 px 溢出时滑块末端对齐。
- Windows Release 编译、版本启动、压缩包完整性及 19 个修改输入一致性检查通过；版本号仍为 beta.6。

## 2026-10-24 确认页层次与统一 UI 字体

- Windows GUI 170 项测试通过。字体测试检查真实解析出的 Segoe UI 字体、400/600 字重及未使用模拟加粗，并沿用 DPI/缓存/终端缩放隔离验证。
- 确认页测试覆盖 720×480 与 1417×872：名称与 PID/用户对齐且互不重叠，存在分隔线与范围摘要；单 PID 跨 3 个 Die 的影响被完整统计；刷新清空实时样本后，已捕获的确认摘要不变化。没有向真实主机发送操作。
- 新试用包为 `TermViAI-monitor-typography-20261024-windows-x86_64.zip`，保留连续滚动与危险操作渐入。字体测试不等同于原生视觉验收；不同 DPI、三种主题、浏览器/原生渲染差异仍需用户屏幕确认。
- Windows Release 编译、EXE `--version` 启动、ZIP CRC/逐文件校验与 19 个修改构建输入一致性检查通过。ZIP SHA-256：`58b01bd9561638d5c5d9662f74b04aeb8b2e76a5a364dd980ab4ab13e58e19f2`。版本字符串仍为 beta.6。

## 2026-10-24 Monitor 滚动越界与窗口标题栏操作

- Windows GUI 172 项测试通过。新增回归覆盖 1725×1091 → 1725×677 → 720×480 的窗口变化，以及 0 / 531.25 / 1200.5 px 滚动位置；长卡片背景必须始终位于 y≥52 的抽屉表面，连续位置不变，固定遮罩仍存在。
- 对 Processes / Stop / Picker / Reason / Remove 五种抽屉检查拖动、最小化、最大化/还原、关闭命中均保留，遮罩点击不会触发下方主机连接。原有部分文本裁切与滚动连续性回归继续通过。
- 自动测试验证布局/事件路由，不代替 Windows 原生鼠标与渲染验收；未连接 SSH 或执行进程终止。
- Windows Release 编译、EXE 版本启动、ZIP CRC/逐文件哈希和 19 个修改构建输入一致性检查通过。试用包 `TermViAI-monitor-window-20261024-windows-x86_64.zip`，仍使用 beta.6 版本号。ZIP SHA-256：`5d0caf25a1b8879f355e06d06ebfae881460ff813ad2f9ddf415a774b66aa892`。

## 2026-10-24 Windows UI 文字栅格化

- 用户确认截图为 100% 缩放；因此高 DPI 默认禁用 hinting 不是本次的直接原因。截图边缘像素符合灰度混合，不能据此声称对照应用启用了 ClearType。发现默认字号 UI 没有进入既有物理像素对齐路径，现统一启用。
- 默认 Windows UI 使用 DirectWrite 灰度分析/网格对齐，保留现有 shaping advance。172 项 GUI 回归通过；新增 2 项字体测试覆盖 96/120/144/192 DPI、400/600 字重的真实字形，确认抗锯齿灰阶、纹理尺寸、原生输出区别于回退输出，终端与普通标题字体不启用 UI 原生路径；强制无效 glyph 与显式 FreeType 标记均返回配置的回退。
- 对照图 `ui-raster-comparison.png` 每两行为同字号/字重：上 FreeType，下 DirectWrite。它由真实 glyph mask 和离线合成生成，不是原生应用截图，没有替代整页 GPU/鼠标验收。
- 应在三种主题实际检查 Hosts、Keychain、主机编号、输入框和监控小字。保留终端等宽字体/配置、显式 frame 字体覆盖、颜色/位图/变量实例和嵌入字体的原有回退；不对整体截图做锐化滤镜。
- 最终字体回退测试与 Windows Release 重建通过，EXE `--version` 启动成功（仍为 beta.6）；ZIP CRC/逐文件哈希及 21 个修改构建输入一致性检查通过。试用包 `TermViAI-ui-text-20261024-windows-x86_64.zip`，SHA-256：`2fb2cb68b3fbf1c5102cc5851d5337ec1ea15b56d466c39cb1e83ae46629ac74`。

## 2026-10-24 Monitor 单词断行

- 原因是固定字符数分块（确认说明按 512 / 8 = 64 个字符），不是英文单词连字。所有监控抽屉的多行说明、命令、工作目录和错误文字改用实际渲染字体测宽；优先 Unicode 换行机会，超长 token 按 grapheme 断行，保留显式段落。
- 新回归使用实际 UI 字体在 96/120/144 DPI 检查截图原句、普通单词完整性与行宽；对照 W / i 比例字宽，检查组合字符、超长路径、空段和极窄宽度。已有抽屉卡片、窗口缩小、连续滚动、标题栏命中等回归使用真实字体测量生成布局。
- 字体/布局回归不是原生屏幕验收，未连接 SSH 或执行进程终止。
- Windows GUI 173 项测试全部通过；Release 编译、EXE 版本启动、ZIP CRC/逐文件哈希及 21 个修改构建输入一致性检查通过。试用包 `TermViAI-ui-wrap-20261024-windows-x86_64.zip`（版本仍为 beta.6），SHA-256：`3616e5236e5c80a172177fc8ff542de21d9bfb44f08b2a339b7070aa26e9f46b`。

## 2026-10-24 Windows 缩放调度与 UI 文字缓存

- GUI 174 项回归通过，另有 1 项手动 CPU 基准通过；窗口 9 项测试通过，包含帧预算扣除工作耗时、合并待处理请求、最终摘要采样有界和真实 CSV 后台写出。
- 最终基准（120 次采样，预热 20 次）：NPU 列表无缓存 P50/P95 0.257/0.470 ms，有缓存 0.024/0.025 ms；长命令抽屉无缓存 0.685/1.166 ms，有缓存 0.074/0.115 ms。两组 glyph 总数分别一致。这是 Monitor 布局和文字 shaping 的 CPU 测试，不包含完整 box model 或 GPU/呈现，不是应用 FPS。
- Windows live resize 合并尺寸更新；更新区域在限帧时也完成验证；计时按帧起始时间，独立模态唤醒避免依赖新的鼠标消息。退出拖动补齐最终通知。现有最大化边界、负坐标屏幕工作区与普通窗口框架回归继续通过。
- computer-use 技能禁止自动操作终端应用，所以没有自动拖动真实 TermViAI。试用包提供 Resize-Diagnostics.cmd，释放鼠标后将阶段统计追加至 resize-trace.csv；不采集终端/主机/认证内容。详见 [RESIZE_PERFORMANCE.md](RESIZE_PERFORMANCE.md)。
- 最终 Windows Release 构建与打包 EXE 版本启动通过；ZIP CRC/逐文件哈希及 32 个修改输入与 Windows 构建副本一致。包名 `TermViAI-resize-20261024-windows-x86_64.zip`，版本仍为 beta.6；SHA-256：`5b008a500b86a4a8c6f362116d2f29e485c13872768c1fb8b13cfc007c142505`。

## 2026-10-24 广播输入接收终端自动跟随

- 复核截图为 4/4 广播：原来的键盘/粘贴调用者只重置源终端 viewport。统一发送入口现在只对非空输入的成功接收成员执行已有 scroll-on-input 逻辑，保留复制/搜索 overlay、用户关闭自动跟随的配置和源终端原处理。
- 既有广播路由 11 项、Windows shell routing 1 项和 Windows GUI 174 项回归全部通过。路由回归涵盖失败隔离、离线跳过、成员选择、空输入等；这些不是新行为的真实 SSH 窗口验收。
- 手工验收：开启组广播，将 216 滚动到历史输出，在 209 输入或粘贴，216 应立即回到底部并跟随输出；取消 216 的广播选择后重复，216 应保留历史位置。仅有远端后台输出时也应保持历史位置。
- Windows Release 构建、33 个修改输入与构建副本一致性、ZIP CRC/逐文件 SHA-256 均通过。试用包 `TermViAI-broadcast-scroll-20261024-windows-x86_64.zip`，版本仍为 beta.6，SHA-256：`f8933b071cce39bbdea46628a8d7bec8e88b0d4ff7cf7201cde37be20bb72e6a`。

## 2026-10-24 Monitor 进程按钮禁用状态

- 详情的批量与单进程操作在禁用分支原来使用普通 text，导致边框消失、无点击区域却看不出原因。现在与主机级按钮共用完整的禁用外观，保持圆角、边框和内边距，不增加可执行命中区域。
- 有进程但不可操作时在列表上方说明原因；空范围显示无可结束进程。保留 controls_enabled 现有条件，不因进程存在就放开结束操作。
- Windows GUI 175 项测试通过；新增真实布局回归检查正常、空范围、暂停、过期、身份缺失、权限、平台限制、采集失败、操作进行中的外观、提示和点击区域，并验证混合权限时可操作的单 PID 仍可点击。未连接远端或结束真实进程。
- Windows Release、打包 EXE 版本启动、33 个修改构建输入校验、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-monitor-controls-20261024-windows-x86_64.zip`（beta.6），SHA-256：`42aebc803f9da7dedf7f0fedd734bcfb34169a8ac6a82824c904257ec6ca3a2d`。

## 2026-10-26 缺失 Python pidfd 绑定的主机兼容

- 根据 216 提供的 Python 3.13.12 输出（pidfd_open=False、pidfd_send_signal=True），将采集与终止的能力判断改为共享适配器；保留原生路径，缺失时使用 libc/已核对 ABI 的 syscall。
- Linux Python 22 项测试通过：原生优先、缺少 open / 两个绑定、真实 libc 和 raw syscall 自身 pidfd/信号 0、模拟 x86_64/AArch64 调用参数、未知 ABI 拒绝、内核 ENOSYS/策略拒绝、关闭描述符，以及原有过期 PID/非 NPU 成员/权限/容器检查。真实探测只作用于测试程序自身，不发送终止信号。
- Windows GUI 175 项测试通过。没有连接 216 或终止远程进程；216 实际内核能力和原生窗口效果留待试用验收。
- Windows Release、EXE 版本启动、33 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-pidfd-compat-20261026-windows-x86_64.zip`（beta.6），SHA-256：`090b5bc91bc575b61008692109f1834888381b57ffb9f591308e5e5996d5cf7f`。

## 2026-10-26 通用按钮左右留白

- 根因：通用 button 使用固定宽度与左侧内边距，剩余宽度全部留在右侧。新增操作按钮整体内容居中，按照当前字体 shaping advance 计算图标+间距+标签总宽，复用已有 UI shaping 缓存，并保留独立的物理像素裁切余量。
- 常规表单、添加、广播和 Monitor 操作按钮采用整体居中；禁用 Monitor 按钮保持同一布局。侧栏、Tab、输入框、卡片、认证选择列表及菜单行保留前缘对齐。Monitor 面包屑用真实字号测宽，保持文字原点，背景与 hit 一起收紧。
- Windows GUI 178 项测试通过。新增 96/120/144 DPI 实际字体对照，检查中英文、图文组合、长标签的留白/裁切/点击区域；另验证导航排除规则及面包屑文字原点和命中尺寸。原生屏幕外观仍需试用验收。
- Windows Release、EXE 版本启动、33 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-button-alignment-20261026-windows-x86_64.zip`（beta.6），SHA-256：`f2d19671782e7d08e5e2f323fd954b49b4adf2ff97582d17254bb55e84025e18`。

## 2026-10-26 组内拖动的完整布局预览

- 旧逻辑仅高亮目标原矩形的一半，忽略源叶移除后兄弟节点的扩展。新增 mux 克隆树预览，共用现有 remove/split 计算，抑制 PTY resize；不更改真实树、活动终端、recency 或广播成员。
- 拖动超过阈值后以不透明布局示意（保留主机标题）覆盖旧终端画面；源位置不再保留，显示补位后的各终端边界及最终插入区域。命中测试基于移除源后的稳定几何；实际终端内容与尺寸直到释放才变化。取消或无有效落点时原布局保持不变。
- Windows mux 46 项和 GUI 179 项测试通过。新增 96 组预览/实际提交对照（两种缩放布局模式 × 四个源 × 三个目标 × 四个方向），逐项核对坐标/尺寸，验证预览不增加 PTY resize、不改变活动 pane，并保留对象身份；另覆盖无效目标及 GUI 对旧位置的全覆盖。真实原生鼠标拖动仍待试用验收。
- Windows Release、EXE 版本启动、34 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-dock-preview-20261026-windows-x86_64.zip`（beta.6），SHA-256：`2a421bbe5177e24f1160c9ecef5edc5375ceea066e684f876aa284a702496ba3`。

## 2026-10-26 结束进程确认中的工作目录

- 在既有名称/PID/用户/卡与 Die 信息下增加 Working directory，使用现有主题正文色与 13 px 字号，不增加额外 SSH 查询。目录随确认目标冻结；不改变结束进程条件和执行流程。
- Windows GUI 180 项测试通过。确认摘要回归验证刷新后仍显示原目录；新增主机与卡范围的长中文路径换行、未获取目录、窄窗口滚动和确认按钮命中测试。未连接远端或结束真实进程，原生屏幕外观仍待试用验收。
- Windows Release、EXE 版本启动、34 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-stop-cwd-20261026-windows-x86_64.zip`（beta.6），SHA-256：`b1b13ab68d51ff2d3ceff76e2bd213841d905a422af05ef3fd62743ebec21613`。

## 2026-10-26 标签关闭确认设置与首页记录删除

- Settings 新增 Confirm before closing tabs，默认开启；修改随 Save 持久化并同步同进程其他窗口，Cancel 不应用。兼容旧 settings.json；键盘 Tab 可访问开关，Space/Enter 切换，小窗口设置列表可滚动到达。
- Saved workspaces 新增删除 ×，仅删除模板，保留活动终端和 Recent connections；移除活动标签的模板关联，防止后续重命名操作引用失效模板。Recent connections 与 Saved workspaces 仅悬浮行内显示 ×，保留稳定的尾部留白、红色悬浮及父行高亮，点击 × 不触发打开。
- Windows GUI 182 项测试通过。覆盖设置默认值、开关持久化及旧配置兼容、窄窗口设置项可达、删除模板并发变更拒绝、旧缓存合并不复活删除项、其他记录保留，以及两类行在未悬浮/悬浮行/悬浮 ×/悬浮其他行时的图标可见性与点击区域。未操作真实连接；原生鼠标和屏幕外观留待试用验收。
- Windows Release、EXE 版本启动、37 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-workspace-controls-20261026-windows-x86_64.zip`（beta.6），SHA-256：`60cb950c4b9a9a3643eb2be211445977cc2b69bc6d08c074bc5a570a7272b598`。

## 2026-10-26 Recent connections 的 Identity 去重

- 已只读核对用户本地记录：209 的一条历史直接保存 root，另一条选择 Identity、原始 username 为空，但该 Identity 的实际 username 为 root。旧 same_host 直接比较原始 username，因此出现相同地址的两行。
- 最近连接去重改用规范化地址、端口、实际账号和密钥路径；读取 hosts.json 的元数据，不读取密码或私钥、不访问 SSH。保留最近一条的原始 Host/认证引用；加载和写入均先去重再截断。缺失引用与未知账号分开保留；不同账号/端口/密钥不合并，Saved workspaces 不变。
- Windows GUI 185 项测试通过。新增旧重复记录加载迁移、手填/Identity 反复连接、旧缓存合并与删除防复活、Keychain 用户变更、不同账号端口密钥/地址、失效引用、密钥旧名称和离线密钥路径测试。未修改用户历史文件或连接实际主机；更新后加载时自动整理显示，下一次历史写入保存整理结果。
- Windows Release、EXE 版本启动、37 个修改构建输入一致性、ZIP CRC/逐文件哈希通过。试用包 `TermViAI-recent-dedup-20261026-windows-x86_64.zip`（beta.6），SHA-256：`c8914635a7e4561508ca71ffeb21acb73ceb097419a9106d841c731144d91ba1`。
