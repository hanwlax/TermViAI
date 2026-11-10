# TermViAI v0.1.0-beta.8

面向 Windows 10/11 x86_64 的第八个 Beta。应用、窗口标题和可执行文件统一使用 **TermViAI**，不再带 `-GPUI` 后缀。TermViAI（Terminal Via AI）表示由 AI 协作制作的终端应用，当前没有内置 AI 功能。

## 本次更新

- **原生界面**：使用 Rust GPUI，复用原有终端内核、SSH、PTY 和存储数据；不使用 WebView 或 Electron。保留 Ink / Navy / Light 三套主题、Hosts、Keychain、Monitor、Port Forwarding 和 Settings 的 TermViAI 交互。
- **多主机与标签**：Hosts 支持 Ctrl+单击多选、Enter 分别打开终端标签。标签溢出时提供左右滚动、可搜索的 All tabs、当前标签自动定位和 Ctrl+Tab / Ctrl+Shift+Tab 切换；搜索框聚焦不显示主题色外框。
- **终端组拖放**：组合后自动命名，允许未保存 Workspace 的标签直接改名。支持组内终端拖出、拖放重组、清晰的前后落位标识，无位置变化时隐藏标识；整组标签只能排序，不能整组嵌入其他终端。拖到标签栏边缘可自动滚动。
- **广播与滚动**：紧凑广播成员栏默认收起，成员顺序与终端空间顺序一致，支持全选/部分选择。终端滚动条按需出现，悬浮和拖动时显示手状指针。
- **输入与窗口**：修正终端 Tab 补全路由、Windows 输入法切换后的候选串残留与回车被拦截，以及标题栏拖动、窗口控制和抽屉点击处理。
- **Monitor**：恢复紧凑主机/NPU/Die 详情交互、工作目录和进程控制确认；添加主机时排除已添加项，失效引用可显式替换，采样间隔集中为 2 / 5 / 10 秒菜单。空状态、错误提示和表单校验统一样式。
- **Port Forwarding 与认证**：可从保存的 Hosts 创建反向端口转发，支持应用拥有的隧道恢复；SSH 按 Keychain → Identity → 手动密码尝试，全部失败后在终端内选择认证方式，成功只回写对应项。

## 下载与升级

下载 `TermViAI-v0.1.0-beta.8-windows-x86_64.zip`，解压到新文件夹，运行 **TermViAI.exe**。

便携包只有三个文件：`TermViAI.exe`、`README.txt`、`LICENSES.txt`。不需要复制旧版旁边的 OpenConsole、ConPTY 或 EGL/GLES DLL；请勿用旧目录中的残留文件覆盖新包。

继续读取 `%APPDATA%\wezterm\termviai`（或 `TERMVIAI_DATA_DIR`）中的已有 Hosts、Keychain 引用、工作区和设置。使用同一份数据时请关闭旧版，尤其不要同时运行两份 Port Forwarding 管理器。也可用 `TermViAI.exe --data-dir C:\Temp\TermViAI-Test` 指定独立数据目录。

## 验证与边界

本轮 Windows 原生 Release、库回归、Python 采集器及发布包验证见[验证记录](https://github.com/hanwlax/TermViAI/blob/v0.1.0-beta.8/docs/termviai/RELEASE_V0.1.0_BETA.8_VALIDATION.md)。本轮未操作用户桌面，未在真实 SSH/NPU 主机执行进程控制；真实输入法、不同 DPI/GPU、拖动手感及长时间连接仍需持续试用。

Monitor 当前面向 Linux + Python + npu-smi 的 Ascend NPU 环境，不包含 NVIDIA 采集。原生界面尚未迁移终端内图片协议绘制、旧 Lua GUI 自定义和完整无障碍支持；旧前端源代码仍保留在仓库内。
