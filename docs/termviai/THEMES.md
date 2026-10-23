# Appearance and terminal workspace

Beta 6 adds the three approved palettes throughout Hosts, Keychain,
New Tab, dialogs, Settings, tab chrome, and terminal panes:

| Settings choice | Surfaces | Accent |
| --- | --- | --- |
| 墨黑 · 鸢尾紫 (Ink) | Neutral ink black | Iris purple |
| 深海 · 冰蓝 (Navy) | Deep navy | Ice blue |
| 瓷白 · 鸢尾紫 (Light) | Porcelain white | Iris purple |

Open **Settings → Appearance** and select a preview card. The current window
previews immediately. **Save** writes `theme` (`ink`, `navy`, or `light`) to the
existing `settings.json` and applies the preference to other application
windows. **Cancel**, Escape, or dismissing the drawer restores the saved theme.
An open preview in another window remains local until it is saved or dismissed.
Existing global font size and SSH keepalive preferences remain in the same
scrollable Settings drawer. Keepalive changes apply to new SSH connections.
Save and Cancel have the same neutral background at rest.

Missing or unrecognized theme names use Ink. Other settings are retained;
a malformed settings file is not overwritten. Terminal defaults and the first
16 ANSI colors follow the theme, including darker ANSI colors in Light mode.
Explicit per-pane palette overrides and extended indexed colors are retained;
application-supplied truecolor text is not recolored.

## Workspace chrome

- The sidebar remains collapsible and is now 176 logical pixels wide.
- Terminal groups start with the broadcast panel collapsed. A 34-pixel bottom
  strip shows the active participant count and connection state. Clicking
  **Broadcast** expands a 52-pixel member strip; clicking again collapses it.
- The member strip supports individual selection and **Select all / Clear all**.
  Members follow screen reading order (top to bottom, left to right within each
  row), updating after pane rearrangement. Long groups page through members at
  narrow widths. Selecting a gray participant
  activates only that selection, without resuming stale paused members.
- A single terminal displays its connection summary in the compact status strip;
  it does not reserve space for expanded broadcast members.
- **⋯** on each terminal Tab opens Rename tab, Save workspace, Reconnect offline
  (when applicable), and Close tab. Existing Ctrl+S and right-click Tab access
  remain available. Offline groups keep separate broadcast and reconnect icons.
- Broadcast members have no resting colored pill background. The tray and compact
  status strip share the canvas surface without duplicate horizontal borders.
- The compact status strip also offers reconnect when an SSH terminal is offline.
  Reconnect only replaces offline SSH sessions, preserving online sessions.
- Active Tabs and pane headers use a thin theme-colored indicator. Pane gutters
  are single 1-pixel shared dividers inside one outer grid border; header backgrounds
  and active indicators align with that grid. Terminal titles use the terminal icon.
  X controls stay transparent and turn red on hover. Nested controls retain the
  container hover and use a distinct hover background.

Terminal PTY dimensions, rendering, mouse coordinates and IME use the same
animated reserved height. ClientDomain/tmux layouts retain their existing
remote-layout handling. Theme preview does not reconnect or rewrite terminal
content. Broadcast collapse is session UI state, not a persisted preference.

## Release acceptance

Beta 6 includes Recent connections, Keychain editing and footer-spacing fixes.
Extract the entire archive into a new directory and run `TermViAI.exe` next to
the bundled DLLs. See [release validation](RELEASE_V0.1.0_BETA.6_VALIDATION.md)
for Windows automated test and package verification results.

Native visual checks still needed on the target Windows display:

1. Preview all three themes in Settings, cancel, save, and restart. Verify Hosts,
   Keychain, New Tab, add/edit drawers and confirmation dialogs in each palette.
2. In a four-terminal group, expand/collapse Broadcast; check the bottom terminal
   row, mouse selection and IME at 100%, 125% and 150% DPI.
3. Switch between single/group Tabs with the panel expanded. Verify space is
   reserved only where needed, and no terminal output crosses the status strip.
4. Check Tab menu placement, nested icon hover, offline reconnect, all/partial
   broadcast selection and long member lists. Use non-sensitive test commands.
