# 广播状态显示修正（2026-09-17）

根因是窗格标题的紫色广播指示仅依赖 `is_member`。新窗格默认预选为成员，但组广播初始关闭，因此显示与实际输入状态不一致。

现在标题按钮与底栏成员的紫色高亮统一要求 `enabled && is_member`：首次打开/恢复组，以及拖入广播关闭的目标组时显示灰色；广播开启后只有实际成员高亮；暂停保留预选，但不再高亮。底栏灰色勾选表示预选，计数区分 `selected` 和 `active`，悬停提示也明确显示 ON/OFF。

没有改变成员预选、输入路由或已有整组开关规则。拖入组沿用目标 Tab 的状态：已开启的目标仍会让新加入的窗格参与广播；来源 Tab 的开关不复制到关闭的目标 Tab。

## 验证

- Windows `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_broadcast::tests -- --test-threads=1`：5 项通过，新增 2 项回归。
- 覆盖新组关闭、暂停保留预选但指示关闭、独立成员不亮、从已开启来源拖合到关闭目标不继承开启，以及既有整组一键开关与 Tab 隔离；同时核对 `targets_for` 与指示状态。
- `cargo build --locked --release -p wezterm-gui`：成功，退出码 0。
- rustfmt、`git diff --check` 通过；48 个新增/修改源码文件与 Windows 快照一致。
- 未自动操作原生 TermViAI 或建立真实 SSH；实际图形显示与输入仍需人工验收。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-status-20260917\wezterm-gui.exe
```

Windows x64 PE Machine `0x8664`；exe 72,819,200 字节；SHA256：`178346fc03745b33a7a67509390846452d3a3670f05905b0d8aa4e8f3065841d`。

日志、脚本及源码/产物校验清单位于 `C:\Users\h00893113\Documents\Codex\termviai-win\broadcast-status-20260917`。新包保留原生关闭确认卡片、New Tab、工作区保存、Mocha 配色和 NFM 字体；旧版目录保留，未停止用户进程。
