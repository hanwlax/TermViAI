# Tab 全组广播开关（2026-09-17）

顶部 Tab 广播按钮现执行全组操作：

- 关闭状态点击：所有窗格加入并开启广播，包括此前独立输入的窗格。
- 开启状态点击：关闭广播并取消全部成员选择，所有窗格恢复独立输入。
- 只影响按钮对应的 Tab，不切换当前 Tab，不改变其他 Tab 的广播状态。

窗格按钮仍用于单独选择成员。底部开关仍暂停/恢复已选成员的广播；顶部全关后，底部开关须先选择成员（或 All）才能开启。顶部按钮可直接一键重新全开。

## 验证与产物

Windows 命令 `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_broadcast::tests -- --test-threads=1` 通过 3 项测试（新增 2 项）。覆盖部分成员/零成员全开、全关后的源输入隔离、其他 Tab 隔离、关闭/新增 pane 和已删除 Tab。`cargo build --locked --release -p wezterm-gui` 通过，退出码 0。

新版：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\group-broadcast-20260917\wezterm-gui.exe`。

- PE Machine：`0x8664`，Windows x64。
- exe 大小：72,733,184 字节。
- SHA256：`c84e33ad1a6f8fbbec8767bfb9450610e38021defd346063398c9bbb11544721`。
- 47 个修改/新增源码文件与 Windows 快照逐字节一致；rustfmt、`git diff --check` 通过。
- 日志、脚本及校验清单位于 `C:\Users\h00893113\Documents\Codex\termviai-win\group-broadcast-20260917`。

保留上游链接器和预留接口的提示。未自动操作原生窗口或登录真实 SSH；此前应用操作限制保持有效，真实窗口点击和输入验收仍需人工进行。上一版 51 项测试记录保留在 [WORKSPACE_UX_VALIDATION.md](WORKSPACE_UX_VALIDATION.md)。
