mod actions;
mod chrome;
mod forms;
mod monitor_view;
mod pages;
mod shortcut_settings;
mod states;
mod tabs;
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
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use termviai_broadcast::BroadcastState;
use termviai_gpui::{
    authentication::{self, Phase},
    forwarding,
    hosts::*,
    layout::{Layout, QuadGrip, QuadSplit, Side},
    monitor,
    session::{self, Session},
    settings::Settings,
    tab_policy, tab_strip,
    theme::{Colors, Theme},
    workspaces::*,
};

/// Instant toolbar actions must not let the focusable root steal keyboard input.
/// Suppress only automatic focus on mouse-down; keep click/drag propagation and
/// explicit focus changes (dialogs, tab switching, text inputs) intact.
fn preserve_input_focus(_: &MouseDownEvent, window: &mut Window, _: &mut App) {
    window.prevent_default();
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Hosts,
    Keychain,
    NewTab,
    EmptyTabGroup,
    Terminal(usize),
    Monitor,
    Forwarding,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ForwardFilter {
    All,
    Active,
    Attention,
    Stopped,
}
impl ForwardFilter {
    fn matches(self, state: forwarding::State) -> bool {
        match self {
            Self::All => true,
            Self::Active => state.active(),
            Self::Attention => matches!(
                state,
                forwarding::State::Attention | forwarding::State::Failed
            ),
            Self::Stopped => state == forwarding::State::Stopped,
        }
    }
}
pub struct Tab {
    id: usize,
    group: usize,
    last_focused: Option<usize>,
    name: String,
    custom_name: bool,
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
    notices: Entity<crate::notice_view::NoticeView>,
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
    RenameTabGroup(usize),
    RenameWorkspace(SavedWorkspace),
    Settings,
    Rename(usize),
    Existing(String),
    Monitor,
    MonitorReplace(Host),
    Forward(Option<forwarding::Rule>),
    Auth(usize),
}
#[derive(Clone)]
pub struct Form {
    shortcut_editor: Option<shortcut_settings::Editor>,
    kind: FormKind,
    fields: BTreeMap<&'static str, Entity<InputState>>,
    mode: Authentication,
    keychain: bool,
    key: String,
    identity: String,
    group: String,
    group_list_open: bool,
    highlighted_group: Option<usize>,
    selected: Vec<Host>,
    error: String,
    theme: Theme,
    confirm: bool,
    bell: bool,
    npu_monitor_enabled: bool,
    network: bool,
    retry: bool,
    challenges: Vec<Entity<InputState>>,
    challenge_stamp: u64,
    show_added: bool,
    field_errors: BTreeMap<&'static str, String>,
}
#[derive(Clone)]
pub enum MenuKind {
    Pane(usize, usize),
    Saved(SavedWorkspace),
    SavedGroup(SavedTabGroup),
    AddCredential,
    Forward(String),
    Host(Host),
    Group(HostGroup),
    Tab(usize),
    TabGroup(usize),
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
    TabGroup(usize),
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
    quad: Option<(QuadSplit, QuadGrip)>,
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
    hosts_search: Entity<InputState>,
    new_tab_search: Entity<InputState>,
    host_selection: termviai_gpui::host_selection::HostSelection,
    tabs: Vec<Tab>,
    tab_groups: Vec<tabs::TabGroup>,
    hovered_tab: Option<(usize, Instant)>,
    hovered_tab_group: Option<(usize, Instant)>,
    active_tab_group: usize,
    panes: HashMap<usize, Pane>,
    next_id: usize,
    focused: Option<usize>,
    ime_epoch: termviai_gpui::terminal_ime::ImeEpoch,
    drawer: Option<Form>,
    menu: Option<(MenuKind, Point<Pixels>)>,
    confirm: Option<Confirm>,
    focus: FocusHandle,
    drag_source: Option<Drag>,
    drop_target: Option<(usize, usize, Side)>,
    tab_join_target: Option<usize>,
    tab_drop_target: Option<(usize, bool)>,
    tab_rail: tabs::TabRail,
    tab_search: Entity<InputState>,
    shortcut_keys_down: HashSet<String>,
    tab_picker: bool,
    tab_picker_index: usize,
    monitor: monitor::Monitor,
    monitor_detail: Option<(usize, Option<u32>, Option<u32>)>,
    monitor_reason: Option<usize>,
    monitor_technical: bool,
    forward_filter: ForwardFilter,
    forward_detail: Option<String>,
    expanded_process: Option<(usize, u32)>,
    forwarding: Vec<forwarding::View>,
    forwarding_error: String,
    last_tick: Instant,
    quit_allowed: bool,
    scrollers: HashMap<&'static str, ScrollHandle>,
}
impl AppView {
    fn page_search_input(
        page: Page,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        use gpui_component::input::InputEvent;
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        cx.subscribe_in(
            &input,
            window,
            move |s, input, event: &InputEvent, w, cx| {
                if s.page != page {
                    return;
                }
                match event {
                    InputEvent::Change => {
                        s.host_selection.clear();
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. }
                        if s.drawer.is_none() && s.menu.is_none() && s.confirm.is_none() =>
                    {
                        if !s.host_selection.is_empty() {
                            s.open_selected_hosts(w, cx);
                        } else {
                            let value = input.read(cx).value().to_string();
                            s.quick_connect(&value, w, cx);
                        }
                    }
                    _ => {}
                }
            },
        )
        .detach();
        input
    }

    fn clear_page_search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        for input in [&self.hosts_search, &self.new_tab_search] {
            if !input.read(cx).value().is_empty() {
                input.update(cx, |input, cx| input.set_value("", w, cx));
            }
        }
        self.host_selection.clear();
    }

    fn quick_connect(&mut self, text: &str, w: &mut Window, cx: &mut Context<Self>) {
        match Host::quick_connect(text) {
            Ok(host) => self.connect(Some(host), w, cx),
            Err(e) => {
                self.fail(e);
                cx.notify();
            }
        }
    }
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
        let hosts_search = Self::page_search_input(
            Page::Hosts,
            "Search hosts or connect with user@hostname",
            window,
            cx,
        );
        let new_tab_search = Self::page_search_input(
            Page::NewTab,
            "Find a workspace, host or user@hostname",
            window,
            cx,
        );
        let tab_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search name or host…"));
        cx.subscribe(
            &tab_search,
            |s, _, event: &gpui_component::input::InputEvent, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change) {
                    let query = s.tab_search.read(cx).value().to_string();
                    if query != s.tab_rail.query {
                        s.tab_rail.query = query;
                        s.tab_picker_index = 0;
                        s.scrollers["tab-picker"].set_offset(point(px(0.), px(0.)));
                    }
                    cx.notify();
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
            hosts_search,
            new_tab_search,
            host_selection: Default::default(),
            tabs: vec![],
            tab_groups: vec![],
            hovered_tab: None,
            hovered_tab_group: None,
            active_tab_group: 0,
            panes: HashMap::new(),
            next_id: 1,
            focused: None,
            ime_epoch: termviai_gpui::terminal_ime::ImeEpoch::install(window),
            drawer: None,
            menu: None,
            confirm: None,
            focus: cx.focus_handle(),
            drag_source: None,
            drop_target: None,
            tab_join_target: None,
            tab_drop_target: None,
            tab_rail: tabs::TabRail::default(),
            tab_search,
            shortcut_keys_down: HashSet::new(),
            tab_picker: false,
            tab_picker_index: 0,
            monitor: monitor::Monitor::load(),
            monitor_detail: None,
            monitor_reason: None,
            monitor_technical: false,
            expanded_process: None,
            forward_detail: None,
            forward_filter: ForwardFilter::All,
            forwarding: vec![],
            forwarding_error: String::new(),
            last_tick: Instant::now(),
            quit_allowed: false,
            scrollers: [
                "shortcut-settings",
                "tab-picker",
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
