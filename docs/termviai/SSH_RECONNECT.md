# SSH 原位重连

更新时间：2026-09-18（Asia/Shanghai）

## 用户行为

- SSH 传输结束、远端 shell 退出或 PTY 读取失败后，TermViAI 保留原 pane、组合 Tab 和分屏布局，不再自动移除整个终端组。
- 断线 pane 的名称右侧显示 16px 单箭头圆弧刷新图标，视觉尺寸略大于关闭叉号；点击后使用该 Host 配置建立新的 SSH 传输，在原 pane 内认证和连接。
- 组合 Tab 的所有 pane 均断线时，Tab 广播按钮位置显示同款重连按钮，一次发起组内所有 SSH 重连。部分断线时继续使用各 pane 的独立按钮。
- 重连退出可能残留的备用屏幕，重置滚动区域、光标原点和输入模式，在已有内容末尾换行衔接新会话输出，不插入蓝线或 `Reconnected` 标记。用户先前画的蓝线只是位置说明，不是 UI 要求。
- 重连不再重放应用启动更新横幅；横幅中的绝对光标定位会覆盖原有历史。旧连接的输出解析完毕后才允许重连，连续网络失败和随后成功的输出按顺序追加。
- 保留实际 SSH 网络/认证错误，省略 TermViAI SSH 的底层 `Process RemoteSshDomain` 退出及 Hold 提示，避免另一条异步输出流干扰连接日志。
- 重连准备失败的提示仅显示在对应 pane 内，重新尝试时清除；认证/网络错误由该 pane 的连接输出显示，不写入 Hosts/New Tab 的全局错误栏。
- 终端模型和 pane ID 不变，因此主屏幕滚动历史仍可向上查看，原焦点、布局、临时字号和广播成员关系也会保留。
- 断线期间键盘、IME、粘贴和广播不会写入失效传输；该 pane 暂不参与实际广播。重连后自动恢复为原来的成员状态。
- 每次重连重建终端后台输入线程和缓冲队列，避免断网时线程退出后，出现广播向其他窗格发送成功而源窗格写入失败。旧队列和缓冲输入直接丢弃，各代连接使用独立 writer，不把旧命令重放到新 shell。
- 用户仍可通过关闭按钮显式移除断线 pane。

重连会创建新的远端 shell。远端已结束的进程、备用屏幕自身的瞬时画面、未提交的命令行和密码无法恢复；关闭应用后也不会持久化此次滚动历史。

## 实现位置

- `mux/src/localpane.rs`：持有断线 SSH pane、判断连接结束、替换 PTY/child/writer，并保留原终端历史。
- `mux/src/domain.rs`：同一代连接的 Terminal 与 Pane 共享 writer；重连新建独立 wrapper，旧后台写入不能访问新传输。
- `term/src/terminalstate/mod.rs`：重建异步输入线程，取消旧队列并无刷新地丢弃 BufWriter 缓冲，保留原终端模型和捕获 ID。
- `mux/src/lib.rs`、`mux/src/pane.rs`：PTY reader 和 parser 全部结束后通知可重连；原 pane reader 重启时不发送新终端专用的启动横幅。
- `mux/src/ssh.rs`：显式重连始终新建传输；新 Tab 复用缓存会话申请 PTY 失败时，也新建传输重试一次，不要求关闭旧组。主机验证及认证照常执行。
- `wezterm-gui/src/termwindow/termviai_ui.rs`：断线状态与刷新图标。
- `wezterm-gui/src/termwindow/termviai_workspace.rs`：异步重连、并发保护和错误提示。
- `wezterm-gui/src/termwindow/termviai_broadcast.rs`：断线输入与广播隔离；具体失败 pane/原因写日志，后续非空广播全部提交成功时清除旧错误栏。

## 验证

Windows x64 Release 环境已通过：

- 主屏及备用屏幕历史保留、连续失败后成功且残留光标模式、满屏和窄屏折行测试：3 项；
- 通用 Socket 错误恢复、健康缓存保留及新连接失败不循环重试：2 项；
- 新旧 writer 代际隔离测试：1 项；
- 终端实际按键/粘贴/协议回复、输入捕获、失败后重建输入线程及旧队列取消：5 项；
- 广播策略与 Windows shell 测试：12 项；
- GUI TermViAI 回归（含整组重连显示条件）：61 项；
- `wezterm-gui` Release 构建。

本次共 84 项不同测试通过，Windows x64 Release 构建成功；重连与输入过滤器有 2 项交集，未重复计数。验证包目录为 `ssh-reconnect-input-20260918`，校验值见包内 `SHA256SUMS.txt`。前次历史保留修正已另行通过 Mux Tab 20 项回归。

仍需使用真实 SSH 主机人工覆盖：网络断开、服务端主动断开、睡眠唤醒、密码/密钥重新认证、多个 pane 依次断线、vim/tmux 备用屏幕及广播开关保持。
