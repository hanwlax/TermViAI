# TermViAI v0.1.0-beta.4

第四个 Beta 新增 Keychain Identities 和 Host 密码认证，改进终端组的重排、广播、重连与错误提示，并精简界面。

## Keychain 与认证

- Keychain 分为 Keys 和 Identities。Identity 支持保存名称、用户名、密码，并提供编辑和删除；被已保存 Host 使用的身份需先解除关联才能删除。
- 添加/编辑 Host 时支持输入密码、显示/隐藏、记住密码，并可从认证选择器选取 Keychain 中的密钥或 Identity。
- 记住的密码由当前 Windows 用户的凭据管理器保存，配置文件仅保存凭据引用。未记住的密码仅存在本次应用运行内存中，重启后需要重新输入。
- 编辑 Identity 时密码留空表示保留原密码；已有临时密码可通过“记住密码”切换保存方式。临时密码过期后会明确要求重新输入。
- 保存的密码每次认证最多自动尝试一次，失败后仍可手动输入。私钥口令、验证码及其他多因素挑战保留交互输入。
- 旧 Host/Keychain 配置继续读取，私钥文件仍保留在原位置。凭据不会随便携包或普通配置文件迁移到其他设备。

## 终端组

- 可拖动组内终端标题，将它与同组另一终端交换位置，保留连接、布局比例、滚屏、焦点和广播成员状态。
- 标签组只要有离线 SSH 终端，即显示一键重连入口，只重连离线成员。
- 标签广播按钮采用全选语义：全部成员参与且广播运行时为紫色；取消任意成员后变灰，再点灰色按钮会重新全选并启用。
- 终端标题和底部广播成员按钮独立显示悬浮及按下反馈；实际成员状态仍同步。
- 广播失败和重连错误显示在所属终端标题右侧的红色圆形叹号中，悬浮查看详情、点击清除；相关错误不再跨标签悬浮。
- 修复 SSH 空闲轮询退避可能延误 libssh keepalive 的问题，并增加断连诊断日志。该修复不代表所有网络断连原因均已消除。

## 界面与编辑

- Host 三点按钮单击打开 Edit Host / Duplicate Host 菜单，取消 Host 右键菜单；复制保留配置，名称自动追加 `-copy` 及重名序号。
- Host 表单和搜索框支持鼠标选区、Shift 选择、Ctrl+A/C/X/V；输入光标使用真实字形宽度定位。
- 调整网格、广播和关闭图标；静态按钮减少背景色，关闭 X 悬浮变红，广播悬浮显示柔和背景。
- 移除 Host 三点按钮、广播总开关及底栏展开/收起按钮的冗余提示。
- 移除终端侧栏 WORKSPACE 区域及静态 Grid view 标识。保存组仍可用标签右键菜单和 Ctrl+S。
- 默认 JetBrainsMono Nerd Font Mono 使用 Medium 字重。当前默认关闭编程连字以避免 `]#` 合并；这也会关闭 `>=`、`<=` 等连字，显式字体配置仍可覆盖。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.4-windows-x86_64.zip`，完整解压到新目录，关闭旧程序后运行 `TermViAI.exe`。保留同目录的 `OpenConsole.exe`、`conpty.dll`、`libEGL.dll` 和 `libGLESv2.dll`。

支持 Windows 10/11 x86_64，继续使用 `%APPDATA%\wezterm\termviai` 或 `TERMVIAI_DATA_DIR`。附件提供 ZIP 的 `.sha256`，包内 `SHA256SUMS.txt` 提供逐文件校验。

## 验证与边界

Windows 原生测试、版本资源及打包校验见 [验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.4/docs/termviai/RELEASE_V0.1.0_BETA.4_VALIDATION.md)。真实 SSH 密码/MFA、长时间空闲与睡眠唤醒、实际拖放、DPI/GPU/IME 仍需持续人工验收。

重连创建新的远端 shell，不恢复已结束的远端进程。TermViAI = Terminal Via AI，表示应用通过 AI 制作；当前没有内置 AI 功能。项目基于 WezTerm，保留上游归属和 MIT 许可证。

完整改动：[v0.1.0-beta.3...v0.1.0-beta.4](https://github.com/hanwlax/TermViAI/compare/v0.1.0-beta.3...v0.1.0-beta.4)。
