# Native GPUI trial validation · 2026-11-09

## Architecture and scope

The production terminal core, SSH libraries and PTY implementation are path dependencies. Hosts, workspaces, authentication control, Monitor and Forwarding compile directly from their existing source files. Native GPUI elements own layout, input focus, painting and application navigation. This was initially a separate trial executable. As of v0.1.0-beta.8, the native frontend is published as TermViAI.exe; the trial-era validation entries below remain historical evidence.

The previous probe's vt100 parser, example host library and navigation were not adopted. Resolved Cargo metadata contains no WebView, Wry, Tauri or vt100 package. The PE import table contains Windows system libraries; no Chromium runtime is bundled.

## System broadcast notifications · 2026-11-10

- Implements the approved terminal-local blue bell / unread count, hover preview, pinned details, notification/error tabs, clear and expandable original text. All content uses selectable native GPUI text with Ctrl+C, with no copy buttons. Remote text is escaped before native text parsing, so it cannot inject markup or clickable actions. Existing error-click credential-save retry behavior is retained.
- A source terminal owns its inbox; changing pages/zoom dismisses its overlay. Hover does not focus the popup or mark messages read. Clicking opens/marks the current snapshot read. Selecting pins the preview; incoming messages update the badge without moving text in a pinned snapshot. The popup is non-modal, bounded below the titlebar, and does not disable native window controls/dragging.
- The reader uses a bounded streaming recognizer before terminal parsing. Only the known util-linux wall envelope (79-space framing, English banner, sender/TTY/date fields, double BEL, matching line endings and nonempty body lines) is accepted. The 250ms collection window rejects mixed output and ambiguous blank-line continuation. EOF, incomplete/oversized frames, malformed UTF-8/control text and unknown formats release original bytes; raw wire bytes are retained for accepted session notifications. Other programs can deliberately print the same format; this is a conservative presentation heuristic, not authenticated framing or universal broadcast detection.
- Inspected upstream util-linux wall.c (v2.37.2 and master). A local isolated Python PTY experiment verified ONLCR transforms CRLF to CRCRLF; regression fixtures cover both dialects and reject mixed endings. No wall broadcast was sent to any host or local logged-in user.
- Windows MSVC Release build and library suite passed: 108 passed, 0 failed, 3 ignored. Twelve new regressions cover all split points, byte-at-a-time input, old truncated banners, Chinese, multiple frames, normal surrounding output, unknown/control/invalid UTF-8 data, stream control-string context, timeout/EOF, size limits, ambiguous mixed output, inbox capacity/read/clear, literal markup, ONLCR and terminal cursor/prompt preservation. Final UI-only snapshot/focus/styling refinements are release-built after this suite.
- No computer-use, visible application launch, live SSH connection or user clipboard manipulation. On-screen appearance, hover selection and Ctrl+C behavior still require trial feedback; source checks confirm GPUI TextView's native focus, selection and copy action paths.

## Terminal selection restoration · 2026-11-10

- Restores copy-on-left-release, including release outside the source pane. Plain clicks and cancelled drags do not replace the clipboard. Double-click selects the displayed row, including indentation but excluding trailing blank cells, and immediately copies it.
- Selection endpoints use stable rows and refresh as the viewport scrolls, including a stationary pointer. Wheel scrolling during a drag stays with the source pane. Edge scrolling ramps with pointer distance, clamps to scrollback and uses the existing smooth viewport animation. Focus/window deactivation cancels dragging.
- Fixes missing terminal selection backgrounds: GPUI `ShapedLine::paint` paints glyphs without the background pass. Terminal backgrounds now paint as merged cell-aligned rectangles, including ANSI backgrounds, wide cells and selected padding. IME backgrounds use the separate GPUI background pass.
- Windows MSVC Release build passed. Library suite: 96 passed, 0 failed, 3 ignored; eight new tests cover selection lifecycle, stationary-pointer scrolling, edge direction/limits, cross-scrollback copying, reversed selections, soft wraps/wide characters, expired history, line trimming and highlight column spans. Final mouse-release propagation guard is compile-checked after this suite; it does not change the tested selection module.
- No visible application launch, computer-use, clipboard interaction on the desktop or live SSH testing performed. Pointer smoothness and actual screen appearance still need user verification in the trial.

## Host multi-selection · 2026-11-10

- Hosts cards support Ctrl+left-click toggle with themed selected surfaces, borders and check marks. Plain click still opens one connection. Existing menu buttons stop propagation and do not toggle selection.
- Enter consumes the selection before opening individual tabs in current store/list order. Root key handling is scoped to Hosts with no menu/dialog; search input Enter uses the same batch path when selections exist. Terminal keys and form submission retain their existing behavior. Escape or Clear cancels selection.
- Search edits and group/page navigation clear selection. Store changes discard changed/deleted selected records; selection uses complete Host records rather than recycled filtered indices or labels. The selection summary shares the breadcrumb toolbar rather than inserting a new row above the cards at normal window widths.
- Windows Release build and library suite passed: 75 passed, 0 failed, 3 ignored. New regressions cover selection toggle/list ordering/one-shot consumption, changed/deleted records, separate accounts and cancellation. A final UI-only visible-host filter was release-built after the suite. No computer-use or visible application launch is used; native pointer behavior remains unverified on screen.

## Empty states and Monitor refinement · 2026-11-10

- Implements the approved `termviai-states-monitor-refinement.html` interaction proposal: shared small-icon empty states for Hosts/search/groups, Keychain, workspaces/recent connections, Forwarding, Monitor and process lists; field-local required/port errors and neutral action-error notices. Existing blank Host-label defaults and password semantics remain supported.
- Monitor uses typed collection/connection/attention failures plus the saved-host reference to choose a recovery action. Missing/changed hosts offer explicit replacement or removal, ordinary failures retain retry/pause, and technical errors expand locally. A failed collection is never presented as an idle NPU.
- The 392px Monitor picker groups available hosts and excludes already monitored entries from selection; existing entries are available through a read-only disclosure. No saved hosts, no matches and all-added states have distinct messages. Save counts eligible selections and revalidates Host records. Replacement persists first, cancels the old collection only after success, rejects duplicates/in-flight process actions and preserves index/other monitors.
- Sampling is a single clock / Every Ns / chevron control. Its menu marks the current 2/5/10-second choice and explains its scope; failed preference persistence restores the previous interval.
- All colors follow the current Ink/Navy/Light palette, action buttons stay neutral until hover, and titlebar controls remain outside drawers. Validation is through source inspection, background Windows compilation and tests; no computer-use, app window launch, remote process action or on-screen visual/interaction verification is performed.
- Executed: final Windows MSVC Release build succeeded. Library regression suite: 72 passed, 0 failed, 3 ignored; remote collector/forwarding Python suite: 34 passed. New cases cover picker account identity/duplicate exclusion, replacement persistence failure and cancellation, in-flight stop rejection, and stale-reference classification without parsing error text.

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

## Tab/group follow-up · 2026-11-10

- Group titles follow the legacy member-count format and refresh after docking, detaching and closing panes; user-renamed titles survive layout changes. Restored names differing from the default are treated as custom.
- Rename Tab edits the live title independently of saved-workspace existence. Explicit Save Workspace persists the current name/layout; a deleted saved reference no longer blocks a local rename.
- Dock previews and committed docking share source-membership validation. Whole groups remain sortable in the tab bar but never show a terminal docking preview; individual group panes remain dockable.
- Tab left/right halves choose the insertion gap. The marker and commit share index adjustment, including hidden markers for both no-op gaps. Dragging a group pane to a tab edge, + or blank titlebar detaches it using the same pane/session, without reconnecting. The remaining group updates its default name.
- Executed: Windows x86_64 MSVC Release build succeeded; 79 library tests passed, 0 failed, 3 external-environment tests ignored. Four new cases cover naming, docking eligibility, insertion/no-op boundaries and detached pane identity/layout. Source/mirror and archive integrity checks accompany the trial.
- No application window or computer-use interaction was used. Actual on-screen pointer/focus behavior and drag smoothness remain manually unverified. The prior tab-overflow UX remains unimplemented pending approval.

## Approved tab-overflow UX · 2026-11-10

- Implements the approved single-row rail: 242px terminal tabs, overflow-only left/right buttons and searchable All tabs count button, fixed visible + and native window controls. Arrow/discrete wheel scrolling uses 160ms easing; precise trackpad deltas stay direct. Scroll offsets are clipped without GPUI's second immediate wheel handler.
- Active tabs are revealed on selection, connect/restore, list changes and window resize. Manual browsing does not snap back on ordinary updates. Dragging near the rail edges scrolls at a time-based rate; insertion positions are recomputed against the scrolled viewport and reuse the prior no-op/group restrictions.
- All tabs searches live names and all grouped host labels/addresses/accounts. Current-tab checkmark, offline indicator, hover-only close controls, close confirmation and no-results state use existing theme tokens. New Tab remains a single selectable placeholder until used/closed.
- Search keeps its neutral border when focused (`focus_bordered(false)`), per the approval correction. Up/Down/Enter/Escape are scoped to this picker, including the child Input context; Ctrl+Tab / Ctrl+Shift+Tab cycle open tabs. Other input fields and terminal Tab remain covered by regression checks.
- Executed: Windows x86_64 MSVC Release build succeeded; 84 library tests passed, 0 failed, 3 external-environment tests ignored. The initial keyboard-precedence regression failed and led to adding a descendant Input context; the final rerun passed. Five added regressions cover overflow control reservation, reveal/clamp behavior, scrolled insertion/edge limits, cycling and keyboard scope.
- No computer-use or application window launch. On-screen geometry, actual focus interactions and perceived animation smoothness have not been manually verified; source/mirror and archive integrity checks accompany the trial. The previous pending-approval note is superseded by this approved implementation. A final UI-only adjustment appends the scrollbar after list rows to preserve ScrollHandle row indexes; that adjustment was release-built again.

## Windows IME / terminal scrollbar pointer · 2026-11-10

- Source diagnosis: GPUI 0.2.2 handles `WM_IME_COMPOSITION`, including explicit empty composition, but does not forward `WM_IME_ENDCOMPOSITION` to terminal unmark. TerminalView kept painting its stale preedit and rejected keydown while that string was nonempty, matching the reported phantom candidate text and ignored Enter.
- Added a window-local subclass observer for composition-end/language-change notifications. It only advances a shared epoch, delegates native processing and releases its state on window destruction. It does not enter GPUI from the native callback or change system keyboard settings. The existing terminal refresh loop, render and keydown reconcile the epoch. New composition records its own epoch so an earlier cancellation cannot erase it; blur also clears terminal preedit. Only explicit nonempty IME commit text reaches the session.
- Terminal scrollbar edge hover and active thumb dragging use PointingHand; normal terminal text keeps IBeam. No desktop cursor automation was performed.
- Executed: Windows MSVC Release build succeeded. 88 library tests passed, 0 failed, 3 external-environment tests ignored. New tests cover cancellation, explicit commit/UTF-16 selection, rapid restart, and actual Windows end/language messages plus hook lifetime on an invisible test window. This test neither calls ShowWindow nor changes the user's input method or foreground window.
- No computer-use or visible application launch. Actual third-party IME switching and scrollbar pointer appearance remain manually unverified on screen. Package/source integrity is checked separately.
