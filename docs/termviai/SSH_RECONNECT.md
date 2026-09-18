# SSH 原位重连

更新时间：2026-09-18（Asia/Shanghai）

## 用户行为

- SSH 传输结束、远端 shell 退出或 PTY 读取失败后，TermViAI 保留原 pane、组合 Tab 和分屏布局，不再自动移除整个终端组。
- 断线 pane 的名称右侧显示 Catppuccin Blue 刷新图标；点击后使用该 Host 对应的 SSH Domain 在原 pane 内重新建立会话。
- 重连成功时先退出可能残留的备用屏幕和输入模式，再插入一条蓝色 `Reconnected` 分隔线；新会话输出接在分隔线下方。
- 终端模型和 pane ID 不变，因此主屏幕滚动历史仍可向上查看，原焦点、布局、临时字号和广播成员关系也会保留。
- 断线期间键盘、IME、粘贴和广播不会写入失效传输；该 pane 暂不参与实际广播。重连后自动恢复为原来的成员状态。
- 用户仍可通过关闭按钮显式移除断线 pane。

重连会创建新的远端 shell。远端已结束的进程、备用屏幕自身的瞬时画面、未提交的命令行和密码无法恢复；关闭应用后也不会持久化此次滚动历史。

## 实现位置

- `mux/src/localpane.rs`：持有断线 SSH pane、判断连接结束、替换 PTY/child/writer，并向原终端写入分隔线。
- `mux/src/domain.rs`：共享输入 writer 可原子替换，保证 Terminal 和 Pane 的输入都切换到新传输。
- `mux/src/lib.rs`、`mux/src/pane.rs`：PTY reader 结束通知及原 pane reader 重启。
- `mux/src/ssh.rs`：复用 Host/Domain 配置创建新 SSH backend。
- `wezterm-gui/src/termwindow/termviai_ui.rs`：断线状态与刷新图标。
- `wezterm-gui/src/termwindow/termviai_workspace.rs`：异步重连、并发保护和错误提示。
- `wezterm-gui/src/termwindow/termviai_broadcast.rs`：断线输入与广播隔离。

## 验证

Windows x64 Release 环境已通过：

- 重连分隔线测试：1 项；
- 共享 writer 切换测试：1 项；
- 广播策略与 Windows shell 测试：12 项；
- Mux Tab 回归：20 项；
- GUI TermViAI 回归：60 项；
- `wezterm-gui` Release 构建。

共 94 项测试通过。验证二进制 SHA-256：

`17093c8b1162a674aa70512765fc69c1fce0cc11a08177f01e20a474e63b17fb`

仍需使用真实 SSH 主机人工覆盖：网络断开、服务端主动断开、睡眠唤醒、密码/密钥重新认证、多个 pane 依次断线、vim/tmux 备用屏幕及广播开关保持。
