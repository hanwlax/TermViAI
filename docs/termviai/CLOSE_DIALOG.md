# 原生关闭确认卡片（2026-09-17）

关闭窗口、单个 Tab、SSH 组、单个终端及退出应用的 TermViAI 确认界面统一为居中卡片，替换旧版终端文本 `Really kill ... [Y]es / [N]o`。保留原来的确认策略：无需确认的空窗口、`NeverPrompt` 与明确 `confirm:false` 继续直接关闭；非 TermViAI 界面沿用上游逻辑。

## 外观与交互

- 使用现有 UI 字体与深色配色，圆角卡片、轻阴影、背景遮罩及自绘电源图标；原会话保持在背景中。
- 显示关闭对象、稳定 Host/Tab 名称及连接数量；连接数量放在名称前，避免长名称遮掉数量。
- 明确的 Cancel 和关闭/断开按钮，默认焦点为取消。Tab / 左右方向键切换按钮；Enter / Space 在松开时执行当前按钮，Esc 松开时取消。
- 点击遮罩取消，卡片空白不取消；只有鼠标按下与抬起命中同一控件才执行。
- 160ms 入场、140ms 退场，带淡入淡出和轻微位移；小窗口改用紧凑排版。
- 取消后回到原页面，保留 Hosts/Keychain/重命名表单中尚未保存的草稿。

接受应用退出后，卡片保持显示 Closing… 并拦截输入，直到退出完成。确认期间键盘、原始快捷键、IME 提交/预编辑、粘贴、拖入文件/文字/URL 均不会进入 SSH。等待中的粘贴通过界面代次校验作废；长按确认按键的重复和释放也被消费。

关闭目标按窗口、Tab、pane 的 ID 绑定，执行前重新校验所属关系；目标已关闭或移走时不改关其他会话。关闭最后一个终端仍保留 Hosts 窗口；明确关闭窗口或应用才退出。

## 验证

- `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_ -- --test-threads=1`：38 项通过。
- 测试覆盖确认状态进出、单次接受、目标移动/关闭、空应用退出、退出等待期间的输入屏蔽、卡片几何/命中、默认取消焦点、长按键松开后单次执行，以及既有动画、广播、拖放和表单逻辑。
- 另外只读核对了草稿保留、键鼠/IME/剪贴板/拖入输入路径与图层顺序。
- Windows GUI Release 构建成功，退出码 0；保留既有预留图标/接口提示及 OpenSSL PDB 的 LNK4099。
- 本轮 7 个源码文件同步清单、48 个修改/新增源码的完整 SHA256 清单、测试/构建日志与脚本位于 `C:\Users\h00893113\Documents\Codex\termviai-win\close-dialog-20260917`。
- rustfmt 与 `git diff --check` 通过，48 个源码文件与 Windows 快照逐字节一致。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\close-dialog-20260917\wezterm-gui.exe
```

PE Machine：`0x8664`，Windows x64。exe：72,787,456 字节。SHA256：`52fde2bdf11f04dd838e1e5941c34a21a684e47050eaea70335bc7b91db4f212`。

同目录保留 ConPTY、ANGLE、Mesa 依赖。没有停止用户当前应用进程，没有覆盖旧版目录。

本轮参考了用户提供的截图，但没有自动操作原生 TermViAI 或登录真实 SSH。此前 Computer Use 对该应用的操作限制仍有效，未通过替代工具绕过。实际窗口的字体/阴影/动画、DPI、按钮点击、IME 与真实 SSH 关闭仍需人工验收；构建和逻辑测试不等同于这些运行检查。
