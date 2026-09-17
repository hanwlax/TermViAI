# TermViAI 响应式布局与窗口缩放

日期：2026-09-17

## 本轮修正

- Broadcast 标题和 `x/x selected` 状态不再使用固定横坐标，改为跟随当前内容区左边界。左侧栏展开、收起及动画过程中，标题、图标和成员列表保持同一布局关系。
- TermViAI Windows 窗口增加原生最小客户区约束：720 × 480 逻辑像素。约束通过 `WM_GETMINMAXINFO` 实现，并按当前 DPI 换算为实际窗口尺寸。
- Windows 实时拖动窗口时仍逐帧更新界面尺寸；只有终端行列、像素网格或 DPI 真正变化时，才重新调整 Mux、PTY、覆盖层和标题，避免每移动 1 像素都触发整组终端重排。

## 验证

- `git diff --check`：通过。
- Windows x64 Release Mux 测试：20 项通过。
- Windows x64 Release TermViAI GUI 测试：57 项通过，其中包含最小窗口下广播工具栏可用性测试。
- `cargo build --locked --release -p wezterm-gui`：通过。

构建产物：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\responsive-resize-20260917\wezterm-gui.exe
```

SHA-256：

```text
a2ea0bb2cce63cb7657cd0357ea9de9b24cf06616e634717f5ac0284886b00ce
```

## 验证边界

自动测试覆盖布局计算、最小尺寸选择和完整 Windows 编译链。窗口拖动的实际帧率受 GPU、显示器刷新率和活动 SSH 输出影响，仍需在真实窗口中拖动四个边角，确认主观流畅度及不同 DPI 下的最小尺寸。
