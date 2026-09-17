# TermViAI 原生界面（2026-09-08）

按用户提供的 Termius 截图调整 Windows x64 GUI。直接扩展 WezTerm 原生渲染与窗口事件处理，没有引入网页外壳。

## 2026-09-17 矢量图标、过渡动画与 Tab 拖放

参考本次 Termius 录屏，原生界面已换用统一矢量图标，修正标题/按钮留白、窄窗口工具栏和细滚动条圆角，加入抽屉滑动、背景遮罩、悬停及页面过渡。单会话 Tab 可拖入当前工作区，在四个方向显示半窗格预览；移动现有连接，支持 Esc/失焦/区域外松手取消。Mux 移动失败会保留源和目标布局，增加 resize 故障回滚测试。

Windows x64 GUI Release 构建通过；本轮 23 项 Windows 测试通过。新版入口：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\modern-ui-20260917\wezterm-gui.exe`。使用说明见 [MODERN_UI.md](MODERN_UI.md)，证据和未验证边界见 [MODERN_UI_VALIDATION.md](MODERN_UI_VALIDATION.md)。[HTML 设计预览](modern-ui-preview.html) 使用示例数据，不是原生截图。真实窗口视觉、SSH 拖放、IME 与广播回归仍需人工验收。

## 2026-09-17 多 SSH 与广播

同 Tab 多 SSH 分屏、广播开关及成员/独立输入现已接入，见 [BROADCAST.md](BROADCAST.md)；当前构建测试见 [BROADCAST_VALIDATION.md](BROADCAST_VALIDATION.md)。

## 已实现

- 深色工作区顶栏：终端 tab 切换/关闭、新建本地终端、窗口最小化/最大化/关闭。
- 顶栏空白处及 TermViAI 标题支持窗口拖动，菜单按钮、tab 与窗口控制按钮保持独立命中区域。
- 固定左侧栏：Hosts、Keychain，以及本地 Terminal 入口。
- Hosts：搜索、SSH 快速连接、响应式主机卡片、带数量的分组卡片和溢出切换。
- Add Host / Edit Host：Label、Address、Port、Username、Group、可选私钥路径。保存后清除筛选，显示完整主机列表。
- 主机卡片点击通过现有 `RemoteSshDomain` 创建 SSH tab；认证及主机密钥检查保留原有 SSH 流程。
- Keychain：添加本机已有私钥文件的引用，并在主机表单中选择。只保存名称和路径，不复制私钥内容，不保存密码。
- Add Host、Edit Host、Add Key 共用右侧抽屉。点击外部收起且不触发底下主机；内部空白保留，右上箭头、Cancel 和 Escape 均可关闭。抽屉独立于列表绘制，避免底层文字穿透；较小窗口下滚动字段，Tab / Shift+Tab 自动显示当前字段，底部保存按钮固定。
- 表单：Tab / Shift+Tab 切换字段，Enter 保存，Escape 取消，Ctrl+A 全选，Ctrl+V 粘贴，光标移动和中文组合输入路径。异步粘贴核对焦点版本，避免写入已切换的表单。
- Ctrl+Shift+H 回到 Hosts。Hosts/Keychain 输入在终端快捷键和 transport 之前处理，不发送给隐藏的 shell。
- 为侧栏预留终端像素宽度；初始尺寸、resize、绘制、鼠标和 IME 坐标采用同一宽度。

## 按要求不提供的入口

顶栏没有 SFTP、Vaults；侧栏没有 Port Forwarding、Snippets、Known Hosts、Logs；没有账号注册、登录管理或 Telnet。

当前 Keychain 只管理本地密钥文件引用。没有实现云端账号、密钥同步或密钥生成。

## 主机数据

默认文件是 `config::DATA_DIR/termviai/hosts.json`，Windows 下通常为 `%APPDATA%\wezterm\termviai\hosts.json`。保存使用同目录临时文件加原子替换；读取损坏文件时报告错误并保留原文件。保存前重新加载磁盘数据，合并其他窗口的添加；编辑目标已变化时拒绝覆盖。

环境变量 `TERMVIAI_DATA_DIR` 可指定独立数据目录。本次测试使用 `C:\Users\h00893113\Documents\Codex\termviai-win\ui-validation-20260908`，其中示例主机均为 `127.0.0.1:1`。正式主机库未添加测试记录，没有尝试连接截图中的主机。

`termviai_ui` 配置默认开启。需要回到上游界面排查时，可同时设置 `termviai_ui=false` 和 `window_decorations="TITLE|RESIZE"`。

## 实现位置

- `wezterm-gui/src/hosts.rs`：本机数据模型、保存、SSH 地址解析和校验。
- `wezterm-gui/src/termwindow/termviai_ui.rs`：布局、命中区域、表单与交互、SSH domain 接入。
- `termwindow/keyevent.rs`、`mouseevent.rs`、`render/paint.rs`：输入和绘制入口。
- `termwindow/resize.rs`、`render/mod.rs`：侧栏对应的终端尺寸和坐标调整。
- `config/src/config.rs`：原生 UI 开关和 Windows 默认窗口装饰。

## 验证范围

Windows 原生 5 项测试覆盖抽屉外部点击隔离、内部命中、顶栏命中保留、窄窗口布局、键盘焦点与滚动边界，以及 SSH/IPv6 地址与无效协议、主机库保存/重读与损坏文件保护、调色板解析、Unicode 编辑。构建和测试的最终结果见同目录验证记录。

已启动过包含主机卡片的原生窗口，系统查询确认存在 `TermViAI — Hosts` 窗口且 `Responding=True`，该次运行日志无错误。之后还有分组、地址边界与输入保护的收尾改动；最终产物需人工视觉与交互验收。

Computer Use 工具对 `wezterm-gui.exe` 返回 `product policy blocks this app`，所以未进行自动截图或真实点击/IME/拖动验收。不能将进程响应和单元测试写成完整 GUI 交互通过。真实 SSH 登录、ConPTY 回归和广播仍需后续验证；首次 UI 迭代未接入广播；2026-09-17 已接入，真实 SSH/ConPTY 图形运行仍待验收。

原 `--version` 占位文本问题仍存在，未为本次界面改造扩展依赖或修改版本链路。
