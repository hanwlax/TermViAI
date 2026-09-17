# Tab 内多主机与选择性广播（2026-09-17）

## 使用

1. 在 Hosts 打开第一台 SSH 主机，进入它的终端 Tab。
2. 左侧点 **+ SSH · split right** 或 **+ SSH · split below**，选择另一台主机或输入 SSH 地址并点 Connect。新连接加入刚才的 Tab，右分屏/下分屏以发起选择时的窗格为目标；不会因切换 Tab 把主机加到别处。
3. 重复添加需要的主机。原有分割线继续支持调整大小；点终端或左侧窗格名称切换输入源。左侧窗格列表显示保存的主机名称，支持滚轮。
4. 左侧 **✓** 是广播成员，**−** 是独立窗格。点击它可切换成员资格；All 全选，None 清空并关闭广播。
5. 打开 **Broadcast ON**。从任意成员输入时，字节发送给该 Tab 所有成员（源窗格恰好一次）；从独立窗格输入时，只发给它自己。关闭广播后，各窗格独立输入，并保留原成员选择。

每个 Tab 有独立状态。新窗格默认加入当前 Tab 的成员集合；关闭窗格会移除其成员记录；关闭 Tab 会清除对应记录。缩放隐藏的窗格仍属于原广播集合。新建/迁移到另一窗口的 Tab 默认关闭广播；状态目前不保存到磁盘。

## 实现

- `termviai/` 继续是可独立测试的策略包，同时作为 GUI 的本地依赖。
- `termwindow/termviai_broadcast.rs` 根据真实 Mux Tab/pane 快照维护成员，验证源 pane 的对象身份，避免 overlay 冒用同一 ID。
- `term/user_input.rs` 提供明确作用域的线程局部捕获，`mux/user_input.rs` 复用并做集成测试。终端的用户输入在进入 BufWriter/ThreadedWriter 前捕获；直接 Pane 写入由带 pane ID 的 WriterWrapper 捕获。返回/错误/panic 均清除作用域。
- 普通键、raw Alt 分支、Kitty、Win32-input、组合输入、剪贴板、拖入内容以及 SendKey/SendString 都经过统一入口。字符选择器等 modal 和 overlay 保持独立输入；鼠标、focus、resize 和解析器协议回复不进入广播。
- 编码只发生在源终端，成员通过 send_raw_input 向现有终端后台写入队列提交相同字节，不调用目标的 key_down/send_paste 再编码，也不递归广播。
- 异步剪贴板返回时重新核对焦点 pane、Tab、接收成员和页面/modal 状态；目标已变化则取消该次粘贴。
- 断线目标跳过，单目标入队错误不阻止其他目标；部分发送失败后记录状态，原事件仍标为已消费，避免 raw-key fallback 重放。
- 单次广播编码缓冲上限 8 MiB，超过时显示错误，不向任一成员发送截断数据。

特别注意：不能只在 `WriterWrapper` 中捕获普通键和粘贴。终端原本会经 `ThreadedWriter` 切换线程，导致线程局部捕获失效。当前实现专门在 `TerminalState::send_raw_input`、进入缓冲/后台线程之前拦截用户字节；真实终端编码测试覆盖了这个边界。

## 验收范围与边界

验证结果、Windows 新版路径及日志见 `BROADCAST_VALIDATION.md`。

真实 SSH 认证、ConPTY、IME、vim/tmux 的图形交互尚需人工运行验收。以前 Computer Use 对该应用返回 `product policy blocks this app`，本轮没有通过替代自动化绕过限制。

广播保留源终端的键盘和括号粘贴模式；各目标应用模式不同可能产生不同语义。当前复用终端已有异步写入队列，入队错误可隔离且后台短写入通过 write_all 补齐；入队成功不等于远端执行成功。尚未新增有界队列/背压调度，也未做 16/32 主机压力测试。没有新增会话恢复或自动重连映射。
