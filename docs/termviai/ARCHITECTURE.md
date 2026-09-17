# WezTerm 基线与 TermViAI 接入笔记

## 2026-09-17 接入更新

GUI adapter 已实现，详见 [BROADCAST.md](BROADCAST.md)。关键边界是 TerminalState::send_raw_input：必须在 BufWriter/ThreadedWriter 之前捕获源用户字节，否则线程局部捕获会漏掉普通键和粘贴。直接 Pane writer 路径仍由 WriterWrapper 捕获，目标通过 send_raw_input 异步提交。下面保留接入前的架构调查历史；其“尚未接入”结论已被本段取代。

基线：`d2f3f05b38f26a872f4b0bfbb3d2eaa7bdfc1b0b`。以下结论来自本地源码。

## 现有对象与生命周期

| 对象 | 现有位置 | 结论 |
| --- | --- | --- |
| Pane | `mux/src/pane.rs:25` | `PaneId = usize`，输入由 `Pane` trait 暴露 |
| Tab | `mux/src/tab.rs:21` | `TabId = usize`，Tab 内部用 Mutex 保存状态 |
| 布局 | `mux/src/tab.rs:17` | 已有 `bintree::Tree<Arc<dyn Pane>, SplitDirectionAndSize>`，无需复制布局树 |
| 查询 | `mux/src/lib.rs:774` | `Mux::get_pane` 取得运行时 pane |
| 归属 | `mux/src/lib.rs:1080` | `resolve_pane_id` 返回 domain/window/tab 归属 |
| Pane 关闭 | `mux/src/lib.rs:820` | 从映射移除，调用 `kill`，发出 `PaneRemoved`，更新统计 |
| Tab 关闭 | `mux/src/lib.rs:835` | 从 tabs/windows 移除，遍历其中 pane，逐个移除 |

应用元数据按 TabId 保存 `BroadcastState<PaneId>`。使用
`iter_panes_ignoring_zoom()` 的完整成员快照调用 `sync_panes`，不能只同步当前可见 pane。
Tab 关闭时销毁对应元数据。路由前需要同步最新 Mux 状态，避免过期 ID 或跨 Tab 发送。

`sync_panes` 仅保留成员快照，不创建、销毁或拥有 PTY。断线不等于关闭：若底层已销毁
断线 pane，需要应用层保留稳定会话标识并在重连时映射新 runtime ID；当前包没有实现该恢复层。

## 实际输入路径

```text
WindowEvent
  → TermWindow::key_event_impl                 wezterm-gui/src/termwindow/keyevent.rs:599
    → process_key / key bindings / modal
    → 普通键: Pane::key_down / key_up
      → LocalPane                              mux/src/localpane.rs:399
        → TerminalState::key_up_down            term/src/terminalstate/keyboard.rs
          → key.encode(source terminal modes)
          → TerminalState.writer.write_all + flush
    → Kitty / Win32: GUI 编码 → Pane::writer().write_all
    → IME / composed: Pane::send_composed_text → Pane::writer().write_all

Clipboard
  → TermWindow::paste_from_clipboard            wezterm-gui/src/termwindow/clipboard.rs
  → 异步读取后重新查找 pane / overlay
  → Pane::send_paste
    → TerminalState::send_paste                 term/src/terminalstate/mod.rs:824
      → bracketed paste + newline canon + de-fang
      → TerminalState.writer.write_all + flush
```

GUI 的拖入文本、URL 和路径也有 `send_paste` 调用，位于 `termwindow/mod.rs`。
不能假定修改 `key_event_impl` 的一个分支就覆盖所有输入。

## InputRouter 插入策略

推荐保持策略在应用层，并在用户输入的编码完成与 transport 写入之间接入。
当前 `termviai` 包实现这一边界的**纯策略部分**，尚未连接到上述生产路径。

需要一个具有明确用户输入作用域的 writer/adapter：普通键和 paste 在源 pane 编码一次；
Kitty、Win32、IME 已生成的字节也进入相同路由；目标端只执行原始 transport 写入。
不能通过目标 `key_down` 或 `send_paste` 再次编码，也不能在目标 writer 上递归路由。

`mux/src/domain.rs:498` 的 `WriterWrapper` 为 Pane 和 Terminal 共享 writer。
它既承载用户输入，也可能承载设备状态查询等终端协议响应。因此直接在该 writer
无条件广播会导致协议响应、鼠标或焦点事件错误扇出。未来实现必须区分输入来源，并覆盖
异步 paste 的作用域。Overlay、搜索和命令面板不能作为真实 pane 广播。

这需要单独的集成改动和测试；当前没有修改 parser、renderer 或 transport。

## 编码、错误和性能边界

- 当前 router 原样共享 `&[u8]`，不解析命令，不保存输入内容，不为目标重新编码。
- 源 pane 的 application cursor、Kitty 和 bracketed paste 模式决定编码。
  目标应用模式不一致时，收到相同字节可能有不同解释；需用 vim/tmux 等实测，不能承诺语义相同。
- 单目标失败会写入报告并继续其他目标；部分写入后失败不能重放整段数据。
- 当前 callback 是同步接口，错误隔离不等于慢连接隔离。生产接入要验证 writer 的阻塞行为，
  必要时使用有界队列并显示背压错误；不得无限排队或默默丢弃输入。
- Mouse、resize、终端协议响应不进入广播路由。
- 快照中的成员状态是运行时状态。磁盘持久化应使用独立 PersistentPaneId，恢复后重新映射。

## 已完成与待完成

已完成策略测试和真实 shell 管道测试。尚待 Windows 基线、用户输入 adapter、
可见开关、GUI 快捷键、IME、剪贴板、PTY/ConPTY、SSH、关闭/重连、持久化和性能验收。
