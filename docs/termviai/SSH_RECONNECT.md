# SSH 原位重连

更新时间：2026-10-12（Asia/Shanghai）

## 2026-10-12 同主机整组重连时部分终端失败

四终端组合中三个 pane 使用同一 SSH 主机，整组重连后前两个在认证成功后显示 `SSH session is dead`，最后一个成功。每个 pane 的显式重连都会创建独立传输并替换域缓存；原有 `Session::drop` 在任意克隆释放时发送 `SessionDropped`。后续 pane 替换缓存会使前一个尚未打开 PTY 的会话收到关闭通知，后台线程在没有 channel 时结束。

修复为所有 `Session` 克隆共享一个引用计数生命周期，仅最后一个会话句柄释放时发送关闭通知。PTY/SFTP 的请求 sender 不参与该引用计数，继续保留既有的“会话释放后等待已有 channel 结束”行为。缓存替换不会再关闭仍由认证线程持有的连接；无需串行化整组重连或复用已经失效的传输。

新增回归测试覆盖缓存克隆释放、三个同主机连接依次替换缓存、最后一个句柄释放后发送关闭通知；Windows CI 增加这些测试。真实四 pane SSH 场景仍需人工验收。

Windows x64 验证通过：SSH 新增回归 2、mux 39、广播 12、GUI 61、存储 7，共 121 项；Release 构建成功。本地验证包为 `releases/ssh-group-reconnect-20261012.zip`，版本字符串仍为 beta.2，未发布新 GitHub Release。验收请使用原四终端组合，三个 pane 指向同一主机，断线后整组重连，检查三个远端 shell、历史与广播，并重复重连。

## 2026-10-12 SSH 建连等待导致界面卡死

用户反馈连接停在服务器 `Authorized Only...` 提示后，点击顶栏 `+` 导致应用无响应。定位到认证完成后 `match block_on(session.request_pty(... *size.lock() ...))` 中的临时 MutexGuard：Rust 会把锁保留到整个 match 结束，GUI 对 Connecting pane 的 resize/get_size 因此等待远端 PTY/shell 创建请求。

修复在发起请求前取出尺寸副本并释放锁；认证成功后显示 `Authenticated. Opening remote terminal…` 阶段提示。新会话和缓存会话的 PTY 创建请求均增加 30 秒等待上限，失败显示在原终端；缓存请求失败继续走既有的新传输重试流程。该时限不包含交互输入密码，也不限制已经建立的 shell。

这是调用方等待上限，不是强制中断 SSH 库内部的阻塞调用；底层请求仍可能等待库/网络返回。服务器认证慢、PAM/登录脚本慢等原因尚未通过真实主机验证，不应把修复界面锁等待等同于修复所有服务器延迟。

Windows x64 本轮验证：Mux 39（新增 3 项等待期间尺寸可访问、超时释放、正常结果/错误保留测试）、广播 12、GUI 61、存储 7，共 119 项通过，Release 构建成功。构建目录已迁至本机 `termviai-win`，使用 `termviai-build-20260924.cmd`；本地修复包为 `releases/ssh-connect-responsive-20261012`，仍使用 beta.2 版本字符串，未发布新 GitHub Release。

本次只改 SSH 建连代码；工作树内已有的窗口缩放修改原样保留，Windows 快照中的这些修改与工作树一致并随本地包构建。未在真实 GUI/SSH 主机上自动复现；人工验收需覆盖慢连接时点 `+`、切换 Hosts、缩放、关闭 pane、超时后重连、密码认证和重连后广播。

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

## 2026-09-18 重连验证记录

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
