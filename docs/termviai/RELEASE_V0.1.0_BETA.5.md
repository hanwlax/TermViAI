# TermViAI v0.1.0-beta.5

第五个 Beta 新增嵌套 Host 分组，重新设计互斥认证表单，并修复启动输出重叠和多处界面交互问题。

## Host 分组与管理

- 支持持久化多层分组。父分组显示所有后代主机，蓝色面包屑可返回任意可见上级；较长路径支持翻页。
- 新建分组后自动进入，可新建主机或从已有主机中多选加入。加入已有主机是移动归属，保留认证配置与当前连接。
- 分组三点菜单提供 New Host / Add Existing Hosts / Delete Group。从菜单新建主机只预填目标分组，不跳转当前页面。
- 删除分组仅移除容器，直属主机与子分组移至父级；深层主机、凭据和连接保留。遇到同名子分组冲突时拒绝操作并说明原因。
- All Hosts 首页的 Add Host 直接新建；添加已有主机仅在具体分组可用，避免意外将主机移到未分组。
- Host 三点菜单统一为图标与文字，支持 Edit / Duplicate / Delete。删除主机需确认，保留现有连接、Keychain、最近连接和已保存工作区。
- 旧文本分组自动读取为一级分组；删除、移动和编辑检查最新库，过期条目不会按旧索引修改其他主机。

## 认证表单

- Password / SSH Key / Identity 三种模式互斥，默认 Password；保存和连接仅采用当前模式。
- Password 模式随保存持久化密码，移除 Remember password 勾选框。已有密码未修改时显示固定八个实心圆点，不把原密码加载到编辑框。
- SSH Key 支持手填路径或选择 Keychain 密钥，支持带空格的路径及交互私钥口令。
- Identity 选择已有身份，显示只读摘要。切换模式保留本次表单草稿，不混用隐藏字段。
- 显式密码/身份模式不自动尝试 SSH agent 密钥；显式密钥模式只尝试所选密钥，完成密钥第一因子后保留后续交互挑战。
- 密码仍由当前 Windows 用户的凭据管理器保存，JSON 仅记录引用。旧主机未编辑前保留原连接行为；过期临时密码需重新输入。
- Save 和 Cancel 默认同色，Save 悬浮显示柔和紫色；下拉列表支持滚动和键盘选择。

## 终端与界面修复

- 修复刚连接时窗口布局变化把普通行首光标退回上一行，导致 SSH 启动输出叠写的问题；保留真实折行边界处理。
- Recent connections 新增 X，可移除单条记录；旧窗口缓存不会把已删除历史重新写回。保存的工作区不受影响。
- Add Key 与 Add Identity 统一采用图标加文字。
- 精简分组卡片，移除冗余右箭头；面包屑缩紧留白、加深分隔符，All Hosts 背景按实际文字宽度计算。
- 删除分组确认使用完整圆角焦点框。
- 鼠标移至 tab、Host 或 Group 内部图标时，容器继续高亮；图标悬浮面与容器区分，关闭 X 仍无背景且悬浮变红。
- New Tab 与相邻加号不再共享悬浮动画；终端与底部广播按钮保持独立反馈。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.5-windows-x86_64.zip`，完整解压到新目录，关闭旧程序后运行 `TermViAI.exe`。保留同目录的 `OpenConsole.exe`、`conpty.dll`、`libEGL.dll` 和 `libGLESv2.dll`。

支持 Windows 10/11 x86_64，继续使用 `%APPDATA%\wezterm\termviai` 或 `TERMVIAI_DATA_DIR`。升级前可备份该目录；密码属于当前 Windows 用户，不随便携包迁移。旧版不理解新的分组/认证字段，不建议用旧版编辑已升级的数据。

附件包含 ZIP 的 `.sha256`，包内 `SHA256SUMS.txt` 提供逐文件校验。认证和分组说明随包附带。

## 验证与边界

Windows 原生 260 项测试通过，Release 构建完成；详见[验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.5/docs/termviai/RELEASE_V0.1.0_BETA.5_VALIDATION.md)。Windows CI 新增终端核心测试。

真实 SSH/MFA、长期空闲与睡眠唤醒、原生鼠标动画、DPI/GPU/IME 和 vim/tmux 仍需持续人工验收。重连创建新的远端 shell，不恢复已结束的远端进程。

TermViAI = Terminal Via AI，表示应用通过 AI 制作；当前没有内置 AI 功能。项目基于 WezTerm，保留上游归属和 MIT 许可证。

完整改动：[v0.1.0-beta.4...v0.1.0-beta.5](https://github.com/hanwlax/TermViAI/compare/v0.1.0-beta.4...v0.1.0-beta.5)。
