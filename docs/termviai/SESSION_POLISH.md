# 终端交互与渲染改进（2026-09-17）

## 本轮九项反馈

| 反馈 | 实现 |
| --- | --- |
| 图标、圆角出现黑线 | 截图暗线位于圆角背景的分块拼接处；原始角蒙版边缘 alpha 正常。标准圆角改用单 quad 解析覆盖率，GL/WGSL 共用算法，消除内部拼接；矢量图标使用正确乘透明度的绘制层。 |
| All/None 状态反直觉 | 合并为一个轨道，只有当前选项显示高亮底色，另一项透明；部分选中时两项都不高亮。 |
| 悬浮提示太多 | 删除窗口控制、普通文字按钮、Tab、翻页等提示，保留广播行为及少数图标控件的必要说明。 |
| 放大只扩张末端窗格 | 本地/直接 SSH Tab 启用稳定比例布局，连续微小缩放无累计偏置；手动调整后保存新比例，嵌套最小尺寸、zoom 与拓扑变化均处理。 |
| 不要终端标题背景条 | 标题透明且无悬浮底色；焦点由名称、图标与窗格周围的圆角描边颜色表示。 |
| 关闭广播的深色圆洞 | Tab/窗格广播按钮 OFF 时无背景；ON 保留紫色。 |
| 当前会话字体缩放 | 终端内容区 Ctrl + 滚轮调整当前选中 pane，每事件 ±1 pt，6–48 pt，仅本次会话有效，其他 pane 不受影响；操作不转发为远端滚轮输入。 |
| 全局字体大小 | 左侧 Settings → Global font size → Save；支持 6–48 pt，保存到 hosts.json 同目录的 settings.json，键为 terminal_font_size。立即应用所有 TermViAI 窗口，重启恢复；已有会话字号覆盖保留。 |
| 收起时终端下沉生硬、分隔线方正 | 广播栏与终端可用空间共同过渡；对网格增量加入连续像素插值，正文、标题、圆角边框、鼠标和 IME 共用位置；PTY 仅按实际网格变化调整，不拉伸字形。 |

## 实现与边界

`termviai_font.rs` 管理全局字号与 pane 会话覆盖；独立 FontConfiguration/RenderMetrics 和字体 ID 缓存键防止不同字号混用字形。Mux 在每次布局中直接计算独立字号对应的 PTY 网格，避免先基础字号、后独立字号的双重 reflow。关闭 pane 后清理状态；同窗口 Tab 合并/移动保留会话字号。

广播栏保留 200ms 收放，窗格位置对单元格取整增量做 90ms 插值；过渡完成仍安排必要的最终帧，防止停在倒数一帧。绘制时裁掉不完整末行；分隔拖拽热区在视觉位置追平实际网格前暂停，避免误捕获正文点击。

比例布局仅对由本机直接管理的 PTY Tab 启用；ClientDomain/tmux 远端布局保留原语义。会话字号覆盖属于当前 GUI 窗口的运行时状态，不保存到工作区模板或磁盘。上游跨 GUI 窗口迁移不在本轮会话字号的验收范围内。

## 验证

- Windows mux `cargo test --locked --release -p mux tab::test -- --test-threads=1`：19 项通过，包括新增 7 项比例与 4 项独立字体网格/迁移回滚回归。
- Windows GUI `cargo test --locked --release -p wezterm-gui --bin wezterm-gui termviai_ -- --test-threads=1`：52 项通过，包括字号、缓存、鼠标定位、存储保留/损坏保护、动画末行裁剪与最终帧、Settings 抽屉及解析圆角参数/WGSL 验证。
- 合计 71 项 Windows 测试通过；`cargo build --locked --release -p wezterm-gui` 成功，退出码 0，最终构建 47.47 秒。
- 实际比例算法的独立 4 项 Rust 测试和独立 WGSL parse/validation 通过；rustfmt、`git diff --check` 通过。
- 55 个修改/新增 Rust 文件、shader 和 Cargo 清单与 Windows 快照校验一致。
- 未操作原生 TermViAI GUI 或建立真实 SSH，未关闭用户进程。此前自动操作工具对该应用的限制保持有效；实际 GPU 显示、GLSL 运行时编译、动画手感及真实 SSH/IME 仍需人工验收。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\session-polish-20260917\wezterm-gui.exe
```

Windows x64 PE Machine `0x8664`，exe 72,873,984 字节；SHA256：`730f4f0c4d80c44208d914200baa20489d431893a1cbef1849b6dc21f00bdac6`。

验证日志、脚本及校验清单位于 `C:\Users\h00893113\Documents\Codex\termviai-win\session-polish-20260917`。独立发布目录含运行依赖，旧目录保留。

## 建议人工验收

1. 检查顶栏广播按钮、加号、关闭确认卡片的圆角边缘，确认无黑色拼接线。
2. All/None/部分选择、广播 ON/OFF 相互切换；OFF 的 Tab 广播没有深色背景块。
3. 四分屏先拖成不等比例，反复放大缩小，检查所有窗格同比例调整；关注最小窗口后的恢复。
4. 点击不同终端并悬浮标题，只有焦点名称/图标和圆角边框变色。
5. 选中一个终端 Ctrl+滚轮，确认只改该会话；全组广播开启时字号操作仍不会发到远端。检查选字、vim mouse、滚动条和 IME 位置。
6. Settings 保存全局字号并重启，确认恢复；临时会话字号不在新连接中保留。
7. 广播栏快速收起/展开及中途反向，检查下排终端平滑移动、正文不越过边框、最终空间完整回收。
