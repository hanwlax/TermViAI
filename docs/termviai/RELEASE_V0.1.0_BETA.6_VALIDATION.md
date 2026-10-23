# TermViAI v0.1.0-beta.6 验证记录

日期：2026-10-23（Asia/Shanghai）。Windows 10/11 x86_64 便携 Beta。

## 构建与测试

Windows 原生 MSVC Release。发布脚本 `termviai-beta6.cmd`，日志 `termviai-beta6.log`；证据归档于本地 Windows 构建目录 `releases/verify-v0.1.0-beta.6`。

| 检查 | 结果 |
| --- | --- |
| SSH 库 | 19 通过 |
| window 库 | 5 通过 |
| 终端核心 | 54 通过 |
| mux | 42 通过 |
| 广播策略 | 12 通过（11 单元 + 1 集成） |
| 完整 GUI | 140 通过 |
| 合计 | 272 项通过，0 失败 |
| Windows Release 编译 | 成功 |
| CLI | `TermViAI 0.1.0-beta.6` |
| PE 产品字符串 | `TermViAI` / `0.1.0-beta.6` |
| PE FileVersion / ProductVersion 数值 | `0.1.0.6` |
| 嵌入图标 | ID 0x101，10 个尺寸与源 ICO 逐字节一致 |
| 构建输入 | 557 个 Rust/TOML/lock/版本/图标文件与工作树哈希一致 |
| 格式与差异 | nightly rustfmt 与 git diff --check 通过 |

GUI 回归覆盖 Key 编辑/删除、过期编辑与名称别名、主题持久化和未知主题回退、主题对比度与终端调色板保留、紧凑工作区几何、广播直接成员选择、Recent connections 嵌套悬浮、New Tab 按下隔离，以及主/备用屏幕 Ctrl + 单击链接绑定。该版本沿用现有 Windows CI；本地结果不代表远端 CI 已完成。上游已有弃用、dead code、PDB 等警告未阻止构建。

## 发布前审查

范围为 beta.5 后全部本地变更与新模块/文档。重点核对 Keychain 别名及旧快照兼容、删除保护、偏好保留、窗口内主题预览/缓存刷新、终端显式颜色、广播成员切换、共享边框与 PTY 几何、链接打开和文本选择的事件分离。No actionable findings：未发现需要阻止本次发布的已证实问题。

Key 删除保护覆盖保存的 Host；仅被最近连接或保存工作区快照引用的密钥仍可删除，之后恢复该快照时可能需要重新选择认证。删除不动磁盘私钥。显式自定义鼠标绑定保留优先级。

## 发布包

- ZIP：`TermViAI-v0.1.0-beta.6-windows-x86_64.zip`。
- 大小：29,952,458 字节。
- ZIP SHA-256：`6977363509dc9d237b2cd071259b4effdcf0717400e64d871183d338108c5364`。
- EXE SHA-256：`82d75fae84d95f337973c226d1f2f2d93811d642a4eb273e1c4120d14c149b8c`。

包含 TermViAI.exe、四个运行依赖、README、发布说明、认证/分组/主题说明、MIT 许可证和 SHA256SUMS.txt。5 个 PE 文件均为 AMD64。ZIP CRC 及清单内 11 个文件的 SHA-256 校验通过。

发布附件为 ZIP 与 `.sha256`，保留旧 Beta。发布流程还需从 GitHub Release 下载附件，核对 ZIP SHA-256、CRC 及逐文件清单，并将回传验证记录留在本地证据目录。Release：<https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.6>。

## 验证边界

凭据测试使用合成数据，不读取真实 Host 密码。真实 SSH/MFA、长时间空闲/睡眠唤醒、原生鼠标与浏览器拉起、DPI/GPU/IME 和 vim/tmux 仍需持续人工验收。自动化通过不等于以上场景已完成原生实测。
