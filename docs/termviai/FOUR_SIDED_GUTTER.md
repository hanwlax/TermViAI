# 终端与 Broadcast 四边等距

日期：2026-09-17。

下排终端与 Broadcast 面板之间的视觉间距已统一为 12 个逻辑像素，与终端区域顶部、左右两侧使用同一间距。窗口高度不能被终端单元格高度整除时，剩余像素由下排终端的可视外框吸收，不再全部堆积到 Broadcast 上方。

这项调整只影响圆角外框的可视边界。PTY 仍按完整单元格计算，终端行列数、分屏比例、文字位置、鼠标映射和 SSH 会话尺寸均不改变。Broadcast 收起后，终端区域底部同样保留 12 个逻辑像素；收放过程继续使用现有动画。

Windows 原生验证：

- `cargo test --locked --release -p mux tab::test -- --test-threads=1`：20 项通过。
- `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termviai_ -- --test-threads=1`：56 项通过。
- `cargo build --locked --release -p wezterm-gui`：通过。
- `git diff --check`：通过。

交付目录：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\four-sided-gutter-20260917
```

`wezterm-gui.exe` SHA-256：

```text
d2d7a3a8528f7fca3fa2ad99072b2bfb0ad82e92c9f6825e81930a1e24db0642
```

自动化测试覆盖 Broadcast 展开和收起时的 12px 底部几何关系。真实 Windows 窗口的不同 DPI、字号及 GPU 动画效果仍需人工运行验收。
