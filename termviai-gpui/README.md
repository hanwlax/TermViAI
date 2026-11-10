# TermViAI · Native Windows UI

Windows x86_64 Beta。使用 GPUI 原生窗口与 DirectWrite 文本渲染，不使用 WebView、Electron，也不使用之前 GPUI probe 的 vt100 内核或演示工作流。

## 启动

解压整个压缩包，运行 `TermViAI.exe`。默认读取与现有 TermViAI 相同的 Hosts、Keychain 引用、主题、历史和保存工作区。窗口默认进入 Hosts，不自动连接示例主机。

退出原 TermViAI 后再测试 Monitor / Port Forwarding。端口转发沿用原版的独占管理锁，同一份数据不能同时由两个实例管理；新界面会显示锁冲突原因。

需要完全独立的数据目录时：

```powershell
.\TermViAI.exe --data-dir C:\Temp\TermViAI-Test
```

测试目录仍使用系统 Credential Manager 保存主动输入的密码，不在 JSON 中写入密码。

## 保留的 TermViAI 操作

- 标题、标签和窗口控制位于同一个 52px 顶栏；常规标签宽 242px，新增 `+` 紧随标签。重试使用环形箭头，关闭 X 悬浮变红、不添加背景。
- 标签溢出时保留 242px 宽度，显示左右滚动按钮与带数量的 All tabs 列表入口；`+` 和窗口控制始终可见。支持滚轮/触控板横移、160ms 按钮滚动缓动、切换/新建/窗口缩放后的当前标签定位及拖动贴边自动滚动。
- All tabs 可搜索标签名和组内主机，显示当前标签及离线状态，悬浮显示关闭 X。支持 ↑/↓ 选择、Enter 切换、Esc 关闭；Ctrl+Tab / Ctrl+Shift+Tab 前后切换。搜索框聚焦时保留中性边框，不显示主题色外框。
- Hosts 启动页、分组层级与面包屑、父组显示全部下属主机、三点菜单、复制和删除。
- Edit Host 右侧编辑栏；Password / SSH Key / Identity 的编辑交互保持独立，Save 默认中性背景、悬浮强调。
- Keychain 的 Key 和 Identity 可编辑、删除。New Tab 保留最近连接与保存工作区，删除 X 仅在所在卡片悬浮时显示。
- 顶部终端 Tab 可重排，左右半区决定插入前后，无位置变化时不显示落位线；整组 Tab 仅可排序，不可拖入其他终端。终端可拖到其他终端的上下左右进行嵌套组合。组内拖动预览先移除原位置；拖到标签间、顶栏空白处或 + 可成为独立 Tab，连接不中断。
- 组合后的默认标题随成员更新（例如 `209 + 212`、`209 + 3 hosts`），手动名称始终保留。标签可直接重命名，无需先保存 Workspace；仅显式 Save Workspace 更新保存项。
- 分隔条拖动缩放、单终端放大、离线终端重连。工作区命名、保存、重连放在 Tab 菜单中。
- 广播区域按审核稿改为 34px 紧凑底栏，默认收起；点击摘要展开 49px 成员行，180ms 平滑过渡。成员按先左后右、先上后下排序，显示勾选和离线状态，全选/取消全选合并为一个操作。底栏图标与 Tab 广播按钮都表示全选，部分选择时再次点击恢复全选。复用现有输入编码及发送路由，离线错误留在对应终端。
- 终端使用现有 `wezterm-term` 解析和滚动历史、`wezterm-ssh`、`portable-pty`。Ctrl+Shift+C/V 复制粘贴，Ctrl+单击打开链接，Ctrl+滚轮调整字号；滚动条贴边/滚动出现，可拖动。
- 终端输入光标使用块状样式，按主题设置底色及反色文字，移动到已有字符上仍可辨认内容。
- 左键拖选显示选区背景，松开后自动复制；双击单词按终端词边界选中并复制（支持自动折行）；双击空白或三击选中当前显示行，保留缩进、排除末尾空白。拖选时可滚轮上下扩展，贴近上下边缘自动平滑滚动；普通单击不覆盖剪贴板。远端应用启用鼠标捕获时可按住 Shift 使用本地选区。
- SSH 系统广播以柔和蓝色铃铛显示在终端标题右侧，与红色错误共用提示区；悬浮预览、点击固定展开，正文和原文可选中后 Ctrl+C 复制，没有复制按钮。拖选预览时保持展开，点击外部或 Esc 关闭；切换页面不会把浮层带到其他终端。通知在当前连接内保存，可清除，最多保留 100 条 / 1 MiB 原始报文。
- 广播识别仅处理已知 util-linux wall 带 79 列填充、双 BEL 和明确边界的英文报文（含旧版截断消息头、CRLF / PTY ONLCR 的 CRCRLF）。候选收集窗口为 250ms、上限 64 KiB；不完整、含控制序列、混合输出或其他无法确认的格式原样输出。远端普通程序也能打印相同字节，因此这不是发送者身份验证，也不承诺所有系统广播都能转换。
- Windows 输入法取消组字或切换后会清理预编辑残留，恢复回车等终端按键；只有输入法明确提交的文字才发送到终端。滚动条悬浮与拖动期间显示手状指针，正文保持文本指针。
- Tab / Shift+Tab 在终端焦点内发送给远端程序，不触发 GPUI 的表单焦点切换；在设置和主机编辑表单内仍按正常方式切换输入框。
- 按 chainKey → Identity → 手动密码尝试，失败后在对应终端显示认证选择。认证成功只回写对应项，失败不覆盖主机。保存冲突会显示错误，可点终端错误标记重试保存。
- Settings 切换 Ink / Navy / Light、字号、SSH keepalive、关闭确认与终端提示音。
- Monitor 和 Port Forwarding 在侧栏进入，直接复用现有采集、连接复用、进程校验、转发所有权和自动恢复代码。Monitor 恢复旧版的紧凑主机概览、按 NPU 聚合的 Die 网格、悬浮进程速览和右侧详情抽屉，支持 All/Die 筛选、暂停/重试、2/5/10 秒采样间隔。详情在切换 Tab 后保留。

- 空状态统一提供图标、标题、说明及相关操作；错误原文可展开查看。Monitor 添加主机只允许选择未添加项，已有项折叠展示；支持显式替换失效主机引用，采样间隔集中在时钟按钮中。

- Hosts 支持 Ctrl + 左键切换多选，Enter 按列表顺序为所选主机分别打开终端 Tab；Esc 清空。普通单击仍直接连接，分组和搜索条件改变时清空选择。

## 构建

此目录是原仓库内的独立 Cargo workspace，须放在 TermViAI 仓库根目录下。共享后端通过 path 依赖与 `#[path]` 编译原文件，没有复制一套行为实现。需要 Windows MSVC Rust 工具链及原项目的 OpenSSL/libssh 构建依赖。

```powershell
cargo build --locked --release --manifest-path termviai-gpui/Cargo.toml
cargo test --locked --release --manifest-path termviai-gpui/Cargo.toml --lib -- --test-threads=1
```

便携发布包仅含 EXE、README.txt 和 LICENSES.txt；许可证合并保留全文。源码、构建日志及验证记录放在独立的 developer 压缩包中，运行应用无需解压它。`VALIDATION.md` 记录实测与范围；`validation/source-sha256.json` 标识实际构建使用的共享源文件。当前发布版本为 v0.1.0-beta.8；目录/库名保留 termviai-gpui，公开应用名和可执行文件统一为 TermViAI。
