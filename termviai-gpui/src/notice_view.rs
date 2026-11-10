//! Per-terminal, non-modal messages. Remote text is escaped before the native
//! GPUI text renderer; it is never interpreted as links, commands or markup.
use crate::terminal_view::TerminalView;
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, IconName,
    text::{TextView, TextViewStyle},
};
use std::{collections::HashSet, sync::Arc, time::Duration};
use termviai_gpui::{
    session::Session,
    terminal_notifications::Notice,
    theme::{Colors, Theme},
};

pub struct RetryCredentialSave(pub usize);
impl EventEmitter<RetryCredentialSave> for NoticeView {}
pub struct NoticeView {
    session: Arc<Session>,
    terminal: WeakEntity<TerminalView>,
    label: String,
    theme: Theme,
    error: String,
    items: Vec<Notice>,
    unread: usize,
    revision: u64,
    open: bool,
    detailed: bool,
    error_tab: bool,
    pinned: bool,
    epoch: u64,
    expanded: HashSet<u64>,
    bounds: Bounds<Pixels>,
    focus: FocusHandle,
    scroll: ScrollHandle,
}
impl NoticeView {
    pub fn new(
        session: Arc<Session>,
        terminal: WeakEntity<TerminalView>,
        label: String,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update(cx, |s, cx| {
                        let revision = s.session.state.lock().unwrap().notifications.revision;
                        if revision != s.revision {
                            // Do not move selectable text under the pointer when a
                            // new notice arrives. Refresh the snapshot on reopening.
                            if s.open && s.pinned {
                                let st = s.session.state.lock().unwrap();
                                s.revision = st.notifications.revision;
                                s.unread = st.notifications.unread();
                            } else {
                                s.refresh();
                            }
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
            terminal,
            label,
            theme,
            error: String::new(),
            items: vec![],
            unread: 0,
            revision: 0,
            open: false,
            detailed: false,
            error_tab: false,
            pinned: false,
            epoch: 0,
            expanded: HashSet::new(),
            bounds: Bounds::default(),
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
        }
    }
    pub fn appearance(
        &mut self,
        theme: Theme,
        error: String,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        if self.theme != theme || self.error != error {
            self.theme = theme;
            self.error = error;
            if self.error.is_empty() {
                self.error_tab = false;
            }
            cx.notify();
        }
        if !visible {
            self.epoch += 1;
            if self.open {
                self.dismiss(cx);
            }
        }
    }
    fn refresh(&mut self) {
        let st = self.session.state.lock().unwrap();
        self.revision = st.notifications.revision;
        self.unread = st.notifications.unread();
        self.items = st.notifications.items.iter().cloned().collect();
        self.expanded
            .retain(|id| self.items.iter().any(|n| n.id == *id));
    }
    fn c(&self, s: &str) -> Hsla {
        Colors::color(s)
    }
    fn button(&self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(id)
            .px_2()
            .py_1()
            .rounded_md()
            .cursor(CursorStyle::PointingHand)
            .text_size(px(12.))
            .hover(|s| s.bg(self.c(self.theme.colors().icon_hover)))
            .child(label.into())
    }
    fn open_details(&mut self, error: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.open = true;
        self.detailed = true;
        self.error_tab = error;
        self.pinned = true;
        if error {
            cx.emit(RetryCredentialSave(self.session.id));
        }
        if !error {
            self.session.state.lock().unwrap().notifications.mark_read();
        }
        self.refresh();
        self.focus.focus(w);
        cx.notify();
    }
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.open = false;
        self.pinned = false;
        self.detailed = false;
        cx.notify();
    }
    fn return_focus(&self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = self.terminal.upgrade() {
            t.focus_handle(cx).focus(w);
        }
    }
    fn hover(&mut self, enter: bool, error: bool, cx: &mut Context<Self>) {
        self.epoch += 1;
        if self.detailed || self.pinned {
            return;
        }
        let epoch = self.epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(if enter { 280 } else { 220 }))
                .await;
            let _ = this.update(cx, |s, cx| {
                if epoch == s.epoch && !s.pinned && !s.detailed {
                    s.open = enter
                        && (if error {
                            !s.error.is_empty()
                        } else {
                            !s.items.is_empty()
                        });
                    s.error_tab = error;
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn text(
        &self,
        id: impl Into<ElementId>,
        text: &str,
        mono: bool,
        w: &mut Window,
        cx: &mut App,
    ) -> TextView {
        let html = termviai_gpui::terminal_notifications::literal_text_html(text);
        TextView::html(id, html, w, cx)
            .selectable(true)
            .font_family(if mono { "Consolas" } else { "Segoe UI" })
            .text_size(px(if mono { 12. } else { 13. }))
            .style(TextViewStyle::default().paragraph_gap(rems(0.)))
    }
    fn content(&self, w: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let c = self.theme.colors();
        let mut content = div().flex().flex_col().gap_2().p_3().min_w_0();
        if self.error_tab {
            return content
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(self.c(c.red))
                        .child("Terminal error"),
                )
                .child(self.text("error-text", &self.error, false, w, cx))
                .into_any_element();
        }
        if self.items.is_empty() {
            return content
                .py_6()
                .text_color(self.c(c.muted))
                .child("No notifications")
                .into_any_element();
        }
        let n = if self.detailed { self.items.len() } else { 1 };
        for item in self.items.iter().take(n) {
            let id = item.id as usize;
            let mut message = div()
                .id(("notice", id))
                .flex()
                .flex_col()
                .gap_2()
                .min_w_0()
                .when(self.detailed, |d| {
                    d.pb_3().border_b_1().border_color(self.c(c.line))
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::Bell)
                                .size(px(14.))
                                .text_color(self.c(c.blue)),
                        )
                        .child(div().flex_1().text_size(px(13.)).child("System broadcast")),
                )
                .child(self.text(
                    ("notice-meta", id),
                    &format!("{} · {}\n{}", item.sender, item.tty, item.date),
                    false,
                    w,
                    cx,
                ))
                .child(self.text(("notice-body", id), &item.body, false, w, cx));
            if self.detailed {
                let raw = self.expanded.contains(&item.id);
                let notice_id = item.id;
                message = message.child(
                    self.button(
                        ("notice-original", id),
                        if raw {
                            "Hide original"
                        } else {
                            "View original"
                        },
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if !s.expanded.remove(&notice_id) {
                            s.expanded.insert(notice_id);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    })),
                );
                if raw {
                    message =
                        message.child(div().p_2().rounded_md().bg(self.c(c.bg)).child(self.text(
                            ("notice-raw", id),
                            &item.original,
                            true,
                            w,
                            cx,
                        )));
                }
            }
            content = content.child(message);
        }
        content.into_any_element()
    }
}
impl Render for NoticeView {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = self.theme.colors();
        let entity = cx.entity();
        let mut root = div()
            .id("terminal-messages")
            .relative()
            .flex()
            .items_center()
            .gap_1()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation());
        if !self.error.is_empty() {
            root = root.child(
                div()
                    .id("error-indicator")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor(CursorStyle::PointingHand)
                    .hover(|d| d.bg(self.c(c.icon_hover)))
                    .child(
                        div()
                            .size(px(17.))
                            .rounded_full()
                            .bg(self.c(c.red))
                            .text_color(rgb(0xffffff))
                            .text_size(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("!"),
                    )
                    .on_hover(cx.listener(|s, hover, _, cx| s.hover(*hover, true, cx)))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.open_details(true, w, cx);
                        cx.stop_propagation();
                    })),
            );
        }
        if !self.items.is_empty() {
            root = root.child(
                div()
                    .id("notification-indicator")
                    .h(px(26.))
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded_md()
                    .cursor(CursorStyle::PointingHand)
                    .text_color(self.c(if self.unread > 0 { c.blue } else { c.muted }))
                    .hover(|d| d.bg(self.c(c.icon_hover)))
                    .child(
                        div()
                            .size(px(19.))
                            .rounded_full()
                            .bg(self.c(c.blue).opacity(0.13))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::Bell).size(px(13.))),
                    )
                    .when(self.unread > 0, |d| {
                        d.child(div().text_size(px(11.)).child(if self.unread > 99 {
                            "99+".into()
                        } else {
                            self.unread.to_string()
                        }))
                    })
                    .on_hover(cx.listener(|s, hover, _, cx| s.hover(*hover, false, cx)))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.open_details(false, w, cx);
                        cx.stop_propagation();
                    })),
            );
        }
        root = root.child(
            canvas(
                move |bounds, _, cx| entity.update(cx, |s, _| s.bounds = bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
        if self.open {
            let viewport = w.viewport_size();
            let height = px(if self.detailed { 420. } else { 220. })
                .min((viewport.height - px(80.)).max(px(100.)));
            let width = px(360.).min((viewport.width - px(24.)).max(px(100.)));
            let y = (self.bounds.bottom() + px(6.))
                .min(viewport.height - height - px(12.))
                .max(px(56.));
            let mut panel = div()
                .id("message-popup")
                .w(width)
                .max_h(height)
                .flex()
                .flex_col()
                .min_h_0()
                .rounded_lg()
                .border_1()
                .border_color(self.c(c.line))
                .bg(self.c(c.card))
                .text_color(self.c(c.text))
                .font_family("Segoe UI")
                .text_size(px(13.))
                .shadow_lg()
                .occlude()
                .track_focus(&self.focus)
                .capture_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                    if e.keystroke.key == "escape" {
                        s.dismiss(cx);
                        s.return_focus(w, cx);
                        cx.stop_propagation();
                    }
                }))
                .on_hover(cx.listener(|s, hover, _, cx| {
                    if *hover {
                        s.epoch += 1;
                    } else {
                        s.hover(false, s.error_tab, cx);
                    }
                }))
                // Pin the same preview tree while selecting, so pointer exit never destroys selection.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|s, _, _, cx| {
                        s.pinned = true;
                        s.epoch += 1;
                        cx.stop_propagation();
                    }),
                )
                .on_click(|_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|s, e: &MouseDownEvent, _, cx| {
                    if !s.bounds.contains(&e.position) {
                        s.dismiss(cx);
                    }
                }));
            if self.detailed {
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .flex_shrink_0()
                        .border_b_1()
                        .border_color(self.c(c.line))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(format!("{} · Messages", self.label)),
                        )
                        .child(
                            div()
                                .id("close-messages")
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor(CursorStyle::PointingHand)
                                .hover(|d| d.text_color(self.c(c.red)))
                                .child(Icon::new(IconName::Close).size(px(14.)))
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.dismiss(cx);
                                    s.return_focus(w, cx);
                                    cx.stop_propagation();
                                })),
                        ),
                );
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_1()
                        .flex_shrink_0()
                        .border_b_1()
                        .border_color(self.c(c.line))
                        .child(
                            self.button(
                                "messages-notices",
                                format!("Notifications {}", self.items.len()),
                            )
                            .text_color(self.c(if !self.error_tab { c.blue } else { c.muted }))
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.open_details(false, w, cx);
                                cx.stop_propagation();
                            })),
                        )
                        .when(!self.error.is_empty(), |d| {
                            d.child(
                                self.button("messages-errors", "Error 1")
                                    .text_color(self.c(if self.error_tab {
                                        c.red
                                    } else {
                                        c.muted
                                    }))
                                    .on_click(cx.listener(|s, _, w, cx| {
                                        s.open_details(true, w, cx);
                                        cx.stop_propagation();
                                    })),
                            )
                        }),
                );
            }
            panel = panel.child(
                div()
                    .id("message-scroll")
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(self.content(w, cx)),
            );
            if self.detailed {
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(self.c(c.line))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(self.c(c.muted))
                                .child("Select text · Ctrl+C"),
                        )
                        .when(!self.error_tab && !self.items.is_empty(), |d| {
                            d.child(
                                self.button("clear-notices", "Clear notifications")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.session.state.lock().unwrap().notifications.clear();
                                        s.refresh();
                                        cx.stop_propagation();
                                        cx.notify();
                                    })),
                            )
                        }),
                );
            } else {
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .flex_shrink_0()
                        .border_t_1()
                        .border_color(self.c(c.line))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(self.c(c.muted))
                                .child("Select text · Ctrl+C"),
                        )
                        .child(
                            self.button("view-all-messages", "View all")
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.open_details(s.error_tab, w, cx);
                                    cx.stop_propagation();
                                })),
                        ),
                );
            }
            root = root.child(
                deferred(
                    anchored()
                        .anchor(Corner::TopRight)
                        .position(point(self.bounds.right().max(width + px(12.)), y))
                        .snap_to_window_with_margin(px(8.))
                        .child(panel),
                )
                .with_priority(2),
            );
        }
        root
    }
}
