# TermViAI

**Terminal Via AI** — a modern Windows SSH terminal application created through human and AI collaboration.

TermViAI combines WezTerm's terminal, SSH, PTY and GPU rendering foundations with a focused host library, multi-host tab groups and selective input broadcasting. The interface uses Catppuccin Mocha and JetBrainsMono Nerd Font Mono by default.

> Current version: **v0.1.0-beta.2**. This beta targets Windows 10/11 x86_64.

## Features

- Local Hosts and Keychain management without an account.
- Multiple SSH hosts in one tab with proportional split layouts.
- Whole-group and per-terminal broadcast controls.
- Saved SSH tab groups and recent connections.
- In-place SSH reconnect with scrollback preserved, including whole-group reconnect.
- Right-side add/edit drawers, animated sidebar and broadcast toolbar.
- Per-session `Ctrl` + mouse-wheel font sizing and a persistent global font size.
- Configurable SSH keepalive interval, defaulting to 30 seconds; `0` disables it.
- A real empty Hosts start page: no hidden local terminal is created at launch.

## Download

Download **TermViAI-v0.1.0-beta.2-windows-x86_64.zip** from the [v0.1.0-beta.2 release](https://github.com/hanwlax/TermViAI/releases/tag/v0.1.0-beta.2), extract it, and run `TermViAI.exe`.

This is a portable beta build. Keep `OpenConsole.exe`, `conpty.dll`, `libEGL.dll` and `libGLESv2.dll` beside the executable.

## Basic use

1. Open **Hosts** and select **Add Host**.
2. Enter the SSH address, port, username and optional private key.
3. Open a host, then add more hosts to the same tab from the terminal controls.
4. Use the tab broadcast control for the whole group, or choose individual terminals from pane headers and the bottom toolbar.
5. Press `Ctrl+S` to save a multi-host tab group.

Settings and local host data stay on this device. On Windows the application data directory is normally `%APPDATA%\wezterm\termviai`; `TERMVIAI_DATA_DIR` can override it.

## Build

The repository is based on WezTerm and uses its Rust workspace. A Windows x64 release build can be produced with:

```powershell
cargo build --locked --release -p wezterm-gui
```

The Cargo output is still named `wezterm-gui.exe` internally; official TermViAI release packages publish it as `TermViAI.exe` with TermViAI Windows product metadata.

Development notes and validation records are under [`docs/termviai`](docs/termviai). The latest behavior is summarized in [the handover document](docs/termviai/HANDOVER.md), and the beta release notes are in [RELEASE_V0.1.0_BETA.2.md](docs/termviai/RELEASE_V0.1.0_BETA.2.md).

## Project origin and license

TermViAI is based on [WezTerm](https://github.com/wezterm/wezterm). The upstream README is preserved as [README.wezterm.md](README.wezterm.md), and the repository retains its Git history and attribution.

The project is distributed under the MIT license; see [LICENSE.md](LICENSE.md).
