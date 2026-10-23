# TermViAI v0.1.0-beta.6

第六个 Beta 面向 Windows 10/11 x86_64，重点是三套主题、紧凑终端工作区和交互修复。TermViAI 表示 Terminal Via AI，当前没有内置 AI 功能。

## 新增与改进

- Settings → Appearance 提供墨黑·鸢尾紫、深海·冰蓝、瓷白·鸢尾紫。即时预览，Save 保存并同步其他窗口，Cancel 恢复原主题；界面和终端默认色、ANSI 16 色一起切换。
- 广播面板默认收起为紧凑状态栏，可展开选择成员或全选/清空。成员按左上、右上、左下、右下的屏幕阅读顺序排列，拖动后同步更新；单终端只显示连接摘要。
- Tab 内 ⋯ 菜单集中重命名、保存工作区、重连离线成员和关闭。离线时保留独立广播/重连入口。
- 缩窄侧栏，统一共享分屏边框、标题激活线、图标和悬浮反馈，去掉底部重复横线及广播成员常驻底色。
- Keychain 的 Key 与 Identity 均可单击编辑；编辑页支持二次确认删除。密钥改名更新保存的 Host，并保留历史名称以兼容工作区和最近连接。

## 修复

- Recent connections 的关闭 X 保持整行悬浮；New Tab 与加号独立响应按下状态。
- 去掉 Identity 重复图标，修正删除按钮与页脚分隔线重叠。
- 广播关闭后，单独选择某个成员不会意外恢复其他旧成员。
- 终端链接默认使用 **Ctrl + 左键单击** 打开，普通单击和拖动只选择文字，减少误触。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.6-windows-x86_64.zip`，完整解压到新文件夹，关闭旧程序后运行 `TermViAI.exe`。请保留同目录的 OpenConsole.exe、conpty.dll、libEGL.dll、libGLESv2.dll。

现有数据目录 `%APPDATA%\wezterm\termviai`（或 TERMVIAI_DATA_DIR）继续使用。主题偏好写入 settings.json；密码仍由当前 Windows 用户的凭据管理器保存。删除 Keychain 密钥条目不删除私钥文件；保存的 Host 仍引用该密钥时禁止删除。仅被历史快照引用的密钥可以删除，对应快照之后可能需要重新选择认证。

包内附 SHA256SUMS.txt，Release 附 ZIP 的 SHA-256 文件。

## 验证边界

Windows 原生自动化测试和 Release 构建结果见 [验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.6/docs/termviai/RELEASE_V0.1.0_BETA.6_VALIDATION.md)。真实 SSH/MFA、长时间空闲/睡眠唤醒、DPI/GPU/IME 和原生鼠标/浏览器拉起仍需持续人工验收。保留现有终端显式颜色覆盖及自定义鼠标绑定。
