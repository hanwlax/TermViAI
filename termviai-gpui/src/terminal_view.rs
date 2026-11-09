use gpui::{prelude::*, *};
use std::{
    ops::Range,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use termviai_gpui::{
    session::{Input, Session},
    theme::{Colors, Theme},
};
use wezterm_term::{KeyCode, KeyModifiers};

pub enum TerminalEvent {
    Input(usize, Input),
    Focused(usize),
}
pub struct TerminalView {
    pub session: Arc<Session>,
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    cell: Pixels,
    line: Pixels,
    pub font_size: f32,
    theme: Theme,
    revision: u64,
    follow: u64,
    scroll: f32,
    target: f32,
    last_scroll: Instant,
    edge: bool,
    thumb_drag: Option<f32>,
    selecting: bool,
    selection: Option<((isize, usize), (isize, usize))>,
    top: isize,
    last_bottom: isize,
    composition: String,
    composition_selection: Range<usize>,
}
impl EventEmitter<TerminalEvent> for TerminalView {}
impl TerminalView {
    pub fn new(
        session: Arc<Session>,
        font_size: f32,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        cx.on_focus(&focus, window, |s, _, cx| {
            s.session.state.lock().unwrap().term.focus_changed(true);
            cx.emit(TerminalEvent::Focused(s.session.id));
            cx.notify();
        })
        .detach();
        cx.on_blur(&focus, window, |s, _, cx| {
            s.session.state.lock().unwrap().term.focus_changed(false);
            cx.notify();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this
                    .update(cx, |s, cx| {
                        let revision = s.session.revision.load(Ordering::Acquire);
                        let follow = s.session.follow.load(Ordering::Acquire);
                        let followed = follow != s.follow;
                        if follow != s.follow {
                            s.follow = follow;
                            s.scroll = 0.;
                            s.target = 0.;
                            s.selection = None;
                        }
                        let delta = s.target - s.scroll;
                        if delta.abs() > 0.01 {
                            s.scroll += delta * 0.3;
                        } else {
                            s.scroll = s.target;
                        }
                        if revision != s.revision
                            || delta.abs() > 0.01
                            || s.last_scroll.elapsed() < Duration::from_millis(1500)
                            || followed
                        {
                            s.revision = revision;
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
            session,
            focus,
            bounds: Bounds::default(),
            cell: px(8.),
            line: px(24.),
            font_size,
            theme,
            revision: 0,
            follow: 0,
            scroll: 0.,
            target: 0.,
            last_scroll: Instant::now() - Duration::from_secs(5),
            edge: false,
            thumb_drag: None,
            selecting: false,
            selection: None,
            top: 0,
            last_bottom: 0,
            composition: String::new(),
            composition_selection: 0..0,
        }
    }
    pub fn appearance(&mut self, size: f32, theme: Theme, cx: &mut Context<Self>) {
        self.font_size = size;
        self.theme = theme;
        self.session.theme(theme);
        cx.notify();
    }
    fn emit(&mut self, input: Input, cx: &mut Context<Self>) {
        if !matches!(input, Input::KeyUp(_, _)) {
            self.scroll = 0.;
            self.target = 0.;
            self.selection = None;
        }
        cx.emit(TerminalEvent::Input(self.session.id, input));
        cx.notify();
    }
    fn coord(&self, p: Point<Pixels>) -> (isize, usize) {
        (
            self.top + ((p.y - self.bounds.top()) / self.line).floor().max(0.) as isize,
            ((p.x - self.bounds.left()) / self.cell).floor().max(0.) as usize,
        )
    }
    fn copy(&self, cx: &mut App) {
        if let Some((a, b)) = self.selection {
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            let st = self.session.state.lock().unwrap();
            let screen = st.term.screen();
            let range = screen.stable_range(&(a.0..b.0 + 1));
            let lines = screen.lines_in_phys_range(range);
            let mut text = String::new();
            for (i, line) in lines.iter().enumerate() {
                let start = if i == 0 { a.1 } else { 0 };
                let end = if i + 1 == lines.len() {
                    b.1 + 1
                } else {
                    line.len()
                };
                text.push_str(line.columns_as_str(start..end).trim_end());
                if i + 1 < lines.len() && !line.last_cell_was_wrapped() {
                    text.push('\n');
                }
            }
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
    fn key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.composition.is_empty() {
            return;
        }
        let k = &e.keystroke;
        if k.modifiers.control && k.modifiers.shift && k.key.eq_ignore_ascii_case("c") {
            self.copy(cx);
            cx.stop_propagation();
            return;
        }
        if k.modifiers.control && k.modifiers.shift && k.key.eq_ignore_ascii_case("v") {
            if let Some(t) = cx.read_from_clipboard().and_then(|c| c.text()) {
                self.emit(Input::Paste(t), cx);
            }
            cx.stop_propagation();
            return;
        }
        if k.modifiers.control && ["+", "=", "-", "0"].contains(&k.key.as_str()) {
            self.font_size = if k.key == "0" {
                16.
            } else {
                (self.font_size + if k.key == "-" { -1. } else { 1. }).clamp(8., 64.)
            };
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(key) = key_code(k) {
            self.emit(Input::Key(key, modifiers(k.modifiers)), cx);
            cx.stop_propagation();
        }
    }
    fn mouse(&mut self, e: &MouseDownEvent, w: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(w);
        cx.emit(TerminalEvent::Focused(self.session.id));
        if e.position.x > self.bounds.right() - px(10.) && self.history() > 0 {
            let h = self.history() as f32;
            let rows = (self.bounds.size.height / self.line).floor();
            let thumb = (self.bounds.size.height * rows / (h + rows)).max(px(28.));
            let top =
                self.bounds.top() + (self.bounds.size.height - thumb) * (1. - self.scroll / h);
            self.thumb_drag = Some(if e.position.y >= top && e.position.y <= top + thumb {
                f32::from(e.position.y - top)
            } else {
                f32::from(thumb) / 2.
            });
            self.drag_scroll(e.position);
            cx.notify();
            cx.stop_propagation();
            return;
        }
        let (row, col) = self.coord(e.position);
        if e.modifiers.control {
            let st = self.session.state.lock().unwrap();
            let sc = st.term.screen();
            let lines = sc.lines_in_phys_range(sc.stable_range(&(row..row + 1)));
            if let Some(line) = lines.first() {
                if let Some(cell) = line
                    .visible_cells()
                    .find(|c| c.cell_index() <= col && col < c.cell_index() + c.width())
                {
                    if let Some(link) = cell.attrs().hyperlink() {
                        let url = link.uri();
                        if url.starts_with("https://") || url.starts_with("http://") {
                            cx.open_url(url);
                            cx.stop_propagation();
                            return;
                        }
                    }
                }
                let text = line.as_str();
                let mut column = 0;
                for word in text.split_inclusive(char::is_whitespace) {
                    let width = wezterm_term::unicode_column_width(word, None);
                    if col >= column && col < column + width {
                        let url = word.trim();
                        if url.starts_with("https://") || url.starts_with("http://") {
                            cx.open_url(url);
                            cx.stop_propagation();
                            return;
                        }
                    }
                    column += width;
                }
            }
        }
        if !e.modifiers.shift && self.session.state.lock().unwrap().term.is_mouse_grabbed() {
            self.send_mouse(
                e.position,
                wezterm_term::MouseEventKind::Press,
                wezterm_term::MouseButton::Left,
                e.modifiers,
            );
            return;
        }
        self.selecting = true;
        self.selection = Some(((row, col), (row, col)));
        cx.notify();
    }
    fn send_mouse(
        &self,
        pos: Point<Pixels>,
        kind: wezterm_term::MouseEventKind,
        button: wezterm_term::MouseButton,
        mods: Modifiers,
    ) {
        let _ = self
            .session
            .state
            .lock()
            .unwrap()
            .term
            .mouse_event(wezterm_term::MouseEvent {
                kind,
                x: ((pos.x - self.bounds.left()) / self.cell).max(0.) as usize,
                y: ((pos.y - self.bounds.top()) / self.line).max(0.) as i64,
                x_pixel_offset: 0,
                y_pixel_offset: 0,
                button,
                modifiers: modifiers(mods),
            });
    }
    fn history(&self) -> usize {
        self.session.state.lock().unwrap().term.screen().phys_row(0)
    }
    fn drag_scroll(&mut self, pos: Point<Pixels>) {
        let h = self.history() as f32;
        let rows = (self.bounds.size.height / self.line).floor();
        let thumb = (self.bounds.size.height * rows / (h + rows)).max(px(28.));
        let y = ((pos.y - self.bounds.top() - px(self.thumb_drag.unwrap_or(0.)))
            / (self.bounds.size.height - thumb).max(px(1.)))
        .clamp(0., 1.);
        self.target = (1. - y) * h;
        self.scroll = self.target;
        self.last_scroll = Instant::now();
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, w: &mut Window, cx: &mut Context<Self>) {
        let mut font = font("JetBrainsMono Nerd Font Mono");
        font.fallbacks = Some(FontFallbacks::from_fonts(vec![
            "JetBrains Mono".into(),
            "Symbols Nerd Font Mono".into(),
            "Microsoft YaHei UI".into(),
        ]));
        let id = w.text_system().resolve_font(&font);
        self.cell = w
            .text_system()
            .advance(id, px(self.font_size), 'M')
            .map(|s| s.width)
            .unwrap_or(px(self.font_size * 0.6));
        self.line = px((self.font_size * 1.38).ceil());
        self.bounds = bounds;
        let cols = ((bounds.size.width - px(10.)) / self.cell).floor().max(2.) as usize;
        let rows = (bounds.size.height / self.line).floor().max(2.) as usize;
        self.session.resize(
            rows,
            cols,
            f32::from(bounds.size.width) as usize,
            f32::from(bounds.size.height) as usize,
            (96. * w.scale_factor()) as u32,
        );
        let (lines, palette, cursor, top, history, alt) = {
            let st = self.session.state.lock().unwrap();
            let term = &st.term;
            let sc = term.screen();
            let history = sc.phys_row(0);
            let bottom = sc.phys_to_stable_row_index(history);
            if self.target > 0. && bottom > self.last_bottom {
                let added = (bottom - self.last_bottom) as f32;
                self.target += added;
                self.scroll += added;
            }
            self.last_bottom = bottom;
            self.target = self.target.clamp(0., history as f32);
            self.scroll = self.scroll.clamp(0., history as f32);
            let start = history.saturating_sub(self.scroll.ceil() as usize);
            (
                sc.lines_in_phys_range(start..start + rows + 1),
                term.palette(),
                term.cursor_pos(),
                sc.phys_to_stable_row_index(start),
                history,
                term.is_alt_screen_active(),
            )
        };
        self.top = top;
        let fractional = if self.scroll.fract() > 0. {
            1. - self.scroll.fract()
        } else {
            0.
        };
        w.with_content_mask(Some(ContentMask { bounds }), |w| {
            for (r, line) in lines.iter().enumerate() {
                let mut text = String::new();
                let mut runs = Vec::new();
                let mut starts = Vec::new();
                for cell in line.visible_cells() {
                    let col = cell.cell_index();
                    if col >= cols {
                        break;
                    }
                    let attrs = cell.attrs();
                    let mut fg = palette.resolve_fg(attrs.foreground());
                    let mut bg = palette.resolve_bg(attrs.background());
                    if attrs.reverse() {
                        std::mem::swap(&mut fg, &mut bg);
                    }
                    if attrs.invisible() {
                        fg = bg;
                    }
                    let selected = self.selection.is_some_and(|(a, b)| {
                        let (a, b) = if a <= b { (a, b) } else { (b, a) };
                        (top + r as isize, col) >= a && (top + r as isize, col) <= b
                    });
                    if selected {
                        bg = palette.selection_bg;
                        fg = palette.selection_fg;
                    }
                    let mut f = font.clone();
                    if attrs.intensity() == wezterm_term::Intensity::Bold {
                        f.weight = FontWeight::BOLD;
                    }
                    if attrs.italic() {
                        f.style = FontStyle::Italic;
                    }
                    let s = cell.str();
                    starts.push((text.len(), col));
                    text.push_str(s);
                    runs.push(TextRun {
                        len: s.len(),
                        font: f,
                        color: rgba_color(fg),
                        background_color: Some(rgba_color(bg)),
                        underline: if attrs.underline() != wezterm_term::Underline::None {
                            Some(UnderlineStyle {
                                color: None,
                                thickness: px(1.),
                                wavy: false,
                            })
                        } else {
                            None
                        },
                        strikethrough: if attrs.strikethrough() {
                            Some(StrikethroughStyle {
                                color: None,
                                thickness: px(1.),
                            })
                        } else {
                            None
                        },
                    });
                }
                let mut shaped =
                    w.text_system()
                        .shape_line(text.into(), px(self.font_size), &runs, None);
                let mut layout = LineLayout {
                    font_size: shaped.font_size,
                    width: self.cell * cols as f32,
                    ascent: shaped.ascent,
                    descent: shaped.descent,
                    runs: shaped.runs.clone(),
                    len: shaped.len(),
                };
                let mut origins = vec![None; starts.len()];
                for run in &mut layout.runs {
                    for glyph in &mut run.glyphs {
                        if let Some(i) = starts
                            .partition_point(|(b, _)| *b <= glyph.index)
                            .checked_sub(1)
                        {
                            let natural = glyph.position.x;
                            let origin = *origins[i].get_or_insert(natural);
                            glyph.position.x = self.cell * starts[i].1 as f32 + natural - origin;
                        }
                    }
                }
                *shaped = Arc::new(layout);
                let y = bounds.top() + self.line * (r as f32 - fractional);
                let _ = shaped.paint(point(bounds.left(), y), self.line, w, cx);
            }
            if self.scroll < 0.01
                && cursor.visibility == termwiz::surface::CursorVisibility::Visible
                && self.focus.is_focused(w)
            {
                let origin = point(
                    bounds.left() + self.cell * cursor.x as f32,
                    bounds.top() + self.line * cursor.y as f32,
                );
                w.paint_quad(fill(
                    Bounds::new(origin, size(px(2.), self.line)),
                    Colors::color(self.theme.colors().accent),
                ));
                if !self.composition.is_empty() {
                    let run = TextRun {
                        len: self.composition.len(),
                        font: font.clone(),
                        color: Colors::color(self.theme.colors().text),
                        background_color: Some(Colors::color(self.theme.colors().soft)),
                        underline: Some(UnderlineStyle {
                            color: None,
                            thickness: px(1.),
                            wavy: false,
                        }),
                        strikethrough: None,
                    };
                    let text = w.text_system().shape_line(
                        self.composition.clone().into(),
                        px(self.font_size),
                        &[run],
                        None,
                    );
                    let _ = text.paint(origin, self.line, w, cx);
                }
            }
            let elapsed = self.last_scroll.elapsed().as_secs_f32();
            let opacity = if self.edge || self.thumb_drag.is_some() {
                1.
            } else {
                ((1.5 - elapsed) / 0.4).clamp(0., 1.)
            };
            if history > 0 && !alt && opacity > 0. {
                let h = (bounds.size.height * rows as f32 / (history + rows) as f32).max(px(28.));
                let y = (bounds.size.height - h) * (1. - self.scroll / history as f32);
                let mut c = Colors::color(self.theme.colors().muted);
                c.a = opacity * 0.65;
                w.paint_quad(
                    fill(
                        Bounds::new(
                            point(bounds.right() - px(7.), bounds.top() + y),
                            size(px(5.), h),
                        ),
                        c,
                    )
                    .corner_radii(px(3.)),
                );
            }
        });
        w.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, cx.entity()),
            cx,
        );
    }
}
fn rgba_color(c: wezterm_term::color::SrgbaTuple) -> Hsla {
    Rgba {
        r: c.0,
        g: c.1,
        b: c.2,
        a: c.3,
    }
    .into()
}
fn modifiers(m: Modifiers) -> KeyModifiers {
    let mut r = KeyModifiers::NONE;
    if m.control {
        r |= KeyModifiers::CTRL;
    }
    if m.shift {
        r |= KeyModifiers::SHIFT;
    }
    if m.alt {
        r |= KeyModifiers::ALT;
    }
    if m.platform {
        r |= KeyModifiers::SUPER;
    }
    r
}
fn key_code(k: &Keystroke) -> Option<KeyCode> {
    Some(match k.key.as_str() {
        "enter" => KeyCode::Enter,
        "backspace" => KeyCode::Backspace,
        "tab" => KeyCode::Tab,
        "escape" => KeyCode::Escape,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::UpArrow,
        "down" => KeyCode::DownArrow,
        "left" => KeyCode::LeftArrow,
        "right" => KeyCode::RightArrow,
        s if s.starts_with('f') && s[1..].parse::<u8>().is_ok() => {
            KeyCode::Function(s[1..].parse().ok()?)
        }
        s if (k.modifiers.control || k.modifiers.alt) && s.chars().count() == 1 => {
            KeyCode::Char(s.chars().next()?)
        }
        _ => return None,
    })
}
impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .id("terminal")
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context(termviai_gpui::terminal_keys::CONTEXT)
            .bg(Colors::color(self.theme.colors().terminal))
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key))
            .on_key_up(cx.listener(|s, e: &KeyUpEvent, _, cx| {
                if let Some(key) = key_code(&e.keystroke) {
                    if s.session.online() {
                        s.emit(Input::KeyUp(key, modifiers(e.keystroke.modifiers)), cx);
                    }
                }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse))
            .on_mouse_move(cx.listener(|s, e: &MouseMoveEvent, _, cx| {
                let edge = e.position.x > s.bounds.right() - px(12.);
                if s.edge != edge {
                    s.edge = edge;
                    cx.notify();
                }
                if s.thumb_drag.is_some() {
                    s.drag_scroll(e.position);
                    cx.notify();
                } else if s.selecting {
                    if let Some((a, _)) = s.selection {
                        s.selection = Some((a, s.coord(e.position)));
                        cx.notify();
                    }
                } else if s.session.state.lock().unwrap().term.is_mouse_grabbed() {
                    s.send_mouse(
                        e.position,
                        wezterm_term::MouseEventKind::Move,
                        wezterm_term::MouseButton::None,
                        e.modifiers,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, e: &MouseUpEvent, _, cx| {
                    if !s.selecting && s.thumb_drag.is_none() {
                        s.send_mouse(
                            e.position,
                            wezterm_term::MouseEventKind::Release,
                            wezterm_term::MouseButton::Left,
                            e.modifiers,
                        );
                    }
                    s.selecting = false;
                    s.thumb_drag = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _, _, cx| {
                    s.selecting = false;
                    s.thumb_drag = None;
                    cx.notify();
                }),
            )
            .on_hover(cx.listener(|s, hover, _, cx| {
                if !hover {
                    s.edge = false;
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|s, e: &ScrollWheelEvent, _, cx| {
                if e.modifiers.control {
                    s.font_size = (s.font_size
                        + if e.delta.pixel_delta(s.line).y > px(0.) {
                            1.
                        } else {
                            -1.
                        })
                    .clamp(8., 64.);
                } else if s.session.state.lock().unwrap().term.is_mouse_grabbed()
                    && !e.modifiers.shift
                {
                    let button = if e.delta.pixel_delta(s.line).y > px(0.) {
                        wezterm_term::MouseButton::WheelUp(3)
                    } else {
                        wezterm_term::MouseButton::WheelDown(3)
                    };
                    s.send_mouse(
                        e.position,
                        wezterm_term::MouseEventKind::Press,
                        button,
                        e.modifiers,
                    );
                } else {
                    s.target = (s.target + e.delta.pixel_delta(s.line).y / s.line)
                        .clamp(0., s.history() as f32);
                    s.last_scroll = Instant::now();
                }
                cx.notify();
                cx.stop_propagation();
            }))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, w, cx| entity.update(cx, |s, cx| s.paint(bounds, w, cx)),
                )
                .size_full(),
            )
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
        (!self.composition.is_empty()).then(|| 0..self.composition.encode_utf16().count())
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
        self.emit(Input::Text(text.into()), cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        range: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition = text.into();
        let n = text.encode_utf16().count();
        self.composition_selection = range.unwrap_or(n..n);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let c = self.session.state.lock().unwrap().term.cursor_pos();
        Some(Bounds::new(
            point(
                self.bounds.left() + self.cell * c.x as f32,
                self.bounds.top() + self.line * c.y as f32,
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
