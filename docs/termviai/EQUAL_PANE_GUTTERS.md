# 组合终端间隔统一（2026-09-17）

组合终端的上下、左右视觉间隔现在统一为 12 个逻辑像素。此前 Mux 在分隔轴保留一个终端单元格；JetBrainsMono NFM 的单元格高度明显大于宽度，因此上下间隔约为一行高，左右间隔约为一个字符宽，看起来不一致。

本次只调整圆角窗格边框在分隔单元格中的视觉边界：上下边框向分隔区扩展，左右边框适当内收。PTY 行列、分屏比例、拖动区域和终端内容坐标保持原有逻辑；不同 DPI、全局字号和会话临时字号下都使用相同逻辑像素间隔。

## 验证

- Windows `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_ -- --test-threads=1`：49 项通过。
- 新增回归使用宽 10、高 20 的非方形终端单元格，验证上下和左右间隔均为 12 像素。
- Windows x64 Release 构建成功，退出码 0，耗时 52.12 秒。
- rustfmt 与 `git diff --check` 通过。
- 未自动操作原生 TermViAI 窗口；实际 DPI、不同字号和拖动时的视觉效果仍需人工验收。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\equal-pane-gutters-20260917\wezterm-gui.exe
```

该目录保留上一版的圆角修复、比例缩放、会话/全局字号和广播区连续过渡。

Windows x64 PE Machine `0x8664`；exe 72,892,416 字节；SHA256：`eb21d06767cbd403a9e33048469824bc59cbe50cabd3c70664e63d37a4844e44`。构建日志、脚本及校验清单位于 `C:\Users\h00893113\Documents\Codex\termviai-win\equal-pane-gutters-20260917`。
