# Windows 构建阻塞诊断（2026-09-07）

## 恢复结果（后续更新）

用户授权并手动关闭 Smart App Control 后，原生查询确认其为 Off，Defender 杀毒和实时保护仍开启。Perl Encode 复测退出码 0，Windows GUI Release 构建退出码 0，耗时 12m 14s，已生成 x64 exe。下文为关闭前的诊断历史；当前结果见 [WINDOWS_BUILD_VALIDATION.md](WINDOWS_BUILD_VALIDATION.md)。图形窗口和 SSH 等仍未验收。

## 已确认原因（关闭前）

通过 Windows 原生 PowerShell 查询，`Get-MpComputerStatus` 返回 `SmartAppControlState: On`。Defender 杀毒与实时保护也为开启状态。

CodeIntegrity/Operational 日志中的 3077 和 3118 事件共同确认：拦截来自 Smart App Control，策略名 `VerifiedAndReputableDesktop`，GUID `{0283ac0f-fff1-49ae-ada1-8a933130cad6}`。

受阻文件均位于 `C:\Users\h00893113\Documents\Codex\termviai-win`：

| 相对路径 | Authenticode 状态 | SHA256 |
| --- | --- | --- |
| `perl\perl\lib\auto\Encode\Encode.xs.dll` | NotSigned | `8F782C0B20F9E9D2E329FD92A3DD05BF86147D9DA25AF7FB2730E374D96002BA` |
| `src\target\release\build\getrandom-5e84762c2ed0350c\build-script-build.exe` | NotSigned | `5E0392E44E9F57B19C1DAFE1EEE545AF40EFCCF1B16054E7AAD09915D3513E6B` |

DLL 及 EXE 的当前哈希与之前拦截事件的 SHA256 Flat Hash 一致。

## 本次复现

从 Windows 本地目录经 cmd 执行：

```bat
C:\Users\h00893113\Documents\Codex\termviai-win\perl\perl\bin\perl.exe -MEncode -e 1
```

退出码为 25，明确报告 Encode.xs.dll 被应用程序控制策略阻止。阻塞仍存在。本次未重新运行完整 GUI 构建，也未修改源码、安全策略或系统环境变量。

## 处置路径

微软说明 Smart App Control 不支持单应用例外；受信任 CA 颁发证书的代码签名可用于允许应用运行。使用其他已批准的 Windows 开发环境也是选项。

本机直接继续开发的可选方案是在 Windows 安全中心 → 应用和浏览器控制 → 智能应用控制设置中关闭 Smart App Control。这是整机安全设置变更，需要用户明确选择，不能将“解决构建阻塞”推断为已授权关闭整机防护。若设备由组织管理，还需遵守组织授权。

微软最新 FAQ 表示近期 Windows 更新支持重新开启；其他官方旧页面仍有重装限制说明。应以本机设置界面的具体提示为准，不能保证本机关闭后能够直接恢复。

用户确认并完成环境调整后：先复测 Perl Encode，随后直接运行已有 build-gui.cmd，核实 Cargo 退出码与实际 GUI 产物。继续保留 Defender 杀毒和实时保护。GUI 构建通过后仍需单独运行验收。

## 官方参考

- https://support.microsoft.com/en-US/Windows/Security/threat-malware-protection/smart-app-control-frequently-asked-questions
- https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control

主仓库当前仍有未提交内容：Cargo.toml、README.md、README.wezterm.md、docs/termviai/、termviai/。本次未清理或重置工作树。
