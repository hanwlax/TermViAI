# TermViAI

**Terminal Via AI** — a modern Windows SSH terminal application created through human and AI collaboration.

TermViAI combines WezTerm's terminal, SSH and PTY foundations with a native Rust GPUI interface, a focused host library, multi-host tab groups and selective input broadcasting. Settings offers Ink, Navy and Light themes across the interface and terminal; the default font is JetBrainsMono Nerd Font Mono.

> Current version: **v0.1.0-beta.10**. This beta targets Windows 10/11 x86_64.

## Features

- Native GPUI interface with shared terminal/SSH engines, without WebView or Electron.
- Ctrl+click to select hosts and press Enter to open them as panes in one tab.
- Overflow tab navigation with scrolling, searchable All tabs, automatic active-tab reveal and Ctrl+Tab cycling.
- Nested Host groups with breadcrumbs, host moves and safe group deletion.
- Exclusive Password / SSH Key / Identity authentication, with passwords saved in Windows Credential Manager.
- Multiple SSH hosts in one tab with proportional split layouts.
- Drag tabs along the tab bar, or move pane headers to create new splits with a full layout preview.
- Whole-group and per-terminal broadcast controls, with a collapsed-by-default member panel in screen order.
- Three switchable themes with instant preview and persistent Settings.
- Tab menus for workspace actions; Ctrl + click to open terminal links.
- Editable Keychain keys and identities, with consistent delete controls.
- Save and restore tab groups from New Tab, reorder groups in the sidebar, and remove individual recent connections.
- NPU Monitor for existing Linux hosts: card/Die metrics, process details and guarded process termination.
- Port Forwarding waits for a network path and automatically reconnects eligible tunnels. Manually stopped services stay stopped.
- Known Hosts stores trusted SSH fingerprints per host and port. Confirming a key resumes eligible Port Forwarding and Monitor connections.
- Inline SSH authentication recovery with Keychain → Identity → manual fallback.
- Configurable tab-close confirmation, enabled by default.
- In-place SSH reconnect with scrollback preserved, including whole-group reconnect.
- Edit keys and identities in their cards, and find or create a group while editing a host.
- Per-session `Ctrl` + mouse-wheel font sizing and a persistent global font size.
- Configurable SSH keepalive interval, defaulting to 30 seconds; `0` disables it.
- Launch into New Tab with the search field focused. Use Local terminal or Quick Connect to start a session.

## Download

Download **TermViAI-v0.1.0-beta.10-windows-x86_64.zip** from the [v0.1.0-beta.10 release](https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.10), extract it, and run `TermViAI.exe`.

This portable package contains `TermViAI.exe`, `README.txt` and `LICENSES.txt`. Extract into a new folder; no WebView or separate Chromium runtime is required. Existing Hosts, Keychain references and workspaces are reused.

## Basic use

1. Open **Hosts**, open the **Add** menu, and select **Add Host**.
2. Enter the address and port, then choose **Password**, **SSH Key**, or **Identity**. Passwords are saved with the Host; SSH keys support a file path or Keychain selection.
3. Open hosts in separate tabs, then drag a tab into another terminal to combine them.
4. Use the tab broadcast control for the whole group, or choose individual terminals from pane headers and the bottom toolbar.
5. Save a workspace from the tab menu. Right-click a sidebar tab group to save its set of tabs.

Settings and local host data stay on this device. On Windows the application data directory is normally `%APPDATA%\wezterm\termviai`; `TERMVIAI_DATA_DIR` can override it.

SSH connection diagnostics are enabled automatically in the native app and saved
under that data directory's `logs` folder (`ssh-<process>-<start-time>.jsonl`).
Each JSON line includes a UTC timestamp, connection ID, local/peer IP and port,
the underlying channel/transport error, and keepalive interval, attempts, send
results and last-send times. `terminal_ended` records when the terminal observes
EOF, an error, cancellation or process exit; correlate its connection ID with
`channel_read_closed` and `transport_error` for the underlying cause. Channel
read/write ages measure terminal payload activity, not TCP ACKs. IGNORE keepalive
send success does **not** establish that the server replied. Proxy connections
may expose proxy/socketpair endpoints instead of the final SSH server.
Terminal contents, commands, passwords and keys are not recorded. Logs rotate at
4 MiB with three backups per app process; logs untouched for seven days are
removed when a new process log is created. Logging failures do not disconnect
sessions. Logs begin after starting the updated app; they cannot recover earlier
disconnects.

## Build

The repository is based on WezTerm and uses its Rust workspace. A Windows x64 release build can be produced with:

```powershell
cargo build --locked --release --manifest-path termviai-gpui/Cargo.toml
```

The Cargo output is `termviai-gpui/target/release/TermViAI.exe`, with TermViAI Windows product metadata. The internal crate/directory name remains `termviai-gpui`; the older frontend remains in the repository for reference.

CI builds the Windows x86_64 GUI and runs broadcast policy, SSH, terminal core, window, mux, and GUI tests on relevant pushes and pull requests to `main`. Rust formatting and the Linux remote collector tests are checked separately. Windows CI can also be started manually; scheduled upstream builds and publishing workflows are archived under [`ci/upstream-workflows`](ci/upstream-workflows).

Development notes and validation records are under [`docs/termviai`](docs/termviai). The latest behavior is summarized in [the handover document](docs/termviai/HANDOVER.md), and the beta release notes are in [RELEASE_V0.1.0_BETA.10.md](docs/termviai/RELEASE_V0.1.0_BETA.10.md).

## Project origin and license

TermViAI is based on [WezTerm](https://github.com/wezterm/wezterm). The upstream README is preserved as [README.wezterm.md](README.wezterm.md), and the repository retains its Git history and attribution.

The project is distributed under the MIT license; see [LICENSE.md](LICENSE.md).
