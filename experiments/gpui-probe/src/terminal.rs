use crate::pty::Terminal;
use gpui::prelude::*;
use gpui::*;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub struct TerminalView {
    pty: Terminal,
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    cell: Pixels,
    line: Pixels,
    font_size: f32,
    selection: Option<((u16, u16), (u16, u16))>,
    selecting: bool,
    composition: String,
    composition_selection: Range<usize>,
    error: String,
    last_revision: u64,
    pub paints: u64,
    pub paint_us: u64,
    pub max_paint_us: u64,
    light: bool,
}
impl TerminalView {
    pub fn new(pty: Terminal, cx: &mut Context<Self>) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let exited = this.pty.poll_exit();
                        let revision = this.pty.revision.load(Ordering::Acquire);
                        if exited || revision != this.last_revision {
                            this.last_revision = revision;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            pty,
            focus: cx.focus_handle(),
            bounds: Bounds::default(),
            cell: px(8.),
            line: px(20.),
            font_size: 14.,
            selection: None,
            selecting: false,
            composition: String::new(),
            composition_selection: 0..0,
            error: String::new(),
            last_revision: 0,
            paints: 0,
            paint_us: 0,
            max_paint_us: 0,
            light: false,
        }
    }
    pub fn appearance(&mut self, font_size: f32, light: bool, cx: &mut Context<Self>) {
        self.font_size = font_size;
        self.light = light;
        cx.notify();
    }
    pub fn stats(&self) -> serde_json::Value {
        let o = self.pty.output.lock().unwrap();
        serde_json::json!({"bytes":o.bytes,"ended":o.ended,"paints":self.paints,"paint_cpu_us_total":self.paint_us,"paint_cpu_us_max":self.max_paint_us,"rows":self.pty.dimensions.0,"cols":self.pty.dimensions.1})
    }
    fn send(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if let Err(e) = self.pty.send(bytes) {
            self.error = e.to_string();
        }
        self.selection = None;
        cx.notify();
    }
    fn coordinate(&self, p: Point<Pixels>) -> (u16, u16) {
        let row = ((p.y - self.bounds.top()) / self.line).floor().max(0.) as u16;
        let col = ((p.x - self.bounds.left()) / self.cell).floor().max(0.) as u16;
        (
            row.min(self.pty.dimensions.0 - 1),
            col.min(self.pty.dimensions.1 - 1),
        )
    }
    fn copy(&self, cx: &mut App) {
        if let Some((a, b)) = self.selection {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            let o = self.pty.output.lock().unwrap();
            cx.write_to_clipboard(ClipboardItem::new_string(
                o.parser
                    .screen()
                    .contents_between(a.0, a.1, b.0, b.1.saturating_add(1)),
            ));
        }
    }
    fn key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.composition.is_empty() {
            return;
        }
        let k = &e.keystroke;
        let key = k.key.as_str();
        if k.modifiers.control && k.modifiers.shift && key.eq_ignore_ascii_case("c") {
            self.copy(cx);
            cx.stop_propagation();
            return;
        }
        if k.modifiers.control && k.modifiers.shift && key.eq_ignore_ascii_case("v") {
            if let Some(s) = cx.read_from_clipboard().and_then(|c| c.text()) {
                let bracketed = self
                    .pty
                    .output
                    .lock()
                    .unwrap()
                    .parser
                    .screen()
                    .bracketed_paste();
                let s = if bracketed {
                    format!("\x1b[200~{s}\x1b[201~")
                } else {
                    s.replace("\r\n", "\r").replace('\n', "\r")
                };
                self.send(s.as_bytes(), cx);
            }
            cx.stop_propagation();
            return;
        }
        let app = self
            .pty
            .output
            .lock()
            .unwrap()
            .parser
            .screen()
            .application_cursor();
        if let Some(bytes) = key_bytes(key, k.modifiers.control, k.modifiers.alt, app) {
            self.send(&bytes, cx);
            cx.stop_propagation();
        }
        // Printable characters and IME commits are delivered by EntityInputHandler.
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let start = Instant::now();
        let font = font("Consolas");
        let id = window.text_system().resolve_font(&font);
        let cell = window
            .text_system()
            .advance(id, px(self.font_size), 'M')
            .map(|s| s.width)
            .unwrap_or(px(8.));
        self.cell = cell;
        self.line = px((self.font_size * 1.45).ceil());
        self.bounds = bounds;
        let rows = (bounds.size.height / self.line).floor().max(2.) as u16;
        let cols = (bounds.size.width / self.cell).floor().max(2.) as u16;
        if let Err(e) = self.pty.resize(rows, cols) {
            self.error = e.to_string();
        }
        let (rows, cols) = self.pty.dimensions;
        let (lines, cursor, scroll, ended) = {
            let o = self.pty.output.lock().unwrap();
            let s = o.parser.screen();
            let mut lines = Vec::new();
            for r in 0..rows {
                let mut text = String::new();
                let mut runs: Vec<TextRun> = vec![];
                let mut cell_starts = Vec::new();
                for c in 0..cols {
                    let Some(v) = s.cell(r, c) else { continue };
                    if v.is_wide_continuation() {
                        continue;
                    }
                    let t = if v.contents().is_empty() {
                        " "
                    } else {
                        v.contents()
                    };
                    let selected = self.selection.is_some_and(|(a, b)| {
                        let (a, b) = if a <= b { (a, b) } else { (b, a) };
                        (r, c) >= a && (r, c) <= b
                    });
                    let mut fg = color(v.fgcolor(), self.light, false);
                    let mut bg = color(v.bgcolor(), self.light, true);
                    if v.inverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                    if v.dim() {
                        fg.a = 0.65;
                    }
                    if selected {
                        bg = rgb(0x465886).into();
                        fg = rgb(0xffffff).into();
                    }
                    let mut f = font.clone();
                    if v.bold() {
                        f.weight = FontWeight::BOLD;
                    }
                    if v.italic() {
                        f.style = FontStyle::Italic;
                    }
                    let run = TextRun {
                        len: t.len(),
                        font: f,
                        color: fg,
                        background_color: Some(bg),
                        underline: None,
                        strikethrough: None,
                    };
                    if let Some(last) = runs.last_mut().filter(|last| {
                        last.font == run.font
                            && last.color == run.color
                            && last.background_color == run.background_color
                    }) {
                        last.len += t.len();
                    } else {
                        runs.push(run);
                    }
                    cell_starts.push((text.len(), c));
                    text.push_str(t);
                }
                lines.push((text, runs, cell_starts));
            }
            (
                lines,
                if s.hide_cursor() {
                    None
                } else {
                    Some(s.cursor_position())
                },
                s.scrollback(),
                o.ended,
            )
        };
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for (r, (text, runs, cell_starts)) in lines.into_iter().enumerate() {
                let mut line =
                    window
                        .text_system()
                        .shape_line(text.into(), px(self.font_size), &runs, None);
                // GPUI's force_width advances once per glyph, not per terminal
                // column: it squeezes CJK and separates combining glyphs. Keep
                // native shaping within each cell, then anchor it to the grid.
                // Copy the layout so we never mutate GPUI's cached shaping.
                let mut layout = LineLayout {
                    font_size: line.font_size,
                    width: self.cell * cols as f32,
                    ascent: line.ascent,
                    descent: line.descent,
                    runs: line.runs.clone(),
                    len: line.len(),
                };
                let mut grid = GridAlignment::new(&cell_starts, self.cell);
                for run in &mut layout.runs {
                    for glyph in &mut run.glyphs {
                        glyph.position.x = grid.position(glyph.index, glyph.position.x);
                    }
                }
                *line = Arc::new(layout);
                let _ = line.paint(
                    point(bounds.left(), bounds.top() + self.line * r),
                    self.line,
                    window,
                    cx,
                );
            }
            if let Some((r, c)) = cursor.filter(|_| scroll == 0 && !ended) {
                let origin = point(
                    bounds.left() + self.cell * c as f32,
                    bounds.top() + self.line * r as f32,
                );
                if self.focus.is_focused(window) {
                    window.paint_quad(fill(
                        Bounds::new(origin, size(px(2.), self.line)),
                        rgb(0xb2a3ed),
                    ));
                }
                if !self.composition.is_empty() {
                    let run = TextRun {
                        len: self.composition.len(),
                        font: font.clone(),
                        color: rgb(0xffffff).into(),
                        background_color: Some(rgb(0x514870).into()),
                        underline: Some(UnderlineStyle {
                            color: None,
                            thickness: px(1.),
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let line = window.text_system().shape_line(
                        self.composition.clone().into(),
                        px(self.font_size),
                        &[run],
                        None,
                    );
                    let _ = line.paint(origin, self.line, window, cx);
                }
            }
        });
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, cx.entity()),
            cx,
        );
        let us = start.elapsed().as_micros() as u64;
        self.paints += 1;
        self.paint_us += us;
        self.max_paint_us = self.max_paint_us.max(us);
    }
}

/// UTF-8 byte starts and columns come from the terminal model, including its
/// double-width continuation cells. Glyph count and font advance are not widths.
struct GridAlignment<'a> {
    cells: &'a [(usize, u16)],
    origins: Vec<Option<Pixels>>,
    width: Pixels,
}
impl<'a> GridAlignment<'a> {
    fn new(cells: &'a [(usize, u16)], width: Pixels) -> Self {
        Self {
            cells,
            origins: vec![None; cells.len()],
            width,
        }
    }
    fn position(&mut self, byte_index: usize, natural_x: Pixels) -> Pixels {
        let Some(i) = self
            .cells
            .partition_point(|(start, _)| *start <= byte_index)
            .checked_sub(1)
        else {
            return natural_x;
        };
        let origin = *self.origins[i].get_or_insert(natural_x);
        self.width * self.cells[i].1 as f32 + (natural_x - origin)
    }
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let bg = if self.light { 0xf9fafc } else { 0x171a24 };
        div()
            .id("terminal-view")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(bg))
            .overflow_hidden()
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, w, cx| {
                    this.focus.focus(w);
                    let p = this.coordinate(e.position);
                    this.selection = Some((p, p));
                    this.selecting = true;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                if this.selecting {
                    if let Some((a, _)) = this.selection {
                        this.selection = Some((a, this.coordinate(e.position)));
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let delta = e.delta.pixel_delta(this.line).y / this.line;
                this.pty.scroll(delta.round() as i32);
                this.selection = None;
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        entity.update(cx, |t, cx| t.paint(bounds, window, cx));
                    },
                )
                .size_full(),
            )
            .when(!self.error.is_empty(), |d| {
                d.child(
                    div()
                        .text_color(rgb(0xee8899))
                        .text_size(px(12.))
                        .child(self.error.clone()),
                )
            })
    }
}
impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        r: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let v: Vec<u16> = self.composition.encode_utf16().collect();
        let r = r.start.min(v.len())..r.end.min(v.len());
        *actual = Some(r.clone());
        Some(String::from_utf16_lossy(&v[r]))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.composition_selection.clone(),
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if self.composition.is_empty() {
            None
        } else {
            Some(0..self.composition.encode_utf16().count())
        }
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        self.composition_selection = 0..0;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition.clear();
        self.composition_selection = 0..0;
        self.send(text.as_bytes(), cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        r: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition = text.into();
        let n = text.encode_utf16().count();
        self.composition_selection = r.unwrap_or(n..n);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (r, c) = self
            .pty
            .output
            .lock()
            .unwrap()
            .parser
            .screen()
            .cursor_position();
        Some(Bounds::new(
            point(
                self.bounds.left() + self.cell * c as f32,
                self.bounds.top() + self.line * r as f32,
            ),
            size(self.cell, self.line),
        ))
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}
fn color(c: vt100::Color, light: bool, bg: bool) -> Hsla {
    let value = match c {
        vt100::Color::Default => {
            if bg {
                if light { 0xf9fafc } else { 0x171a24 }
            } else if light {
                0x202633
            } else {
                0xd7dcea
            }
        }
        vt100::Color::Rgb(r, g, b) => ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
        vt100::Color::Idx(i) => {
            let base = [
                0x20232d, 0xe67886, 0x89c99a, 0xe4c58a, 0x8eafe1, 0xb5a0dd, 0x83c5c6, 0xd4d8e4,
                0x788196, 0xf4919d, 0xa3dab1, 0xf1d6a4, 0xaac5ed, 0xc9b6e7, 0xa1d8d9, 0xf5f6fa,
            ];
            if i < 16 {
                base[i as usize]
            } else if i < 232 {
                let n = i - 16;
                let f = |v: u8| if v == 0 { 0 } else { 55 + v as u32 * 40 };
                (f(n / 36) << 16) | (f(n / 6 % 6) << 8) | f(n % 6)
            } else {
                let v = 8 + (i as u32 - 232) * 10;
                (v << 16) | (v << 8) | v
            }
        }
    };
    rgb(value).into()
}
fn key_bytes(key: &str, ctrl: bool, alt: bool, app: bool) -> Option<Vec<u8>> {
    let seq = match key {
        "enter" => "\r",
        "backspace" => "\x7f",
        "tab" => "\t",
        "escape" => "\x1b",
        "delete" => "\x1b[3~",
        "home" => "\x1b[H",
        "end" => "\x1b[F",
        "pageup" => "\x1b[5~",
        "pagedown" => "\x1b[6~",
        "up" => {
            if app {
                "\x1bOA"
            } else {
                "\x1b[A"
            }
        }
        "down" => {
            if app {
                "\x1bOB"
            } else {
                "\x1b[B"
            }
        }
        "right" => {
            if app {
                "\x1bOC"
            } else {
                "\x1b[C"
            }
        }
        "left" => {
            if app {
                "\x1bOD"
            } else {
                "\x1b[D"
            }
        }
        _ => {
            if ctrl && key.len() == 1 {
                let b = key.as_bytes()[0].to_ascii_uppercase();
                if (b'@'..=b'_').contains(&b) {
                    return Some(vec![b & 31]);
                }
            }
            if alt && key.chars().count() == 1 {
                return Some(format!("\x1b{key}").into_bytes());
            }
            return None;
        }
    };
    Some(seq.as_bytes().to_vec())
}
#[cfg(test)]
mod tests {
    use super::GridAlignment;
    use gpui::px;

    #[test]
    fn mixed_cjk_and_combining_glyphs_use_terminal_columns() {
        // A中e\u{301}文B: wide cells at columns 1 and 4; the accent
        // belongs to e at column 3, even if the fallback emits another glyph.
        let cells = [(0, 0), (1, 1), (4, 3), (7, 4), (10, 6)];
        let mut grid = GridAlignment::new(&cells, px(8.));
        for (byte, natural, expected) in [
            (0, 0., 0.),
            (1, 8., 8.),
            (4, 22., 24.),
            (5, 24., 26.),
            (7, 30., 32.),
            (10, 44., 48.),
        ] {
            assert_eq!(grid.position(byte, px(natural)), px(expected));
        }
    }

    #[test]
    fn chinese_copyright_does_not_use_one_column_per_glyph() {
        let text = "Corporation.保留所有权利。X";
        let mut parser = vt100::Parser::new(2, 80, 0);
        parser.process(text.as_bytes());
        let mut cells = Vec::new();
        let mut rebuilt = String::new();
        for column in 0..parser.screen().cursor_position().1 {
            let cell = parser.screen().cell(0, column).unwrap();
            if !cell.is_wide_continuation() {
                cells.push((rebuilt.len(), column));
                rebuilt.push_str(cell.contents());
            }
        }
        assert_eq!(rebuilt, text);
        let mut grid = GridAlignment::new(&cells, px(8.));
        let positions: Vec<_> = text
            .char_indices()
            .enumerate()
            .map(|(i, (byte, _))| grid.position(byte, px(i as f32 * 8.)))
            .collect();
        assert_eq!(
            &positions[12..],
            &[
                px(96.),
                px(112.),
                px(128.),
                px(144.),
                px(160.),
                px(176.),
                px(192.),
                px(208.)
            ]
        );
    }

    #[test]
    fn terminal_keys_and_application_cursor() {
        assert_eq!(super::key_bytes("c", true, false, false), Some(vec![3]));
        assert_eq!(
            super::key_bytes("up", false, false, true),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(super::key_bytes("a", false, false, false), None);
    }
}
