# 原生界面现代化验证（2026-09-17）

本轮实现统一矢量图标、布局间距及文字避让、抽屉/悬停/页面过渡、单会话 Tab 拖入当前工作区、绿色半窗格落点预览。使用说明见 [MODERN_UI.md](MODERN_UI.md)。

## Windows 构建与测试

使用已有隔离 Rust 1.98.1 / MSVC 14.50.35717 环境，仅构建 Windows x64 GUI 产品；没有构建 Linux GUI、Windows CLI 或独立 mux-server 产品。

脚本：`C:\Users\h00893113\Documents\Codex\termviai-win\modern-ui-validation-20260917.cmd`。

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked --release -p mux tab::test:: -- --test-threads=1` | 5 项通过 |
| `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termwindow::termviai_ -- --test-threads=1` | 16 项通过 |
| `cargo test --locked --release -p wezterm-gui --bin wezterm-gui hosts::tests -- --test-threads=1` | 2 项通过 |
| `cargo build --locked --release -p wezterm-gui` | 通过，验证脚本退出码 0 |
| Rustfmt（本轮 9 个 Rust 文件）与 `git diff --check` | 通过；stable 忽略上游 nightly-only 配置提示 |

23 项 Windows 原生测试包含原有测试；本轮新增 15 项，不将先前广播策略测试重复计为本轮执行。覆盖：矢量路径及 DPI、动画端点/反转/停帧、抽屉关闭期间禁用旧输入目标、移动中的点击隔离、落点与分屏方向一致性、拖动后回到原处仍不触发点击、分屏失败时保留源/目标 Tab、同 Tab 回滚及回滚 resize 也失败的情形。

底层修复保留原源窗格，直到目标成功接收；目标 resize 出错会恢复布局树。断连后的外部 PTY 尺寸回滚是尽力执行，不承诺恢复断开的连接。通用 Domain API 在计算分屏尺寸前已有取消缩放行为；本轮拖放拒绝缩放目标，不将该既有通用 API 行为视为已修复。

编译仍包含上游弃用 API、生命周期和 OpenSSL 缺少调试 PDB 的警告，以及预留矢量图标未使用警告；未作为错误忽略编译失败。

## 产物与证据

- 新版目录：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\modern-ui-20260917`，入口 `wezterm-gui.exe`。
- 最终格式化后重建也通过，退出码 0；PE Machine 为 `0x8664`（x64）。
- exe 大小：72,462,336 字节。
- SHA256：`71dc751088e88650f93fe03e84cf563ea92cfdc5566bf27d0906f8148709d934`。
- 同目录包含 ConPTY、OpenConsole、ANGLE 和 Mesa 依赖，独立打包，没有关闭当前用户窗口。
- 证据目录：`C:\Users\h00893113\Documents\Codex\termviai-win\ui-modernization-20260917`，包含 `windows-validation.log`、`windows-release-final.log`、`exit-codes.json` 与 `artifact-sha256.json`。

源码先同步到 Windows 构建快照，再执行验证。本轮 9 个 Rust 文件的 SHA256 清单保存在 `ui-modernization-20260917/source-sha256.json`，只证明这些文件同步一致。原始文件备份位于该目录的 `baseline/`。编译、测试日志和打包校验清单一并归档在该目录，没有修改正式主机库或连接录屏中的主机。

## 运行验收边界

没有本轮原生窗口截图或实际点击验收。此前 Computer Use 对该应用返回 `product policy blocks this app`，未借助其他 UI 自动化手段绕过该限制。[HTML 预览](modern-ui-preview.html) 是标注样例数据的设计说明，不是原生截图，也不代替运行验证。

待实际窗口验证：100%/125%/150%/200% DPI 下图标和按钮位置；快速打开/关闭抽屉与 IME；Tab 的点击/拖放/取消；正在输出的真实 SSH 会话合并、断线时拖放；移动后的广播成员与独立输入。构建和故障注入测试通过不表示以上全部通过。
