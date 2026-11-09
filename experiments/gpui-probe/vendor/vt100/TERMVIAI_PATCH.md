# Evaluation-only vt100 patch

Based on the crates.io vt100 0.16.2 source (MIT, license included).
Only `src/row.rs`, `Row::resize` differs from the published source.

Resizing can truncate the continuation cell of a wide glyph, leaving the wide
lead in the last column. `Screen::text` subsequently assumes the continuation
exists and panics. Clear that orphaned lead exactly as `Row::truncate` already
does. The implementation also tolerates an empty row via `last_mut`.

Reproducer: write `abc中` into five columns, shrink to four, then write at
row 1, column 4. Probe regression:
`shrinking_through_wide_character_can_accept_more_output`.

This patch is isolated to the GPUI evaluation workspace and does not modify
TermViAI's terminal implementation. It is not evidence for or against GPUI's
renderer. Replacing the prototype parser with the existing terminal model is a
separate integration task.
