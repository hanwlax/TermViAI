//! Local selection uses stable rows so scrolling never moves its anchor.
use wezterm_term::screen::Screen;

pub type Cell = (isize, usize);

#[derive(Default)]
pub struct Selection {
    pub range: Option<(Cell, Cell)>,
    pub dragging: bool,
    extended: bool,
}
impl Selection {
    pub fn begin(&mut self, anchor: Cell) {
        self.range = Some((anchor, anchor));
        self.dragging = true;
        self.extended = false;
    }
    pub fn extend(&mut self, end: Cell) {
        if self.dragging {
            if let Some((anchor, _)) = self.range {
                self.extended |= anchor != end;
                self.range = Some((anchor, end));
            }
        }
    }
    /// Both inside and outside mouse releases use this. A click must not copy.
    pub fn finish(&mut self) -> bool {
        let copy = self.dragging && self.extended;
        self.dragging = false;
        if !self.extended {
            self.range = None;
        }
        copy
    }
    pub fn has_extended(&self) -> bool {
        self.extended
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Positive velocity reveals older rows. Speed increases towards/outside edges.
pub fn edge_velocity(y: f32, height: f32, line: f32) -> f32 {
    let band = (line * 1.5).min(height / 3.).max(1.);
    if y < band {
        36. * ((band - y) / band).clamp(0., 2.)
    } else if y > height - band {
        -36. * ((y - height + band) / band).clamp(0., 2.)
    } else {
        0.
    }
}

pub fn cell_at(
    top: isize,
    x: f32,
    y: f32,
    cell: f32,
    line: f32,
    width: f32,
    height: f32,
    fractional: f32,
) -> Cell {
    let cols = ((width - 10.) / cell).floor().max(2.);
    let rows = (height / line).floor().max(2.);
    // The first painted row can be partly clipped during smooth scrolling.
    let row = (y.clamp(0., (height - 0.01).max(0.)) / line + fractional)
        .floor()
        .clamp(0., (rows + fractional).ceil() - 1.);
    (
        top + row as isize,
        (x / cell).floor().clamp(0., cols - 1.) as usize,
    )
}

/// Columns highlighted on a physical display row, including selected blank cells.
pub fn selected_columns(
    range: Option<(Cell, Cell)>,
    row: isize,
    cols: usize,
) -> Option<std::ops::Range<usize>> {
    let (a, b) = range?;
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    if row < a.0 || row > b.0 {
        return None;
    }
    let start = if row == a.0 { a.1.min(cols) } else { 0 };
    let end = if row == b.0 {
        b.1.saturating_add(1).min(cols)
    } else {
        cols
    };
    (start < end).then_some(start..end)
}

/// Select this displayed row, retaining indentation, excluding padding.
pub fn line_text_range(screen: &Screen, row: isize) -> Option<(Cell, Cell)> {
    let physical = screen.stable_row_to_phys(row)?;
    let lines = screen.lines_in_phys_range(physical..physical + 1);
    let end = lines
        .first()?
        .visible_cells()
        .filter(|cell| !cell.str().chars().all(char::is_whitespace))
        .map(|cell| cell.cell_index() + cell.width())
        .last()?;
    Some(((row, 0), (row, end - 1)))
}

/// Prefer the clicked word; whitespace keeps the existing whole-row gesture.
/// Work in visible cells, not bytes, including both halves of wide glyphs.
pub fn double_click_range(screen: &Screen, at: Cell, boundaries: &str) -> Option<(Cell, Cell)> {
    let physical = screen.stable_row_to_phys(at.0)?;
    let mut first = physical;
    while first > 0 && screen.lines_in_phys_range(first - 1..first)[0].last_cell_was_wrapped() {
        first -= 1;
    }
    let mut last = physical;
    while last + 1 < screen.scrollback_rows()
        && screen.lines_in_phys_range(last..last + 1)[0].last_cell_was_wrapped()
    {
        last += 1;
    }
    let lines = screen.lines_in_phys_range(first..last + 1);
    let cells: Vec<_> = lines
        .iter()
        .enumerate()
        .flat_map(|(offset, line)| {
            let row = screen.phys_to_stable_row_index(first + offset);
            line.visible_cells().map(move |cell| {
                let text = cell.str();
                let blank = text.chars().all(char::is_whitespace);
                let word = !blank && (text.chars().count() > 1 || !boundaries.contains(text));
                (
                    (row, cell.cell_index()),
                    (row, cell.cell_index() + cell.width() - 1),
                    blank,
                    word,
                )
            })
        })
        .collect();
    let Some(index) = cells
        .iter()
        .position(|(start, end, _, _)| *start <= at && at <= *end)
    else {
        return line_text_range(screen, at.0);
    };
    if cells[index].2 {
        return line_text_range(screen, at.0);
    }
    let mut left = index;
    let mut right = index;
    if cells[index].3 {
        while left > 0 && cells[left - 1].3 {
            left -= 1;
        }
        while right + 1 < cells.len() && cells[right + 1].3 {
            right += 1;
        }
    }
    Some((cells[left].0, cells[right].1))
}

pub fn selected_text(screen: &Screen, range: (Cell, Cell)) -> Option<String> {
    let (a, b) = range;
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let first = screen.phys_to_stable_row_index(0);
    let last = screen.phys_to_stable_row_index(screen.scrollback_rows().checked_sub(1)?);
    if b.0 < first || a.0 > last {
        return None;
    }
    let start = a.0.max(first);
    let end = b.0.min(last);
    let physical = screen.stable_row_to_phys(start)?..screen.stable_row_to_phys(end)? + 1;
    let lines = screen.lines_in_phys_range(physical);
    let mut text = String::new();
    for (i, line) in lines.iter().enumerate() {
        let row = start + i as isize;
        let left = if row == a.0 { a.1 } else { 0 };
        let right = if row == b.0 {
            b.1.saturating_add(1)
        } else {
            line.len()
        };
        text.push_str(line.columns_as_str(left..right).trim_end());
        if i + 1 < lines.len() && !line.last_cell_was_wrapped() {
            text.push('\n');
        }
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};
    #[derive(Debug)]
    struct Config;
    impl TerminalConfiguration for Config {
        fn color_palette(&self) -> wezterm_term::color::ColorPalette {
            wezterm_term::color::ColorPalette::default()
        }
    }
    fn terminal() -> Terminal {
        Terminal::new(
            TerminalSize {
                rows: 3,
                cols: 8,
                ..TerminalSize::default()
            },
            Arc::new(Config),
            "TermViAI",
            "test",
            Box::new(Vec::new()),
        )
    }
    #[test]
    fn release_copies_once_but_click_and_cancel_do_not() {
        let mut s = Selection::default();
        s.begin((10, 2));
        assert!(!s.finish());
        assert!(s.range.is_none());
        s.begin((10, 2));
        s.extend((2, 0));
        assert!(s.finish());
        assert!(!s.finish());
        assert_eq!(s.range, Some(((10, 2), (2, 0))));
        s.begin((10, 2));
        s.extend((12, 5));
        s.clear();
        assert!(!s.finish());
    }
    #[test]
    fn stationary_pointer_extends_selection_when_viewport_scrolls() {
        let mut s = Selection::default();
        s.begin((10, 2));
        s.extend(cell_at(6, 32., 0., 8., 20., 170., 100., 0.));
        assert_eq!(s.range, Some(((10, 2), (6, 4))));
        s.extend(cell_at(14, 32., 999., 8., 20., 170., 100., 0.));
        assert_eq!(s.range, Some(((10, 2), (18, 4))));
        assert!(s.finish());
    }
    #[test]
    fn edge_speed_is_directional_bounded_and_zero_in_center() {
        assert_eq!(edge_velocity(50., 100., 20.), 0.);
        assert!(edge_velocity(10., 100., 20.) > 0.);
        assert!(edge_velocity(90., 100., 20.) < 0.);
        assert_eq!(edge_velocity(-999., 100., 20.), 72.);
        assert_eq!(edge_velocity(999., 100., 20.), -72.);
        assert_eq!(cell_at(5, -100., -100., 8., 20., 170., 100., 0.), (5, 0));
        assert_eq!(cell_at(5, 999., 999., 8., 20., 170., 100., 0.), (9, 19));
        assert_eq!(cell_at(5, 0., 19., 8., 20., 170., 100., 0.5), (6, 0));
    }
    #[test]
    fn copy_spans_scrollback_in_both_directions() {
        let mut t = terminal();
        t.advance_bytes(b"first\r\nsecond\r\nthird\r\nfourth\r\nfifth");
        assert!(t.screen().phys_row(0) > 0);
        let a = (t.screen().phys_to_stable_row_index(0), 1);
        let b = (a.0 + 4, 3);
        let expected = Some("irst\nsecond\nthird\nfourth\nfift".into());
        assert_eq!(selected_text(t.screen(), (a, b)), expected);
        assert_eq!(selected_text(t.screen(), (b, a)), expected);
    }
    #[test]
    fn copy_keeps_soft_wraps_and_wide_characters() {
        let mut t = terminal();
        t.advance_bytes("abc中文xyz\r\nend".as_bytes());
        let first = t.screen().phys_to_stable_row_index(0);
        assert_eq!(
            selected_text(t.screen(), ((first, 0), (first + 2, 2))),
            Some("abc中文xyz\nend".into())
        );
    }
    #[test]
    fn expired_anchor_does_not_apply_its_column_to_a_different_row() {
        let mut t = terminal();
        t.advance_bytes(b"first\r\nsecond");
        let first = t.screen().phys_to_stable_row_index(0);
        assert_eq!(
            selected_text(t.screen(), ((first - 2, 4), (first + 1, 2))),
            Some("first\nsec".into())
        );
        assert_eq!(
            selected_text(t.screen(), ((first - 2, 0), (first - 1, 3))),
            None
        );
    }
    #[test]
    fn line_selection_excludes_trailing_padding() {
        let mut t = terminal();
        t.advance_bytes("  中文  \r\nx   \r\n   ".as_bytes());
        let first = t.screen().phys_to_stable_row_index(0);
        let range = line_text_range(t.screen(), first).unwrap();
        assert_eq!(range, ((first, 0), (first, 5)));
        assert_eq!(selected_text(t.screen(), range), Some("  中文".into()));
        assert_eq!(
            line_text_range(t.screen(), first + 1),
            Some(((first + 1, 0), (first + 1, 0)))
        );
        assert_eq!(line_text_range(t.screen(), first + 2), None);
    }
    fn double_clicked(t: &Terminal, row: usize, col: usize) -> Option<String> {
        let range = double_click_range(
            t.screen(),
            (t.screen().phys_to_stable_row_index(row), col),
            " \t\n{[}]()\"'`",
        )?;
        selected_text(t.screen(), range)
    }
    #[test]
    fn double_click_prefers_words_but_blank_space_selects_the_row() {
        let mut t = terminal();
        t.advance_bytes(b"one two\r\n(x_y)\r\n   ");
        assert_eq!(double_clicked(&t, 0, 1).as_deref(), Some("one"));
        assert_eq!(double_clicked(&t, 0, 5).as_deref(), Some("two"));
        assert_eq!(double_clicked(&t, 0, 3).as_deref(), Some("one two"));
        assert_eq!(double_clicked(&t, 0, 7).as_deref(), Some("one two"));
        assert_eq!(double_clicked(&t, 1, 2).as_deref(), Some("x_y"));
        assert_eq!(double_clicked(&t, 1, 0).as_deref(), Some("("));
        assert_eq!(double_clicked(&t, 2, 1), None);
    }
    #[test]
    fn double_click_follows_soft_wraps_in_scrollback_but_not_newlines() {
        let mut t = terminal();
        t.advance_bytes(b" abcdefghijkl\r\nnext\r\nlast");
        assert!(t.screen().phys_row(0) > 0);
        for (row, col) in [(0, 3), (1, 2)] {
            assert_eq!(
                double_clicked(&t, row, col).as_deref(),
                Some("abcdefghijkl")
            );
        }
        assert_eq!(double_clicked(&t, 2, 1).as_deref(), Some("next"));
    }
    #[test]
    fn double_click_preserves_wide_and_combining_graphemes() {
        let mut t = terminal();
        t.advance_bytes(" 中文 e\u{301}\r\n".as_bytes());
        for col in 1..5 {
            assert_eq!(double_clicked(&t, 0, col).as_deref(), Some("中文"));
        }
        assert_eq!(double_clicked(&t, 0, 6).as_deref(), Some("e\u{301}"));
    }
    #[test]
    fn highlight_covers_only_selected_columns_in_both_directions() {
        for range in [((5, 2), (7, 3)), ((7, 3), (5, 2))] {
            assert_eq!(selected_columns(Some(range), 4, 8), None);
            assert_eq!(selected_columns(Some(range), 5, 8), Some(2..8));
            assert_eq!(selected_columns(Some(range), 6, 8), Some(0..8));
            assert_eq!(selected_columns(Some(range), 7, 8), Some(0..4));
            assert_eq!(selected_columns(Some(range), 8, 8), None);
        }
        assert_eq!(selected_columns(Some(((0, 0), (0, 0))), 0, 8), Some(0..1));
    }
}
