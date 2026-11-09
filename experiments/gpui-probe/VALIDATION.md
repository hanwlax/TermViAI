# Windows GPUI evaluation — 2026-11-09

Outcome: the native UI and disposable real terminal prototype run successfully.
This is a feasibility result, not approval to migrate the production application.

## Build and correctness

- Windows MSVC Release build succeeded with GPUI 0.2.2 / GPUI Component 0.5.1.
- Six distinct unit regressions passed: ANSI/Unicode/history bound, key encoding,
  fragmented ConPTY cursor query, resizing through a double-width character,
  mixed CJK/combining-glyph placement, and the Chinese copyright-line placement.
- Actual ConPTY ASCII/Chinese input round trip and 12×40 → 32×120 resize passed.
- The runtime dependency tree contains no Wry, Tauri or WebView dependency.
- Native Hosts, four-terminal output, and light-theme input form screenshots were
  inspected. No browser screenshot or HTML mockup was used as runtime evidence.

## Unicode-fix automatic run

- Windows Release build and all six distinct regressions passed.
- Four ConPTY producers each completed 30,000 colored Unicode lines during
  eight window-size changes; normal exit code 0 and empty stderr.
- UI process peak private bytes: 104.61 MiB; peak working set: 69.60 MiB.
  These exclude producer/ConPTY processes and shared GPU allocations.
- This verifies output and resize completion, not input latency or frame rate.
- The packaged `validation/` reports are from this corrected build.

## Initial automatic run (before Unicode positioning correction)

- Four independent ConPTY producers; 30,000 colored Unicode lines per producer.
- Eight window-size changes, from 820×580 through 1280×840 logical pixels.
- All four producers completed; each delivered about 2.90 MB and was painted
  1,660 times. Application exited with code 0, with empty stderr.
- UI-process peak private bytes: **103.64 MiB**. Peak resident working set in this
  run: **69.36 MiB**. Working-set residency varies with OS trimming and activity;
  an earlier constant-size run reached approximately 84 MiB.
- After closing all terminals, the last sample had approximately 69.82 MiB of
  private bytes. This one cycle is not a long-duration memory-leak test.
- Mean paint-preparation CPU cost, summed across four panes: approximately
  **2.08 ms**. This is not GPU frame time, displayed FPS, or input latency.
- These numbers cover the GPUI UI process, excluding synthetic producers,
  ConPTY and shared GPU allocations. They are not total application memory.
- No same-workload comparison against production TermViAI was performed.

## Issues discovered

The first trial incorrectly passed a one-column `force_width` to GPUI's line
shaper. GPUI 0.2.2 advances once per shaped glyph in that mode, so Chinese
double-width cells overlapped and following ASCII no longer matched the cursor.
The `unicode-fix` build shapes normally, copies the cached line layout, and
anchors glyphs to the parser's UTF-8-byte-to-column map. It preserves native
relative offsets for combining glyphs in the same cell and sets the line width
to the terminal grid. The original cache is never modified.

Native Windows screenshots of four fresh cmd.exe sessions now show the complete
Chinese copyright text without overlap. Four synthetic fixtures were also
visually checked for Chinese/Japanese/Korean, ASCII after CJK, combining accents,
and colored Chinese output. The ConPTY input/resize check passed again.

ConPTY INHERIT_CURSOR requires a cursor-position response before normal output.
The probe now responds through the terminal parser, including fragmented CSI.

A separate vt100 0.16.2 bug was reproduced during shrinking across Chinese text:
`Row::resize` retained a wide lead after removing its continuation. The isolated
vendored fix and minimal regression are documented in
`vendor/vt100/TERMVIAI_PATCH.md`. The production TermViAI terminal is unchanged.

## Remaining decision gates

- User comparison of text, density and controls on the actual Windows display.
- Manual Chinese IME composition/candidate positioning, clipboard and selection.
- Multi-monitor and 125%/150% DPI behavior; drag-resize input latency and GPU time.
- Accessibility, keyboard traversal and keyboard shortcut completeness.
- Integrating the existing TermViAI terminal model, SSH authentication, reconnect,
  broadcast, docking, Monitor and Port Forwarding; none is claimed migrated.
- Longer duration and matched baseline memory/performance measurements.

The prototype uses its own vt100 screen model. It demonstrates that GPUI can
host a real PTY and draw its output; it does not establish that the existing
WezTerm-derived terminal renderer can be reused without adaptation.
