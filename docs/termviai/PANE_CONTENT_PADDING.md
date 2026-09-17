# 终端内容边距修正（2026-09-17）

组合终端的文字现在与窗格边框保留 10 个逻辑像素的四边内边距。该值随 Windows DPI 缩放，并在计算 PTY 行列前从窗格可用像素中对称扣除，因此底部窗格不会再出现文字贴住或覆盖下方分隔线的情况，上、下、左、右的留白也不会受终端单元格宽高比影响。

渲染起点、可见行裁剪、输入法光标、鼠标单元格映射和窗格点击范围使用同一套坐标。内边距区域仍可点击激活对应终端；独立会话字号和全局字号继续按各自的真实单元格尺寸计算。远程 mux/tmux 布局不启用本地窗格内边距，避免重复缩小远端网格。

## 验证

- Windows `cargo test --locked --release -p mux tab::test -- --test-threads=1`：20 项通过。
- Windows `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termviai_ -- --test-threads=1`：53 项通过。
- 新增回归覆盖 10 像素对称边距、标题行和独立字体单元格组合。
- Windows x64 Release 构建成功，退出码 0，最终阶段耗时 55.73 秒。
- `git diff --check` 通过。
- 未自动操作原生 TermViAI 窗口；实际 DPI 和四窗格视觉效果仍需人工验收。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\pane-content-padding-20260917\wezterm-gui.exe
```

Windows x64 PE；exe 72,887,296 字节；SHA256：`01ab2afa48102ed3c5fefbea7b7863d769a482cd9d5481275326a00bd17a47cb`。
