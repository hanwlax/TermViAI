# 终端配色与字体（2026-09-17）

TermViAI 的终端默认使用内置 `Catppuccin Mocha` 配色与 `JetBrainsMono NFM` 字体。Mocha 包含背景、前景、ANSI 16 色、光标、选区及分割线颜色；背景为 `#1e1e2e`、默认文字为 `#cdd6f4`。

用户所说的 JetBrainsMono NF Mono 在本机 Windows 注册为 **JetBrainsMono NFM**。字体元数据还包含 `JetBrainsMono Nerd Font Mono` 家族别名。Regular、Bold、Italic 和 Bold Italic 已通过 Windows 字体枚举确认存在，无需安装或配置额外字体目录。

实现为 `config/src/config.rs` 中的终端默认参数，字号沿用原配置；系统 UI 字体继续由窗口字体配置管理。原有内置字体、Emoji 和 Nerd Font 符号回退链保持可用。若用户的 `wezterm.lua` 显式指定 `font`、`color_scheme` 或 `colors`，仍按用户配置解析。对应配置写法：

```lua
config.color_scheme = 'Catppuccin Mocha'
config.font = wezterm.font('JetBrainsMono NFM')
```

[离线设计预览](modern-ui-preview.html) 的示例终端已同步上述背景、文字及字体家族；它仍是示例设计，不是原生运行截图。

## 本轮验证

Windows 原生 config 库现有 8 项测试通过；只调整默认参数，未新增重复验证默认字符串的测试。Windows GUI Release 构建通过，验证脚本退出码 0，PE Machine 为 `0x8664`（x64）。

新版入口：`C:\Users\h00893113\Documents\Codex\termviai-win\releases\mocha-font-20260917\wezterm-gui.exe`。

exe 大小：72,462,336 字节；SHA256：`0d5eb9a25280b476bf23702ffd05e67340bfa01079edd1fe08e95544fd4e38f9`。源码 `config/src/config.rs` 与 Windows 快照一致；Rustfmt 及 `git diff --check` 通过。

字体核验、构建日志、源文件与产物 SHA256 保存在 `C:\Users\h00893113\Documents\Codex\termviai-win\mocha-font-20260917`。未自动操作原生窗口，实际显示仍以运行窗口为准。
