# TermViAI 终端背景一致性修正

日期：2026-09-17

## 原因与修正

WezTerm 默认会通过 `inactive_pane_hsb` 压暗未选中 pane。终端内容背景经过该变换，TermViAI 标题和内容内边距则由另一绘制层合成，因此未选中 pane 出现了两种背景颜色；选中 pane 不经过压暗，所以没有色差。

TermViAI 已使用圆角边框、终端名称和图标颜色表示当前焦点。本轮让所有 TermViAI pane 的基础背景保持原始 Catppuccin Mocha 色值，不再按选中状态压暗。文字层的非活动状态处理、选中边框和普通 WezTerm 模式均保持原行为。

## 验证

- `git diff --check`：通过。
- Windows x64 Release Mux 测试：20 项通过。
- Windows x64 Release TermViAI GUI 测试：58 项通过，包含活动/非活动 pane 基础背景一致性回归测试。
- Windows x64 GUI Release 构建：通过。

构建产物：

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\pane-background-20260917\wezterm-gui.exe
```

SHA-256：

```text
cffede25f0f09b80b150a77d6cbf77b593480b9a82064e587f839c0372875260
```

真实 GPU 合成结果仍需打开该版本，使用多个 pane 在选中状态间切换进行视觉确认。
