//! Monitor's native layouts use the same tiles, font, theme keys and hit testing as Hosts.
use super::super::termviai_motion::SmoothScroll;
use super::*;
use crate::monitor::{Monitor, Process, Snapshot, Target};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Command {
    Open,
    Add,
    Pick(usize),
    AcceptAdd,
    Home,
    Host(usize),
    Npu(u32),
    Filter(Option<u32>),
    Menu(usize),
    Pause(usize),
    Retry(usize),
    Remove(usize),
    AcceptRemove,
    Interval,
    SetInterval(u64),
    Reason,
    Close,
    Noop,
    StopScope,
    StopHost,
    StopPid(u32),
    CancelStop,
    AcceptStop,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Dialog {
    Picker,
    Processes,
    Stop,
    Remove,
    Reason,
}
#[derive(Clone, Copy)]
struct ScrollBar {
    x: f32,
    top: f32,
    bottom: f32,
    thumb: f32,
}
impl ScrollBar {
    fn travel(self) -> f32 {
        (self.bottom - self.top - self.thumb).max(0.)
    }
    fn thumb_top(self, scroll: f32, max_scroll: f32) -> f32 {
        self.top + self.travel() * scroll / max_scroll.max(f32::EPSILON)
    }
}
struct StopRow {
    name: String,
    metadata: String,
    locations: String,
    cwd: String,
}

pub(super) struct MonitorUi {
    pub data: Monitor,
    pub selected: Option<usize>,
    pub dialog: Option<Dialog>,
    pub menu: Option<usize>,
    interval_open: bool,
    reason_return: Option<usize>,
    pub chosen: HashSet<usize>,
    pub npu: u32,
    pub die: Option<u32>,
    pub scroll: usize,
    pub max_scroll: usize,
    drawer_scroll: SmoothScroll,
    scroll_hover: bool,
    scroll_hover_motion: Tween,
    scrollbar: Option<ScrollBar>,
    scroll_drag: Option<f32>,
    pub focus: Option<Action>,
    pub error: String,
    targets: Vec<Target>,
    stop_rows: Vec<StopRow>,
    stop_label: String,
    stop_dies: usize,
    stop_from: Option<Dialog>,
    loaded: bool,
    text_wrap: std::cell::RefCell<TextWrapCache>,
}
impl Default for MonitorUi {
    fn default() -> Self {
        Self {
            data: Monitor::default(),
            selected: None,
            dialog: None,
            menu: None,
            interval_open: false,
            reason_return: None,
            chosen: HashSet::new(),
            npu: 0,
            die: None,
            scroll: 0,
            max_scroll: 0,
            drawer_scroll: SmoothScroll::default(),
            scroll_hover: false,
            scroll_hover_motion: Tween::new(0.),
            scrollbar: None,
            scroll_drag: None,
            focus: None,
            error: String::new(),
            targets: vec![],
            stop_rows: vec![],
            stop_label: String::new(),
            stop_dies: 0,
            stop_from: None,
            loaded: false,
            text_wrap: Default::default(),
        }
    }
}
impl MonitorUi {
    fn reset_scroll(&mut self) {
        if self.dialog.is_some() {
            self.drawer_scroll = SmoothScroll::default();
        } else {
            self.scroll = 0;
        }
    }
    pub fn scrolling(&self, now: Instant) -> bool {
        self.dialog.is_some()
            && (self.drawer_scroll.active() || self.scroll_hover_motion.is_active(now))
    }
    pub fn scroll_by(&mut self, lines: isize) {
        if self.dialog.is_some() {
            self.drawer_scroll
                .move_by(lines as f32 * 24., Instant::now());
        } else {
            self.scroll = self
                .scroll
                .saturating_add_signed(lines)
                .min(self.max_scroll);
        }
    }
    fn scroll_page(&mut self, direction: f32) {
        if self.dialog.is_some() {
            self.drawer_scroll.move_by(
                direction * self.drawer_scroll.viewport * 0.9,
                Instant::now(),
            );
        } else {
            self.scroll_by(direction as isize);
        }
    }
    pub fn scrollbar_mouse(
        &mut self,
        kind: &MouseEventKind,
        x: f32,
        y: f32,
        left_down: bool,
    ) -> bool {
        let Some(bar) = self.scrollbar.filter(|_| self.dialog.is_some()) else {
            self.scroll_drag = None;
            self.scroll_hover = false;
            return false;
        };
        let now = Instant::now();
        let was_hover = self.scroll_hover;
        self.scroll_hover = x >= bar.x && x <= bar.x + 14. && y >= bar.top && y <= bar.bottom;
        let position = self.drawer_scroll.sample(now);
        let update = |this: &mut Self, grab: f32| {
            let ratio = ((y - bar.top - grab) / bar.travel().max(f32::EPSILON)).clamp(0., 1.);
            this.drawer_scroll
                .jump_to(ratio * this.drawer_scroll.maximum, now);
        };
        match kind {
            MouseEventKind::Press(MousePress::Left) if self.scroll_hover => {
                let thumb_top = bar.thumb_top(position, self.drawer_scroll.maximum);
                if y >= thumb_top && y <= thumb_top + bar.thumb {
                    self.scroll_drag = Some(y - thumb_top);
                    self.drawer_scroll.jump_to(position, now);
                } else {
                    self.scroll_page(if y < thumb_top { -1. } else { 1. });
                }
                true
            }
            MouseEventKind::Move if self.scroll_drag.is_some() => {
                if left_down {
                    update(self, self.scroll_drag.unwrap());
                } else {
                    self.scroll_drag = None;
                }
                true
            }
            MouseEventKind::Release(MousePress::Left) if self.scroll_drag.is_some() => {
                let grab = self.scroll_drag.take().unwrap();
                update(self, grab);
                true
            }
            MouseEventKind::VertWheel(_) => {
                self.scroll_drag = None;
                false
            }
            MouseEventKind::Move => self.scroll_hover || was_hover,
            _ => false,
        }
    }

    pub fn tick(&mut self, store: &HostStore, keepalive: u64) {
        if !self.loaded {
            return;
        }
        self.data.tick(store, |h| {
            let domain = super::super::termviai_workspace::ssh_domain(h, keepalive);
            let mut cfg = mux::ssh::ssh_domain_to_ssh_config(&domain)?;
            // Explicit overrides also avoid treating IPv6 colons as a port delimiter.
            cfg.insert("hostname".into(), h.address.clone());
            cfg.insert("port".into(), h.port.to_string());
            cfg.insert("connecttimeout".into(), "10".into());
            Ok(cfg)
        });
    }
    pub fn active(&self) -> bool {
        self.loaded && !self.data.entries.is_empty()
    }
    pub fn handle(&mut self, c: Command, store: &HostStore) -> anyhow::Result<()> {
        self.scroll_drag = None;
        self.scrollbar = None;
        self.error.clear();
        if !matches!(c, Command::Interval | Command::SetInterval(_)) {
            self.interval_open = false;
        }
        self.focus = match &c {
            Command::Pick(_) | Command::Filter(_) => Some(Action::Monitor(c.clone())),
            Command::Add => Some(Action::Search),
            _ => None,
        };
        match c {
            Command::Open => {
                if !self.loaded {
                    self.data = Monitor::load();
                    self.loaded = true;
                }
                self.selected = None;
                self.dialog = None;
                self.menu = None;
                self.reset_scroll();
            }
            Command::Add => {
                self.dialog = Some(Dialog::Picker);
                self.chosen.clear();
                self.reset_scroll();
            }
            Command::Pick(i) => {
                if store.hosts.get(i).is_some_and(|h| !self.data.contains(h)) {
                    if !self.chosen.insert(i) {
                        self.chosen.remove(&i);
                    }
                }
            }
            Command::AcceptAdd => {
                for i in &self.chosen {
                    if let Some(h) = store.hosts.get(*i) {
                        self.data.add(h.clone());
                    }
                }
                self.data.save()?;
                self.dialog = None;
                self.reset_scroll();
            }
            Command::Home => {
                self.selected = None;
                self.menu = None;
                self.reset_scroll();
            }
            Command::Host(i) => {
                self.selected = Some(i);
                self.menu = None;
                self.reset_scroll();
            }
            Command::Npu(n) => {
                self.npu = n;
                self.die = None;
                self.dialog = Some(Dialog::Processes);
                self.reset_scroll();
            }
            Command::Filter(d) => {
                self.die = d;
                self.reset_scroll();
            }
            Command::Menu(i) => {
                self.menu = if self.menu == Some(i) { None } else { Some(i) };
            }
            Command::Pause(i) => {
                if let Some(e) = self.data.entries.get_mut(i) {
                    e.paused = !e.paused;
                }
                self.menu = None;
            }
            Command::Retry(i) => {
                self.data.retry(i);
                if self.dialog == Some(Dialog::Reason) {
                    self.selected = self.reason_return;
                }
                self.menu = None;
                self.dialog = None;
            }
            Command::Remove(i) => {
                self.selected = Some(i);
                self.dialog = Some(Dialog::Remove);
                self.menu = None;
            }
            Command::AcceptRemove => {
                if let Some(i) = self.selected.take() {
                    self.data.entries.remove(i);
                }
                self.data.save()?;
                self.dialog = None;
                self.reset_scroll();
            }
            Command::Interval => {
                self.interval_open = !self.interval_open;
                self.menu = None;
            }
            Command::SetInterval(seconds) => {
                anyhow::ensure!([2, 5, 10].contains(&seconds), "Invalid sample interval");
                self.data.interval = seconds;
                self.data.save()?;
                self.interval_open = false;
            }
            Command::Reason => {
                self.reason_return = self.selected;
                if let Some(i) = self.menu {
                    self.selected = Some(i);
                }
                self.dialog = Some(Dialog::Reason);
                self.reset_scroll();
                self.menu = None;
            }
            Command::Close => {
                if self.dialog == Some(Dialog::Reason) {
                    self.selected = self.reason_return;
                }
                self.interval_open = false;
                if self.dialog == Some(Dialog::Stop) {
                    self.dialog = self.stop_from;
                } else {
                    self.dialog = None;
                }
                self.menu = None;
                self.drawer_scroll = SmoothScroll::default();
            }
            Command::StopHost | Command::StopScope | Command::StopPid(_) => {
                let e = self
                    .selected
                    .and_then(|i| self.data.entries.get(i))
                    .context("Select a host")?;
                anyhow::ensure!(e.live(), "Resume sampling before controlling processes.");
                let s = e.snapshot.as_ref().context("No current sample")?;
                anyhow::ensure!(s.processes_available, "Process information is unavailable");
                let scope = if c == Command::StopHost {
                    None
                } else {
                    Some(self.npu)
                };
                let die = if c == Command::StopHost {
                    None
                } else {
                    self.die
                };
                let ps = s
                    .processes(scope, die)
                    .into_iter()
                    .filter(|p| match c {
                        Command::StopPid(pid) => p.pid == pid,
                        _ => true,
                    })
                    .collect::<Vec<_>>();
                anyhow::ensure!(!ps.is_empty(), "There are no processes in this scope");
                anyhow::ensure!(
                    ps.iter().all(|p| p.controllable && !p.token.is_empty()),
                    "Some process identities / permissions are unavailable. {}",
                    s.stop_reason
                );
                let targets = ps
                    .iter()
                    .map(|p| Target {
                        pid: p.pid,
                        token: p.token.clone(),
                    })
                    .collect();
                let stop_rows = ps
                    .iter()
                    .map(|p| StopRow {
                        name: clean(&p.name),
                        metadata: format!("PID {} · {}", p.pid, clean(&p.owner)),
                        locations: locations(s, p.pid),
                        cwd: if p.cwd.is_empty() {
                            "Unavailable".into()
                        } else {
                            clean(&p.cwd)
                        },
                    })
                    .collect();
                // Include every Die occupied by these PIDs, even outside the
                // selected filter. Capture together with the confirmed targets.
                let pids: HashSet<_> = ps.iter().map(|p| p.pid).collect();
                let stop_dies = s
                    .processes
                    .iter()
                    .filter(|p| pids.contains(&p.pid))
                    .map(|p| (p.npu, p.die))
                    .collect::<HashSet<_>>()
                    .len();
                let stop_label = match c {
                    Command::StopHost => format!("{} · All NPU processes", e.host.label),
                    Command::StopPid(pid) => format!("{} · PID {}", e.host.label, pid),
                    _ => format!("{} · {}", e.host.label, scope_name(self.npu, self.die)),
                };
                self.targets = targets;
                self.stop_rows = stop_rows;
                self.stop_label = stop_label;
                self.stop_dies = stop_dies;
                self.stop_from = self.dialog;
                self.dialog = Some(Dialog::Stop);
                self.reset_scroll();
            }
            Command::CancelStop => {
                self.dialog = self.stop_from;
                self.reset_scroll();
            }
            Command::AcceptStop => {
                self.data.stop(
                    self.selected.context("No host selected")?,
                    self.targets.clone(),
                )?;
                self.dialog = self.stop_from;
                self.reset_scroll();
            }
            Command::Noop => (),
        }
        Ok(())
    }
}
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
fn scope_name(n: u32, d: Option<u32>) -> String {
    d.map(|d| format!("NPU {n} · Die {d}"))
        .unwrap_or_else(|| format!("NPU {n}"))
}
fn locations(s: &Snapshot, pid: u32) -> String {
    s.processes
        .iter()
        .filter(|p| p.pid == pid)
        .map(|p| scope_name(p.npu, Some(p.die)))
        .collect::<Vec<_>>()
        .join(", ")
}
fn gb(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v / 1024.))
        .unwrap_or_else(|| "—".into())
}
fn percent(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0}%")).unwrap_or_else(|| "—".into())
}
fn text(t: &mut Vec<Tile>, x: f32, y: f32, w: f32, h: f32, s: impl Into<String>, fg: &'static str) {
    tile(t, x, y, w, h, s, CARD, fg, false);
    if let Some(t) = t.last_mut() {
        t.transparent = true;
        t.text_padding = 0.;
        t.text_size = Some(12);
    }
}
fn ctl(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: impl Into<String>,
    c: Command,
    hover: &Option<Action>,
) {
    button(t, hits, x, y, w, h, s, CARD, Action::Monitor(c), hover);
    let v = t.last_mut().unwrap();
    v.text_size = Some(12);
    v.border_color = Some("#353648");
}
/// Keep the same outline and padding as an enabled control, without a hit target.
fn disabled_ctl(t: &mut Vec<Tile>, x: f32, y: f32, w: f32, h: f32, label: impl Into<String>) {
    tile(t, x, y, w, h, label, CARD, MUTED, true);
    let v = t.last_mut().unwrap();
    v.text_size = Some(12);
    v.text_padding = 12.;
    v.border_color = Some("#353648");
    v.opacity = 0.65;
    v.content_centered = true;
}
fn label(
    t: &mut Vec<Tile>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: impl Into<String>,
    fg: &'static str,
    size: u16,
    bold: bool,
    center: bool,
) {
    text(t, x, y, w, h, s, fg);
    if let Some(v) = t.last_mut() {
        v.text_size = Some(size);
        v.text_bold = bold;
        v.text_centered = center;
    }
}
fn symbol(t: &mut Vec<Tile>, x: f32, y: f32, size: f32, icon: Icon, fg: &'static str) {
    text(t, x, y, size, size, "", fg);
    let v = t.last_mut().unwrap();
    v.icon = Some(icon);
    v.icon_size = size - 2.;
}
fn right_text(
    t: &mut Vec<Tile>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: impl Into<String>,
    fg: &'static str,
    size: u16,
) {
    label(t, x, y, w, h, s, fg, size, false, false);
    t.last_mut().unwrap().text_right_aligned = true;
}
fn status(e: &crate::monitor::Entry) -> (&'static str, &'static str) {
    if !e.error.is_empty() {
        ("! Unavailable", "#ed8193")
    } else if e.paused {
        ("Paused", MUTED)
    } else if e.stopping {
        ("Ending processes", MUTED)
    } else if e.snapshot.is_some() {
        ("● Collecting", "#59b79e")
    } else {
        ("Connecting", MUTED)
    }
}
fn reason(e: &crate::monitor::Entry) -> &'static str {
    let error = e.error.to_lowercase();
    if error.contains("timed out") || error.contains("timeout") {
        "Connection timed out"
    } else if error.contains("auth") || error.contains("password") {
        "Authentication required"
    } else if error.contains("fingerprint") || error.contains("host key") {
        "Host verification required"
    } else if error.contains("collector") || error.contains("npu-smi") || error.contains("python") {
        "Sampling failed"
    } else {
        "Connection unavailable"
    }
}
fn icon_control(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    c: Command,
    icon: Icon,
    hover: &Option<Action>,
) {
    ctl(t, hits, x, y, w, 36., label, c, hover);
    let v = t.last_mut().unwrap();
    v.icon = Some(icon);
    v.icon_size = 16.;
}
fn metric(t: &mut Vec<Tile>, x: f32, y: f32, w: f32, value: Option<f64>) {
    tile(t, x, y, w, 5., "", "#353648", INK, true);
    if let Some(v) = value {
        tile(
            t,
            x,
            y,
            w * (v.clamp(0., 100.) as f32 / 100.),
            5.,
            "",
            ACCENT,
            INK,
            true,
        );
    }
}
fn rows_range(
    count: usize,
    available: f32,
    row: f32,
    scroll: &mut usize,
    max: &mut usize,
) -> std::ops::Range<usize> {
    let visible = (available / row).floor().max(1.) as usize;
    *max = count.saturating_sub(visible);
    *scroll = (*scroll).min(*max);
    *scroll..(*scroll + visible).min(count)
}
fn controls_enabled(e: &crate::monitor::Entry, ps: &[Process]) -> bool {
    e.live() && !ps.is_empty() && ps.iter().all(|p| p.controllable && !p.token.is_empty())
}

fn control_disabled_reason<'a>(e: &'a crate::monitor::Entry, ps: &[Process]) -> &'a str {
    if !e.error.is_empty() {
        return "Process control unavailable: sampling failed. Retry monitoring.";
    }
    if e.paused {
        return "Process control paused. Resume monitoring to continue.";
    }
    if e.stopping {
        return "Waiting for the current process operation to finish.";
    }
    if !e.live() {
        return "Waiting for a fresh sample before enabling process control.";
    }
    if e.snapshot.as_ref().is_some_and(|s| !s.processes_available) {
        return "Process information is unavailable; process control is disabled.";
    }
    if ps.is_empty() {
        return "There are no processes to end in this scope.";
    }
    if let Some(s) = &e.snapshot {
        if !s.stop_reason.is_empty() {
            return &s.stop_reason;
        }
    }
    if ps.iter().any(|p| p.token.is_empty()) {
        return "Some process identities could not be verified. Wait for a fresh sample.";
    }
    "Some processes cannot be controlled with the current SSH account or host environment."
}

pub(super) fn draw(
    ui: &mut TermviaiUi,
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    left: f32,
    w: f32,
    h: f32,
    hover: &Option<Action>,
    fonts: &wezterm_font::FontConfiguration,
    scale: f32,
) -> anyhow::Result<()> {
    tile(t, left, 52., w - left, h - 52., "", BG, INK, false);
    let x = left + 26.;
    let width = (w - x - 26.).max(1.);
    let m = &mut ui.monitor;
    if m.dialog.is_none() {
        m.scrollbar = None;
        m.scroll_drag = None;
        m.scroll_hover = false;
    }
    if let Some(index) = m.selected.filter(|i| *i < m.data.entries.len()) {
        let e = &m.data.entries[index];
        let breadcrumb_width =
            text_positions(fonts.ui_font(12, false)?.as_ref(), "Monitor", scale)?
                .last()
                .copied()
                .unwrap_or(0.);
        ctl(
            t,
            hits,
            x - 6.,
            83.,
            breadcrumb_width + 12.,
            24.,
            "Monitor",
            Command::Home,
            hover,
        );
        {
            let v = t.last_mut().unwrap();
            v.transparent = true;
            v.fg = ACCENT;
            v.text_padding = 6.;
            v.border_color = None;
        }
        let separator_x = x + breadcrumb_width + 12.;
        symbol(t, separator_x, 86., 16., Icon::ChevronRight, MUTED);
        text(
            t,
            separator_x + 23.,
            83.,
            (width - breadcrumb_width - 35.).max(1.),
            24.,
            &e.host.label,
            MUTED,
        );
        label(
            t,
            x,
            128.,
            (width - 286.).max(100.),
            32.,
            &e.host.label,
            INK,
            24,
            true,
            false,
        );
        let mut display = e.host.clone();
        let _ = display.resolve_identity(&ui.store);
        let endpoint = if display.username.is_empty() {
            display.endpoint()
        } else {
            format!("{}@{}", display.username, display.endpoint())
        };
        text(
            t,
            x,
            165.,
            width,
            24.,
            format!(
                "{}{}",
                endpoint,
                if display.group.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", display.group)
                }
            ),
            MUTED,
        );
        let (action, word, icon) = if !e.error.is_empty() {
            (Command::Retry(index), "Retry", Icon::Refresh)
        } else if e.paused {
            (Command::Pause(index), "Resume", Icon::Play)
        } else {
            (Command::Pause(index), "Pause", Icon::Pause)
        };
        icon_control(
            t,
            hits,
            x + width - 284.,
            132.,
            96.,
            word,
            action,
            icon,
            hover,
        );
        let ps = e
            .snapshot
            .as_ref()
            .map(|s| s.processes(None, None))
            .unwrap_or_default();
        if controls_enabled(e, &ps) {
            icon_control(
                t,
                hits,
                x + width - 180.,
                132.,
                180.,
                "End all NPU processes",
                Command::StopHost,
                Icon::Checkbox,
                hover,
            );
        } else {
            disabled_ctl(
                t,
                x + width - 180.,
                132.,
                180.,
                36.,
                "End all NPU processes",
            );
            let v = t.last_mut().unwrap();
            v.icon = Some(Icon::Checkbox);
            v.icon_size = 16.;
        }
        tile(t, x, 207., width, 1., "", "#353648", INK, false);
        let (state, fg) = status(e);
        text(t, x, 219., 115., 24., state, fg);
        let topology = e
            .snapshot
            .as_ref()
            .map(|s| format!("{} NPU / {} Die", s.cards().len(), s.dies.len()))
            .unwrap_or_else(|| "Topology unknown".into());
        text(t, x + 120., 219., 130., 24., topology, INK);
        let capacity = e.snapshot.as_ref().and_then(|s| {
            let first = s.dies.first()?.total_mb?;
            s.dies
                .iter()
                .all(|d| d.total_mb == Some(first))
                .then_some(first)
        });
        text(
            t,
            x + 256.,
            219.,
            110.,
            24.,
            capacity
                .map(|v| format!("{} GB / Die", gb(Some(v))))
                .unwrap_or_else(|| "Capacity —".into()),
            MUTED,
        );
        if width > 560. {
            text(
                t,
                x + 374.,
                219.,
                width - 374.,
                24.,
                if e.paused {
                    format!(
                        "Snapshot · {}s ago",
                        e.updated.map(|v| v.elapsed().as_secs()).unwrap_or(0)
                    )
                } else {
                    format!("Every {}s", m.data.interval)
                },
                MUTED,
            );
        }
        if !e.error.is_empty() || e.snapshot.is_none() {
            let cy = 260. + ((h - 280. - 256.).max(0.) / 2.);
            symbol(
                t,
                x + (width - 36.) / 2.,
                cy,
                36.,
                if e.error.is_empty() {
                    Icon::Activity
                } else {
                    Icon::Unplug
                },
                ACCENT,
            );
            label(
                t,
                x,
                cy + 44.,
                width,
                30.,
                if e.error.is_empty() {
                    "Waiting for first sample"
                } else {
                    "No sample available"
                },
                INK,
                17,
                true,
                true,
            );
            label(
                t,
                x,
                cy + 84.,
                width,
                24.,
                if e.paused && e.error.is_empty() {
                    "Resume sampling to receive NPU metrics."
                } else if e.error.is_empty() {
                    "Connecting to this host to collect NPU metrics."
                } else {
                    reason(e)
                },
                MUTED,
                12,
                false,
                true,
            );
            if !e.error.is_empty() {
                icon_control(
                    t,
                    hits,
                    x + (width - 140.) / 2.,
                    cy + 118.,
                    140.,
                    "View reason",
                    Command::Reason,
                    Icon::Alert,
                    hover,
                );
                icon_control(
                    t,
                    hits,
                    x + (width - 150.) / 2.,
                    cy + 164.,
                    150.,
                    "Retry connection",
                    Command::Retry(index),
                    Icon::Refresh,
                    hover,
                );
            }
            m.scroll = 0;
            m.max_scroll = 0;
        } else if let Some(s) = &e.snapshot {
            let cards = s.cards();
            let cols = (((width + 10.) / 290.).floor().max(1.) as usize).min(cards.len().max(1));
            let cw = (width - 10. * (cols - 1) as f32) / cols as f32;
            let max_dies = cards
                .iter()
                .map(|n| s.dies.iter().filter(|d| d.npu == *n).count())
                .max()
                .unwrap_or(1);
            let rh = 42. + max_dies as f32 * 34.;
            let range = rows_range(
                (cards.len() + cols - 1) / cols,
                h - 294.,
                rh + 10.,
                &mut m.scroll,
                &mut m.max_scroll,
            );
            for (row, ri) in range.enumerate() {
                for col in 0..cols {
                    let Some(n) = cards.get(ri * cols + col) else {
                        continue;
                    };
                    let cx = x + col as f32 * (cw + 10.);
                    let cy = 260. + row as f32 * (rh + 10.);
                    ctl(t, hits, cx, cy, cw, rh, "", Command::Npu(*n), hover);
                    label(
                        t,
                        cx + 16.,
                        cy + 7.,
                        55.,
                        22.,
                        format!("NPU {n}"),
                        INK,
                        12,
                        true,
                        false,
                    );
                    label(
                        t,
                        cx + 83.,
                        cy + 7.,
                        cw - 169.,
                        22.,
                        "HBM · GB",
                        MUTED,
                        11,
                        false,
                        false,
                    );
                    right_text(t, cx + cw - 74., cy + 7., 58., 22., "AICore", MUTED, 11);
                    for (j, d) in s.dies.iter().filter(|d| d.npu == *n).enumerate() {
                        let dy = cy + 31. + j as f32 * 34.;
                        text(t, cx + 16., dy, 55., 24., format!("Die {}", d.die), MUTED);
                        metric(
                            t,
                            cx + 83.,
                            dy + 12.,
                            (cw - 207.).max(16.),
                            d.used_mb
                                .zip(d.total_mb)
                                .filter(|(_, b)| *b > 0.)
                                .map(|(a, b)| a / b * 100.),
                        );
                        right_text(t, cx + cw - 115., dy, 29., 24., gb(d.used_mb), INK, 12);
                        right_text(t, cx + cw - 74., dy, 58., 24., percent(d.core), INK, 12);
                    }
                }
            }
            text(
                t,
                x,
                h - 34.,
                width,
                22.,
                "Hover a card to preview processes · Click to inspect",
                MUTED,
            );
        }
    } else {
        label(
            t,
            x,
            82.,
            width - 145.,
            32.,
            "Monitor",
            INK,
            24,
            true,
            false,
        );
        text(t, x, 119., width, 24., "Ascend NPU · Host resources", MUTED);
        icon_control(
            t,
            hits,
            x + width - 128.,
            89.,
            128.,
            "Add hosts",
            Command::Add,
            Icon::Plus,
            hover,
        );
        let tight = width < 650.;
        let grid_y = if tight { 250. } else { 217. };
        let totals = [
            (format!("{} hosts", m.data.entries.len()), INK),
            (
                format!(
                    "{} collecting",
                    m.data.entries.iter().filter(|e| e.live()).count()
                ),
                "#59b79e",
            ),
            (
                format!(
                    "{} paused",
                    m.data.entries.iter().filter(|e| e.paused).count()
                ),
                MUTED,
            ),
            (
                format!(
                    "{} unavailable",
                    m.data
                        .entries
                        .iter()
                        .filter(|e| !e.error.is_empty())
                        .count()
                ),
                "#ed8193",
            ),
        ];
        for (i, (s, fg)) in totals.iter().cloned().enumerate() {
            text(t, x + i as f32 * 100., 166., 100., 24., s, fg);
        }
        let iy = if tight { 205. } else { 159. };
        text(t, x + width - 166., iy + 4., 80., 28., "Interval", MUTED);
        ctl(
            t,
            hits,
            x + width - 80.,
            iy,
            80.,
            36.,
            format!("{}s", m.data.interval),
            Command::Interval,
            hover,
        );
        t.last_mut().unwrap().right_inset = 18.;
        symbol(t, x + width - 25., iy + 10., 16., Icon::ChevronDown, MUTED);
        let cols = ((width + 10.) / 295.).floor().max(1.) as usize;
        let cw = (width - 10. * (cols - 1) as f32) / cols as f32;
        let range = rows_range(
            (m.data.entries.len() + cols - 1) / cols,
            h - grid_y - 18.,
            154.,
            &mut m.scroll,
            &mut m.max_scroll,
        );
        // Menus are drawn after all cards to keep their visual and hit order aligned.
        let mut menu_anchor = None;
        for (row, ri) in range.enumerate() {
            for col in 0..cols {
                let index = ri * cols + col;
                let Some(e) = m.data.entries.get(index) else {
                    continue;
                };
                let cx = x + col as f32 * (cw + 10.);
                let cy = grid_y + row as f32 * 154.;
                ctl(t, hits, cx, cy, cw, 144., "", Command::Host(index), hover);
                symbol(t, cx + 15., cy + 22., 20., Icon::Hosts, MUTED);
                label(
                    t,
                    cx + 42.,
                    cy + 10.,
                    cw - 90.,
                    24.,
                    &e.host.label,
                    INK,
                    16,
                    true,
                    false,
                );
                text(t, cx + 42., cy + 33., cw - 66., 21., &e.host.address, MUTED);
                ctl(
                    t,
                    hits,
                    cx + cw - 42.,
                    cy + 14.,
                    30.,
                    30.,
                    "",
                    Command::Menu(index),
                    hover,
                );
                let v = t.last_mut().unwrap();
                v.icon = Some(Icon::More);
                v.transparent = true;
                let (state, fg) = status(e);
                text(t, cx + 16., cy + 62., 100., 24., state, fg);
                if !e.error.is_empty() || e.paused {
                    let message = if !e.error.is_empty() {
                        reason(e).to_owned()
                    } else {
                        format!(
                            "{}s ago",
                            e.updated.map(|v| v.elapsed().as_secs()).unwrap_or(0)
                        )
                    };
                    text(t, cx + 112., cy + 62., cw - 216., 24., message, MUTED);
                    ctl(
                        t,
                        hits,
                        cx + cw - 100.,
                        cy + 62.,
                        86.,
                        24.,
                        if e.paused { "Resume" } else { "Retry" },
                        if e.paused {
                            Command::Pause(index)
                        } else {
                            Command::Retry(index)
                        },
                        hover,
                    );
                    let v = t.last_mut().unwrap();
                    v.fg = ACCENT;
                    v.icon = Some(if e.paused { Icon::Play } else { Icon::Refresh });
                    v.icon_size = 14.;
                    v.text_padding = 4.;
                    v.border_color = None;
                    v.transparent = true;
                } else if let Some(s) = &e.snapshot {
                    label(
                        t,
                        cx + cw - 126.,
                        cy + 62.,
                        110.,
                        24.,
                        format!("{} NPU · {} Die", s.cards().len(), s.dies.len()),
                        MUTED,
                        12,
                        false,
                        true,
                    );
                }
                if let Some(s) = e.snapshot.as_ref().filter(|_| e.error.is_empty()) {
                    let total: f64 = s.dies.iter().filter_map(|d| d.total_mb).sum();
                    let used: f64 = s.dies.iter().filter_map(|d| d.used_mb).sum();
                    let mem = (s
                        .dies
                        .iter()
                        .all(|d| d.used_mb.is_some() && d.total_mb.is_some())
                        && total > 0.)
                        .then_some(used / total * 100.);
                    let core = s.dies.iter().all(|d| d.core.is_some()).then(|| {
                        s.dies.iter().filter_map(|d| d.core).sum::<f64>() / s.dies.len() as f64
                    });
                    let mw = (cw - 54.) / 2.;
                    for (i, (title, value)) in
                        [("HBM", mem), ("AICore", core)].iter().cloned().enumerate()
                    {
                        let sx = cx + 16. + i as f32 * (mw + 22.);
                        text(t, sx, cy + 98., mw - 52., 23., title, MUTED);
                        label(
                            t,
                            sx + mw - 54.,
                            cy + 94.,
                            54.,
                            28.,
                            percent(value),
                            INK,
                            18,
                            true,
                            true,
                        );
                        metric(t, sx, cy + 126., mw, value);
                    }
                } else {
                    symbol(
                        t,
                        cx + 16.,
                        cy + 104.,
                        18.,
                        if e.error.is_empty() {
                            Icon::Activity
                        } else {
                            Icon::Alert
                        },
                        if e.error.is_empty() {
                            ACCENT
                        } else {
                            "#ed8193"
                        },
                    );
                    text(
                        t,
                        cx + 42.,
                        cy + 92.,
                        cw - 58.,
                        22.,
                        if e.error.is_empty() {
                            "Establishing monitoring connection"
                        } else {
                            "HBM and AICore data unavailable"
                        },
                        INK,
                    );
                    text(
                        t,
                        cx + 42.,
                        cy + 113.,
                        cw - 58.,
                        22.,
                        if e.error.is_empty() {
                            "Sampling starts once connected."
                        } else {
                            "Host is retained. Retry to reconnect."
                        },
                        MUTED,
                    );
                }
                if m.menu == Some(index) {
                    menu_anchor = Some((index, cx + cw - 184., cy + 46.));
                }
            }
        }
        if let Some((index, mx, my)) = menu_anchor {
            let start = t.len();
            tile(t, mx, my, 174., 116., "", SIDE, INK, true);
            t.last_mut().unwrap().border_color = Some("#353648");
            let e = &m.data.entries[index];
            let action = if !e.error.is_empty() {
                ("Retry connection", Command::Retry(index), Icon::Refresh)
            } else if e.paused {
                ("Resume sampling", Command::Pause(index), Icon::Play)
            } else {
                ("Pause sampling", Command::Pause(index), Icon::Pause)
            };
            for (j, (name, cmd, icon)) in [
                action,
                ("View connection", Command::Reason, Icon::Alert),
                ("Remove monitor", Command::Remove(index), Icon::Trash),
            ]
            .iter()
            .cloned()
            .enumerate()
            {
                icon_control(
                    t,
                    hits,
                    mx + 4.,
                    my + 4. + j as f32 * 36.,
                    166.,
                    name,
                    cmd,
                    icon,
                    hover,
                );
                t.last_mut().unwrap().border_color = None;
                t.last_mut().unwrap().content_centered = false;
            }
            for v in &mut t[start..] {
                v.zindex = 34;
            }
        }
        if m.interval_open {
            let start = t.len();
            tile(t, x + width - 80., iy + 40., 80., 112., "", SIDE, INK, true);
            for (i, v) in [2, 5, 10].iter().cloned().enumerate() {
                ctl(
                    t,
                    hits,
                    x + width - 76.,
                    iy + 44. + i as f32 * 34.,
                    72.,
                    32.,
                    format!("{v}s"),
                    Command::SetInterval(v),
                    hover,
                );
            }
            for v in &mut t[start..] {
                v.zindex = 35;
            }
        }
        if m.data.entries.is_empty() {
            let cy = 250. + ((h - 420.).max(0.) / 2.);
            symbol(t, x + (width - 36.) / 2., cy, 36., Icon::Activity, ACCENT);
            label(
                t,
                x,
                cy + 48.,
                width,
                30.,
                "Add existing hosts to start monitoring",
                INK,
                17,
                true,
                true,
            );
            label(
                t,
                x,
                cy + 86.,
                width,
                24.,
                "Reuse saved Password, SSH Key or Identity authentication.",
                MUTED,
                12,
                false,
                true,
            );
            icon_control(
                t,
                hits,
                x + (width - 168.) / 2.,
                cy + 126.,
                168.,
                "Choose from Hosts",
                Command::Add,
                Icon::Plus,
                hover,
            );
        }
    }
    if !m.data.load_error.is_empty() {
        text(t, x, h - 62., width, 40., &m.data.load_error, "#ed8193");
    }
    if let Some(dialog) = m.dialog {
        draw_dialog(ui, t, hits, w, h, dialog, hover, fonts, scale)?;
    } else {
        if !m.error.is_empty() {
            text(t, x, h - 62., width, 40., &m.error, "#ed8193");
        }
    }
    if let Some(focus) = &ui.monitor.focus {
        if let Some(hit) = hits.iter().find(|h| {
            &h.action == focus
                && (h.h <= 64.
                    || matches!(
                        h.action,
                        Action::Monitor(Command::Host(_) | Command::Npu(_))
                    ))
        }) {
            tile(
                t,
                hit.x - 2.,
                hit.y - 2.,
                hit.w + 4.,
                hit.h + 4.,
                "",
                ACCENT,
                INK,
                true,
            );
            let v = t.last_mut().unwrap();
            v.outline_width = 1.;
            v.zindex = 44;
        }
    }
    Ok(())
}

fn draw_dialog(
    ui: &mut TermviaiUi,
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    w: f32,
    h: f32,
    dialog: Dialog,
    hover: &Option<Action>,
    fonts: &wezterm_font::FontConfiguration,
    scale: f32,
) -> anyhow::Result<()> {
    let width = 560_f32.min(w - 32.);
    let x = w - width;
    let cx = x + 24.;
    let cw = width - 48.;
    let footer = h - 66.;
    let start = t.len();
    tile(t, 0., 52., w, h - 52., "", "#080911", INK, false);
    t.last_mut().unwrap().opacity = 0.55;
    tile(t, x, 52., width, h - 52., "", SIDE, INK, false);
    // Match other drawers: block the page, not the application caption and
    // window controls (including the maximize / restore button).
    hits.retain(|hit| hit.y < 52.);
    hits.push(Hit {
        x: 0.,
        y: 52.,
        w: x,
        h: h - 52.,
        action: Action::Monitor(Command::Close),
    });
    hits.push(Hit {
        x,
        y: 52.,
        w: width,
        h: h - 52.,
        action: Action::Monitor(Command::Noop),
    });
    let store = &ui.store;
    let m = &mut ui.monitor;
    let cache = &m.text_wrap;
    let wrap = |text: &str, width: f32, size: u16, bold: bool| {
        let font = fonts.ui_font(size, bold)?;
        cache.borrow_mut().wrap(&font, text, width, scale)
    };
    let title = match dialog {
        Dialog::Picker => "Add to Monitor".into(),
        Dialog::Processes => format!("NPU {}", m.npu),
        Dialog::Stop => "End processes".into(),
        Dialog::Remove => "Remove monitor".into(),
        Dialog::Reason => "Connection details".into(),
    };
    label(t, cx, 76., cw - 42., 30., title, INK, 17, true, false);
    ctl(t, hits, w - 54., 75., 30., 30., "", Command::Close, hover);
    t.last_mut().unwrap().icon = Some(Icon::Close);
    t.last_mut().unwrap().transparent = true;
    let mut body_top = 152.;
    let mut body = Vec::new();
    let mut body_hits = Vec::new();
    let mut y = body_top;
    match dialog {
        Dialog::Picker => {
            text(
                t,
                cx,
                111.,
                cw,
                24.,
                "Choose saved hosts · Existing authentication is reused",
                MUTED,
            );
            button(
                t,
                hits,
                cx,
                149.,
                cw,
                38.,
                if ui.query.value.is_empty() {
                    "Search hosts or groups…".into()
                } else {
                    ui.query.value.clone()
                },
                FIELD,
                Action::Search,
                hover,
            );
            t.last_mut().unwrap().input = Some(InputPaint {
                masked: false,
                input: ui.query.clone(),
                focused: ui.search_focus,
            });
            body_top = 203.;
            y = body_top;
            let query = ui.query.value.to_lowercase();
            let mut candidates: Vec<_> = store
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, host)| {
                    format!("{} {} {}", host.label, host.address, host.group)
                        .to_lowercase()
                        .contains(&query)
                })
                .collect();
            candidates.sort_by_key(|(_, host)| store.group_path(store.host_group_id(host)));
            let mut group = String::new();
            for (i, host) in candidates {
                let path = store.group_path(store.host_group_id(host));
                if group != path || y == body_top {
                    group = path;
                    text(
                        &mut body,
                        cx,
                        y,
                        cw,
                        24.,
                        if group.is_empty() {
                            "Ungrouped"
                        } else {
                            &group
                        },
                        MUTED,
                    );
                    y += 30.;
                }
                let added = m.data.contains(host);
                if !added {
                    ctl(
                        &mut body,
                        &mut body_hits,
                        cx,
                        y,
                        cw,
                        58.,
                        "",
                        Command::Pick(i),
                        hover,
                    );
                } else {
                    tile(&mut body, cx, y, cw, 58., "", CARD, MUTED, true);
                }
                tile(
                    &mut body,
                    cx + 8.,
                    y + 14.,
                    28.,
                    28.,
                    "",
                    CARD,
                    ACCENT,
                    false,
                );
                let v = body.last_mut().unwrap();
                v.transparent = true;
                v.icon = Some(if added || m.chosen.contains(&i) {
                    Icon::CheckboxChecked
                } else {
                    Icon::Checkbox
                });
                text(
                    &mut body,
                    cx + 44.,
                    y + 6.,
                    cw - 130.,
                    24.,
                    &host.label,
                    INK,
                );
                text(
                    &mut body,
                    cx + 44.,
                    y + 30.,
                    cw - 90.,
                    22.,
                    host.endpoint(),
                    MUTED,
                );
                if added {
                    text(&mut body, cx + cw - 70., y + 7., 65., 24., "Added", MUTED);
                }
                y += 66.;
            }
            if y == body_top {
                text(&mut body, cx, y, cw, 28., "No matching hosts", MUTED);
                y += 36.;
            }
            ctl(
                t,
                hits,
                w - 232.,
                footer + 12.,
                84.,
                36.,
                "Cancel",
                Command::Close,
                hover,
            );
            if !m.chosen.is_empty() {
                ctl(
                    t,
                    hits,
                    w - 138.,
                    footer + 12.,
                    114.,
                    36.,
                    format!("Add {} hosts", m.chosen.len()),
                    Command::AcceptAdd,
                    hover,
                );
            }
        }
        Dialog::Processes => {
            if let Some(e) = m.selected.and_then(|i| m.data.entries.get(i)) {
                text(
                    t,
                    cx,
                    111.,
                    cw,
                    24.,
                    format!("{} · {}", e.host.label, e.status()),
                    MUTED,
                );
                if let Some(s) = &e.snapshot {
                    let dies: Vec<_> = s.dies.iter().filter(|d| d.npu == m.npu).collect();
                    let mut fx = cx;
                    for d in std::iter::once(None).chain(dies.iter().map(|d| Some(d.die))) {
                        ctl(
                            t,
                            hits,
                            fx,
                            148.,
                            72.,
                            32.,
                            d.map(|d| format!("Die {d}"))
                                .unwrap_or_else(|| "All".into()),
                            Command::Filter(d),
                            hover,
                        );
                        if m.die == d {
                            t.last_mut().unwrap().bg = "#34314f";
                            t.last_mut().unwrap().fg = ACCENT;
                        }
                        fx += 78.;
                    }
                    let shown: Vec<_> = dies
                        .iter()
                        .filter(|d| m.die.map_or(true, |v| v == d.die))
                        .collect();
                    let sum = |total: bool| {
                        let values: Vec<_> = shown
                            .iter()
                            .map(|d| if total { d.total_mb } else { d.used_mb })
                            .collect();
                        values
                            .iter()
                            .all(Option::is_some)
                            .then(|| values.into_iter().flatten().sum::<f64>())
                    };
                    let core = shown.iter().all(|d| d.core.is_some()).then(|| {
                        shown.iter().filter_map(|d| d.core).sum::<f64>() / shown.len().max(1) as f64
                    });
                    text(
                        t,
                        cx,
                        192.,
                        cw,
                        26.,
                        format!(
                            "HBM {} / {} GB     AICore {}",
                            gb(sum(false)),
                            gb(sum(true)),
                            percent(core)
                        ),
                        INK,
                    );
                    let ps = s.processes(Some(m.npu), m.die);
                    text(
                        t,
                        cx,
                        235.,
                        cw - 180.,
                        30.,
                        format!(
                            "Processes · {}",
                            if s.processes_available {
                                ps.len().to_string()
                            } else {
                                "Unavailable".into()
                            }
                        ),
                        INK,
                    );
                    let label = m
                        .die
                        .map(|d| format!("End Die {d} tasks"))
                        .unwrap_or_else(|| "End all card tasks".into());
                    if controls_enabled(e, &ps) {
                        ctl(
                            t,
                            hits,
                            cx + cw - 166.,
                            232.,
                            166.,
                            34.,
                            label,
                            Command::StopScope,
                            hover,
                        );
                    } else {
                        disabled_ctl(t, cx + cw - 166., 232., 166., 34., label);
                    }
                    body_top = 282.;
                    y = body_top;
                    if !controls_enabled(e, &ps) && !ps.is_empty() {
                        for line in wrap(control_disabled_reason(e, &ps), cw - 8., 12, false)? {
                            text(&mut body, cx, y, cw, 22., line, MUTED);
                            y += 22.;
                        }
                        y += 10.;
                    }
                    for p in &ps {
                        let process_start = body.len();
                        let cwd = wrap(
                            &format!(
                                "Working directory: {}",
                                if p.cwd.is_empty() {
                                    "Unavailable".into()
                                } else {
                                    clean(&p.cwd)
                                }
                            ),
                            cw - 32.,
                            13,
                            false,
                        )?;
                        let cwd_height = cwd.len() as f32 * 22. + 6.;
                        let lines = wrap(&clean(&p.command), cw - 32., 13, false)?;
                        let lines = if lines.is_empty() {
                            vec!["Command unavailable".into()]
                        } else {
                            lines
                        };
                        let loc = wrap(&locations(s, p.pid), cw - 32., 13, false)?;
                        let ph = 110. + cwd_height + (lines.len() + loc.len()) as f32 * 22.;
                        tile(&mut body, cx, y, cw, ph, "", CARD, INK, true);
                        text(
                            &mut body,
                            cx + 14.,
                            y + 10.,
                            cw - 140.,
                            24.,
                            clean(&p.name),
                            INK,
                        );
                        body.last_mut().unwrap().text_bold = true;
                        text(
                            &mut body,
                            cx + 14.,
                            y + 35.,
                            cw - 28.,
                            22.,
                            format!("PID {} · {}", p.pid, clean(&p.owner)),
                            MUTED,
                        );
                        if controls_enabled(e, &[p.clone()]) {
                            ctl(
                                &mut body,
                                &mut body_hits,
                                cx + cw - 114.,
                                y + 12.,
                                100.,
                                32.,
                                "End process",
                                Command::StopPid(p.pid),
                                hover,
                            );
                        } else {
                            disabled_ctl(
                                &mut body,
                                cx + cw - 114.,
                                y + 12.,
                                100.,
                                32.,
                                "End process",
                            );
                        }
                        let usage: Vec<_> = s
                            .processes
                            .iter()
                            .filter(|v| {
                                v.pid == p.pid
                                    && v.npu == m.npu
                                    && m.die.map_or(true, |d| v.die == d)
                            })
                            .collect();
                        let memory = usage
                            .iter()
                            .all(|p| p.memory_mb.is_some())
                            .then(|| usage.iter().filter_map(|p| p.memory_mb).sum());
                        text(
                            &mut body,
                            cx + 14.,
                            y + 59.,
                            cw - 28.,
                            22.,
                            format!("HBM {} GB · selected scope", gb(memory)),
                            MUTED,
                        );
                        for (j, line) in cwd.iter().enumerate() {
                            text(
                                &mut body,
                                cx + 14.,
                                y + 87. + j as f32 * 22.,
                                cw - 28.,
                                22.,
                                line,
                                INK,
                            );
                        }
                        for (j, line) in lines.iter().enumerate() {
                            text(
                                &mut body,
                                cx + 14.,
                                y + 87. + cwd_height + j as f32 * 22.,
                                cw - 28.,
                                22.,
                                line,
                                INK,
                            );
                        }
                        for (j, line) in loc.iter().enumerate() {
                            text(
                                &mut body,
                                cx + 14.,
                                y + 94. + cwd_height + (lines.len() + j) as f32 * 22.,
                                cw - 28.,
                                22.,
                                line,
                                MUTED,
                            );
                        }
                        for tile in &mut body[process_start..] {
                            if !tile.text.is_empty() {
                                tile.text_size = Some(13);
                            }
                        }
                        y += ph + 12.;
                    }
                    if ps.is_empty() {
                        text(
                            &mut body,
                            cx,
                            y,
                            cw,
                            30.,
                            if s.processes_available {
                                "No NPU processes in this scope"
                            } else {
                                "Process information is unavailable"
                            },
                            MUTED,
                        );
                        y += 40.;
                    }
                    for result in &e.results {
                        text(
                            &mut body,
                            cx,
                            y,
                            cw,
                            24.,
                            format!("PID {} · {}", result.pid, clean(&result.status)),
                            MUTED,
                        );
                        y += 26.;
                    }
                    let note = if ps.is_empty() {
                        control_disabled_reason(e, &ps)
                    } else {
                        "Ending a shared process affects every Die it occupies."
                    };
                    for line in wrap(note, cw - 8., 12, false)? {
                        text(&mut body, cx, y, cw, 22., line, MUTED);
                        y += 22.;
                    }
                }
            }
            ctl(
                t,
                hits,
                w - 110.,
                footer + 12.,
                86.,
                36.,
                "Close",
                Command::Close,
                hover,
            );
        }
        Dialog::Stop => {
            if let Some(e) = m.selected.and_then(|i| m.data.entries.get(i)) {
                text(
                    t,
                    cx,
                    113.,
                    cw,
                    24.,
                    format!("{} · {}", clean(&e.host.label), e.host.endpoint()),
                    MUTED,
                );
            }
            text(&mut body, cx, y, cw, 18., "Operation scope", MUTED);
            y += 24.;
            for line in wrap(&m.stop_label, cw, 14, true)? {
                label(&mut body, cx, y, cw, 22., line, INK, 14, true, false);
                y += 22.;
            }
            text(
                &mut body,
                cx,
                y,
                cw,
                20.,
                format!(
                    "{} unique processes · {} affected Dies",
                    m.targets.len(),
                    m.stop_dies
                ),
                MUTED,
            );
            y += 36.;
            tile(&mut body, cx, y, cw, 1., "", "#353648", INK, false);
            for row in &m.stop_rows {
                y += 10.;
                let name_width = (cw - 12.) * 0.46;
                let meta_width = cw - name_width - 12.;
                let names = wrap(&row.name, name_width, 13, true)?;
                let meta = wrap(&row.metadata, meta_width, 12, false)?;
                let lines = names.len().max(meta.len());
                for (i, line) in names.into_iter().enumerate() {
                    label(
                        &mut body,
                        cx,
                        y + i as f32 * 20.,
                        name_width,
                        20.,
                        line,
                        INK,
                        13,
                        true,
                        false,
                    );
                }
                for (i, line) in meta.into_iter().enumerate() {
                    right_text(
                        &mut body,
                        cx + name_width + 12.,
                        y + i as f32 * 20.,
                        meta_width,
                        20.,
                        line,
                        MUTED,
                        12,
                    );
                }
                y += lines as f32 * 20. + 4.;
                for line in wrap(&row.locations, cw, 12, false)? {
                    text(&mut body, cx, y, cw, 18., line, MUTED);
                    y += 18.;
                }
                y += 4.;
                for line in wrap(&format!("Working directory: {}", row.cwd), cw, 13, false)? {
                    label(&mut body, cx, y, cw, 22., line, INK, 13, false, false);
                    y += 22.;
                }
                y += 10.;
                tile(&mut body, cx, y, cw, 1., "", "#353648", INK, false);
            }
            y += 16.;
            for note in [
                "Running tasks will be interrupted. Shared processes are ended once and affect all of their Dies.",
                "Only the listed NPU processes are included. Each PID is checked again before requesting exit (SIGTERM).",
            ] {
                for line in wrap(note, cw, 12, false)? {
                    text(&mut body, cx, y, cw, 20., line, MUTED);
                    y += 20.;
                }
                y += 8.;
            }
            ctl(
                t,
                hits,
                w - 280.,
                footer + 12.,
                90.,
                36.,
                "Cancel",
                Command::CancelStop,
                hover,
            );
            ctl(
                t,
                hits,
                w - 180.,
                footer + 12.,
                156.,
                36.,
                format!("End {} processes", m.targets.len()),
                Command::AcceptStop,
                hover,
            );
        }
        Dialog::Reason => {
            if let Some(e) = m.selected.and_then(|i| m.data.entries.get(i)) {
                text(
                    t,
                    cx,
                    112.,
                    cw,
                    24.,
                    format!("{} · {}", e.host.label, e.host.endpoint()),
                    MUTED,
                );
                for line in wrap(
                    if e.error.is_empty() {
                        "No connection error was reported."
                    } else {
                        &e.error
                    },
                    cw - 8.,
                    12,
                    false,
                )? {
                    text(&mut body, cx, y, cw, 24., line, INK);
                    y += 26.;
                }
                icon_control(
                    t,
                    hits,
                    w - 174.,
                    footer + 12.,
                    150.,
                    "Retry connection",
                    Command::Retry(m.selected.unwrap()),
                    Icon::Refresh,
                    hover,
                );
            }
        }
        Dialog::Remove => {
            let label = m
                .selected
                .and_then(|i| m.data.entries.get(i))
                .map(|e| e.host.label.as_str())
                .unwrap_or("");
            for line in wrap(&format!("Stop monitoring {label}? The saved Host, credentials and open terminals will remain."),cw-10.,12,false)?{text(&mut body,cx,y,cw,26.,line,INK);y+=28.;}
            ctl(
                t,
                hits,
                w - 246.,
                footer + 12.,
                90.,
                36.,
                "Cancel",
                Command::Close,
                hover,
            );
            ctl(
                t,
                hits,
                w - 146.,
                footer + 12.,
                122.,
                36.,
                "Remove",
                Command::AcceptRemove,
                hover,
            );
        }
    }
    if !m.error.is_empty() {
        for line in wrap(&m.error, cw - 10., 12, false)? {
            text(&mut body, cx, y, cw, 24., line, "#ed8193");
            y += 26.;
        }
    }
    let body_bottom = footer - 10.;
    let now = Instant::now();
    let viewport = (body_bottom - body_top).max(0.);
    m.drawer_scroll
        .bounds(viewport, (y - body_top).max(0.), now);
    let shift = m.drawer_scroll.position;
    let content_start = t.len();
    for mut v in body {
        v.y -= shift;
        // Keep partial lines and card corners. The viewport masks below clip
        // them smoothly, instead of dropping each whole text row at the edge.
        if v.y < body_bottom && v.y + v.h > body_top && v.h > 0. {
            if v.text.is_empty() && v.icon.is_none() && v.input.is_none() {
                // A long card can begin above the entire window after scrolling.
                // Header/footer masks cover only the drawer, so bound its solid
                // geometry to that surface first. Keep the new rounded corners
                // under the masks; text baselines and visible card edges retain
                // their original continuous scroll positions.
                let bottom = (v.y + v.h).min(h);
                v.y = v.y.max(52.);
                v.h = (bottom - v.y).max(0.);
            }
            t.push(v);
        }
    }
    let content_end = t.len();
    for mut v in body_hits {
        v.y -= shift;
        // A partly obscured process action is not clickable.
        if v.y >= body_top && v.y + v.h <= body_bottom {
            hits.push(v);
        }
    }
    // Opaque masks sit above scrollable content but below the fixed header and
    // footer. This also clips glyphs without changing their texture or baseline.
    let mask_start = t.len();
    tile(
        t,
        x,
        52.,
        width,
        (body_top - 52.).max(0.),
        "",
        SIDE,
        INK,
        false,
    );
    tile(
        t,
        x,
        body_bottom,
        width,
        h - body_bottom,
        "",
        SIDE,
        INK,
        false,
    );
    let mask_end = t.len();
    m.scrollbar = None;
    if m.drawer_scroll.maximum > 0. && viewport > 0. {
        let thumb = (viewport * viewport / (viewport + m.drawer_scroll.maximum))
            .max(28.)
            .min(viewport);
        let bar = ScrollBar {
            x: w - 14.,
            top: body_top,
            bottom: body_bottom,
            thumb,
        };
        m.scrollbar = Some(bar);
        let engaged = m.scroll_hover || m.scroll_drag.is_some();
        m.scroll_hover_motion.set_target(
            if engaged { 1. } else { 0. },
            now,
            Duration::from_millis(120),
        );
        let emphasis = m.scroll_hover_motion.value(now);
        tile(t, w - 12., body_top, 10., viewport, "", FIELD, INK, true);
        t.last_mut().unwrap().opacity = emphasis * 0.55;
        let thickness = 3. + 3. * emphasis;
        tile(
            t,
            w - 4. - thickness,
            bar.thumb_top(shift, m.drawer_scroll.maximum),
            thickness,
            thumb,
            "",
            MUTED,
            INK,
            true,
        );
        t.last_mut().unwrap().opacity = 0.5 + emphasis * 0.4;
    } else {
        m.scroll_drag = None;
        m.scroll_hover = false;
        m.scroll_hover_motion = Tween::new(0.);
    }
    tile(t, cx, footer, cw, 1., "", "#353648", INK, false);
    for (index, v) in t.iter_mut().enumerate().skip(start) {
        v.zindex = if index < start + 2 {
            40
        } else if (content_start..content_end).contains(&index) {
            if v.transparent {
                42
            } else {
                41
            }
        } else if (mask_start..mask_end).contains(&index) {
            43
        } else {
            44
        };
    }
    Ok(())
}

/// Read-only preview: no hit regions, so hovering does not add another action.
pub(super) fn draw_preview(m: &MonitorUi, n: u32, anchor: &Hit, t: &mut Vec<Tile>, w: f32, h: f32) {
    if m.dialog.is_some() || h < 180. || w < 180. {
        return;
    }
    let Some(e) = m.selected.and_then(|i| m.data.entries.get(i)) else {
        return;
    };
    if !e.error.is_empty() {
        return;
    }
    let Some(s) = e.snapshot.as_ref() else {
        return;
    };
    let ps = s.processes(Some(n), None);
    let width = 330_f32.min(w - 16.);
    // Reserve room for the header, footer and overflow count in short windows.
    let limit = (((h - 60. - 8. - 94.) / 54.).floor().max(0.) as usize).min(5);
    let count = if s.processes_available {
        ps.len().min(limit)
    } else {
        0
    };
    let more = if s.processes_available {
        ps.len().saturating_sub(count)
    } else {
        0
    };
    let rows_height = if !s.processes_available || ps.is_empty() {
        30.
    } else {
        count as f32 * 54.
    };
    let height = 74. + rows_height + if more > 0 { 20. } else { 0. };
    let x = anchor.x.min(w - width - 8.).max(8.);
    let y = if anchor.y + anchor.h + 8. + height <= h - 8. {
        anchor.y + anchor.h + 8.
    } else {
        (anchor.y - height - 8.).max(60.).min(h - height - 8.)
    };
    let start = t.len();
    // Subtle layered shadow, then the same surface and border as the design card.
    for (spread, offset, alpha) in [(4., 5., 0.06), (2., 3., 0.10)] {
        tile(
            t,
            x - spread,
            y + offset - spread,
            width + spread * 2.,
            height + spread * 2.,
            "",
            "#000000",
            INK,
            true,
        );
        let v = t.last_mut().unwrap();
        v.opacity = alpha;
        v.corner_radius = 9. + spread;
    }
    tile(t, x, y, width, height, "", CARD, INK, true);
    t.last_mut().unwrap().border_color = Some("#353648");
    label(
        t,
        x + 12.,
        y + 12.,
        62.,
        18.,
        format!("NPU {n}"),
        INK,
        12,
        true,
        false,
    );
    let count_label = if s.processes_available {
        format!(
            "{} {}{}",
            ps.len(),
            if ps.len() == 1 {
                "process"
            } else {
                "processes"
            },
            if e.paused { " · Paused snapshot" } else { "" }
        )
    } else {
        "Unavailable".into()
    };
    right_text(
        t,
        x + 78.,
        y + 12.,
        width - 90.,
        18.,
        count_label,
        MUTED,
        11,
    );
    let mut row_y = y + 38.;
    if !s.processes_available || ps.is_empty() {
        text(
            t,
            x + 12.,
            row_y,
            width - 24.,
            30.,
            if s.processes_available {
                "No running NPU processes"
            } else {
                "Process information unavailable"
            },
            MUTED,
        );
        row_y += 30.;
    } else {
        for p in ps.iter().take(count) {
            tile(
                t,
                x + 12.,
                row_y,
                width - 24.,
                1.,
                "",
                "#353648",
                INK,
                false,
            );
            label(
                t,
                x + 12.,
                row_y + 8.,
                width - 110.,
                18.,
                clean(&p.name),
                INK,
                12,
                true,
                false,
            );
            label(
                t,
                x + 12.,
                row_y + 28.,
                width - 110.,
                16.,
                format!(
                    "PID {} · {}",
                    p.pid,
                    if p.owner.is_empty() {
                        "—".into()
                    } else {
                        clean(&p.owner)
                    }
                ),
                MUTED,
                11,
                false,
                false,
            );
            let rows: Vec<_> = s
                .processes
                .iter()
                .filter(|r| r.pid == p.pid && r.npu == n)
                .collect();
            let mem = rows
                .iter()
                .all(|p| p.memory_mb.is_some())
                .then(|| rows.iter().filter_map(|p| p.memory_mb).sum());
            right_text(
                t,
                x + width - 94.,
                row_y + 17.,
                82.,
                20.,
                format!("{} GB", gb(mem)),
                INK,
                12,
            );
            row_y += 54.;
        }
    }
    if more > 0 {
        label(
            t,
            x + 12.,
            row_y,
            width - 24.,
            20.,
            format!("+ {more} more · Click to view all"),
            MUTED,
            11,
            false,
            false,
        );
        row_y += 20.;
    }
    label(
        t,
        x + 12.,
        row_y + 6.,
        width - 24.,
        18.,
        "Click for full commands and process actions",
        MUTED,
        11,
        false,
        false,
    );
    for v in &mut t[start..] {
        v.zindex = 60;
    }
}

impl TermWindow {
    pub(super) fn termviai_monitor_key(
        &mut self,
        event: &KeyEvent,
        context: &dyn WindowOps,
    ) -> bool {
        if !event.key_is_down {
            return true;
        }
        let mut action = None;
        match event.key {
            KeyCode::Char('\u{1b}') => {
                action = Some(Action::Monitor(
                    if self.termviai_ui.monitor.dialog.is_some() {
                        Command::Close
                    } else {
                        Command::Home
                    },
                ));
            }
            KeyCode::Char('\t') => {
                let mut seen = HashSet::new();
                let actions: Vec<_> = self
                    .termviai_ui
                    .hits
                    .iter()
                    .filter(|h| {
                        (h.h <= 64.
                            || matches!(
                                h.action,
                                Action::Monitor(Command::Host(_) | Command::Npu(_))
                            ))
                            && matches!(h.action, Action::Monitor(_) | Action::Search)
                            && !matches!(h.action, Action::Monitor(Command::Noop))
                    })
                    .map(|h| h.action.clone())
                    .filter(|a| seen.insert(a.clone()))
                    .collect();
                if !actions.is_empty() {
                    let current = self
                        .termviai_ui
                        .monitor
                        .focus
                        .as_ref()
                        .and_then(|a| actions.iter().position(|b| a == b));
                    let next = if event.modifiers.contains(Modifiers::SHIFT) {
                        current
                            .map(|i| (i + actions.len() - 1) % actions.len())
                            .unwrap_or(actions.len() - 1)
                    } else {
                        current.map(|i| (i + 1) % actions.len()).unwrap_or(0)
                    };
                    self.termviai_ui.monitor.focus = Some(actions[next].clone());
                    self.termviai_ui.search_focus = actions[next] == Action::Search;
                }
            }
            KeyCode::Char('\r' | ' ') if !self.termviai_ui.search_focus => {
                action = self.termviai_ui.monitor.focus.clone();
            }
            KeyCode::DownArrow if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_by(1)
            }
            KeyCode::UpArrow if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_by(-1)
            }
            KeyCode::PageDown if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_page(1.)
            }
            KeyCode::PageUp if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_page(-1.)
            }
            KeyCode::Home if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_by(isize::MIN)
            }
            KeyCode::End if !self.termviai_ui.search_focus => {
                self.termviai_ui.monitor.scroll_by(isize::MAX)
            }
            _ => return !self.termviai_ui.search_focus,
        }
        if let Some(action) = action {
            if let Err(e) = self.termviai_action(action) {
                self.termviai_ui.monitor.error = format!("{e:#}");
            }
        }
        context.invalidate();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draw(
        ui: &mut TermviaiUi,
        t: &mut Vec<Tile>,
        hits: &mut Vec<Hit>,
        left: f32,
        w: f32,
        h: f32,
        hover: &Option<Action>,
    ) {
        thread_local! {
            static FONTS: wezterm_font::FontConfiguration = wezterm_font::FontConfiguration::new(
                Some(config::ConfigHandle::default_config()), 96).unwrap();
        }
        FONTS.with(|fonts| super::draw(ui, t, hits, left, w, h, hover, fonts, 1.).unwrap());
    }

    #[test]
    #[ignore = "manual CPU benchmark; excludes GPU presentation and native input"]
    fn benchmark_resize_layout_and_text() {
        let config = config::ConfigHandle::default_config();
        let fonts = wezterm_font::FontConfiguration::new(Some(config.clone()), 96).unwrap();
        for processes in [false, true] {
            let mut ui = if processes {
                long_process_ui()
            } else {
                preview_ui()
            };
            if processes {
                ui.monitor.dialog = Some(Dialog::Processes);
            }
            let mut cache = super::super::super::termviai_text_cache::UiTextCache::new(&config);
            for cached in [false, true] {
                let mut times = Vec::new();
                let mut glyphs = 0;
                for step in 0..140 {
                    let at = Instant::now();
                    let mut tiles = Vec::new();
                    let mut hits = Vec::new();
                    let w = 720. + ((step * 23) % 1000) as f32;
                    let h = 480. + ((step * 13) % 500) as f32;
                    super::draw(
                        &mut ui, &mut tiles, &mut hits, 176., w, h, &None, &fonts, 1.,
                    )
                    .unwrap();
                    for t in &tiles {
                        if t.text.is_empty() || t.y >= h || t.y + t.h <= 0. {
                            continue;
                        }
                        let font = fonts
                            .ui_font(t.text_size.unwrap_or(14), t.text_bold)
                            .unwrap();
                        let count = if cached {
                            cache.shape(&font, &t.text, 0, || {}).unwrap().len()
                        } else {
                            font.shape(
                                &t.text,
                                || {},
                                crate::customglyph::BlockKey::filter_out_synthetic,
                                None,
                                wezterm_bidi::Direction::LeftToRight,
                                None,
                                None,
                            )
                            .unwrap()
                            .len()
                        };
                        glyphs += std::hint::black_box(count);
                    }
                    if step >= 20 {
                        times.push(at.elapsed().as_secs_f64() * 1000.);
                    }
                }
                times.sort_by(f64::total_cmp);
                println!("RESIZE_CPU processes={} cached={} samples={} p50_ms={:.3} p95_ms={:.3} max_ms={:.3} glyphs={}",
                    processes, cached, times.len(), times[59], times[113], times[119], glyphs);
            }
        }
    }

    fn preview_ui() -> TermviaiUi {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.monitor.data.add(Host::default());
        ui.monitor.selected = Some(0);
        let processes = [
            (0, 0, 42, 51200.),
            (0, 1, 42, 40960.),
            (0, 0, 43, 10240.),
            (1, 0, 42, 1024.),
        ]
        .into_iter()
        .map(|(npu, die, pid, memory)| {
            serde_json::json!({
                "npu":npu,"die":die,"pid":pid,"name":"python3","memory_mb":memory,
                "token":"boot:123","command":"python3 train.py","owner":"root","controllable":false
            })
        })
        .collect::<Vec<_>>();
        let dies = (0..8)
            .flat_map(|npu| {
                (0..2).map(move |die| {
                    serde_json::json!({
                        "npu":npu,"die":die,"used_mb":52428,"total_mb":65536,"core":72
                    })
                })
            })
            .collect::<Vec<_>>();
        ui.monitor.data.entries[0].snapshot = Some(
            serde_json::from_value(serde_json::json!({
                "dies":dies,"processes":processes,"processes_available":true,"stop_reason":""
            }))
            .unwrap(),
        );
        ui
    }

    #[test]
    fn process_controls_keep_disabled_outline_and_explain_why() {
        for case in [
            "enabled",
            "empty",
            "paused",
            "stale",
            "identity",
            "permission",
            "platform",
            "error",
            "stopping",
        ] {
            let mut ui = preview_ui();
            ui.monitor.dialog = Some(Dialog::Processes);
            let e = &mut ui.monitor.data.entries[0];
            e.updated = Some(Instant::now());
            let s = e.snapshot.as_mut().unwrap();
            for p in &mut s.processes {
                p.controllable = true;
            }
            let reason = match case {
                "empty" => {
                    s.processes.clear();
                    "There are no processes"
                }
                "paused" => {
                    e.paused = true;
                    "Process control paused"
                }
                "stale" => {
                    e.updated = Some(Instant::now() - Duration::from_secs(31));
                    "Waiting for a fresh sample"
                }
                "identity" => {
                    s.processes[0].token.clear();
                    "Some process identities"
                }
                "permission" => {
                    s.processes[0].controllable = false;
                    "Some processes cannot"
                }
                "platform" => {
                    for p in &mut s.processes {
                        p.controllable = false;
                    }
                    s.stop_reason = "Host pidfd support required.".into();
                    "Host pidfd support required."
                }
                "error" => {
                    e.error = "SSH failed".into();
                    "sampling failed"
                }
                "stopping" => {
                    e.stopping = true;
                    "Waiting for the current"
                }
                _ => "",
            };
            let ps = e.snapshot.as_ref().unwrap().processes(Some(0), None);
            assert_eq!(controls_enabled(e, &ps), case == "enabled", "{case}");
            if case != "enabled" {
                assert!(control_disabled_reason(e, &ps).contains(reason), "{case}");
            }
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., 1258., 827., &None);
            let button = tiles
                .iter()
                .find(|t| t.text == "End all card tasks")
                .unwrap();
            assert!(
                button.rounded && !button.transparent && button.border_color.is_some(),
                "{case}"
            );
            assert_eq!(
                hits.iter()
                    .any(|h| h.action == Action::Monitor(Command::StopScope)),
                case == "enabled",
                "{case}"
            );
            for button in tiles.iter().filter(|t| t.text == "End process") {
                assert!(
                    button.rounded && !button.transparent && button.border_color.is_some(),
                    "{case}"
                );
            }
            if case != "enabled" {
                let text = tiles
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(text.contains(reason), "{case}: {text}");
            }
            // One restricted PID disables the scope action, but a different
            // verified PID keeps its individual action available.
            if ["identity", "permission"].contains(&case) {
                assert!(!hits
                    .iter()
                    .any(|h| h.action == Action::Monitor(Command::StopPid(42))));
                assert!(hits
                    .iter()
                    .any(|h| h.action == Action::Monitor(Command::StopPid(43))));
            } else if case != "enabled" {
                assert!(!hits
                    .iter()
                    .any(|h| matches!(h.action, Action::Monitor(Command::StopPid(_)))));
            }
        }
    }

    #[test]
    fn monitor_breadcrumb_fits_its_label_without_moving_text() {
        let mut ui = preview_ui();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut tiles, &mut hits, 176., 1258., 827., &None);
        let crumb = tiles
            .iter()
            .find(|t| t.action == Some(Action::Monitor(Command::Home)))
            .unwrap();
        let fonts =
            wezterm_font::FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96)
                .unwrap();
        let advance = *text_positions(&fonts.ui_font(12, false).unwrap(), "Monitor", 1.)
            .unwrap()
            .last()
            .unwrap();
        assert!((crumb.w - advance - 12.).abs() < 0.01);
        assert_eq!(crumb.x + crumb.text_padding, 202.);
        assert!(!crumb.content_centered);
        let hit = hits
            .iter()
            .find(|h| h.action == Action::Monitor(Command::Home))
            .unwrap();
        assert_eq!((hit.x, hit.w), (crumb.x, crumb.w));
    }

    fn long_process_ui() -> TermviaiUi {
        let mut ui = preview_ui();
        let s = ui.monitor.data.entries[0].snapshot.as_mut().unwrap();
        let p = s.processes[0].clone();
        s.processes = (42..45)
            .map(|pid| Process {
                pid,
                command: if pid == 44 {
                    "FINAL-COMMAND".into()
                } else {
                    "python --model /workspace/long-project --tokens 800000 ".repeat(60)
                },
                ..p.clone()
            })
            .collect();
        ui
    }

    #[test]
    fn scrollbar_is_proportional_and_dragging_is_continuous_without_easing_lag() {
        // Fractional DPI can leave less than one pixel of overflow.
        for overflow in [0.1, 0.5, 1.] {
            let bar = ScrollBar {
                x: 0.,
                top: 0.,
                bottom: 500.,
                thumb: 500. * 500. / (500. + overflow),
            };
            assert!((bar.thumb_top(overflow, overflow) + bar.thumb - 500.).abs() < 0.001);
        }
        let mut ui = long_process_ui();
        ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        let bar = ui.monitor.scrollbar.unwrap();
        let viewport = bar.bottom - bar.top;
        let expected =
            (viewport * viewport / (viewport + ui.monitor.drawer_scroll.maximum)).max(28.);
        assert!((bar.thumb - expected).abs() < 0.01);
        ui.monitor.scrollbar_mouse(
            &MouseEventKind::Press(MousePress::Left),
            1413.,
            bar.top + 8.,
            true,
        );
        let y = bar.top + bar.travel() * 0.317 + 8.;
        ui.monitor
            .scrollbar_mouse(&MouseEventKind::Move, 1413., y, true);
        let position = ui.monitor.drawer_scroll.position;
        assert!((position / ui.monitor.drawer_scroll.maximum - 0.317).abs() < 0.0001);
        assert!(!ui.monitor.drawer_scroll.active());
        assert!(position % 24. > 0.01);
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        let visible_thumb = ui.monitor.scrollbar.unwrap();
        assert!(
            (visible_thumb.thumb_top(
                ui.monitor.drawer_scroll.position,
                ui.monitor.drawer_scroll.maximum
            ) + 8.
                - y)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn tall_scrolled_cards_never_paint_over_window_caption_after_resize() {
        let mut ui = long_process_ui();
        ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
        // Start tall, then shrink with the drawer open, as in the reported case.
        for (w, h) in [(1725., 1091.), (1725., 677.), (720., 480.)] {
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            for offset in [0., 531.25, 1200.5] {
                ui.monitor.drawer_scroll.jump_to(offset, Instant::now());
                tiles.clear();
                hits.clear();
                draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
                let cards: Vec<_> = tiles.iter().filter(|t| t.zindex == 41).collect();
                assert!(!cards.is_empty());
                assert!(cards.iter().all(|t| t.y >= 52. && t.y + t.h <= h));
                assert!(tiles.iter().filter(|t| t.zindex >= 40).all(|t| t.y >= 52.));
                assert!(tiles.iter().any(|t| t.zindex == 43 && t.y == 52.));
                assert_eq!(
                    ui.monitor.drawer_scroll.position,
                    offset.min(ui.monitor.drawer_scroll.maximum)
                );
            }
        }
    }

    #[test]
    fn monitor_drawers_preserve_caption_controls_and_block_background() {
        for dialog in [
            Dialog::Processes,
            Dialog::Stop,
            Dialog::Picker,
            Dialog::Reason,
            Dialog::Remove,
        ] {
            let mut ui = preview_ui();
            ui.monitor.dialog = Some(dialog);
            let mut tiles = vec![];
            let mut hits = vec![
                Hit {
                    x: 0.,
                    y: 0.,
                    w: 1200.,
                    h: 52.,
                    action: Action::Drag,
                },
                Hit {
                    x: 1074.,
                    y: 0.,
                    w: 42.,
                    h: 52.,
                    action: Action::Minimize,
                },
                Hit {
                    x: 1116.,
                    y: 0.,
                    w: 42.,
                    h: 52.,
                    action: Action::Maximize,
                },
                Hit {
                    x: 1158.,
                    y: 0.,
                    w: 42.,
                    h: 52.,
                    action: Action::Close,
                },
                Hit {
                    x: 200.,
                    y: 260.,
                    w: 250.,
                    h: 120.,
                    action: Action::Connect(0),
                },
            ];
            draw(&mut ui, &mut tiles, &mut hits, 176., 1200., 720., &None);
            assert_eq!(hit_action(&hits, 500., 24.), Some(Action::Drag));
            assert_eq!(hit_action(&hits, 1090., 24.), Some(Action::Minimize));
            assert_eq!(hit_action(&hits, 1135., 24.), Some(Action::Maximize));
            assert_eq!(hit_action(&hits, 1180., 24.), Some(Action::Close));
            assert_eq!(
                hit_action(&hits, 300., 300.),
                Some(Action::Monitor(Command::Close))
            );
            assert!(!hits.iter().any(|hit| hit.action == Action::Connect(0)));
        }
    }

    #[test]
    fn partially_visible_text_is_clipped_by_fixed_viewport_masks_not_dropped() {
        let mut ui = long_process_ui();
        ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        let first_name = tiles
            .iter()
            .find(|t| t.text == "python3" && t.y >= 282.)
            .unwrap();
        let position = first_name.y - 282. + first_name.h / 2.;
        ui.monitor.drawer_scroll.jump_to(position, Instant::now());
        tiles.clear();
        hits.clear();
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        let partial = tiles
            .iter()
            .find(|t| t.text == "python3" && t.y < 282. && t.y + t.h > 282.)
            .unwrap();
        assert_eq!(partial.zindex, 42);
        assert!(tiles
            .iter()
            .any(|t| t.zindex == 43 && t.y == 52. && t.y + t.h == 282.));
        assert!(tiles.iter().any(|t| t.zindex == 44 && t.text == "NPU 0"));
        assert!(hits
            .iter()
            .filter(|t| matches!(t.action, Action::Monitor(Command::StopPid(_))))
            .all(|t| t.y >= 282.));
    }

    #[test]
    fn drawer_scroll_survives_background_paint_refresh_and_reaches_last_process() {
        for (w, h) in [(720., 480.), (1417., 872.)] {
            let mut ui = long_process_ui();
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            assert!(ui.monitor.drawer_scroll.maximum > 480.);
            ui.monitor.scroll_by(10);
            for _ in 0..3 {
                tiles.clear();
                hits.clear();
                draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
                assert_eq!(ui.monitor.drawer_scroll.target, 240.);
            }
            // A refreshed sample must not bounce an open drawer back to the top.
            ui.monitor.data.entries[0]
                .snapshot
                .as_mut()
                .unwrap()
                .processes[0]
                .memory_mb = Some(1024.);
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            assert_eq!(ui.monitor.drawer_scroll.target, 240.);
            ui.monitor.scroll_by(isize::MAX);
            ui.monitor
                .drawer_scroll
                .jump_to(ui.monitor.drawer_scroll.target, Instant::now());
            tiles.clear();
            hits.clear();
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            assert_eq!(
                ui.monitor.drawer_scroll.position,
                ui.monitor.drawer_scroll.maximum
            );
            assert!(tiles
                .iter()
                .any(|t| t.text.contains("Ending a shared process")));
            // In a short window the final note can push the last command above
            // the viewport at the absolute bottom; both must be reachable.
            let mut reached_command = tiles.iter().any(|t| t.text == "FINAL-COMMAND");
            for _ in 0..6 {
                ui.monitor.scroll_by(-1);
                ui.monitor
                    .drawer_scroll
                    .jump_to(ui.monitor.drawer_scroll.target, Instant::now());
                tiles.clear();
                hits.clear();
                draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
                reached_command |= tiles.iter().any(|t| t.text == "FINAL-COMMAND");
            }
            assert!(reached_command);
            assert!(hits
                .iter()
                .filter(|t| matches!(t.action, Action::Monitor(Command::StopPid(_))))
                .all(|t| t.y >= 282. && t.y + t.h <= h - 76.));
            ui.monitor.scroll_by(isize::MIN);
            assert_eq!(ui.monitor.drawer_scroll.target, 0.);
        }
    }

    #[test]
    fn closing_drawer_restores_background_grid_position() {
        let mut ui = long_process_ui();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
        ui.monitor.scroll_by(2);
        draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
        let page = ui.monitor.scroll;
        assert!(page > 0);
        ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
        draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
        ui.monitor.scroll_by(15);
        draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
        assert_eq!(ui.monitor.scroll, page);
        ui.monitor.handle(Command::Close, &ui.store).unwrap();
        draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
        assert_eq!(ui.monitor.scroll, page);
        assert!(ui.monitor.scrollbar.is_none());
    }

    #[test]
    fn scrollbar_drag_and_track_click_scroll_without_activating_content() {
        let mut ui = long_process_ui();
        ui.monitor.handle(Command::Npu(0), &ui.store).unwrap();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        let bar = ui.monitor.scrollbar.unwrap();
        assert!(ui.monitor.scrollbar_mouse(
            &MouseEventKind::Press(MousePress::Left),
            1413.,
            bar.top + 10.,
            true
        ));
        assert!(ui
            .monitor
            .scrollbar_mouse(&MouseEventKind::Move, 800., bar.bottom + 100., true));
        draw(&mut ui, &mut tiles, &mut hits, 176., 1417., 872., &None);
        assert_eq!(
            ui.monitor.drawer_scroll.position,
            ui.monitor.drawer_scroll.maximum
        );
        assert!(ui.monitor.scrollbar_mouse(
            &MouseEventKind::Release(MousePress::Left),
            800.,
            bar.bottom + 100.,
            false
        ));
        assert!(ui.monitor.scroll_drag.is_none());
        assert!(ui.monitor.scrollbar_mouse(
            &MouseEventKind::Press(MousePress::Left),
            1413.,
            (bar.top + bar.bottom) / 2.,
            true
        ));
        assert!(
            ui.monitor.drawer_scroll.target > 0.
                && ui.monitor.drawer_scroll.target < ui.monitor.drawer_scroll.maximum
        );
        ui.monitor
            .handle(Command::Filter(Some(0)), &ui.store)
            .unwrap();
        assert!(ui.monitor.scroll_drag.is_none());
        assert!(!ui
            .monitor
            .scrollbar_mouse(&MouseEventKind::Move, 1413., bar.bottom, true));
    }

    #[test]
    fn process_preview_matches_card_hierarchy_and_sums_only_hovered_npu() {
        let mut ui = preview_ui();
        let mut renders = vec![];
        for (w, h) in [(1058., 814.), (1403., 908.)] {
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(
                &mut ui,
                &mut tiles,
                &mut hits,
                176.,
                w,
                h,
                &Some(Action::Monitor(Command::Npu(0))),
            );
            let anchor = hits
                .iter()
                .find(|v| v.action == Action::Monitor(Command::Npu(0)))
                .unwrap();
            let start = tiles.len();
            draw_preview(&ui.monitor, 0, anchor, &mut tiles, w, h);
            let preview = &tiles[start..];
            let panel = preview.iter().find(|t| t.border_color.is_some()).unwrap();
            assert_eq!((panel.w, panel.h, panel.bg), (330., 182., CARD));
            assert!(preview.iter().all(|t| t.action.is_none() && t.zindex == 60));
            assert!(preview.iter().any(|t| t.text == "2 processes"
                && t.text_right_aligned
                && t.text_size == Some(11)));
            assert_eq!(
                preview
                    .iter()
                    .filter(|t| t.text == "python3" && t.text_bold)
                    .count(),
                2
            );
            assert!(preview
                .iter()
                .any(|t| t.text == "PID 42 · root" && t.fg == MUTED));
            assert!(preview
                .iter()
                .any(|t| t.text == "90.0 GB" && t.text_right_aligned));
            assert!(preview
                .iter()
                .any(|t| t.text == "10.0 GB" && t.text_right_aligned));
            assert_eq!(
                preview
                    .iter()
                    .filter(|t| t.h == 1. && t.bg == "#353648")
                    .count(),
                2
            );
            assert!(preview
                .iter()
                .filter(|t| !t.text.is_empty())
                .all(|t| t.x >= panel.x + 12.
                    && t.x + t.w <= panel.x + panel.w - 12.
                    && t.y + t.h <= panel.y + panel.h - 12.));
            renders.push(serde_json::json!({"width":w,"height":h,"screen":"process-preview","tiles":tiles.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"w":t.w,"h":t.h,"text":t.text,"size":t.text_size.unwrap_or(13),"bold":t.text_bold,"center":t.text_centered,"right":t.text_right_aligned,"fg":t.fg,"bg":t.bg,"transparent":t.transparent,"border":t.border_color,"icon":t.icon.map(|v|format!("{v:?}")),"radius":if t.rounded{t.corner_radius}else{0.},"padding":t.text_padding,"opacity":t.opacity,"z":t.zindex})).collect::<Vec<_>>()}));
        }
        if let Ok(path) = std::env::var("TERMVIAI_MONITOR_LAYOUT_OUT") {
            std::fs::write(
                format!("{path}.preview.json"),
                serde_json::to_vec_pretty(&renders).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn process_preview_distinguishes_empty_unavailable_and_paused() {
        let mut ui = preview_ui();
        let anchor = Hit {
            x: 200.,
            y: 260.,
            w: 400.,
            h: 110.,
            action: Action::Monitor(Command::Npu(0)),
        };
        for available in [true, false] {
            let s = ui.monitor.data.entries[0].snapshot.as_mut().unwrap();
            s.processes.clear();
            s.processes_available = available;
            let mut tiles = vec![];
            draw_preview(&ui.monitor, 0, &anchor, &mut tiles, 1058., 814.);
            let expected = if available {
                "No running NPU processes"
            } else {
                "Process information unavailable"
            };
            assert!(tiles.iter().any(|t| t.text == expected));
            assert_eq!(tiles.iter().any(|t| t.text == "0 processes"), available);
        }
        ui = preview_ui();
        ui.monitor.data.entries[0].paused = true;
        let mut tiles = vec![];
        draw_preview(&ui.monitor, 0, &anchor, &mut tiles, 1058., 814.);
        assert!(tiles.iter().any(|t| t.text.contains("Paused snapshot")));
        for dialog in [Dialog::Processes, Dialog::Stop, Dialog::Picker] {
            ui.monitor.dialog = Some(dialog);
            tiles.clear();
            draw_preview(&ui.monitor, 0, &anchor, &mut tiles, 1058., 814.);
            assert!(tiles.is_empty());
        }
    }

    #[test]
    fn process_preview_stays_in_window_and_reports_overflow() {
        let mut ui = preview_ui();
        let s = ui.monitor.data.entries[0].snapshot.as_mut().unwrap();
        let process = s.processes[0].clone();
        s.processes = (42..62)
            .map(|pid| Process {
                pid,
                ..process.clone()
            })
            .collect();
        for (w, h) in [(720., 480.), (1024., 768.), (1440., 900.)] {
            for anchor_y in [260., h - 120.] {
                let anchor = Hit {
                    x: w - 300.,
                    y: anchor_y,
                    w: 280.,
                    h: 110.,
                    action: Action::Monitor(Command::Npu(0)),
                };
                let mut tiles = vec![];
                draw_preview(&ui.monitor, 0, &anchor, &mut tiles, w, h);
                let panel = tiles.iter().find(|t| t.border_color.is_some()).unwrap();
                assert!(
                    panel.x >= 8.
                        && panel.y >= 60.
                        && panel.x + panel.w <= w - 8.
                        && panel.y + panel.h <= h - 8.
                );
                assert!(tiles
                    .iter()
                    .any(|t| t.text.starts_with("+ ") && t.text.contains("more")));
                assert!(tiles.iter().any(|t| t.text == "20 processes"));
                assert!(tiles
                    .iter()
                    .filter(|t| !t.text.is_empty())
                    .all(|t| t.y >= panel.y && t.y + t.h <= panel.y + panel.h));
            }
        }
    }

    #[test]
    fn device_cards_follow_responsive_design_and_right_align_readings() {
        for (w, cols) in [(1058., 2), (1403., 4)] {
            let mut ui = preview_ui();
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., w, 908., &None);
            assert_eq!(
                hits.iter()
                    .filter(|h| h.y == 260. && matches!(h.action, Action::Monitor(Command::Npu(_))))
                    .count(),
                cols
            );
            assert!(tiles
                .iter()
                .any(|t| t.text == "72%" && t.text_right_aligned));
            assert!(tiles
                .iter()
                .any(|t| t.text == "51.2" && t.text_right_aligned));
        }
    }

    #[test]
    fn process_details_show_working_directory_without_overlapping_command() {
        for cwd in [
            "/workspace".to_string(),
            String::new(),
            format!("/workspace/{}/train", "project/".repeat(18)),
        ] {
            let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
            ui.monitor.data.add(Host::default());
            ui.monitor.selected = Some(0);
            ui.monitor.dialog = Some(Dialog::Processes);
            let process = serde_json::json!({"npu":0,"die":0,"pid":42,"name":"python3","memory_mb":100,"token":"boot:123","command":"python3 train.py","cwd":cwd,"owner":"root","controllable":false});
            ui.monitor.data.entries[0].snapshot = Some(
                serde_json::from_value(serde_json::json!({
                    "dies":[{"npu":0,"die":0,"used_mb":100,"total_mb":65536,"core":2}],
                    "processes":[process],"processes_available":true,"stop_reason":""
                }))
                .unwrap(),
            );
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., 1024., 1000., &None);
            let directory = tiles
                .iter()
                .find(|t| t.text.starts_with("Working directory:"))
                .unwrap();
            let command = tiles.iter().find(|t| t.text == "python3 train.py").unwrap();
            assert_eq!(command.text_size, Some(13));
            assert_eq!(directory.text_size, Some(13));
            assert!(tiles
                .iter()
                .any(|t| t.text == "python3" && t.text_size == Some(13) && t.text_bold));
            assert!(command.y >= directory.y + directory.h);
            if cwd.is_empty() {
                assert!(directory.text.contains("Unavailable"));
            } else if cwd == "/workspace" {
                assert!(directory.text.contains(&cwd));
            } else {
                let path_lines: Vec<_> = tiles
                    .iter()
                    .filter(|t| !t.text.is_empty() && t.y >= directory.y && t.y < command.y)
                    .collect();
                assert!(path_lines.len() > 1);
                assert!(path_lines.iter().all(|t| t.y + t.h <= command.y));
                let joined = path_lines
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                assert!(joined.ends_with(&cwd));
            }
        }
    }
    #[test]
    fn npu_card_has_one_hit_and_no_die_actions() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.monitor.data.add(Host::default());
        ui.monitor.selected = Some(0);
        ui.monitor.data.entries[0].snapshot=Some(serde_json::from_str(r#"{"dies":[{"npu":4,"die":2,"used_mb":100,"total_mb":65536,"core":2},{"npu":4,"die":3,"used_mb":100,"total_mb":65536,"core":2}],"processes":[],"processes_available":true,"stop_reason":""}"#).unwrap());
        let mut t = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut t, &mut hits, 176., 1024., 768., &None);
        let card = hits
            .iter()
            .filter(|h| h.action == Action::Monitor(Command::Npu(4)))
            .collect::<Vec<_>>();
        assert_eq!(card.len(), 1);
        assert!(!hits
            .iter()
            .any(|h| matches!(h.action, Action::Monitor(Command::Filter(_)))));
    }
    #[test]
    fn removing_monitor_does_not_mutate_host_store() {
        let mut store = HostStore::default();
        store.hosts.push(Host::default());
        let mut m = MonitorUi::default();
        m.handle(Command::Add, &store).unwrap();
        m.handle(Command::Pick(0), &store).unwrap();
        assert_eq!(m.chosen.len(), 1);
        assert_eq!(store.hosts.len(), 1);
    }
    #[test]
    fn drawer_hits_are_contained_and_background_cannot_activate_cards() {
        for (w, h) in [(720., 480.), (1024., 768.), (1440., 900.)] {
            let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
            ui.monitor.data.add(Host::default());
            ui.monitor.selected = Some(0);
            ui.monitor.data.entries[0].snapshot=Some(serde_json::from_str(r#"{"dies":[{"npu":0,"die":0,"used_mb":100,"total_mb":65536,"core":2}],"processes":[],"processes_available":true,"stop_reason":""}"#).unwrap());
            for dialog in [
                Dialog::Picker,
                Dialog::Processes,
                Dialog::Remove,
                Dialog::Reason,
            ] {
                ui.monitor.dialog = Some(dialog);
                let mut tiles = vec![];
                let mut hits = vec![];
                draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
                assert!(hits
                    .iter()
                    .all(|a| a.x >= 0. && a.y >= 52. && a.x + a.w <= w && a.y + a.h <= h));
                assert!(!hits.iter().any(|a| matches!(
                    a.action,
                    Action::Monitor(Command::Npu(_) | Command::Host(_))
                )));
                assert_eq!(
                    hit_action(&hits, 100., 200.),
                    Some(Action::Monitor(Command::Close))
                );
            }
        }
    }
    #[test]
    fn approved_overview_and_failure_layout() {
        let mut renders = vec![];
        for (w, h) in [(720., 480.), (1024., 768.), (1488., 814.)] {
            let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
            for n in [209, 212, 216, 217] {
                ui.monitor.data.add(Host {
                    label: n.to_string(),
                    address: format!("192.168.25.{n}"),
                    username: "root".into(),
                    port: 22,
                    ..Default::default()
                });
            }
            ui.monitor.data.entries[2].error =
                "Remote collector failed (exit 1): test error".into();
            ui.monitor.data.entries[3].paused = true;
            for e in &mut ui.monitor.data.entries {
                e.snapshot=Some(serde_json::from_str(r#"{"dies":[{"npu":0,"die":0,"used_mb":52428,"total_mb":65536,"core":68},{"npu":0,"die":1,"used_mb":52428,"total_mb":65536,"core":68}],"processes":[],"processes_available":true,"stop_reason":""}"#).unwrap());
                e.updated = Some(std::time::Instant::now());
            }
            for selected in [None, Some(2)] {
                ui.monitor.selected = selected;
                let mut tiles = vec![];
                let mut hits = vec![];
                draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
                assert!(hits
                    .iter()
                    .all(|a| a.x >= 176. && a.x + a.w <= w && a.y >= 52. && a.y + a.h <= h));
                assert!(tiles.iter().any(|t| t.text_size == Some(24) && t.text_bold));
                if selected.is_none() {
                    assert!(tiles.iter().any(|t| t.icon == Some(Icon::Hosts)));
                    assert!(tiles.iter().any(|t| t.text == "1 paused"));
                    assert!(tiles.iter().any(|t| t.text == "1 unavailable"));
                    if let Some(child) = tiles
                        .iter()
                        .find(|t| t.action == Some(Action::Monitor(Command::Retry(2))))
                    {
                        assert!(child.transparent);
                        let parent = tiles
                            .iter()
                            .find(|t| t.action == Some(Action::Monitor(Command::Host(2))))
                            .unwrap();
                        let hit = hits
                            .iter()
                            .find(|t| t.action == Action::Monitor(Command::Retry(2)))
                            .unwrap();
                        assert!(parent.hovered_by(Some(hit)));
                    }
                } else {
                    assert!(tiles
                        .iter()
                        .any(|t| t.text == "No sample available" && t.text_centered));
                    assert!(hits
                        .iter()
                        .any(|a| a.action == Action::Monitor(Command::Reason)));
                    assert!(!hits
                        .iter()
                        .any(|a| a.action == Action::Monitor(Command::Pause(2))));
                    assert!(!hits
                        .iter()
                        .any(|a| a.action == Action::Monitor(Command::StopHost)));
                    assert!(!tiles
                        .iter()
                        .any(|t| t.text.contains("Remote collector failed")));
                }
                renders.push(serde_json::json!({"width":w,"height":h,"screen":if selected.is_none(){"overview"}else{"error"},"tiles":tiles.iter().map(|t|serde_json::json!({"x":t.x,"y":t.y,"w":t.w,"h":t.h,"text":t.text,"size":t.text_size.unwrap_or(13),"bold":t.text_bold,"center":t.text_centered,"fg":t.fg,"bg":t.bg,"transparent":t.transparent,"border":t.border_color,"icon":t.icon.map(|v|format!("{v:?}")),"radius":if t.rounded{t.corner_radius}else{0.},"padding":t.text_padding,"opacity":t.opacity,"z":t.zindex})).collect::<Vec<_>>()}));
            }
        }
        if let Ok(path) = std::env::var("TERMVIAI_MONITOR_LAYOUT_OUT") {
            std::fs::write(path, serde_json::to_vec_pretty(&renders).unwrap()).unwrap();
        }
    }
    #[test]
    fn reason_dialog_returns_to_overview_and_interval_is_a_choice() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.monitor.data.add(Host::default());
        ui.monitor.menu = Some(0);
        ui.monitor.handle(Command::Reason, &ui.store).unwrap();
        assert_eq!(ui.monitor.selected, Some(0));
        ui.monitor.handle(Command::Close, &ui.store).unwrap();
        assert_eq!(ui.monitor.selected, None);
        ui.monitor.handle(Command::Interval, &ui.store).unwrap();
        assert_eq!(ui.monitor.data.interval, 2);
        let mut t = vec![];
        let mut hits = vec![];
        draw(&mut ui, &mut t, &mut hits, 176., 1024., 768., &None);
        for v in [2, 5, 10] {
            assert!(hits
                .iter()
                .any(|h| h.action == Action::Monitor(Command::SetInterval(v))));
        }
    }
    #[test]
    fn termination_confirmation_keeps_scope_and_process_hierarchy() {
        for (w, h) in [(720., 480.), (1417., 872.)] {
            let mut ui = preview_ui();
            ui.monitor.data.entries[0].updated = Some(Instant::now());
            for p in &mut ui.monitor.data.entries[0]
                .snapshot
                .as_mut()
                .unwrap()
                .processes
            {
                p.controllable = true;
                p.cwd = "/workspace".into();
            }
            ui.monitor.die = Some(0);
            ui.monitor.handle(Command::StopPid(42), &ui.store).unwrap();
            assert_eq!(ui.monitor.targets.len(), 1);
            assert_eq!(ui.monitor.stop_dies, 3);
            // Refreshing the live sample must not change the frozen confirmation.
            ui.monitor.data.entries[0]
                .snapshot
                .as_mut()
                .unwrap()
                .processes
                .clear();
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., w, h, &None);
            assert!(tiles
                .iter()
                .any(|t| t.text == "Operation scope" && t.fg == MUTED));
            assert!(tiles
                .iter()
                .any(|t| t.text == "1 unique processes · 3 affected Dies"));
            let name = tiles.iter().find(|t| t.text == "python3").unwrap();
            let meta = tiles.iter().find(|t| t.text == "PID 42 · root").unwrap();
            assert!(name.text_bold && name.text_size == Some(13));
            assert!(meta.text_right_aligned && meta.fg == MUTED);
            assert_eq!(name.y, meta.y);
            assert!(name.x + name.w < meta.x);
            assert!(
                tiles
                    .iter()
                    .filter(|t| t.h == 1. && t.bg == "#353648")
                    .count()
                    >= 3
            );
            assert!(hits
                .iter()
                .any(|h| h.action == Action::Monitor(Command::AcceptStop)));
            assert!(ui.monitor.stop_rows[0].locations.contains("NPU 1"));
            assert_eq!(ui.monitor.stop_rows[0].cwd, "/workspace");
            let cwd = tiles
                .iter()
                .find(|t| t.text == "Working directory: /workspace")
                .unwrap();
            assert!(cwd.y > name.y && cwd.text_size == Some(13));
        }
    }

    #[test]
    fn termination_confirmation_wraps_working_directories_and_scrolls() {
        for command in [Command::StopHost, Command::StopScope] {
            let mut ui = preview_ui();
            let path = format!("/workspace/{}/train", "模型-project/".repeat(12));
            let entry = &mut ui.monitor.data.entries[0];
            entry.updated = Some(Instant::now());
            for p in &mut entry.snapshot.as_mut().unwrap().processes {
                p.controllable = true;
                p.cwd = if p.pid == 42 {
                    path.clone()
                } else {
                    String::new()
                };
            }
            ui.monitor.handle(command, &ui.store).unwrap();
            assert_eq!(ui.monitor.stop_rows.len(), 2);
            assert_eq!(ui.monitor.stop_rows[0].cwd, path);
            assert_eq!(ui.monitor.stop_rows[1].cwd, "Unavailable");
            entry_snapshot_clear(&mut ui);
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, 176., 720., 1600., &None);
            let start = tiles
                .iter()
                .position(|t| t.text.starts_with("Working directory:"))
                .unwrap();
            let lines: Vec<_> = tiles[start..]
                .iter()
                .take_while(|t| !t.text.is_empty())
                .collect();
            assert!(lines.len() > 1);
            assert_eq!(
                lines.iter().map(|t| t.text.as_str()).collect::<String>(),
                format!("Working directory: {}", path)
            );
            assert!(lines
                .windows(2)
                .all(|pair| pair[0].y + pair[0].h <= pair[1].y));
            assert!(tiles
                .iter()
                .any(|t| t.text == "Working directory: Unavailable"));
            tiles.clear();
            hits.clear();
            draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
            assert!(ui.monitor.drawer_scroll.maximum > 0.);
            ui.monitor
                .drawer_scroll
                .jump_to(ui.monitor.drawer_scroll.maximum, Instant::now());
            tiles.clear();
            hits.clear();
            draw(&mut ui, &mut tiles, &mut hits, 176., 720., 480., &None);
            assert!(tiles
                .iter()
                .any(|t| t.text == "Working directory: Unavailable"));
            assert!(hits
                .iter()
                .any(|h| h.action == Action::Monitor(Command::AcceptStop)));
        }

        fn entry_snapshot_clear(ui: &mut TermviaiUi) {
            ui.monitor.data.entries[0]
                .snapshot
                .as_mut()
                .unwrap()
                .processes
                .clear();
        }
    }

    #[test]
    fn monitor_typography_is_cached_and_tracks_dpi_not_terminal_zoom() {
        let fonts =
            wezterm_font::FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96)
                .unwrap();
        let body = fonts.ui_font(12, false).unwrap();
        let heading = fonts.ui_font(24, true).unwrap();
        if cfg!(windows) {
            // Check the resolved face, not just the requested family: a silent
            // fallback or synthetic bold would bring back the visual mismatch.
            for (font, weight) in [
                (&body, config::FontWeight::REGULAR),
                (&heading, config::FontWeight::DEMIBOLD),
            ] {
                let handles = font.clone_handles();
                assert_eq!(handles[0].names().family, "Segoe UI");
                assert_eq!(handles[0].weight(), weight);
                assert!(!handles[0].synthesize_bold);
            }
        }
        let height = |f: &wezterm_font::LoadedFont| {
            RenderMetrics::with_font_metrics(&f.metrics())
                .cell_size
                .height
        };
        assert!(height(&heading) > height(&body));
        assert!(std::rc::Rc::ptr_eq(
            &heading,
            &fonts.ui_font(24, true).unwrap()
        ));
        fonts.change_scaling(2., 96);
        assert_eq!(height(&heading), height(&fonts.ui_font(24, true).unwrap()));
        fonts.change_scaling(1., 192);
        let large = fonts.ui_font(24, true).unwrap();
        assert!(height(&large) > height(&heading));
        assert!(!std::rc::Rc::ptr_eq(&heading, &large));
    }
}
