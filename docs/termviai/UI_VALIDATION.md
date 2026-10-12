# TermViAI UI 验证

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
