# Termius 风格原生 SSH 客户端技术设计方案

## 1. 项目概述

开发一个面向 Windows 的轻量级原生 SSH Terminal Client。

核心目标：

- UI 和交互风格接近 Termius：
  - 简洁现代
  - 深色主题
  - Workspace / Tab / Split Pane
  - pane 顶部有轻量 Header
  - Host 管理
- 不使用 Electron。
- 不使用 Chromium / WebView 作为主终端渲染层。
- 优先控制常驻内存和多终端场景下的 CPU/GPU 开销。
- 使用 Rust。
- 基于 WezTerm 现有终端、PTY、Mux、SSH 能力进行二次开发。
- 第一版主要解决多 SSH 主机同时操作。
- 提供：
  - Tab 级总广播开关
  - pane 级广播参与/退出
  - 广播目标状态持久化
  - 快速切换广播成员

项目暂定代号：

`TermViAI`

名称后续可替换。

---

# 2. 技术路线

## 2.1 总体原则

不要重新实现以下基础设施：

- VT/ANSI terminal parser
- scrollback
- Unicode / CJK cell handling
- keyboard encoding
- mouse protocol
- terminal resize
- PTY
- Windows ConPTY
- SSH protocol
- terminal GPU renderer
- font shaping

优先复用 WezTerm。

WezTerm 当前为 Rust 实现的 GPU accelerated terminal emulator 和 multiplexer，其 workspace 包含：

- `wezterm-gui`
- `wezterm-ssh`
- `wezterm-mux-server`
- `wezterm-term`
- renderer/surface/window 等模块。

其中 `wezterm-term` 已提供：

- escape sequence parsing
- screen cell model
- scrollback
- keyboard/mouse encoding
- Sixel
- iTerm2 images
- OSC 8 hyperlinks

但本身不负责 GUI 或 PTY。

WezTerm Mux 本身已经按照：

```text
Workspace
 └── Window
      └── Tab
           └── Pane
```

管理运行中的终端。

因此第一版采用：

```text
Fork WezTerm
    │
    ├── 保留 terminal engine
    ├── 保留 PTY / ConPTY
    ├── 保留 terminal renderer
    ├── 保留 mux
    ├── 保留 SSH
    │
    └── 重构 GUI Chrome
         ├── Sidebar
         ├── Workspace
         ├── Tab Bar
         ├── Pane Header
         ├── Host Manager
         └── Broadcast Controller
```

---

# 3. 平台范围

## 第一阶段

只支持：

```text
Windows 10 / Windows 11
x86_64
```

不需要第一阶段支持：

- macOS
- Linux GUI
- ARM Windows

Windows 版本要求跟随 WezTerm。

WezTerm 当前要求至少 Windows 10 build 17763，因为 Windows 版本依赖 ConPTY。

---

# 4. 产品定位

本项目不是完整复制 Termius。

第一版定位：

> Lightweight native SSH workspace optimized for operating multiple servers simultaneously.

重点是：

```text
SSH
+
Tabs
+
Split Panes
+
Broadcast
+
Low Resource Usage
```

---

# 5. MVP 非目标

第一版不要实现：

- Cloud Sync
- Account system
- AI assistant
- SFTP GUI
- SCP GUI
- serial terminal
- telnet
- cloud vault
- team sharing
- snippets cloud
- password manager
- port-forwarding GUI
- tunnels GUI
- plugin marketplace
- mobile client
- SSH server management
- container management
- Kubernetes integration
- remote file editor

除非这些能力已经能够几乎零成本沿用 WezTerm，否则不要在 MVP 添加。

---

# 6. UI 总体布局

目标布局：

```text
┌──────────────────────────────────────────────────────────────┐
│ App     Workspace A                             Settings  × │
├──────────────┬───────────────────────────────────────────────┤
│              │ node-group-A    node-group-B     +           │
│ Hosts        ├───────────────────────────────────────────────┤
│              │ Broadcast  ● ON     3/4      All    None     │
│ Favorites    ├───────────────────────┬───────────────────────┤
│              │ ● node-01             │ ● node-02             │
│ A3           │ root@192.168.1.101    │ root@192.168.1.102    │
│  node01      │                       │                       │
│  node02      │ #                     │ #                     │
│  node03      │                       │                       │
│  node04      ├───────────────────────┼───────────────────────┤
│              │ ● node-03             │ ○ node-04             │
│ H20          │ root@192.168.1.103    │ root@192.168.1.104    │
│  node05      │                       │                       │
│              │ #                     │ #                     │
└──────────────┴───────────────────────┴───────────────────────┘
```

---

# 7. UI 视觉原则

参考 Termius 的现代桌面应用布局，但不要复制其品牌素材。

设计原则：

## 7.1 颜色

默认深色模式。

建议使用接近：

```text
Background          #111214
Sidebar             #17181B
Pane background     #0D0E10
Border              #292B30
Secondary text      #8C9098
Primary text        #E6E8EC
Accent              configurable
Success/Broadcast   accent-derived
Danger              muted red
```

实际代码中不要散落硬编码颜色。

建立：

```rust
struct Theme {
    app_bg: Color,
    sidebar_bg: Color,
    terminal_bg: Color,
    border: Color,

    text_primary: Color,
    text_secondary: Color,

    accent: Color,
    danger: Color,
}
```

---

# 8. UI 层级

主界面分成：

```text
ApplicationWindow
├── TitleBar
├── Sidebar
└── WorkspaceArea
     ├── TabBar
     ├── BroadcastBar
     └── PaneLayout
```

---

# 9. Sidebar

第一版 Sidebar 只实现：

```text
Hosts

Favorites

Groups
 ├── A3
 ├── H20
 └── Dev
```

Host 数据：

```rust
struct HostProfile {
    id: HostId,

    name: String,
    hostname: String,

    username: Option<String>,
    port: Option<u16>,

    group_id: Option<GroupId>,

    ssh_config_host: Option<String>,

    favorite: bool,
}
```

建议第一版允许 Host 直接引用：

```text
~/.ssh/config
```

例如：

```text
Host C01-B01
Host C01-B02
Host C01-B03
```

HostProfile 可以只保存：

```rust
ssh_config_host = "C01-B01"
```

避免复制复杂 SSH 配置。

---

# 10. SSH Backend

优先使用 WezTerm 当前内置 SSH backend。

WezTerm 已支持集成 SSH client，并且同一 SSH connection 创建新的 tab/pane 时，可以创建新的 channel，而不必重新认证。

同时 WezTerm 能从 `~/.ssh/config` 发现 SSH domains。

但必须保留一个设计约束：

> 不要假设 WezTerm 能完整实现 OpenSSH 所有 ssh_config 行为。

官方明确说明：

```text
wezterm ssh
```

只解析 `ssh_config` 的部分配置。

因此 architecture 要预留：

```rust
enum SshBackend {
    Embedded,
    SystemOpenSsh,
}
```

MVP 可以只实现：

```text
Embedded
```

第二阶段增加：

```text
SystemOpenSsh
```

SystemOpenSsh Windows 路径：

```text
ssh.exe
   │
ConPTY
   │
Pane
```

这样可以覆盖：

- 复杂 ProxyCommand
- ControlMaster
- 特殊 ssh_config
- 企业内部 SSH 认证
- 自定义 agent
- 第三方 OpenSSH extension

---

# 11. Workspace 数据模型

定义：

```rust
struct Workspace {
    id: WorkspaceId,
    name: String,

    tabs: Vec<TabId>,
    active_tab: TabId,
}
```

Workspace 第一版只作为 UI / layout 容器。

不要在 MVP 引入远端 daemon workspace semantics。

---

# 12. Tab 数据模型

```rust
struct TerminalTab {
    id: TabId,

    title: String,

    layout: PaneLayout,

    active_pane: PaneId,

    broadcast: BroadcastState,
}
```

---

# 13. Split Layout

不要用二维坐标直接保存 pane。

使用 tree：

```rust
enum PaneLayout {
    Leaf(PaneId),

    Split {
        direction: SplitDirection,
        ratio: f32,

        first: Box<PaneLayout>,
        second: Box<PaneLayout>,
    },
}

enum SplitDirection {
    Horizontal,
    Vertical,
}
```

例如：

```text
┌─────────┬─────────┐
│    A    │    B    │
├─────────┴────┬────┤
│      C       │ D  │
└──────────────┴────┘
```

应表示成 split tree，而不是绝对位置。

如果 WezTerm mux 已存在等价 pane tree，优先直接包装已有模型，不复制第二份 layout source of truth。

---

# 14. Pane 数据模型

应用层扩展 Pane metadata：

```rust
struct AppPaneState {
    pane_id: PaneId,

    host_id: Option<HostId>,

    title: String,

    broadcast_member: bool,

    connection_state: ConnectionState,
}
```

状态：

```rust
enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Reconnecting,
    Failed(String),
}
```

---

# 15. Broadcast 核心设计

这是项目最重要的自定义功能。

状态：

```rust
struct BroadcastState {
    enabled: bool,

    targets: HashSet<PaneId>,
}
```

Broadcast state 属于：

```text
Tab
```

而不是：

```text
App
Workspace
Window
Pane
```

即：

```text
Tab A
Broadcast ON
targets = {1,2,3}

Tab B
Broadcast OFF
targets = {5,6}
```

互不影响。

---

# 16. Broadcast 基本语义

假设：

```text
● pane1
● pane2
● pane3
○ pane4
```

其中：

```text
● = broadcast member
○ = independent pane
```

---

## 16.1 Broadcast OFF

无论 pane membership 是什么：

```text
focused pane
     │
     ▼
only focused pane
```

例如 focus pane2：

```text
keyboard
   │
   └── pane2
```

此时：

```rust
broadcast.targets
```

仍然保留。

不要清除 membership。

---

# 17. Broadcast ON + 当前 pane 是 member

例如：

```text
● pane1
● pane2
● pane3
○ pane4
```

focus 在 pane2：

```text
keyboard input
   │
   ├── pane1
   ├── pane2
   └── pane3
```

pane4 完全不接收。

---

# 18. Broadcast ON + 当前 pane 不是 member

focus：

```text
○ pane4
```

输入：

```text
keyboard
   │
   └── pane4
```

不要向：

```text
pane1
pane2
pane3
```

发送。

也就是说：

> Non-member pane behaves as an independent terminal.

这一点必须作为明确 invariant。

---

# 19. Broadcast 输入层

不要广播：

```text
command string
```

而应该广播 terminal input bytes / encoded input。

必须支持：

```text
normal chars

Enter
Tab
Backspace

Ctrl+C
Ctrl+D
Ctrl+Z

Alt+key
Ctrl+key

ESC

Arrow keys

Home
End
PageUp
PageDown

F1-F12

paste
```

原则：

> broadcast should happen after terminal keyboard processing has produced input suitable for the terminal session, but before it is written to the pane transport.

避免对不同 pane 重复进行 UI key translation。

---

# 20. Broadcast Input Router

建立独立模块：

```text
Input Event
    │
Terminal Input Encoder
    │
    ▼
InputRouter
    │
    ├── normal
    │     └── active pane
    │
    └── broadcast
          ├── member source
          │     └── all target panes
          │
          └── non-member source
                └── source pane only
```

伪代码：

```rust
fn route_input(
    tab: &TerminalTab,
    source: PaneId,
    data: &[u8],
) {
    if !tab.broadcast.enabled {
        write_to_pane(source, data);
        return;
    }

    if !tab.broadcast.targets.contains(&source) {
        write_to_pane(source, data);
        return;
    }

    for target in &tab.broadcast.targets {
        write_to_pane(*target, data);
    }
}
```

---

# 21. Broadcast 安全约束

广播操作很容易误操作，因此 UI 必须始终明确显示状态。

Broadcast ON 时：

```text
Broadcast  ● ON   3 / 4
```

应该始终可见。

不能只靠一个难以注意的图标。

当 Broadcast OFF：

```text
Broadcast  ○ OFF
```

---

# 22. Pane Header

每个 pane 显示：

```text
● node-01                     ×
```

或者：

```text
○ node-04                     ×
```

建议 Header 高度：

```text
26–32 px
```

避免占用过多 terminal area。

信息：

```text
broadcast indicator
host name
connection indicator
close
```

例如：

```text
● node01
● node02
○ node03
```

---

# 23. Pane Broadcast 操作

必须支持：

### 点击状态图标

```text
● ↔ ○
```

只修改此 pane。

---

### Ctrl + Click Pane Header

执行：

```text
toggle broadcast membership
```

即：

```text
● ↔ ○
```

---

### Broadcast Bar

```text
Broadcast ● ON    3 / 4     All    None
```

操作：

```text
ON/OFF
All
None
```

---

# 24. None 的语义

当：

```text
targets = {}
```

Broadcast ON 没有实际意义。

建议自动：

```rust
broadcast.enabled = false;
```

即点击：

```text
None
```

后：

```text
Broadcast OFF
0/4
```

---

# 25. 新建 Pane 时的 Membership

规则：

如果：

```text
Broadcast OFF
```

新 pane 默认：

```text
member = true
```

但不会实际广播。

如果：

```text
Broadcast ON
```

新 pane 默认：

```text
member = true
```

因此新拆出来的 pane 自动参与当前广播组。

后续可以在设置里提供：

```text
New panes automatically join broadcast
```

但 MVP 不需要。

---

# 26. 关闭 Pane

关闭：

```text
pane3
```

必须同时：

```rust
broadcast.targets.remove(&pane3);
```

不能留下 stale PaneId。

---

# 27. Tab 切换

每个 tab 自己保存：

```text
enabled
targets
```

例如：

```text
Tab A
● ● ● ○
Broadcast ON

Tab B
● ●
Broadcast OFF
```

切 Tab 后自动恢复对应状态。

不需要重新配置。

---

# 28. 快捷键

建议：

```text
Ctrl+Shift+I
Toggle Broadcast
```

```text
Ctrl+Shift+A
Select All Broadcast Members
```

不建议给：

```text
Select None
```

默认快捷键，避免误触导致状态变化难以意识。

Pane：

```text
Ctrl+Shift+D
Split Horizontal

Ctrl+Shift+E
Split Vertical
```

具体按键后续允许配置。

---

# 29. Mouse Interaction

Pane Header：

```text
Single click
→ focus pane

Ctrl + Click
→ toggle broadcast membership

Middle click
→ close pane
```

不要让普通 click 改变 broadcast membership。

---

# 30. Paste 行为

Paste 应当视为普通 terminal input。

即：

```text
Broadcast ON
● ● ●
```

执行：

```text
Ctrl+V
```

应发送给全部成员。

但必须避免：

```text
Paste event
+
keyboard event
```

导致重复写入。

---

# 31. IME

中文输入必须特别验证。

不要根据 GUI keyboard events 对每个 pane 独立重建 IME 输入。

理想路径：

```text
IME
 ↓
committed text
 ↓
terminal encoding
 ↓
InputRouter
 ↓
target panes
```

---

# 32. Resize

Broadcast 与 terminal resize 完全解耦。

每个 pane 保持自己的：

```text
rows
cols
```

即使多个广播 pane 大小不同，也不要尝试强制同步 terminal geometry。

命令广播 ≠ terminal size broadcast。

---

# 33. Pane Layout Persistence

应用退出时保存：

```text
workspace
tabs
layout tree
pane host bindings
broadcast membership
broadcast enabled
```

但不要默认恢复已经断开的真实 SSH transport 对象。

恢复时重新建立 SSH。

---

# 34. Session Restore

配置：

```rust
struct PersistedWorkspace {
    tabs: Vec<PersistedTab>,
}

struct PersistedTab {
    title: String,

    layout: PersistedLayout,

    panes: Vec<PersistedPane>,

    broadcast_enabled: bool,

    broadcast_members: Vec<PersistedPaneId>,
}
```

PersistedPaneId 不应该直接使用 runtime PaneId。

需要：

```text
PersistentPaneId
        ↓ startup mapping
Runtime PaneId
```

---

# 35. 配置存储

第一版建议：

```text
JSON / TOML
```

不要上数据库。

例如：

```text
%APPDATA%\TermViAI\
├── config.toml
├── hosts.json
└── workspace.json
```

不要把 private SSH key 内容复制进去。

只存 key path / ssh config host reference。

---

# 36. Host Manager

交互：

```text
Host
├── Name
├── Address
├── Username
├── Port
└── SSH Config Alias
```

如果：

```text
SSH Config Alias
```

存在，则优先使用 alias。

例如：

```text
Name:
A3 node 01

SSH Config Alias:
C01-A01
```

启动：

```text
C01-A01
```

而不是重新拼：

```text
root@192.168.x.x
```

---

# 37. Host Group

```rust
struct HostGroup {
    id: GroupId,
    name: String,
}
```

例如：

```text
A3
├── 133-C01-B01
├── 133-C01-B02
├── 133-C01-B03
└── 133-C01-B04

H20
├── ...
```

拖拽 Host 到 workspace 第一版不是必须。

支持：

```text
double click
```

连接即可。

---

# 38. 快速批量连接

第一版可以增加一个很有价值的操作：

右键 Group：

```text
Open All
Open All in Grid
```

例如四台机器：

```text
Open All in Grid
```

自动生成：

```text
2 × 2
```

并：

```text
Broadcast OFF
targets = all panes
```

用户随后只要：

```text
Ctrl+Shift+I
```

即可开始广播。

---

# 39. Grid 生成规则

```text
1 → 1x1
2 → 1x2
3 → 2x2 with one empty slot avoided
4 → 2x2
5–6 → 2x3
7–9 → 3x3
```

更复杂情况后续处理。

---

# 40. Terminal Engine

尽量保持 WezTerm 原有 terminal stack 不变。

不要为了 UI 重构而修改：

```text
wezterm-term
escape parsing
cell storage
scrollback
font shaping
terminal protocol handling
```

终端核心属于高风险区域。

只有当功能直接需要时才修改。

---

# 41. Mux 集成

优先让 WezTerm mux 继续作为 pane lifecycle source of truth。

WezTerm mux 当前负责管理：

```text
programs
panes
tabs
windows
workspaces
```

并且 mux 可以独立于 GUI 存在。

应用层增加：

```text
AppTabMetadata
AppPaneMetadata
BroadcastState
```

不要建立第二套真正的 PTY/process hierarchy。

---

# 42. 推荐模块结构

建议新增：

```text
termviai/
├── app/
│   ├── app_state.rs
│   ├── workspace.rs
│   └── commands.rs
│
├── ui/
│   ├── title_bar.rs
│   ├── sidebar.rs
│   ├── tab_bar.rs
│   ├── broadcast_bar.rs
│   ├── pane_header.rs
│   └── host_manager.rs
│
├── broadcast/
│   ├── mod.rs
│   ├── state.rs
│   └── input_router.rs
│
├── hosts/
│   ├── model.rs
│   ├── repository.rs
│   └── ssh_config.rs
│
├── persistence/
│   ├── config.rs
│   ├── workspace.rs
│   └── hosts.rs
│
└── ssh/
    ├── embedded.rs
    └── system.rs
```

实际可以嵌入 WezTerm workspace，不要求创建独立 top-level crate，但逻辑边界保持一致。

---

# 43. App State

建议：

```rust
struct AppState {
    workspaces: HashMap<WorkspaceId, Workspace>,

    active_workspace: WorkspaceId,

    hosts: HostRepository,

    theme: Theme,

    settings: Settings,
}
```

不要把：

```text
terminal screen content
scrollback
PTY state
SSH transport internals
```

放入 AppState。

这些继续归 WezTerm。

---

# 44. UI Rendering

不要引入：

```text
Electron
WebView
React
HTML
CSS
```

主 terminal window 保持原生 Rust GUI/render pipeline。

第一版优先修改现有 WezTerm GUI。

不要第一版尝试：

```text
Slint + WezTerm renderer embedding
```

因为这会同时引入：

- GPU surface ownership
- window event routing
- DPI
- IME
- mouse
- keyboard
- clipping
- terminal renderer embedding

等额外复杂度。

---

# 45. 性能目标

这不是严格 benchmark SLA，但开发过程中应该作为参考。

目标机器：

```text
Windows 11
16 GB RAM
```

场景：

```text
1 application
4 tabs
4 panes/tab
= 16 active SSH panes
```

目标：

```text
Idle CPU:
接近 0%，避免持续轮询

UI:
切换 tab / focus 不产生明显延迟

Typing:
广播到 16 pane 时无明显输入延迟

Memory:
显著低于典型 Electron SSH client
```

不要为了动画持续 60 FPS redraw。

没有内容更新时应尽可能 idle。

---

# 46. UI 动画

动画尽量少。

允许：

```text
hover
button transition
sidebar selection
broadcast indicator
```

不要做：

```text
continuous blur
large transparency
animated background
constant GPU redraw
```

现代感主要通过：

```text
spacing
typography
border
contrast
iconography
layout
```

实现。

---

# 47. Broadcast 性能

广播时不要为每一个 pane重新构造 input。

应该：

```text
encode once
↓
clone/share bytes
↓
write to N panes
```

概念：

```rust
let data = Arc::<[u8]>::from(data);

for pane in targets {
    pane.write(data.clone());
}
```

具体 API 根据 WezTerm transport 实现调整。

---

# 48. Broadcast Failure Isolation

如果：

```text
pane1 connected
pane2 connected
pane3 disconnected
```

广播：

```text
ls
```

不应因为 pane3 write failure 导致：

```text
pane1
pane2
```

发送失败。

每个 target 独立处理。

概念：

```rust
for pane in targets {
    if let Err(error) = write_to_pane(pane, data) {
        mark_write_failure(pane, error);
    }
}
```

---

# 49. Disconnect 行为

如果广播 member 断开：

```text
● node03 DISCONNECTED
```

建议保留其 membership：

```text
●
```

但不尝试发送。

如果 reconnect 成功：

自动重新参与 broadcast。

这样网络抖动不会改变操作组。

---

# 50. Reconnect

MVP：

允许手动：

```text
Reconnect
```

不要第一版自动无限重连。

第二阶段可以提供：

```text
Auto reconnect
```

WezTerm 官方说明普通 integrated SSH session 在网络中断后并不持久，相关 tabs 会失效，因此这部分不能假设底层天然具有 Termius 式自动重连。

---

# 51. Terminal Context Menu

右键 pane：

```text
Split Right
Split Down

Reconnect
Close

Join Broadcast
Leave Broadcast

Copy
Paste
```

如果：

```text
broadcast_member = true
```

显示：

```text
Leave Broadcast
```

否则：

```text
Join Broadcast
```

---

# 52. Close Tab 保护

如果一个 Tab 有：

```text
active foreground job
```

可以沿用 WezTerm 当前 close confirmation 行为。

Broadcast 本身不增加额外确认。

不要每次关闭 SSH 都弹大量提示。

---

# 53. Broadcast 高风险命令

MVP 不做命令内容识别。

不要试图检测：

```text
rm
reboot
shutdown
kill
```

并弹确认。

因为 terminal 输入不是可靠 command AST：

```text
shell
vim
python
tmux
sudo
interactive program
```

都会导致错误判断。

安全应该通过明确 Broadcast UI 状态解决，而不是命令检查。

---

# 54. 测试重点

## Unit Tests

### Broadcast

```text
OFF:
source A → A only
```

```text
ON:
A member
targets A,B,C
→ A,B,C
```

```text
ON:
D non-member
→ D only
```

```text
close B
→ target set removes B
```

```text
switch tab
→ independent BroadcastState
```

---

# 55. Integration Tests

创建：

```text
Pane A → local shell
Pane B → local shell
Pane C → local shell
Pane D → local shell
```

设置：

```text
● A
● B
● C
○ D
```

输入：

```text
echo test
```

验证：

```text
A/B/C receive
D does not
```

focus D：

```text
echo independent
```

验证：

```text
only D receives
```

---

# 56. 必测键盘行为

逐一测试：

```text
characters
Enter
Backspace
Tab
Ctrl+C
Ctrl+D
Ctrl+Z
Ctrl+L
ESC
Alt
Arrow
Home
End
PageUp
PageDown
F keys
paste
IME
```

---

# 57. 必测程序

至少：

```text
bash
zsh
vim
tmux
top
less
python REPL
ssh nested SSH
```

Windows local pane：

```text
PowerShell
cmd
```

---

# 58. SSH 测试

至少覆盖：

```text
password auth

public key auth

encrypted private key

ssh-agent

non-22 port

ssh_config alias

ProxyCommand

disconnect

reconnect
```

ProxyCommand 如果 embedded backend 当前不支持对应配置，应明确暴露错误，而不是静默忽略。

---

# 59. 性能测试

自动打开：

```text
4
8
16
32
```

个 pane。

测试：

```text
idle RSS
idle CPU
typing latency
broadcast latency
scroll throughput
tab switching
pane resize
```

记录 baseline：

```text
upstream WezTerm
```

以及：

```text
our fork
```

任何 UI 重构都不应该导致 idle resource 明显恶化。

---

# 60. 开发阶段

## Phase 0 — Upstream Baseline

目标：

成功构建原始 WezTerm。

完成：

```text
clone
build
run
SSH
split pane
Windows ConPTY
```

不要先改 UI。

---

## Phase 1 — Broadcast Core

完全不改大 UI。

只增加：

```text
BroadcastState
InputRouter
Pane membership
```

用最简单 indicator 验证。

验收：

```text
4 pane
3 broadcast
1 independent
```

完全正确。

这是整个项目第一优先级。

---

## Phase 2 — Pane Header

加入：

```text
●/○
hostname
close
```

实现：

```text
Ctrl+Click
```

toggle member。

---

## Phase 3 — Broadcast Bar

实现：

```text
Broadcast ON/OFF
3/4
All
None
```

完成 per-tab state。

---

## Phase 4 — Modern Tab Bar

重新设计：

```text
tabs
new tab
close
active state
```

不要碰 terminal renderer。

---

## Phase 5 — Sidebar / Hosts

实现：

```text
Host repository
Groups
Favorites
Connect
```

支持读取已有 SSH config aliases。

---

## Phase 6 — Workspace Restore

实现：

```text
tabs
pane tree
hosts
broadcast state
```

恢复。

---

## Phase 7 — UI Polish

统一：

```text
spacing
fonts
icon set
theme
hover
borders
context menu
```

目标是现代 SSH IDE 风格，而不是 WezTerm 默认 terminal utility 风格。

---

## Phase 8 — System OpenSSH Backend

如实际企业环境证明 embedded SSH 无法完整覆盖，再实现：

```text
ssh.exe
+
ConPTY
```

不要提前做。

---

# 61. 第一版验收标准

满足以下条件即可认为 MVP 完成。

### Terminal

- 能正常 SSH 到远端 Linux。
- vim/tmux/top 正常。
- UTF-8/CJK 正常。
- resize 正常。
- clipboard 正常。

### Layout

- 支持 tab。
- 支持 horizontal split。
- 支持 vertical split。
- pane resize 正常。

### Broadcast

一个 Tab 内：

```text
4 panes

● A
● B
● C
○ D
```

开启 Broadcast 后：

- A 输入 → A/B/C
- B 输入 → A/B/C
- C 输入 → A/B/C
- D 输入 → D only

关闭 Broadcast：

- 任意 pane 输入只进入自身。

重新打开：

```text
A/B/C membership
```

自动恢复。

### Persistence

重启应用之后：

- Host 配置存在。
- Tab layout 恢复。
- Broadcast membership 恢复。

---

# 62. Agent 开发约束

Coding agent 必须遵循以下规则。

## Rule 1

不要重新实现 terminal emulator。

---

## Rule 2

不要为了 UI 引入 Electron。

---

## Rule 3

不要引入 WebView。

---

## Rule 4

尽量保持：

```text
wezterm-term
PTY
renderer
font shaping
SSH
```

不变。

---

## Rule 5

优先在 GUI/application layer 新增功能。

---

## Rule 6

Broadcast 必须是：

```text
raw terminal input routing
```

而不是：

```text
command text sending
```

---

## Rule 7

非 broadcast member pane 必须完全独立。

---

## Rule 8

Broadcast OFF 不得清除成员集合。

---

## Rule 9

Broadcast state 是 per-tab。

---

## Rule 10

任何 UI 改动不能破坏：

```text
IME
keyboard encoding
mouse protocol
terminal resizing
clipboard
```

---

# 63. Agent 首轮任务

第一轮不要做完整 UI。

Agent 首先执行：

```text
1. Fork / clone upstream WezTerm.

2. 在 Windows 成功编译并运行。

3. 定位：
   - pane abstraction
   - tab abstraction
   - mux pane lookup
   - GUI keyboard input path
   - 最终 pane input write path

4. 写一份简短 architecture notes：
   - keyboard input 从 GUI 到 Pane 的实际调用链
   - 哪个模块最适合作为 Broadcast InputRouter 插入点
   - PaneId 和 TabId 的实际类型
   - pane close 生命周期
   - tab close 生命周期

5. 不修改 terminal parser。

6. 不修改 renderer。

7. 实现最小 BroadcastState：

   struct BroadcastState {
       enabled: bool,
       targets: HashSet<PaneId>,
   }

8. 先通过硬编码或 debug shortcut 测试：
   4 panes
   3 targets
   1 independent

9. 验证：
   Ctrl+C
   Arrow
   Tab
   paste
   vim

10. Broadcast core 验证通过后，再进入 UI 开发。
```

---

# 64. Agent 不应提前做的事情

在 Broadcast Core 完成前，不要：

```text
重写 sidebar
开发 Host Manager
设计数据库
开发 cloud sync
写 SFTP
重构 entire WezTerm GUI
替换 renderer
替换 SSH library
加入 AI
重做 configuration subsystem
```

先证明：

```text
selected-pane real-time broadcast
```

能够稳定工作。

---

# 65. 最终目标

最终产品体验应该是：

```text
打开 Workspace
      ↓
四台机器已经恢复
      ↓
● ● ● ●
      ↓
Ctrl+Shift+I
      ↓
同时操作四台
      ↓
node04 需要特殊操作
      ↓
Ctrl+Click node04
      ↓
● ● ● ○
      ↓
直接在 node04 单独操作
      ↓
Ctrl+Click node04
      ↓
● ● ● ●
      ↓
继续广播
```

整个过程中：

- 不需要重新选择颜色。
- 不需要打开 MultiExec 配置窗口。
- 不需要切换 broadcast group。
- 不需要重新建立 pane。
- 不需要频繁开关全局广播。
- 不会因为独立操作某个 pane 而污染其他 pane。

核心产品价值即：

> **Termius-like UX, native resource usage, and first-class selective terminal broadcasting.**

---

# 66. 实现优先级总结

```text
P0
├── WezTerm fork builds
├── terminal unchanged
├── SSH works
├── split works
└── selective broadcast works

P1
├── pane header
├── broadcast indicator
├── global broadcast toggle
├── all / none
└── per-tab state

P2
├── modern tab bar
├── host sidebar
├── host groups
├── favorites
└── layout persistence

P3
├── reconnect
├── system OpenSSH backend
├── advanced settings
└── UI polish

P4
├── SFTP
├── forwarding GUI
├── snippets
└── other convenience features
```

开发过程中始终优先：

```text
correct terminal behavior
>
broadcast correctness
>
resource efficiency
>
UI appearance
>
additional features
```
