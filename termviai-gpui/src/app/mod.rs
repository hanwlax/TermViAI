mod actions;
mod chrome;
mod forms;
mod monitor_view;
mod pages;
mod validation;
mod workspace;
use crate::terminal_view::{TerminalEvent, TerminalView};
use gpui::{prelude::*, *};
use gpui_component::{
    Icon, IconName,
    input::{Input, InputState},
    scroll::ScrollableElement,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::{Duration, Instant},
};
use termviai_broadcast::BroadcastState;
use termviai_gpui::{
    authentication::{self, Phase},
    forwarding,
    hosts::*,
    layout::{Layout, Side},
    monitor,
    session::{self, Session},
    settings::Settings,
    theme::{Colors, Theme},
    workspaces::*,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Hosts,
    Keychain,
    NewTab,
    Terminal(usize),
    Monitor,
    Forwarding,
}
pub struct Tab {
    id: usize,
    name: String,
    layout: Layout,
    broadcast: BroadcastState<usize>,
    saved: Option<String>,
    tray: bool,
    tray_motion: Option<(Instant, f32)>,
    zoom: Option<usize>,
}
impl Tab {
    fn tray_fraction(&self, now: Instant) -> f32 {
        let target = if self.tray { 1. } else { 0. };
        let Some((start, from)) = self.tray_motion else {
            return target;
        };
        let progress = (now.duration_since(start).as_secs_f32() / 0.18).min(1.);
        from + (target - from) * (1. - (1. - progress).powi(3))
    }
    fn toggle_tray(&mut self) {
        let now = Instant::now();
        let from = self.tray_fraction(now);
        self.tray = !self.tray;
        self.tray_motion = Some((now, from));
    }
}
pub struct Pane {
    host: Option<Host>,
    session: Arc<Session>,
    view: Entity<TerminalView>,
    form: Option<Form>,
    pending: Option<Pending>,
    save_attempted: bool,
    error: String,
    recorded: bool,
    auth_revision: u64,
    auth_scroll: ScrollHandle,
}
pub struct Pending {
    original: Host,
    selected: Host,
    mode: Authentication,
    secret: String,
    saved: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if !self.secret.is_empty() {
            let _ = wezterm_ssh::credentials::delete(&self.secret);
        }
    }
}
#[derive(Clone)]
pub enum FormKind {
    Host(Option<Host>),
    Key(Option<KeyFile>),
    Identity(Option<Identity>),
    Group(String),
    Settings,
    Rename(usize),
    Existing(String),
    Monitor,
    Forward(Option<forwarding::Rule>),
    Auth(usize),
}
#[derive(Clone)]
pub struct Form {
    kind: FormKind,
    fields: BTreeMap<&'static str, Entity<InputState>>,
    mode: Authentication,
    keychain: bool,
    key: String,
    identity: String,
    group: String,
    selected: Vec<Host>,
    error: String,
    theme: Theme,
    confirm: bool,
    bell: bool,
    network: bool,
    retry: bool,
    challenges: Vec<Entity<InputState>>,
    challenge_stamp: u64,
}
#[derive(Clone)]
pub enum MenuKind {
    Host(Host),
    Group(HostGroup),
    Tab(usize),
    AddHost(String),
    Monitor(usize),
    MonitorInterval,
}
#[derive(Clone)]
pub enum Confirm {
    Host(Host),
    Group(HostGroup),
    Key(KeyFile),
    Identity(Identity),
    Tab(usize),
    Pane(usize),
    Quit,
    Stop(usize, Vec<monitor::Process>),
    Forward(String),
    RemoveMonitor(usize),
}
#[derive(Clone)]
pub struct Drag {
    pane: Option<usize>,
    tab: usize,
    label: String,
}
impl Render for Drag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x25282f))
            .text_color(rgb(0xf1f2f5))
            .text_size(px(13.))
            .child(self.label.clone())
    }
}
#[derive(Clone)]
pub struct ResizeDrag {
    tab: usize,
    path: Vec<bool>,
}
impl Render for ResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub struct AppView {
    store: HostStore,
    workspaces: WorkspaceStore,
    settings: Settings,
    theme: Theme,
    load_error: String,
    error: String,
    page: Page,
    group: String,
    sidebar: bool,
    search: Entity<InputState>,
    tabs: Vec<Tab>,
    panes: HashMap<usize, Pane>,
    next_id: usize,
    focused: Option<usize>,
    drawer: Option<Form>,
    menu: Option<(MenuKind, Point<Pixels>)>,
    confirm: Option<Confirm>,
    focus: FocusHandle,
    drag_source: Option<Drag>,
    drop_target: Option<(usize, usize, Side)>,
    monitor: monitor::Monitor,
    monitor_detail: Option<(usize, Option<u32>, Option<u32>)>,
    monitor_reason: Option<usize>,
    forwarding: Vec<forwarding::View>,
    forwarding_error: String,
    last_tick: Instant,
    quit_allowed: bool,
    scrollers: HashMap<&'static str, ScrollHandle>,
}
impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut errors = vec![];
        let store = HostStore::load().unwrap_or_else(|e| {
            errors.push(format!("{e:#}"));
            HostStore::default()
        });
        let workspaces = WorkspaceStore::load().unwrap_or_else(|e| {
            errors.push(format!("{e:#}"));
            WorkspaceStore::default()
        });
        let settings = Settings::load().unwrap_or_else(|e| {
            errors.push(format!("{e:#}"));
            Settings::default()
        });
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search hosts or connect with user@hostname")
        });
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &search,
            window,
            |s, _, event: &gpui_component::input::InputEvent, w, cx| {
                if matches!(event, gpui_component::input::InputEvent::PressEnter { .. }) {
                    let text = s.search.read(cx).value().to_string();
                    match Host::quick_connect(&text) {
                        Ok(h) => s.connect(Some(h), w, cx),
                        Err(e) => {
                            s.fail(e);
                            cx.notify();
                        }
                    }
                }
            },
        )
        .detach();
        let theme = settings.theme;
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            weak.update(cx, |s, cx| {
                if s.quit_allowed || s.tabs.is_empty() || !s.settings.confirm_close() {
                    return true;
                }
                s.confirm = Some(Confirm::Quit);
                cx.notify();
                false
            })
            .unwrap_or(true)
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |s, cx| s.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            store,
            workspaces,
            settings,
            theme,
            load_error: errors.join("\n"),
            error: String::new(),
            page: Page::Hosts,
            group: String::new(),
            sidebar: true,
            search,
            tabs: vec![],
            panes: HashMap::new(),
            next_id: 1,
            focused: None,
            drawer: None,
            menu: None,
            confirm: None,
            focus: cx.focus_handle(),
            drag_source: None,
            drop_target: None,
            monitor: monitor::Monitor::load(),
            monitor_detail: None,
            monitor_reason: None,
            forwarding: vec![],
            forwarding_error: String::new(),
            last_tick: Instant::now(),
            quit_allowed: false,
            scrollers: [
                "hosts-page",
                "keychain-page",
                "new-tab-page",
                "monitor-page",
                "monitor-detail",
                "monitor-process",
                "forward-page",
                "drawer-scroll",
                "group-picker",
                "key-picker",
                "identity-picker",
                "stop-process-list",
            ]
            .into_iter()
            .map(|id| (id, ScrollHandle::new()))
            .collect(),
        }
    }
    pub fn initialize(&mut self, local: bool, w: &mut Window, cx: &mut Context<Self>) {
        self.apply_theme(self.settings.theme, w, cx);
        if local {
            self.connect(None, w, cx);
        }
    }
    fn c(&self, s: &str) -> Hsla {
        Colors::color(s)
    }
    fn id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn col(&self) -> &'static termviai_gpui::theme::Colors {
        self.theme.colors()
    }
    fn fail(&mut self, e: impl std::fmt::Display) {
        self.error = e.to_string();
    }
    fn notify_error(&mut self, result: anyhow::Result<()>, cx: &mut Context<Self>) {
        if let Err(e) = result {
            self.fail(format!("{e:#}"));
        }
        cx.notify();
    }
    fn input(&self, form: &Form, key: &str, cx: &App) -> String {
        form.fields
            .get(key)
            .map(|f| f.read(cx).value().to_string())
            .unwrap_or_default()
    }
    fn apply_theme(&mut self, theme: Theme, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = theme;
        gpui_component::Theme::change(
            if theme == Theme::Light {
                gpui_component::ThemeMode::Light
            } else {
                gpui_component::ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        let c = theme.colors();
        let native = gpui_component::Theme::global_mut(cx);
        native.colors.background = Colors::color(c.bg);
        native.colors.foreground = Colors::color(c.text);
        native.colors.input = Colors::color(c.line);
        native.colors.ring = Colors::color(c.accent);
        native.colors.muted_foreground = Colors::color(c.muted);
        native.colors.border = Colors::color(c.line);
        native.colors.primary = Colors::color(c.accent);
        native.colors.selection = Colors::color(c.input_selection);
        for p in self.panes.values() {
            p.view
                .update(cx, |v, cx| v.appearance(v.font_size, theme, cx));
        }
        cx.notify();
    }
    fn button(&self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .px_3()
            .h(px(32.))
            .flex_shrink_0()
            .rounded_md()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .cursor_pointer()
            .hover(|s| s.bg(self.c(self.col().hover)))
            .child(label.into())
    }
    fn icon(&self, id: impl Into<ElementId>, icon: IconName) -> Stateful<Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .flex_shrink_0()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(self.c(self.col().icon_hover)))
            .child(Icon::new(icon).size(px(16.)))
    }
    fn close(&self, id: impl Into<ElementId>) -> Stateful<Div> {
        div()
            .id(id)
            .group("close-control")
            .flex()
            .items_center()
            .justify_center()
            .size(px(26.))
            .flex_shrink_0()
            .cursor_pointer()
            .hover(|s| s.text_color(self.c(self.col().red)))
            .child(
                svg()
                    .path("icons/close.svg")
                    .size(px(15.))
                    .text_color(self.c(self.col().muted))
                    .group_hover("close-control", |s| s.text_color(self.c(self.col().red))),
            )
    }
    fn title(&self, label: impl Into<SharedString>) -> Div {
        div()
            .text_size(px(24.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(label.into())
    }
    fn muted(&self, label: impl Into<SharedString>) -> Div {
        div()
            .text_size(px(12.))
            .text_color(self.c(self.col().muted))
            .child(label.into())
    }
    fn radio(&self, on: bool) -> impl IntoElement {
        svg()
            .path("termviai/broadcast.svg")
            .size(px(17.))
            .text_color(self.c(if on {
                self.col().accent
            } else {
                self.col().muted
            }))
    }
}
