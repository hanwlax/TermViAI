# 从组合 Tab 单独移除终端（2026-09-17）

组合 Tab 中每个终端标题右侧现在依次显示广播按钮和关闭按钮。关闭按钮只在当前 Tab 包含多个终端时出现，点击后精确定位该 pane，并沿用现有的原生关闭确认卡片；确认后只断开并移除这个终端，不会关闭整个 Tab 或应用。

移除后 Mux 会重新铺满剩余空间，广播状态同步清理已关闭的 pane。组内只剩一个终端时，它继续作为普通单会话 Tab，组内移除按钮随即隐藏。关闭按钮常态与标题背景融合，悬停时使用现有的危险操作强调色。

## 验证

- Windows `cargo test --locked --release -p mux tab::test -- --test-threads=1`：20 项通过。
- Windows `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termviai_ -- --test-threads=1`：54 项通过。
- 新增回归验证组内按钮顺序为“广播、关闭”、按钮间隔为 4 个逻辑像素，单会话不显示组内关闭按钮。
- 既有关闭目标测试覆盖 pane 在确认期间被关闭或移动时不会误关其他终端。
- Windows x64 Release 构建成功，退出码 0，耗时 54.21 秒。
- `git diff --check` 通过。
- 未自动操作原生 TermViAI 窗口；真实 SSH 断开和视觉悬停效果仍需人工验收。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\group-pane-remove-20260917\wezterm-gui.exe
```

Windows x64 PE；exe 72,882,688 字节；SHA256：`3ee96ceedb35f6f9963a179d065b1df59cb03b2b1f289a3faaef7fd617a8ebb9`。
