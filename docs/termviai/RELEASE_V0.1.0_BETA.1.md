# TermViAI v0.1.0-beta.1

This is the first public beta of **TermViAI — Terminal Via AI**.

## Highlights

- Modern native Windows interface with Catppuccin Mocha and JetBrainsMono Nerd Font Mono.
- Local Hosts and Keychain management without user accounts.
- Multiple SSH connections in one tab with proportional, rounded split panes.
- Whole-group broadcast plus per-terminal selection and independent input.
- Saved tab groups, recent groups, recent connections and tab renaming.
- Animated add/edit drawers, collapsible sidebar and bottom broadcast toolbar.
- Global and per-session terminal font sizing.
- Configurable SSH keepalive interval; default 30 seconds, with `0` disabling it.
- Hosts page at startup and after closing the final terminal, without creating a hidden local shell.

## Package

`TermViAI-v0.1.0-beta.1-windows-x86_64.zip` is a portable Windows 10/11 x86_64 build. Extract the complete archive and launch `TermViAI.exe`; the adjacent DLL and OpenConsole files are required runtime components.

The release includes a matching `.sha256` file. Verify the archive with:

```powershell
Get-FileHash .\TermViAI-v0.1.0-beta.1-windows-x86_64.zip -Algorithm SHA256
```

## Beta boundaries

- Existing SSH sessions keep the keepalive interval they started with; reconnect to apply a changed value.
- Saved workspaces reconnect hosts and restore split layout metadata. They do not restore remote processes, terminal scrollback, passwords or previous command input.
- Real-world SSH servers, IME combinations, GPU drivers and complex vim/tmux mouse workflows can vary and remain important beta feedback areas.

## Application data

This first release uses the `termviai_*` internal modules, the `termviai_ui` configuration key, the `TERMVIAI_DATA_DIR` override and `%APPDATA%\wezterm\termviai` as its default data directory. No earlier product-name compatibility layer is included.

TermViAI is based on WezTerm and remains MIT licensed with upstream attribution preserved.
