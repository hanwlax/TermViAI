# SSH 原位重连

更新时间：2026-09-18（Asia/Shanghai）

## 用户行为

- SSH 传输结束、远端 shell 退出或 PTY 读取失败后，TermViAI 保留原 pane、组合 Tab 和分屏布局，不再自动移除整个终端组。
- 断线 pane 的名称右侧显示 16px 单箭头圆弧刷新图标，视觉尺寸略大于关闭叉号；点击后使用该 Host 配置建立新的 SSH 传输，在原 pane 内认证和连接。
- 组合 Tab 的所有 pane 均断线时，Tab 广播按钮位置显示同款重连按钮，一次发起组内所有 SSH 重连。部分断线时继续使用各 pane 的独立按钮。
- 重连退出可能残留的备用屏幕和输入模式，换行后衔接新会话输出，不插入蓝线或 `Reconnected` 标记。用户先前画的蓝线只是位置说明，不是 UI 要求。
- 重连准备失败的提示仅显示在对应 pane 内，重新尝试时清除；认证/网络错误由该 pane 的连接输出显示，不写入 Hosts/New Tab 的全局错误栏。
- 终端模型和 pane ID 不变，因此主屏幕滚动历史仍可向上查看，原焦点、布局、临时字号和广播成员关系也会保留。
- 断线期间键盘、IME、粘贴和广播不会写入失效传输；该 pane 暂不参与实际广播。重连后自动恢复为原来的成员状态。
- 用户仍可通过关闭按钮显式移除断线 pane。

重连会创建新的远端 shell。远端已结束的进程、备用屏幕自身的瞬时画面、未提交的命令行和密码无法恢复；关闭应用后也不会持久化此次滚动历史。

## 实现位置

- `mux/src/localpane.rs`：持有断线 SSH pane、判断连接结束、替换 PTY/child/writer，并保留原终端历史。
- `mux/src/domain.rs`：共享输入 writer 可原子替换，保证 Terminal 和 Pane 的输入都切换到新传输。
- `mux/src/lib.rs`、`mux/src/pane.rs`：PTY reader 结束通知及原 pane reader 重启。
- `mux/src/ssh.rs`：显式重连始终新建传输；新 Tab 复用缓存会话申请 PTY 失败时，也新建传输重试一次，不要求关闭旧组。主机验证及认证照常执行。
- `wezterm-gui/src/termwindow/termviai_ui.rs`：断线状态与刷新图标。
- `wezterm-gui/src/termwindow/termviai_workspace.rs`：异步重连、并发保护和错误提示。
- `wezterm-gui/src/termwindow/termviai_broadcast.rs`：断线输入与广播隔离。

## 验证

Windows x64 Release 环境已通过：

- 主屏及备用屏幕重连后历史保留、无分隔线测试：1 项；
- 通用 Socket 错误恢复、健康缓存保留及新连接失败不循环重试：2 项；
- 共享 writer 切换测试：1 项；
- 广播策略与 Windows shell 测试：12 项；
- Mux Tab 回归：20 项；
- GUI TermViAI 回归（含整组重连显示条件）：61 项；
- `wezterm-gui` Release 构建。

共 97 项测试通过，Windows x64 Release 构建成功；验证包校验值见包内 `SHA256SUMS.txt`。

仍需使用真实 SSH 主机人工覆盖：网络断开、服务端主动断开、睡眠唤醒、密码/密钥重新认证、多个 pane 依次断线、vim/tmux 备用屏幕及广播开关保持。
