# 左侧栏动画切换

日期：2026-09-17。

顶栏左上角的菜单按钮现在只负责收起或显示左侧栏，不再跳转到 Hosts。切换时保留当前页面、当前 Tab、活动终端和输入焦点。

侧栏采用 220 毫秒双向过渡。Hosts、Keychain、New Tab、终端窗格和底部 Broadcast 工具栏会随可用宽度同步移动与重排；侧栏控件的鼠标命中区随画面一起移出或恢复，隐藏后不会留下不可见点击区域。终端页在动画期间按实际可用宽度更新 PTY 网格和多窗格比例。

Windows 原生验证：

- `cargo test --locked --release -p mux tab::test -- --test-threads=1`：20 项通过。
- `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termviai_ -- --test-threads=1`：55 项通过。
- `cargo build --locked --release -p wezterm-gui`：通过。
- `git diff --check`：通过。

交付目录：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\sidebar-toggle-20260917
```

`wezterm-gui.exe` SHA-256：

```text
577487bf61520616a9f10fd6184e52b1c5d2de24b3a375747d20337bd547af98
```

自动化测试覆盖页面状态保持、动画双向终点、窄窗口广播栏布局及终端尺寸计算。真实窗口中的 GPU 动画观感、不同 DPI、SSH/ConPTY 和 IME 仍需人工运行验收。
