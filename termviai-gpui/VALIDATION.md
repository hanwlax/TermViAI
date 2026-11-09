# Native GPUI trial validation · 2026-11-09

## Architecture and scope

The production terminal core, SSH libraries and PTY implementation are path dependencies. Hosts, workspaces, authentication control, Monitor and Forwarding compile directly from their existing source files. Native GPUI elements own layout, input focus, painting and application navigation. This is a separate trial executable; the production executable was not replaced.

The previous probe's vt100 parser, example host library and navigation were not adopted. Resolved Cargo metadata contains no WebView, Wry, Tauri or vt100 package. The PE import table contains Windows system libraries; no Chromium runtime is bundled.

## Terminal Tab routing · 2026-11-09

- GPUI Component binds Tab/Shift+Tab to Root focus navigation. GPUI dispatches matching actions before raw key listeners, so the terminal's existing Tab encoder did not receive those keys.
- Add a terminal-only key context with NoAction overrides for those two bindings. The actual keystrokes then pass through the existing key listener, production terminal encoder and broadcast input route. Form fields keep Root navigation; no global key bindings or remote shell settings are removed.
- Regression checks exercise GPUI's real keymap matching with terminal versus form contexts, plus production terminal encoding of Tab (`09`) and Shift+Tab (`1b 5b 5a`). These checks run without a visible window; no desktop automation or real remote shell completion test is performed.
- Executed: Windows MSVC Release build succeeded; library tests report 69 passed, 0 failed, 3 ignored, including both new Tab regression cases.

## Monitor interaction parity · 2026-11-09

- Compared the user's side-by-side screenshots with the approved Monitor mockup and legacy `termviai_monitor.rs`. Restored compact host summaries, status counts, interval selection, host menu/pause/retry/remove confirmation, breadcrumb navigation, and one tile per NPU containing its actual Die IDs.
- Clicking an NPU opens a right-hand drawer, preserving the host grid behind it. All/Die filters, scoped HBM/AICore totals and bulk actions stay above the independently scrolling process list. Hover shows a read-only preview, and closing/Escape returns to the same host. Changing tabs preserves selection.
- Processes are displayed once per PID, with memory summed only across the selected scope; locations still show all occupied Dies. Working directories and full commands are retained. Missing metrics stay unavailable rather than becoming partial totals or zero. Process actions require fresh data and verified control tokens and retain existing confirmation/backend validation.
- Background library regression suite: 67 passed, 0 failed, 3 ignored. New tests cover incomplete metrics and scoped cross-Die aggregation. The final UI-only fixed-header adjustment is release-built afterward. No new remote collection/process termination or desktop interaction was performed; visual/pointer parity is source-reviewed, not screenshot-verified.

## Drawer click and packaging follow-up · 2026-11-09

- The dismissal shade now occupies only the space outside the right-hand drawer. The drawer panel and shade have separate occluding hitboxes, preventing clicks/scrolls from reaching the page behind them. Settings and the other shared editor drawers retain normal input, preview, Save, Cancel and outside-dismiss behavior.
- The runtime ZIP contains exactly three files: executable, short README, and consolidated license texts. Source, build logs and validation manifests are delivered separately in the developer ZIP. Existing trial archives are retained.
- Background Windows Release compilation and ZIP integrity/source checks are performed for this change. No desktop interaction or native-window launch is used; the final pointer/focus behavior is not manually verified. Earlier 65-test results are baseline evidence, not a new UI test.

## Titlebar follow-up · 2026-11-09

- Window-controls follow-up: minimize, maximize/restore and close now have explicit GPUI click handlers on Windows. The first two post native system commands; close posts `WM_CLOSE`, preserving the existing close-confirmation callback. Windows non-client control-area markers are removed from these buttons to prevent competing event paths. No broadcast-panel redesign is included in this build.
- Drag follow-up: the title text and blank spacer now handle left presses directly on Windows and post `WM_SYSCOMMAND` for native move or double-click maximize/restore. This avoids relying on GPUI's cached non-client hit test for those two regions. The event is not attached to tabs or buttons. Posting avoids reentering GPUI while its input callback is active.
- This follow-up is compiled in the background; native drag/snap/double-click gestures remain manually unverified to avoid interacting with the user's desktop.
- Unified the app title, tabs and native Windows control hit areas in one 52px bar. Regular tabs use the legacy visible width of 242px; the new-tab button follows the tab strip.
- Embedded a circular reconnect SVG. Close icons use explicit SVG colors and group hover so the whole button hit area turns the glyph red without a background.
- Windows MSVC release compilation is rerun for this update. The baseline tests and screenshots below precede this titlebar update; they are not a new visual verification of the final titlebar.
- Final hover, minimize/maximize/restore and resize gestures have not been manually retested. No computer-use automation or window activation is used for final verification, to avoid interrupting the user's work.

## Compact broadcast follow-up · 2026-11-09

- Implements the approved compact member bar: 34px collapsed footer, 49px expanded member strip, 180ms reversible easing, separate broadcast-all and expand controls, checkboxes, one select-all/clear action and online/offline status. Uses the existing Ink/Navy/Light semantic colors. Many members remain accessible in a horizontally scrollable strip; bulk selection stays visible.
- Member order is derived from split positions, not depth-first traversal. Zoom does not remove hidden group members. New regression cases cover the 209/212/216/217 grid, subsequent docking and unequal split heights.
- Selecting a member while broadcast is disabled starts with that member only, rather than implicitly activating retained targets. Selection regression cases cover partial/all/none and stale pane IDs. The shared broadcast transport and offline error routing are unchanged.
- Validation uses background MSVC builds and library tests only. No application window is launched, no computer-use is used, and actual animation smoothness, pointer interactions and visual parity remain unverified on screen.
- Executed: Windows Release build succeeded; the current library suite reports 65 passed, 0 failed, 3 ignored. Current build and test logs are included outside the baseline directory. The final subsequent adjustment only aligns member-label font size with the approved mockup and was release-built again.

## Baseline executed checks (before titlebar follow-up)

- Windows x86_64 MSVC Release build succeeded with embedded TermViAI icon and embedded JetBrains Mono / Nerd Symbols fallback fonts.
- 61 library tests passed: shared Host/Keychain, workspace compatibility, credential fallback, Monitor retry/refresh, Forwarding ownership, native split-tree and production terminal adapter checks.
- Two additional ignored-by-default tests passed against an isolated `127.0.0.1` SSH fixture: production authentication fallback/recovery using **both libssh and ssh2**, and the native adapter's SSH PTY key authentication, input/output, resize and close. No user SSH account or user known_hosts file was used.
- The real local ConPTY test opened three sessions, routed broadcast bytes through the production router, observed output on every session, submitted successive sizes, checked follow-output generation and waited for process exit.
- Native Windows screenshots: Hosts/Edit Host, Keychain/Edit Identity, New Tab, four live ConPTY panes with Chinese and Latin output, Monitor add-host and Port Forwarding add-service; Ink/Navy/Light were observed at 100% Windows scale. Screenshots are actual application windows, not HTML mockups.
- An initial isolated New Tab preview fixture was correctly rejected by production workspace validation because it contained only one terminal; the fixture was corrected to a two-terminal saved workspace.
- One verification rerun initially failed because its temporary SSH fixture had exited; the final run launches the fixture and tests together and terminates the fixture afterward. Final test logs are included.

- After the empty key-up/follow-output correction, all three non-network terminal adapter tests were rerun and passed; the related final log is included.

## Trial boundaries

- One forwarding end-to-end fixture test remains ignored. No real NPU/GPU collection or remote process termination was executed in this turn. Those owning backend implementations were reused and their unit regressions passed; this is not new hardware validation.
- This frontend paints terminal text/cells, selection and cursor. Inline terminal image protocols are not painted yet. The legacy renderer, Lua GUI customizations and its complete accessibility surface are not migrated by this trial.
- Windows IME composition/candidate positioning is wired to GPUI's input handler, but manual multi-stage IME, screen-reader and mixed-DPI multi-monitor sessions are still unverified.
- Existing core resize/reflow behavior is preserved, and pending PTY sizes are coalesced; this is not evidence that every remote shell prompt redraw issue has been eliminated.
- The new pane-layout/session adapter is deliberately separate from the legacy Mux window management. Cross-window terminal transport and remote Mux attachment are not provided in this trial.

## Resource sample

Four idle local terminals, 1280×820 client area, 100% scale: 56.7 MiB working set and 53.4 MiB private memory for the GUI process; 0.2344 CPU seconds over a 5-second sample. This excludes ConPTY/console child processes and GPU allocations and is a short observation, not a sustained benchmark or a comparison with Electron.

## Reproduction

Run `cargo test --locked --release --lib -- --test-threads=1` from the nested workspace. For SSH integration, start `tests/ssh_fixture.py` with Paramiko using a disposable directory, copy its generated keys to the Windows fixture directory, set `TERMVIAI_AUTH_FIXTURE`, and explicitly run `loopback_authentication_recovery` and `ssh_pty_adapter` with `--ignored --test-threads=1`.

Native page inspection is opt-in: `--validation-page <scene>` also requires `--data-dir` and a `.gpui-validation` marker in that directory. It never runs on normal startup. Available scenes are in `src/app/validation.rs`; fixture records use documentation-only addresses and contain no usable credentials.
