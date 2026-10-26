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

Editable fields and search boxes use a dedicated text-selection color for each
theme. Focused inputs keep their background stable while hovering or dragging
to select, so the selection remains distinct. Selection/text contrast is at
least 4.5:1, and selection/field contrast is at least 3:1 in all three themes.

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
- Drag a pane header toward the left, right, top or bottom of another pane to
  dock it there, using the same half-pane preview and placement as an incoming
  single-session Tab. Its old split closes and the destination splits again;
  repeated moves can build nested layouts without reconnecting sessions.
  Dropping on itself or outside a valid target does nothing; Escape cancels.
  Restore a zoomed pane to the group view before rearranging it.
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

## UI typography (2026-10-24)

TermViAI chrome now uses the same Windows proportional stack as the approved HTML UX: Segoe UI, Microsoft YaHei, then Roboto and configured fallback fonts. The default UI size is 14 logical pixels, regular weight 400; headings and emphasized information use semibold 600. Monitor retains its explicit 11–24 px hierarchy. Breadcrumb and tooltip measurements use the same font as painting. An explicit `window_frame.font` still overrides the UI family; terminal output keeps its configured monospace font, size and shaping.

This corrects a verified family/weight mismatch, not a proven sharpening filter defect. Browser and native rasterization can still differ; check the three themes at the device's actual DPI.

## Windows UI rasterization (2026-10-24)

The default Windows UI font now uses DirectWrite grayscale glyph analysis with natural measuring, symmetric natural rendering and grid fitting. Masks are produced at the requested physical DPI and uploaded without bitmap resizing; the existing proportional shaper still determines advances. Every TermViAI tile uses physical-pixel glyph origins, including default-size Hosts/Keychain text that previously missed the Monitor-only opt-in.

Only UI font objects opt into this path. Terminal fonts and other title/palette consumers retain the configured rasterizer. Explicit window-frame font overrides retain their previous rasterizer, as do embedded faces, named variable instances, color/bitmap faces and per-face raster settings. DirectWrite setup or glyph failures fall back to the already configured rasterizer. Grayscale output works with existing OpenGL/WebGPU text masks, without assuming an RGB panel order.

The comparison artifact uses actual glyph masks with an offline compositor; it is not a screenshot of the application or proof that browser/native text looks identical.
