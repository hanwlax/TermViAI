# TermViAI GPUI evaluation

A separate Windows evaluation executable. It does not replace TermViAI, read its
Host library, access Windows Credential Manager, or connect to SSH hosts. The
host list uses documentation-only IP addresses. Form edits remain in memory.
No WebView feature is enabled.

The `unicode-fix` build corrects overlapping Chinese terminal glyphs. Native
shaping is anchored to terminal columns (wide characters occupy two cells),
while combining glyph offsets are preserved. `--view four-terminals` opens
four local shells; `--view unicode` opens four synthetic mixed-text fixtures
for visual verification, without sending commands to any host.

## What to evaluate

- Hosts: layout, search, group navigation, cards, and a temporary Add Host form.
- Form: text selection, copy/paste, focus, password masking, Chinese IME.
- Typography: 12/13/14/16 px UI text at Regular/Medium/Semibold; three themes.
- Terminals: actual local ConPTY processes, up to four panes; ANSI colors,
  Unicode, keyboard input, mouse selection, Ctrl+Shift+C/V, wheel scrollback,
  and window resizing. The local shell is cmd.exe. It can invoke an installed
  ssh.exe manually, but this is not TermViAI's SSH/authentication integration.
- Output test: a synthetic producer sends 30,000 colored UTF-8 lines over ~30s.
  Each terminal retains at most 2,000 scrollback rows. Close a pane to stop it.

This is a technology probe, not a feature-complete terminal replacement. It uses
vt100 for the disposable terminal model, not TermViAI's existing terminal model.
Advanced terminal protocols, full keyboard encoding, search, group drag docking,
reconnect, credentials, monitoring, forwarding, accessibility and production
lifecycle integration are outside this probe. In particular, passing these checks
does not establish resize/reflow parity with the current application.

## Build

Use a Windows MSVC Rust toolchain with Visual Studio C++ Build Tools. From this
folder, run:

```
cargo test --locked --release
cargo build --locked --release
```

The nested workspace intentionally isolates dependencies from TermViAI. GPUI
0.2.2 and GPUI Component 0.5.1 are pinned to the published compatible pair in the
component release's example. These are not asserted to be the newest releases.
The version pairing/API maintenance cost must be revisited before migration.
The lockfile pins transitive dependencies. Do not enable the optional `webview`
feature of the component library.

## Validation

Run `termviai-gpui-probe.exe --pty-check` to validate an actual ConPTY round trip,
ASCII/Unicode input and resizing against the bundled console-subsystem fixture.
No real host or account is used.

`Measure-Probe.ps1 -Executable <exe> -OutputDirectory <directory>` launches a
50-second automatic test: Hosts page, four real synthetic PTY producers, then a
light-theme typography view. During output it exercises eight window-size changes. Reports include UI-process working set/private
bytes/CPU and per-pane processed bytes and paint-preparation CPU time. The
script uses no keyboard/mouse automation. It stops only its own process if the
test times out. UI-process memory excludes producer children, ConPTY and shared
GPU allocations; it must not be presented as total application memory. Paint
preparation time is not GPU frame time, displayed FPS, or input latency.

For manual testing, open `TermViAI-GPUI.exe`. Start at Windows 100% scaling, then
check 125%/150% and moving between monitors. Compare the actual executable,
not a browser mockup. Verify Chinese IME composition/candidate positioning,
clipboard behavior, text sharpness, resize responsiveness and closing all panes.
These need user testing in addition to the automated test.

No migration should be approved solely because this probe builds. Evaluate
Windows visual quality and text/input behavior first; then separately prototype
adapting the existing TermViAI terminal model and SSH backend into GPUI.

## Terminal-parser issue found during evaluation

The real resize test exposed a vt100 0.16.2 panic when shrinking through a CJK
double-width character. A minimal regression reproduced it independently of
GPUI. The evaluation vendors that crate with a small `Row::resize` invariant
fix; see `vendor/vt100/TERMVIAI_PATCH.md`. The production TermViAI terminal
implementation is untouched.
