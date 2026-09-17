# 底部广播工具栏重设计（2026-09-17）

按用户截图将底部广播区整理为统一工具栏：左侧为广播图标、标题和成员计数，中间为按 Host label 长度适配的紧凑标签，右侧集中 All/None 分段选择、On/Off 滑动开关和收起箭头。采用细描边、统一按钮高度、柔和选中色与一致留白。

- 仅实际参与广播的成员呈紫色；关闭时使用中性色勾选，计数为 selected，开启后为 active。
- 主开关滑块有 160ms 过渡；原有收放动画与终端空间归还行为保留。
- 标签过长显示省略号，成员溢出支持左右翻页与滚轮。窄窗口逐步简化标题/批量选择，优先保留主开关和收起按钮。
- 单个长标签直接使用可用宽度，不显示无效分页按钮；Tab 切换重置成员滚动及开关动画，缩放/关闭成员后页码自动收敛。
- 底栏主开关继续暂停/恢复已选成员，All 仅全选，None 清空并关闭。顶部 Tab 广播按钮仍一键全组开/关，输入路由保持原行为。

## 验证

- Windows `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_ -- --test-threads=1`：41 项通过。
- 几何回归覆盖 320–1920px 宽度的控件避让、成员可达、长标签和成员变化后的分页边界；原有广播状态、收放防点击穿透及关闭确认等测试通过。
- Windows `cargo build --locked --release -p wezterm-gui`：成功，退出码 0，最终构建 52.05 秒。
- rustfmt 与 `git diff --check` 通过；49 个新增/修改 Rust 源码及清单文件与 Windows 快照一致。
- 从当前 Rust 底栏构造代码提取 Tile 坐标，生成 1465px ON/OFF、950px 成员溢出及 800px 长标签的离线预览，目视未发现按钮或文字冲突。预览使用近似字体，**不是原生截图或运行验收**。
- 未自动操作 TermViAI 窗口、建立真实 SSH 或关闭用户进程；原生字体栅格化、动画与真实输入仍需人工验收。既有自动操作工具对该应用的限制保持有效。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\broadcast-toolbar-20260917\wezterm-gui.exe
```

Windows x64 PE Machine `0x8664`；exe 72,823,808 字节；SHA256：`dc26e084683f3eed28cd4fe8653e3e1b3731ecfa6447bcaa65dfe97761d40e31`。

日志、构建脚本、源码/产物校验及离线预览位于 `C:\Users\h00893113\Documents\Codex\termviai-win\broadcast-toolbar-20260917`。新版独立打包，保留关闭确认、Hosts 默认首页、New Tab、保存工作区、重命名、Mocha 配色及 JetBrainsMono NFM 字体。
