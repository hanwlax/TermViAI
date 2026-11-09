#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod pty;
mod terminal;
use gpui::prelude::*;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, IconName, Root, Selectable, Sizable, Theme, ThemeMode};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use terminal::TerminalView;

#[derive(Clone, Copy)]
struct Palette {
    bg: u32,
    side: u32,
    card: u32,
    line: u32,
    text: u32,
    muted: u32,
    accent: u32,
    soft: u32,
    hover: u32,
}
impl Palette {
    fn get(n: usize) -> Self {
        match n {
            1 => Self {
                bg: 0x101c2b,
                side: 0x142335,
                card: 0x1b2d42,
                line: 0x2b4058,
                text: 0xe5edf7,
                muted: 0x9aaec4,
                accent: 0x8ac7ea,
                soft: 0x213d52,
                hover: 0x293d54,
            },
            2 => Self {
                bg: 0xf7f8fc,
                side: 0xeff1f7,
                card: 0xffffff,
                line: 0xdde1eb,
                text: 0x242a39,
                muted: 0x657086,
                accent: 0x7462b4,
                soft: 0xe9e4f5,
                hover: 0xe9edf5,
            },
            _ => Self {
                bg: 0x181b25,
                side: 0x1e222e,
                card: 0x242936,
                line: 0x343b4c,
                text: 0xe6e9f2,
                muted: 0x9ca7ba,
                accent: 0xb4a2dc,
                soft: 0x373248,
                hover: 0x303747,
            },
        }
    }
}
#[derive(Clone)]
struct Host {
    label: String,
    address: String,
    group: usize,
}
struct Form {
    label: Entity<InputState>,
    address: Entity<InputState>,
    user: Entity<InputState>,
    password: Entity<InputState>,
    error: String,
}
struct Probe {
    page: usize,
    theme: usize,
    group: Option<usize>,
    search: Entity<InputState>,
    hosts: Vec<Host>,
    form: Option<Form>,
    terminals: Vec<Entity<TerminalView>>,
    font_size: f32,
    notice: String,
    started: Instant,
    report: Option<PathBuf>,
}
fn button(id: impl Into<ElementId>, label: &str) -> Button {
    Button::new(id).label(label.to_owned()).small()
}
impl Probe {
    fn new(window: &mut Window, cx: &mut Context<Self>, report: Option<PathBuf>) -> Self {
        let search = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search hosts by name or address…"));
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        if report.is_some() {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if this
                        .update(cx, |p, cx| {
                            if !p.terminals.is_empty() {
                                p.write_report(cx);
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }

        Self {
            page: 0,
            theme: 0,
            group: None,
            search,
            hosts: vec![
                Host {
                    label: "209".into(),
                    address: "192.0.2.209".into(),
                    group: 0,
                },
                Host {
                    label: "212".into(),
                    address: "192.0.2.212".into(),
                    group: 0,
                },
                Host {
                    label: "216".into(),
                    address: "192.0.2.216".into(),
                    group: 0,
                },
                Host {
                    label: "217".into(),
                    address: "192.0.2.217".into(),
                    group: 0,
                },
                Host {
                    label: "Build server".into(),
                    address: "192.0.2.88".into(),
                    group: 1,
                },
                Host {
                    label: "Development".into(),
                    address: "192.0.2.151".into(),
                    group: 1,
                },
            ],
            form: None,
            terminals: vec![],
            font_size: 14.,
            notice: "Sample hosts · local evaluation only".into(),
            started: Instant::now(),
            report,
        }
    }
    fn theme(&mut self, n: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = n;
        Theme::change(
            if n == 2 {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        let p = Palette::get(n);
        let t = Theme::global_mut(cx);
        t.font_family = "Segoe UI".into();
        t.font_size = px(14.);
        t.radius = px(7.);
        t.background = rgb(p.bg).into();
        t.foreground = rgb(p.text).into();
        t.border = rgb(p.line).into();
        t.input = rgb(p.line).into();
        t.primary = rgb(p.accent).into();
        t.accent = rgb(p.hover).into();
        t.muted_foreground = rgb(p.muted).into();
        for t in &self.terminals {
            t.update(cx, |t, cx| t.appearance(self.font_size, n == 2, cx));
        }
        cx.notify();
    }
    fn new_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.form = Some(Form {
            label: cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Development server")),
            address: cx.new(|cx| InputState::new(window, cx).placeholder("192.0.2.10")),
            user: cx.new(|cx| InputState::new(window, cx).placeholder("root")),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Password · not persisted")
                    .masked(true)
            }),
            error: String::new(),
        });
        self.form
            .as_ref()
            .unwrap()
            .label
            .read(cx)
            .focus_handle(cx)
            .focus(window);
        cx.notify();
    }
    fn spawn_terminal(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminals.len() >= 4 {
            self.notice = "Close a terminal before adding another (four-pane evaluation).".into();
            cx.notify();
            return;
        }
        let pty = match pty::Terminal::new(mode) {
            Ok(pty) => pty,
            Err(e) => {
                self.notice = e.to_string();
                cx.notify();
                return;
            }
        };
        let terminal = cx.new(|cx| TerminalView::new(pty, cx));
        terminal.update(cx, |t, cx| {
            t.appearance(self.font_size, self.theme == 2, cx)
        });
        terminal.read(cx).focus_handle(cx).focus(window);
        self.terminals.push(terminal);
        self.page = 1;
        cx.notify();
    }
    fn write_report(&self, cx: &App) {
        if let Some(path) = &self.report {
            let v = serde_json::json!({"backend":"GPUI 0.2.2 / Windows DirectX", "webview":false,"elapsed_seconds":self.started.elapsed().as_secs_f64(),"theme":self.theme,"terminals":self.terminals.iter().map(|t|t.read(cx).stats()).collect::<Vec<_>>(),"note":"paint_cpu_us measures application paint preparation, not GPU frame time or input latency"});
            let _ = std::fs::write(path, serde_json::to_vec_pretty(&v).unwrap());
        }
    }
    fn hosts_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::get(self.theme);
        let q = self.search.read(cx).value().to_lowercase();
        let filtered: Vec<_> = self
            .hosts
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                self.group.is_none_or(|g| g == h.group)
                    && (h.label.to_lowercase().contains(&q) || h.address.contains(&q))
            })
            .collect();
        let mut content = div()
            .flex()
            .flex_col()
            .gap_5()
            .p_6()
            .w_full()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(23.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Hosts"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(p.muted))
                                    .child("Your connections, in one place"),
                            ),
                    )
                    .child(
                        button("add-host", "Add Host")
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, w, cx| this.new_form(w, cx))),
                    ),
            )
            .child(
                Input::new(&self.search)
                    .prefix(Icon::new(IconName::Search).small())
                    .cleanable(true),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button("all-hosts", "All Hosts")
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.group = None;
                                cx.notify();
                            })),
                    )
                    .when_some(self.group, |d, g| {
                        d.child(IconName::ChevronRight).child(if g == 0 {
                            "blue-c-4"
                        } else {
                            "Development"
                        })
                    }),
            );
        if self.group.is_none() {
            content = content
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(p.muted))
                        .child("GROUPS"),
                )
                .child(
                    div().flex().gap_3().children(
                        ["blue-c-4", "Development"]
                            .into_iter()
                            .enumerate()
                            .map(|(i, name)| {
                                div()
                                    .id(("group", i))
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px_4()
                                    .py_3()
                                    .w(px(248.))
                                    .rounded_lg()
                                    .bg(rgb(p.card))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(p.hover)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.group = Some(i);
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .p_2()
                                            .rounded_md()
                                            .bg(rgb(p.soft))
                                            .text_color(rgb(p.accent))
                                            .child(Icon::new(IconName::Folder).size_5()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div().font_weight(FontWeight::SEMIBOLD).child(name),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(rgb(p.muted))
                                                    .child(format!(
                                                        "{} hosts",
                                                        self.hosts
                                                            .iter()
                                                            .filter(|h| h.group == i)
                                                            .count()
                                                    )),
                                            ),
                                    )
                            }),
                    ),
                );
        }
        content = content
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child("HOSTS")
                    .child(format!("{} connections", filtered.len())),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(filtered.into_iter().map(|(i, h)| {
                        div()
                            .id(("host", i))
                            .group("host-card")
                            .flex()
                            .items_center()
                            .gap_3()
                            .w(px(248.))
                            .min_w(px(220.))
                            .px_3()
                            .py_3()
                            .rounded_lg()
                            .bg(rgb(p.card))
                            .hover(|s| s.bg(rgb(p.hover)))
                            .child(
                                div()
                                    .size_9()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .rounded_lg()
                                    .bg(rgb(p.soft))
                                    .text_color(rgb(p.accent))
                                    .child(Icon::new(IconName::SquareTerminal).size_5()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(h.label.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(p.muted))
                                            .child(format!("{} · sample", h.address)),
                                    ),
                            )
                            .child(
                                Button::new(("delete-host", i))
                                    .opacity(0.)
                                    .group_hover("host-card", |s| s.opacity(1.))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.hosts.remove(i);
                                        cx.notify();
                                    })),
                            )
                    })),
            );
        if self.hosts.is_empty() {
            content = content.child("No sample hosts. Add Host to try the form.");
        }
        div()
            .id("hosts-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(content)
            .into_any_element()
    }
    fn terminal_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::get(self.theme);
        let n = self.terminals.len();
        let toolbar = div()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(rgb(p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Local terminals"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.muted))
                            .child(format!("{n}/4 · ConPTY")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("add-terminal", "New terminal")
                            .icon(IconName::Plus)
                            .on_click(
                                cx.listener(|this, _, w, cx| this.spawn_terminal("shell", w, cx)),
                            ),
                    )
                    .child(button("four-terminals", "Four panes").on_click(cx.listener(
                        |this, _, w, cx| {
                            while this.terminals.len() < 4 {
                                let before = this.terminals.len();
                                this.spawn_terminal("shell", w, cx);
                                if before == this.terminals.len() {
                                    break;
                                }
                            }
                        },
                    )))
                    .child(button("stream", "Output test").on_click(
                        cx.listener(|this, _, w, cx| this.spawn_terminal("stream", w, cx)),
                    )),
            );
        let mut body = div().flex().flex_col().flex_1().min_h_0().gap_2().p_3();
        if n == 0 {
            body = body
                .items_center()
                .justify_center()
                .child(Icon::new(IconName::SquareTerminal).size_8())
                .child("Open a local terminal to test native input and rendering.");
        }
        for (row, chunk) in self
            .terminals
            .chunks(if n == 1 { 1 } else { 2 })
            .enumerate()
        {
            body = body.child(div().flex().flex_1().min_h_0().gap_2().children(
                chunk.iter().enumerate().map(|(col, t)| {
                    let i = row * 2 + col;
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .border_1()
                        .border_color(rgb(p.line))
                        .rounded_lg()
                        .overflow_hidden()
                        .child(
                            div()
                                .h_9()
                                .flex()
                                .flex_shrink_0()
                                .px_3()
                                .items_center()
                                .justify_between()
                                .bg(rgb(p.side))
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::SquareTerminal).small())
                                        .child(format!("Local {}", i + 1)),
                                )
                                .child(
                                    Button::new(("close-terminal", i))
                                        .ghost()
                                        .small()
                                        .icon(IconName::Close)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.terminals.remove(i);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .p_2()
                                .bg(rgb(if self.theme == 2 { 0xf9fafc } else { 0x171a24 }))
                                .child(t.clone()),
                        )
                }),
            ));
        }
        div().flex().flex_col().flex_1().min_h_0().child(toolbar).child(body)
            .child(div().px_4().pb_2().text_size(px(11.)).text_color(rgb(p.muted)).child("Ctrl+Shift+C / V · drag to select · wheel for history · output test: 30,000 lines / ~30s"))
            .into_any_element()
    }
    fn typography_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::get(self.theme);
        div()
            .id("type-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .text_size(px(23.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Typography & controls"),
            )
            .child(
                div().text_color(rgb(p.muted)).child(
                    "Native GPUI text · Segoe UI · no WebView · compare at Windows 100% scale",
                ),
            )
            .children([12., 13., 14., 16.].into_iter().enumerate().map(|(i, s)| {
                div()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(p.card))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(p.muted))
                            .child(format!("{} px · Regular / Medium / Semibold", s)),
                    )
                    .children(
                        [FontWeight::NORMAL, FontWeight::MEDIUM, FontWeight::SEMIBOLD]
                            .into_iter()
                            .map(move |w| {
                                div()
                                    .id(("font", i * 100 + w.0 as usize))
                                    .text_size(px(s))
                                    .font_weight(w)
                                    .child(
                                        "TermViAI 209 · Hosts · 清晰的文字与输入体验 · 0123456789",
                                    )
                            }),
                    )
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child("Terminal font size")
                    .children([12., 14., 16., 18.].into_iter().map(|s| {
                        button(("size", s as usize), &format!("{s} px"))
                            .selected(self.font_size == s)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.font_size = s;
                                for t in &this.terminals {
                                    t.update(cx, |t, cx| t.appearance(s, this.theme == 2, cx));
                                }
                                cx.notify();
                            }))
                    })),
            )
            .child(
                button("test-form", "Open input form")
                    .on_click(cx.listener(|this, _, w, cx| this.new_form(w, cx))),
            )
            .into_any_element()
    }
    fn form(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = Palette::get(self.theme);
        let f = self.form.as_ref().unwrap();
        div().absolute().inset_0().bg(rgba(0x00000080)).flex().items_center().justify_center()
            .child(div().w(px(440.)).max_h_full().p_6().rounded_xl().bg(rgb(p.card)).border_1().border_color(rgb(p.line)).shadow_lg().flex().flex_col().gap_4()
                .child(div().flex().justify_between().items_center().child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("Add Host"))
                    .child(Button::new("close-form").ghost().small().icon(IconName::Close).on_click(cx.listener(|this,_,_,cx|{this.form=None;cx.notify();}))))
                .child(div().text_size(px(12.)).text_color(rgb(p.muted)).child("Evaluation only · this form never saves credentials"))
                .children([("Label",&f.label),("Address",&f.address),("Username",&f.user),("Password",&f.password)].into_iter().map(|(label,state)|div().flex().flex_col().gap_2().child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(label)).child(Input::new(state).when(label=="Password",|i|i.mask_toggle()))))
                .when(!f.error.is_empty(),|d|d.child(div().text_color(rgb(0xe48191)).child(f.error.clone())))
                .child(div().flex().justify_end().gap_2().pt_2().child(button("cancel-form","Cancel").on_click(cx.listener(|this,_,_,cx|{this.form=None;cx.notify();})))
                    .child(button("save-form","Save").hover(|s|s.bg(rgb(p.soft)).text_color(rgb(p.accent))).on_click(cx.listener(|this,_,_,cx|{let f=this.form.as_mut().unwrap();let label=f.label.read(cx).value().trim().to_owned();let address=f.address.read(cx).value().trim().to_owned();if label.is_empty()||address.is_empty(){f.error="Enter a label and address.".into();}else{this.hosts.push(Host{label,address,group:this.group.unwrap_or(0)});this.form=None;this.notice="Added to the temporary sample list. Nothing was written to disk.".into();}cx.notify();})))))
            .into_any_element()
    }
}
impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::get(self.theme);
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(p.bg))
            .text_color(rgb(p.text))
            .font_family("Segoe UI")
            .text_size(px(14.))
            .child(
                div()
                    .h_12()
                    .flex_shrink_0()
                    .px_5()
                    .flex()
                    .justify_between()
                    .items_center()
                    .bg(rgb(p.side))
                    .border_b_1()
                    .border_color(rgb(p.line))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Icon::new(IconName::SquareTerminal)
                                    .size_5()
                                    .text_color(rgb(p.accent)),
                            )
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("TermViAI"))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(p.soft))
                                    .text_color(rgb(p.accent))
                                    .child("GPUI EVALUATION"),
                            ),
                    )
                    .child(
                        div().flex().gap_2().children(
                            ["Iris", "Ocean", "Light"]
                                .into_iter()
                                .enumerate()
                                .map(|(i, s)| {
                                    button(("theme", i), s)
                                        .ghost()
                                        .selected(i == self.theme)
                                        .on_click(
                                            cx.listener(move |this, _, w, cx| this.theme(i, w, cx)),
                                        )
                                }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .w(px(180.))
                            .flex_shrink_0()
                            .bg(rgb(p.side))
                            .border_r_1()
                            .border_color(rgb(p.line))
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .children(
                                [
                                    (0, "Hosts", IconName::LayoutDashboard),
                                    (1, "Terminals", IconName::SquareTerminal),
                                    (2, "Typography", IconName::ALargeSmall),
                                ]
                                .into_iter()
                                .map(|(i, label, icon)| {
                                    div()
                                        .id(("nav", i))
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .px_3()
                                        .py_3()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .bg(rgb(if self.page == i { p.soft } else { p.side }))
                                        .text_color(rgb(if self.page == i {
                                            p.accent
                                        } else {
                                            p.text
                                        }))
                                        .hover(|s| s.bg(rgb(p.hover)))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.page = i;
                                            cx.notify();
                                        }))
                                        .child(Icon::new(icon).small())
                                        .child(div().font_weight(FontWeight::MEDIUM).child(label))
                                }),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .text_size(px(11.))
                                    .text_color(rgb(p.muted))
                                    .child("NATIVE RUST UI")
                                    .child("GPUI · DirectX · ConPTY"),
                            ),
                    )
                    .child(match self.page {
                        1 => self.terminal_page(cx),
                        2 => self.typography_page(cx),
                        _ => self.hosts_page(cx),
                    }),
            )
            .child(
                div()
                    .h_7()
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(p.line))
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child(self.notice.clone()),
            )
            .when(self.form.is_some(), |d| d.child(self.form(cx)))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" && this.form.is_some() {
                    this.form = None;
                    cx.notify();
                }
            }))
            .when(window.is_window_active(), |d| d)
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--terminal-fixture") {
        pty::fixture();
        return;
    }
    if args.iter().any(|a| a == "--stream") {
        pty::stream();
        return;
    }
    if args.iter().any(|a| a == "--pty-check") {
        if let Err(e) = pty::check() {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    let report = args
        .windows(2)
        .find(|w| w[0] == "--report")
        .map(|w| PathBuf::from(&w[1]));
    let auto = args.iter().any(|a| a == "--auto-check");
    let view_mode = args
        .windows(2)
        .find(|w| w[0] == "--view")
        .map(|w| w[1].clone())
        .unwrap_or_default();
    let initial_theme = args
        .windows(2)
        .find(|w| w[0] == "--theme")
        .and_then(|w| w[1].parse::<usize>().ok())
        .unwrap_or(0)
        .min(2);
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1180.), px(800.)), cx);
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("TermViAI · GPUI Evaluation".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(780.), px(560.))),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Probe::new(window, cx, report));
                    view.update(cx, |p, cx| {
                        p.theme(initial_theme, window, cx);
                        match view_mode.as_str() {
                            "form" => p.new_form(window, cx),
                            "typography" => p.page = 2,
                            "terminal" => p.spawn_terminal("shell", window, cx),
                            "four-terminals" | "unicode" => {
                                for _ in 0..4 {
                                    p.spawn_terminal(
                                        if view_mode == "unicode" {
                                            "fixture"
                                        } else {
                                            "shell"
                                        },
                                        window,
                                        cx,
                                    );
                                }
                            }
                            _ => {}
                        }
                    });
                    if auto {
                        let v = view.downgrade();
                        window
                            .spawn(cx, async move |cx| {
                                cx.background_executor().timer(Duration::from_secs(5)).await;
                                let _ = v.update_in(cx, |p, w, cx| {
                                    for _ in 0..4 {
                                        p.spawn_terminal("stream", w, cx);
                                    }
                                });
                                for (width, height) in [
                                    (960., 640.),
                                    (1280., 840.),
                                    (820., 580.),
                                    (1180., 800.),
                                    (980., 680.),
                                    (1280., 840.),
                                    (1180., 800.),
                                    (1180., 800.),
                                ] {
                                    cx.background_executor().timer(Duration::from_secs(5)).await;
                                    let _ = v.update_in(cx, |_, w, _| {
                                        w.resize(size(px(width), px(height)))
                                    });
                                }
                                let _ = v.update_in(cx, |p, w, cx| {
                                    p.write_report(cx);
                                    p.terminals.clear();
                                    p.page = 2;
                                    p.theme(2, w, cx);
                                });
                                cx.background_executor().timer(Duration::from_secs(5)).await;
                                let _ = cx.update(|_, cx| cx.quit());
                            })
                            .detach();
                    }
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            if let Err(e) = window {
                eprintln!("{e:#}");
                cx.quit();
            } else {
                cx.activate(true);
            }
        });
}
