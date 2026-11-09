//! Native Port Forwarding, matching the reviewed compact grid and responsive drawer.
use super::super::termviai_motion::SmoothScroll;
use super::*;
use crate::forwarding::{manager, Rule, State, View};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Command {
    Open,
    Add,
    FromTab(usize),
    Pick(usize),
    ChangeSource,
    Field(usize),
    Network,
    SetNetwork(bool),
    Retry,
    StartSaved,
    Save,
    Close,
    Noop,
    Details(String),
    Menu(String),
    Edit(String),
    Start(String),
    Stop(String),
    Delete(String),
    AcceptDelete,
    StartAll,
    StopAll,
    AcceptStopAll,
    Probe(String),
    Copy(String),
    Inspect(String),
    Force(String),
    AcceptForce,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Panel {
    Picker,
    Form,
    Details,
    Delete,
    StopAll,
    Force,
}
pub(super) struct Draft {
    rule: Rule,
    fields: [Input; 3],
    editing: bool,
    start: bool,
}
#[derive(Default)]
pub(super) struct ForwardUi {
    pub panel: Option<Panel>,
    pub selected: String,
    pub menu: Option<String>,
    pub sources: Vec<Host>,
    query: Input,
    filtered_query: String,
    draft: Option<Draft>,
    pub focus: Option<Command>,
    pub error: String,
    pub scroll: SmoothScroll,
    pub drawer_scroll: SmoothScroll,
    pub views: Vec<View>,
    pub store_error: String,
    scrollbar: Option<(f32, f32, f32, f32)>,
    grab: Option<f32>,
    network_menu: bool,
    reveal_focus: bool,
    confirmation: Option<(Rule, crate::forwarding::Owner)>,
}
impl ForwardUi {
    pub fn input(&mut self) -> Option<&mut Input> {
        if self.panel == Some(Panel::Picker) && self.focus == Some(Command::Field(3)) {
            return Some(&mut self.query);
        }
        if self.panel == Some(Panel::Form) {
            if let Some(Command::Field(i)) = self.focus {
                return self.draft.as_mut().and_then(|d| d.fields.get_mut(i));
            }
        }
        None
    }
    pub fn refresh(&mut self) {
        let mut m = manager().lock().unwrap();
        self.views = m.views();
        self.store_error = m.load_error.clone();
    }
    fn edit(&mut self, rule: Rule, editing: bool, start: bool) {
        self.draft = Some(Draft {
            fields: [
                Input::new(rule.name.clone()),
                Input::new(rule.remote_port.to_string()),
                Input::new(rule.local_port.to_string()),
            ],
            rule,
            editing,
            start,
        });
        self.panel = Some(Panel::Form);
        self.focus = Some(Command::Field(0));
        self.reveal_focus = true;
        self.drawer_scroll = SmoothScroll::default();
    }
    fn open_picker(&mut self) {
        self.panel = Some(Panel::Picker);
        self.query = Input::default();
        self.filtered_query.clear();
        self.focus = Some(Command::Field(3));
    }
    pub fn scroll_by(&mut self, pixels: f32) {
        let s = if self.panel.is_some() {
            &mut self.drawer_scroll
        } else {
            &mut self.scroll
        };
        s.move_by(pixels, Instant::now());
        self.menu = None;
    }
    pub fn handle(&mut self, c: Command, store: &HostStore) -> anyhow::Result<()> {
        self.error.clear();
        self.refresh();
        let reset_scroll = !matches!(
            c,
            Command::Field(_)
                | Command::Network
                | Command::SetNetwork(_)
                | Command::Retry
                | Command::StartSaved
        );
        match c {
            Command::Open => (),
            Command::Add => {
                self.draft = None;
                self.open_picker();
            }
            Command::ChangeSource => self.open_picker(),
            Command::Pick(i) => {
                let host = self
                    .sources
                    .get(i)
                    .context("Host is no longer available")?
                    .clone();
                if let Some(d) = &mut self.draft {
                    d.rule.host = host;
                    self.panel = Some(Panel::Form);
                    self.focus = Some(Command::Field(0));
                } else {
                    self.edit(Rule::new(host), false, true)
                }
            }
            Command::Field(i) => {
                self.focus = Some(Command::Field(i));
                self.reveal_focus = true;
            }
            Command::Network => self.network_menu = !self.network_menu,
            Command::SetNetwork(network) => {
                if let Some(d) = &mut self.draft {
                    d.rule.network = network;
                }
                self.network_menu = false;
            }
            Command::Retry => {
                if let Some(d) = &mut self.draft {
                    d.rule.retry = !d.rule.retry
                }
            }
            Command::StartSaved => {
                if let Some(d) = &mut self.draft {
                    d.start = !d.start
                }
            }
            Command::Save => {
                let d = self.draft.as_mut().context("No forwarding draft")?;
                d.rule.name = d.fields[0].value.clone();
                d.rule.remote_port = d.fields[1]
                    .value
                    .trim()
                    .parse()
                    .ok()
                    .filter(|p| *p > 0)
                    .context("Remote port must be 1–65535.")?;
                d.rule.local_port = d.fields[2]
                    .value
                    .trim()
                    .parse()
                    .ok()
                    .filter(|p| *p > 0)
                    .context("Local port must be 1–65535.")?;
                let mut m = manager().lock().unwrap();
                let id = m.save(d.rule.clone())?;
                self.selected = id.clone();
                if d.start {
                    if let Err(e) = m.start(&id, store) {
                        self.panel = Some(Panel::Details);
                        self.draft = None;
                        return Err(e.context("Rule saved, but could not start"));
                    }
                }
                self.panel = None;
                self.draft = None;
            }
            Command::Close => {
                self.panel = None;
                self.menu = None;
                self.focus = None;
                self.draft = None
            }
            Command::Details(id) => {
                self.selected = id;
                self.panel = Some(Panel::Details)
            }
            Command::Menu(id) => {
                self.menu = if self.menu.as_ref() == Some(&id) {
                    None
                } else {
                    Some(id)
                };
                return Ok(());
            }
            Command::Edit(id) => {
                let v = self
                    .views
                    .iter()
                    .find(|v| v.rule.id == id)
                    .context("Rule removed")?
                    .clone();
                self.selected = id;
                self.edit(v.rule, true, v.state.active());
            }
            Command::Start(id) => manager().lock().unwrap().start(&id, store)?,
            Command::Stop(id) => manager().lock().unwrap().stop(&id)?,
            Command::Delete(id) => {
                self.selected = id;
                self.panel = Some(Panel::Delete)
            }
            Command::AcceptDelete => {
                manager().lock().unwrap().delete(&self.selected)?;
                self.panel = None
            }
            Command::StartAll => {
                let mut m = manager().lock().unwrap();
                let mut errors = vec![];
                for v in &self.views {
                    if !v.state.active() && !matches!(v.state, State::Attention) {
                        if let Err(e) = m.start(&v.rule.id, store) {
                            errors.push(format!("{}: {e:#}", v.rule.name))
                        }
                    }
                }
                self.error = errors.join("\n");
            }
            Command::StopAll => self.panel = Some(Panel::StopAll),
            Command::AcceptStopAll => {
                let mut m = manager().lock().unwrap();
                let mut errors = vec![];
                for v in &self.views {
                    if let Err(error) = m.stop(&v.rule.id) {
                        errors.push(format!("{}: {error:#}", v.rule.name));
                    }
                }
                self.panel = None;
                self.error = errors.join("\n");
            }
            Command::Inspect(id) => manager().lock().unwrap().inspect(&id, store, None)?,
            Command::Force(id) => {
                let v = self
                    .views
                    .iter()
                    .find(|v| v.rule.id == id)
                    .context("Rule removed")?;
                let owner = v
                    .inspection
                    .as_ref()
                    .filter(|i| i.controllable)
                    .and_then(|i| i.owner.clone())
                    .context("Inspect the listener first")?;
                self.confirmation = Some((v.rule.clone(), owner));
                self.selected = id;
                self.panel = Some(Panel::Force);
            }
            Command::AcceptForce => {
                let (rule, owner) = self
                    .confirmation
                    .clone()
                    .context("Confirm a listener first")?;
                let mut m = manager().lock().unwrap();
                anyhow::ensure!(
                    m.views().iter().any(|v| v.rule == rule),
                    "Rule changed; inspect again"
                );
                m.inspect(&rule.id, store, Some(owner))?;
                self.panel = Some(Panel::Details);
                self.confirmation = None;
            }
            Command::Probe(id) => manager().lock().unwrap().probe(&id),
            Command::Noop | Command::FromTab(_) | Command::Copy(_) => (),
        }
        if reset_scroll {
            self.drawer_scroll = SmoothScroll::default();
        }
        self.menu = None;
        self.refresh();
        Ok(())
    }
}
fn text(
    t: &mut Vec<Tile>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    s: impl Into<String>,
    fg: &'static str,
    size: u16,
    bold: bool,
) {
    tile(t, x, y, w, h, s, CARD, fg, false);
    if let Some(v) = t.last_mut() {
        v.transparent = true;
        v.text_padding = 0.;
        v.text_size = Some(size);
        v.text_bold = bold;
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
    button(t, hits, x, y, w, h, s, CARD, Action::Forward(c), hover);
    let v = t.last_mut().unwrap();
    v.text_size = Some(12);
    v.border_color = Some("#353648");
    v.content_centered = true;
}
fn bare(
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
    ctl(t, hits, x, y, w, h, s, c, hover);
    let v = t.last_mut().unwrap();
    v.transparent = true;
    v.border_color = None;
    v.text_padding = 6.;
}
fn field(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    x: f32,
    y: f32,
    w: f32,
    input: &Input,
    i: usize,
    focus: &Option<Command>,
    hover: &Option<Action>,
) {
    ctl(
        t,
        hits,
        x,
        y,
        w,
        38.,
        &input.value,
        Command::Field(i),
        hover,
    );
    let el = t.last_mut().unwrap();
    el.bg = FIELD;
    el.content_centered = false;
    el.input = Some(InputPaint {
        masked: false,
        input: input.clone(),
        focused: *focus == Some(Command::Field(i)),
    });
}
fn auth_label(h: &Host) -> &'static str {
    if !h.identity_id.is_empty() {
        "Keychain Identity"
    } else if !h.identity_file.is_empty() || !h.key_label.is_empty() {
        "SSH key"
    } else {
        "Saved host credentials"
    }
}
fn status(v: &View) -> &'static str {
    match v.state {
        State::Running => "#59b79e",
        State::Attention | State::Failed => "#ed8193",
        _ => MUTED,
    }
}
fn wrap(
    t: &mut Vec<Tile>,
    x: f32,
    y: &mut f32,
    w: f32,
    s: &str,
    fg: &'static str,
    fonts: &wezterm_font::FontConfiguration,
    scale: f32,
) -> anyhow::Result<()> {
    let mut cache = TextWrapCache::default();
    for line in cache.wrap(fonts.ui_font(12, false)?.as_ref(), s, w, scale)? {
        text(t, x, *y, w, 22., line, fg, 12, false);
        *y += 22.;
    }
    Ok(())
}
fn grid(width: f32) -> (usize, f32) {
    let n = ((width + 10.) / 246.).floor().max(1.) as usize;
    (n, (width - 10. * n.saturating_sub(1) as f32) / n as f32)
}
fn clip_body(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    body: Vec<Tile>,
    bh: Vec<Hit>,
    scroll: &mut SmoothScroll,
    top: f32,
    bottom: f32,
    end: f32,
    z: i8,
    x: f32,
    width: f32,
    h: f32,
    bg: &'static str,
) {
    scroll.bounds((bottom - top).max(0.), (end - top).max(0.), Instant::now());
    let shift = scroll.position;
    for mut v in body {
        v.y -= shift;
        if v.y < bottom && v.y + v.h > top {
            if v.text.is_empty() && v.icon.is_none() && v.input.is_none() {
                let end = (v.y + v.h).min(h);
                v.y = v.y.max(52.);
                v.h = (end - v.y).max(0.);
            }
            v.zindex = z;
            t.push(v)
        }
    }
    for mut v in bh {
        v.y -= shift;
        if v.y >= top && v.y + v.h <= bottom {
            hits.push(v)
        }
    }
    tile(t, x, 52., width, top - 52., "", bg, INK, false);
    t.last_mut().unwrap().zindex = z + 1;
    tile(t, x, bottom, width, h - bottom, "", bg, INK, false);
    t.last_mut().unwrap().zindex = z + 1;
}
fn scroll_bar(m: &mut ForwardUi, t: &mut Vec<Tile>, w: f32, top: f32, bottom: f32, drawer: bool) {
    let s = if drawer { &m.drawer_scroll } else { &m.scroll };
    m.scrollbar = None;
    if s.maximum > 0. && bottom > top {
        let view = bottom - top;
        let thumb = (view * view / (view + s.maximum)).max(24.).min(view);
        m.scrollbar = Some((w - 14., top, bottom, thumb));
        tile(
            t,
            w - 7.,
            top + (view - thumb) * s.position / s.maximum,
            3.,
            thumb,
            "",
            MUTED,
            INK,
            true,
        );
        t.last_mut().unwrap().zindex = if drawer { 44 } else { 34 };
    }
}
impl ForwardUi {
    pub fn scrollbar_mouse(&mut self, kind: &MouseEventKind, x: f32, y: f32, left: bool) -> bool {
        let Some((bx, top, bottom, thumb)) = self.scrollbar else {
            self.grab = None;
            return false;
        };
        let s = if self.panel.is_some() {
            &mut self.drawer_scroll
        } else {
            &mut self.scroll
        };
        let travel = (bottom - top - thumb).max(1.);
        let pos = top + travel * s.position / s.maximum.max(1.);
        match kind {
            MouseEventKind::Press(MousePress::Left)
                if x >= bx && x <= bx + 14. && y >= top && y <= bottom =>
            {
                if y >= pos && y <= pos + thumb {
                    self.grab = Some(y - pos)
                } else {
                    s.move_by(
                        if y < pos {
                            -s.viewport * 0.9
                        } else {
                            s.viewport * 0.9
                        },
                        Instant::now(),
                    )
                }
                true
            }
            MouseEventKind::Move | MouseEventKind::Release(MousePress::Left)
                if self.grab.is_some() =>
            {
                let grab = self.grab.unwrap();
                s.jump_to((y - top - grab) / travel * s.maximum, Instant::now());
                if !left || matches!(kind, MouseEventKind::Release(_)) {
                    self.grab = None
                }
                true
            }
            _ => false,
        }
    }
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
    let m = &mut ui.forward;
    let x = left + 26.;
    let width = (w - x - 26.).max(1.);
    tile(t, left, 52., w - left, h - 52., "", BG, INK, false);
    let header_start = t.len();
    text(
        t,
        x,
        80.,
        (width - 160.).max(1.),
        34.,
        "Port Forwarding",
        INK,
        24,
        true,
    );
    text(
        t,
        x,
        120.,
        width,
        24.,
        "Remote access to local services · Reverse SSH",
        MUTED,
        12,
        false,
    );
    ctl(
        t,
        hits,
        w - 172.,
        87.,
        146.,
        36.,
        "Add forwarding",
        Command::Add,
        hover,
    );
    t.last_mut().unwrap().icon = Some(Icon::Plus);
    text(
        t,
        x,
        160.,
        (width - 206.).max(1.),
        30.,
        format!(
            "{} services   ·   {} established",
            m.views.len(),
            m.views.iter().filter(|v| v.state == State::Running).count()
        ),
        MUTED,
        12,
        false,
    );
    bare(
        t,
        hits,
        w - 220.,
        158.,
        94.,
        32.,
        "Start all",
        Command::StartAll,
        hover,
    );
    bare(
        t,
        hits,
        w - 122.,
        158.,
        96.,
        32.,
        "Stop all",
        Command::StopAll,
        hover,
    );
    let (cols, cw) = grid(width);
    let mut b = vec![];
    let mut bh = vec![];
    let top = 207.;
    for (i, v) in m.views.iter().enumerate() {
        let xx = x + (i % cols) as f32 * (cw + 10.);
        let yy = top + (i / cols) as f32 * 166.;
        let id = &v.rule.id;
        button(
            &mut b,
            &mut bh,
            xx,
            yy,
            cw,
            156.,
            "",
            CARD,
            Action::Forward(Command::Details(id.clone())),
            hover,
        );
        b.last_mut().unwrap().border_color = Some("#353648");
        text(
            &mut b,
            xx + 12.,
            yy + 12.,
            cw - 50.,
            22.,
            &v.rule.name,
            INK,
            14,
            true,
        );
        text(
            &mut b,
            xx + 12.,
            yy + 37.,
            cw - 24.,
            20.,
            format!("{} · {}", v.rule.host.label, v.rule.host.username),
            MUTED,
            12,
            false,
        );
        text(
            &mut b,
            xx + 12.,
            yy + 64.,
            cw - 24.,
            20.,
            if v.rule.network {
                "Remote network → This computer"
            } else {
                "Remote localhost → This computer"
            },
            MUTED,
            12,
            false,
        );
        text(
            &mut b,
            xx + 12.,
            yy + 86.,
            cw - 24.,
            22.,
            format!(
                ":{}  →  127.0.0.1:{}",
                v.rule.remote_port, v.rule.local_port
            ),
            INK,
            12,
            false,
        );
        text(
            &mut b,
            xx + 12.,
            yy + 121.,
            cw - 100.,
            24.,
            v.state.label(),
            status(v),
            12,
            false,
        );
        bare(
            &mut b,
            &mut bh,
            xx + cw - 84.,
            yy + 116.,
            72.,
            32.,
            if v.state.active() {
                "Stop"
            } else if v.state == State::Attention {
                "Details"
            } else {
                "Start"
            },
            if v.state.active() {
                Command::Stop(id.clone())
            } else if v.state == State::Attention {
                Command::Details(id.clone())
            } else {
                Command::Start(id.clone())
            },
            hover,
        );
        bare(
            &mut b,
            &mut bh,
            xx + cw - 36.,
            yy + 8.,
            28.,
            30.,
            "",
            Command::Menu(id.clone()),
            hover,
        );
        b.last_mut().unwrap().icon = Some(Icon::More);
    }
    let mut end = top + m.views.len().div_ceil(cols) as f32 * 166.;
    if m.views.is_empty() {
        text(
            &mut b,
            x,
            top + 38.,
            width,
            30.,
            "Bring a local service to your remote host",
            INK,
            17,
            true,
        );
        wrap(&mut b,x,&mut {top+80.},width,"Choose a saved SSH host to add a reverse tunnel. Its Host / Keychain authentication is reused; no open terminal is required.",MUTED,fonts,scale)?;
        end = top + 170.;
    }
    for msg in [&m.store_error, &m.error] {
        if !msg.is_empty() {
            wrap(&mut b, x, &mut end, width, msg, "#ed8193", fonts, scale)?;
        }
    }
    for v in &mut t[header_start..] {
        v.zindex = 33;
    }
    clip_body(
        t,
        hits,
        b,
        bh,
        &mut m.scroll,
        top,
        h - 28.,
        end,
        31,
        left,
        w - left,
        h,
        BG,
    );
    scroll_bar(m, t, w, top, h - 28., false);
    if let Some(id) = &m.menu {
        if let Some(anchor) = hits
            .iter()
            .find(|v| v.action == Action::Forward(Command::Menu(id.clone())))
            .cloned()
        {
            let mw = 184_f32.min(width);
            let mx = (anchor.x + anchor.w - mw).clamp(x, (w - mw - 12.).max(x));
            let my = (anchor.y + anchor.h + 4.).min(h - 126.).max(52.);
            hits.push(Hit {
                x: left,
                y: 52.,
                w: w - left,
                h: h - 52.,
                action: Action::Forward(Command::Noop),
            });
            let start = t.len();
            tile(t, mx, my, mw, 120., "", CARD, INK, true);
            t.last_mut().unwrap().border_color = Some("#353648");
            for (i, (label, c)) in vec![
                ("View details", Command::Details(id.clone())),
                ("Edit", Command::Edit(id.clone())),
                ("Delete", Command::Delete(id.clone())),
            ]
            .into_iter()
            .enumerate()
            {
                ctl(
                    t,
                    hits,
                    mx + 5.,
                    my + 5. + i as f32 * 36.,
                    mw - 10.,
                    34.,
                    label,
                    c,
                    hover,
                );
                let v = t.last_mut().unwrap();
                v.content_centered = false;
                v.border_color = None;
                if i == 2 {
                    v.fg = "#ed8193";
                }
            }
            for v in &mut t[start..] {
                v.zindex = 40;
            }
        }
    }
    if let Some(panel) = m.panel {
        drawer(
            m, &ui.store, t, hits, left, w, h, panel, hover, fonts, scale,
        )?;
    }
    Ok(())
}
fn drawer(
    m: &mut ForwardUi,
    store: &HostStore,
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    left: f32,
    w: f32,
    h: f32,
    panel: Panel,
    hover: &Option<Action>,
    fonts: &wezterm_font::FontConfiguration,
    scale: f32,
) -> anyhow::Result<()> {
    let width = if panel == Panel::Picker {
        560_f32
    } else {
        438_f32
    }
    .min(w - left);
    let x = w - width;
    let cx = x + 24.;
    let cw = width - 48.;
    let start = t.len();
    tile(t, left, 52., w - left, h - 52., "", "#080911", INK, false);
    t.last_mut().unwrap().opacity = 0.55;
    tile(t, x, 52., width, h - 52., "", SIDE, INK, false);
    hits.retain(|hit| hit.y < 52.);
    hits.push(Hit {
        x: left,
        y: 52.,
        w: w - left,
        h: h - 52.,
        action: Action::Forward(Command::Noop),
    });
    let title = match panel {
        Panel::Picker => "Choose SSH host",
        Panel::Form => {
            if m.draft.as_ref().is_some_and(|d| d.editing) {
                "Edit forwarding"
            } else {
                "Add port forwarding"
            }
        }
        Panel::Details => "Forwarding details",
        Panel::Delete => "Delete forwarding?",
        Panel::StopAll => "Stop all forwarding?",
        Panel::Force => "End occupying SSH session?",
    };
    text(t, cx, 76., cw - 36., 30., title, INK, 18, true);
    bare(t, hits, w - 54., 74., 30., 32., "", Command::Close, hover);
    t.last_mut().unwrap().icon = Some(Icon::Close);
    let footer = h - 64.;
    let top = if panel == Panel::Picker { 203. } else { 128. };
    let mut y = top;
    let mut b = vec![];
    let mut bh = vec![];
    let v = m.views.iter().find(|v| v.rule.id == m.selected).cloned();
    match panel {
        Panel::Picker => {
            text(
                t,
                cx,
                111.,
                cw,
                24.,
                "Choose a host · Existing authentication is reused",
                MUTED,
                12,
                false,
            );
            field(t, hits, cx, 149., cw, &m.query, 3, &m.focus, hover);
            if m.query.value.is_empty() {
                t.last_mut().unwrap().text = "Search hosts or groups…".into();
            }
            let query = m.query.value.trim().to_lowercase();
            if m.filtered_query != query {
                m.drawer_scroll = SmoothScroll::default();
                m.filtered_query = query.clone();
            }
            let candidates = source_candidates(&m.sources, store, &query);
            let mut group = None;
            for (i, path) in &candidates {
                let host = &m.sources[*i];
                if group.as_ref() != Some(path) {
                    text(
                        &mut b,
                        cx,
                        y,
                        cw,
                        24.,
                        if path.is_empty() { "Ungrouped" } else { path },
                        MUTED,
                        12,
                        false,
                    );
                    group = Some(path.clone());
                    y += 30.;
                }
                ctl(
                    &mut b,
                    &mut bh,
                    cx,
                    y,
                    cw,
                    58.,
                    "",
                    Command::Pick(*i),
                    hover,
                );
                text(&mut b, cx + 10., y + 14., 28., 28., "", ACCENT, 14, false);
                b.last_mut().unwrap().icon = Some(Icon::Hosts);
                text(
                    &mut b,
                    cx + 44.,
                    y + 6.,
                    cw - 56.,
                    24.,
                    &host.label,
                    INK,
                    14,
                    false,
                );
                text(
                    &mut b,
                    cx + 44.,
                    y + 30.,
                    cw - 56.,
                    22.,
                    host.endpoint(),
                    MUTED,
                    12,
                    false,
                );
                y += 66.;
            }
            if candidates.is_empty() {
                wrap(
                    &mut b,
                    cx,
                    &mut y,
                    cw,
                    if m.sources.is_empty() {
                        "No SSH hosts yet. Add a host in Hosts, then choose it here."
                    } else {
                        "No matching hosts. Try another name, address or group."
                    },
                    MUTED,
                    fonts,
                    scale,
                )?;
            }
        }
        Panel::Form => {
            if let Some(d) = &m.draft {
                text(&mut b, cx, y, cw, 22., "Name", MUTED, 12, false);
                y += 25.;
                field(&mut b, &mut bh, cx, y, cw, &d.fields[0], 0, &m.focus, hover);
                y += 54.;
                text(&mut b, cx, y, cw, 22., "SSH host", MUTED, 12, false);
                y += 26.;
                tile(&mut b, cx, y, cw, 80., "", CARD, INK, true);
                text(
                    &mut b,
                    cx + 12.,
                    y + 8.,
                    cw - 100.,
                    22.,
                    &d.rule.host.label,
                    INK,
                    14,
                    true,
                );
                text(
                    &mut b,
                    cx + 12.,
                    y + 31.,
                    cw - 24.,
                    22.,
                    format!("{}@{}", d.rule.host.username, d.rule.host.endpoint()),
                    MUTED,
                    12,
                    false,
                );
                text(
                    &mut b,
                    cx + 12.,
                    y + 54.,
                    cw - 24.,
                    20.,
                    auth_label(&d.rule.host),
                    MUTED,
                    12,
                    false,
                );
                if !d.editing {
                    bare(
                        &mut b,
                        &mut bh,
                        cx + cw - 84.,
                        y + 4.,
                        76.,
                        30.,
                        "Change",
                        Command::ChangeSource,
                        hover,
                    );
                }
                y += 96.;
                text(&mut b, cx, y, cw, 24., "Remote listener", INK, 12, true);
                y += 30.;
                let pw = 96.;
                let fw = (cw - pw - 10.).max(1.);
                let px = cx + fw + 10.;
                text(&mut b, cx, y, fw, 20., "Access", MUTED, 12, false);
                text(&mut b, px, y, pw, 20., "Port", MUTED, 12, false);
                y += 24.;
                ctl(
                    &mut b,
                    &mut bh,
                    cx,
                    y,
                    fw,
                    38.,
                    if d.rule.network {
                        "Remote network"
                    } else {
                        "Remote localhost"
                    },
                    Command::Network,
                    hover,
                );
                b.last_mut().unwrap().icon = Some(Icon::ChevronDown);
                field(&mut b, &mut bh, px, y, pw, &d.fields[1], 1, &m.focus, hover);
                y += 46.;
                if m.network_menu {
                    for (label, network) in [
                        ("Remote localhost · 127.0.0.1", false),
                        ("Remote network · 0.0.0.0", true),
                    ] {
                        ctl(
                            &mut b,
                            &mut bh,
                            cx,
                            y,
                            cw,
                            34.,
                            label,
                            Command::SetNetwork(network),
                            hover,
                        );
                        y += 36.;
                    }
                }
                wrap(
                    &mut b,
                    cx,
                    &mut y,
                    cw,
                    if d.rule.network {
                        "Requests all IPv4 interfaces. The SSH server's GatewayPorts policy and firewall determine network access."
                    } else {
                        "Only this remote host can access the listening port."
                    },
                    MUTED,
                    fonts,
                    scale,
                )?;
                y += 18.;
                text(&mut b, cx, y, cw, 24., "Local service", INK, 12, true);
                y += 30.;
                text(&mut b, cx, y, fw, 20., "Address", MUTED, 12, false);
                text(&mut b, px, y, pw, 20., "Port", MUTED, 12, false);
                y += 24.;
                tile(&mut b, cx, y, fw, 38., "127.0.0.1", FIELD, MUTED, true);
                b.last_mut().unwrap().border_color = Some("#353648");
                field(&mut b, &mut bh, px, y, pw, &d.fields[2], 2, &m.focus, hover);
                y += 54.;
                wrap(
                    &mut b,
                    cx,
                    &mut y,
                    cw,
                    &format!(
                        "{} · {}:{} → This computer 127.0.0.1:{}",
                        d.rule.host.label,
                        d.rule.bind(),
                        d.fields[1].value,
                        d.fields[2].value
                    ),
                    MUTED,
                    fonts,
                    scale,
                )?;
                y += 12.;
                for (label, checked, c) in [
                    ("Retry when disconnected", d.rule.retry, Command::Retry),
                    ("Start after saving", d.start, Command::StartSaved),
                ] {
                    if d.editing && c == Command::StartSaved {
                        continue;
                    }
                    ctl(&mut b, &mut bh, cx, y, cw, 34., label, c, hover);
                    let el = b.last_mut().unwrap();
                    el.icon = Some(if checked {
                        Icon::CheckboxChecked
                    } else {
                        Icon::Checkbox
                    });
                    el.border_color = None;
                    el.content_centered = false;
                    y += 38.;
                }
                y += 8.;
                wrap(&mut b,cx,&mut y,cw,"Closing the terminal keeps this tunnel running. Exiting TermViAI disconnects it; next launch restores running rules. Manually stopped rules stay stopped.",MUTED,fonts,scale)?;
                ctl(
                    t,
                    hits,
                    w - 152.,
                    footer + 14.,
                    128.,
                    36.,
                    if d.editing && d.start {
                        "Save & restart"
                    } else {
                        "Save"
                    },
                    Command::Save,
                    hover,
                );
            }
        }
        Panel::Details => {
            if let Some(v) = &v {
                text(&mut b, cx, y, cw, 28., &v.rule.name, INK, 17, true);
                y += 34.;
                text(
                    &mut b,
                    cx,
                    y,
                    cw,
                    24.,
                    v.state.label(),
                    status(v),
                    12,
                    false,
                );
                y += 34.;
                for (label, value) in [
                    (
                        "SSH host",
                        format!("{} · {}", v.rule.host.label, v.rule.host.endpoint()),
                    ),
                    (
                        "Remote listener",
                        format!("{}:{}", v.rule.bind(), v.rule.remote_port),
                    ),
                    ("Local service", format!("127.0.0.1:{}", v.rule.local_port)),
                    (
                        "Authentication",
                        if !v.rule.host.identity_id.is_empty() {
                            "Keychain Identity".into()
                        } else if !v.rule.host.identity_file.is_empty()
                            || !v.rule.host.key_label.is_empty()
                        {
                            "SSH key".into()
                        } else {
                            "Host credentials".into()
                        },
                    ),
                    (
                        "Automatic retry",
                        if v.rule.retry {
                            format!("Enabled · {} / 6", v.attempt)
                        } else {
                            "Off".into()
                        },
                    ),
                    (
                        "Local TCP check",
                        match v.service {
                            Some(true) => "Reachable",
                            Some(false) => "Not responding",
                            None => "Not checked",
                        }
                        .into(),
                    ),
                ] {
                    text(&mut b, cx, y, cw, 20., label, MUTED, 12, false);
                    y += 22.;
                    wrap(&mut b, cx, &mut y, cw, &value, INK, fonts, scale)?;
                    y += 14.;
                }
                ctl(
                    &mut b,
                    &mut bh,
                    cx,
                    y,
                    cw,
                    34.,
                    "Check local service",
                    Command::Probe(v.rule.id.clone()),
                    hover,
                );
                y += 42.;
                ctl(
                    &mut b,
                    &mut bh,
                    cx,
                    y,
                    cw,
                    34.,
                    "Copy remote access address",
                    Command::Copy(v.rule.id.clone()),
                    hover,
                );
                y += 44.;
                if v.busy {
                    text(&mut b, cx, y, cw, 24., "Checking…", MUTED, 12, false);
                    y += 32.;
                }
                if !v.state.active() && !v.busy {
                    ctl(
                        &mut b,
                        &mut bh,
                        cx,
                        y,
                        cw,
                        34.,
                        "Inspect remote listener",
                        Command::Inspect(v.rule.id.clone()),
                        hover,
                    );
                    y += 44.;
                }
                if let Some(info) = &v.inspection {
                    wrap(
                        &mut b,
                        cx,
                        &mut y,
                        cw,
                        if info.occupied {
                            "Remote port is occupied"
                        } else {
                            "No remote listener found"
                        },
                        INK,
                        fonts,
                        scale,
                    )?;
                    if let Some(owner) = &info.owner {
                        wrap(
                            &mut b,
                            cx,
                            &mut y,
                            cw,
                            &format!("PID {} · {}", owner.pid, owner.command),
                            MUTED,
                            fonts,
                            scale,
                        )?;
                    }
                    if !info.reason.is_empty() {
                        wrap(&mut b, cx, &mut y, cw, &info.reason, MUTED, fonts, scale)?;
                    }
                    if info.controllable && !v.busy {
                        y += 12.;
                        ctl(
                            &mut b,
                            &mut bh,
                            cx,
                            y,
                            cw,
                            34.,
                            "End occupying SSH session…",
                            Command::Force(v.rule.id.clone()),
                            hover,
                        );
                        b.last_mut().unwrap().fg = "#ed8193";
                        y += 44.;
                    }
                }
                if !v.error.is_empty() {
                    wrap(&mut b, cx, &mut y, cw, &v.error, "#ed8193", fonts, scale)?;
                    y += 12.;
                }
                wrap(&mut b,cx,&mut y,cw,"Tunnel established confirms the SSH listener, not business health. A refused listener may mean an occupied port or server policy; edit the port or check the remote host.",MUTED,fonts,scale)?;
                ctl(
                    t,
                    hits,
                    cx,
                    footer + 14.,
                    80.,
                    36.,
                    "Edit",
                    Command::Edit(v.rule.id.clone()),
                    hover,
                );
                ctl(
                    t,
                    hits,
                    w - 124.,
                    footer + 14.,
                    100.,
                    36.,
                    if v.state.active() { "Stop" } else { "Start" },
                    if v.state.active() {
                        Command::Stop(v.rule.id.clone())
                    } else {
                        Command::Start(v.rule.id.clone())
                    },
                    hover,
                );
            }
        }
        Panel::Force => {
            if let Some((rule, owner)) = &m.confirmation {
                wrap(
                    &mut b,
                    cx,
                    &mut y,
                    cw,
                    &format!(
                        "{} · {}:{}\nSSH session PID {}\n{}",
                        rule.host.label,
                        rule.bind(),
                        rule.remote_port,
                        owner.pid,
                        owner.command
                    ),
                    INK,
                    fonts,
                    scale,
                )?;
                y += 18.;
                wrap(&mut b,cx,&mut y,cw,"This session may belong to another application. Ending it also interrupts its other commands and tunnels. The process identity and port owner are rechecked before sending SIGTERM. A new tunnel starts only after the port is released.",MUTED,fonts,scale)?;
                ctl(
                    t,
                    hits,
                    w - 156.,
                    footer + 14.,
                    132.,
                    36.,
                    "End & retry",
                    Command::AcceptForce,
                    hover,
                );
                t.last_mut().unwrap().fg = "#ed8193";
            }
        }
        Panel::Delete => {
            if let Some(v) = v {
                wrap(&mut b, cx, &mut y, cw, &v.rule.name, INK, fonts, scale)?;
                y += 15.;
                wrap(&mut b,cx,&mut y,cw,"Deleting stops this tunnel and its active TCP connections. The SSH terminal and saved host stay unchanged.",MUTED,fonts,scale)?;
                ctl(
                    t,
                    hits,
                    w - 132.,
                    footer + 14.,
                    108.,
                    36.,
                    "Delete",
                    Command::AcceptDelete,
                    hover,
                );
                t.last_mut().unwrap().fg = "#ed8193";
            }
        }
        Panel::StopAll => {
            wrap(&mut b,cx,&mut y,cw,"All tunnels and their active TCP connections will stop. Saved rules and SSH terminals stay unchanged.",MUTED,fonts,scale)?;
            ctl(
                t,
                hits,
                w - 132.,
                footer + 14.,
                108.,
                36.,
                "Stop all",
                Command::AcceptStopAll,
                hover,
            );
        }
    }
    if !m.error.is_empty() {
        y += 14.;
        wrap(&mut b, cx, &mut y, cw, &m.error, "#ed8193", fonts, scale)?;
    }
    let end = y + 10.;
    if m.reveal_focus {
        m.drawer_scroll
            .bounds((footer - 12. - top).max(0.), end - top, Instant::now());
        if let Some(focus) = &m.focus {
            if let Some(hit) = bh
                .iter()
                .find(|v| v.action == Action::Forward(focus.clone()))
            {
                let pos = m.drawer_scroll.position;
                let next = if hit.y - pos < top {
                    hit.y - top
                } else if hit.y + hit.h - pos > footer - 12. {
                    hit.y + hit.h - (footer - 12.)
                } else {
                    pos
                };
                m.drawer_scroll.jump_to(next, Instant::now());
            }
        }
        m.reveal_focus = false;
    }
    for v in &mut b {
        if v.action.as_ref()
            == m.focus
                .as_ref()
                .map(|c| Action::Forward(c.clone()))
                .as_ref()
        {
            v.border_color = Some(ACCENT);
        }
    }
    clip_body(
        t,
        hits,
        b,
        bh,
        &mut m.drawer_scroll,
        top,
        footer - 12.,
        end,
        42,
        x,
        width,
        h,
        SIDE,
    );
    scroll_bar(m, t, w, top, footer - 12., true);
    tile(t, cx, footer, cw, 1., "", "#353648", INK, false);
    if panel != Panel::Details {
        ctl(
            t,
            hits,
            w - match panel {
                Panel::Picker => 110.,
                Panel::Form => 250.,
                Panel::Force => 254.,
                _ => 230.,
            },
            footer + 14.,
            86.,
            36.,
            "Cancel",
            Command::Close,
            hover,
        );
    }
    for v in &mut t[start..] {
        if !matches!(v.zindex, 42 | 43 | 44) {
            v.zindex = if v.transparent || v.action.is_some() {
                44
            } else {
                41
            };
        }
    }
    Ok(())
}

// Saved metadata wins over an older snapshot from an open terminal. Keep
// distinct authentication profiles for the same endpoint available separately.
fn same_source(a: &Host, b: &Host) -> bool {
    a.label == b.label
        && a.address == b.address
        && a.port == b.port
        && a.username == b.username
        && a.authentication == b.authentication
        && a.identity_id == b.identity_id
        && a.key_label == b.key_label
        && a.identity_file == b.identity_file
        && a.credential_id == b.credential_id
}
fn add_open_source(result: &mut Vec<Host>, host: &Host) {
    if let Some(saved) = result.iter_mut().find(|saved| same_source(saved, host)) {
        // Keep an in-memory password available for hosts whose password was
        // entered interactively but not saved to the credential store.
        if saved.credential_id.is_empty() && saved.session_credential.is_empty() {
            saved.session_credential = host.session_credential.clone();
        }
    } else {
        result.push(host.clone());
    }
}
fn source_candidates(sources: &[Host], store: &HostStore, query: &str) -> Vec<(usize, String)> {
    let mut candidates: Vec<_> = sources
        .iter()
        .enumerate()
        .filter_map(|(i, host)| {
            let path = store.group_path(store.host_group_id(host));
            format!("{} {} {} {}", host.label, host.address, host.username, path)
                .to_lowercase()
                .contains(query)
                .then_some((i, path))
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| sources[a.0].label.cmp(&sources[b.0].label))
    });
    candidates
}

impl TermWindow {
    pub(super) fn forwarding_sources(&self) -> Vec<Host> {
        let mux = Mux::get();
        let mut result = self.termviai_ui.store.hosts.clone();
        if let Some(window) = mux.get_window(self.mux_window_id) {
            for tab in window.iter_tabs() {
                let mut panes = tab.iter_panes_ignoring_zoom();
                panes.sort_by_key(|p| (p.top, p.left));
                for p in panes {
                    if let Some(host) = self.termviai_ui.workspace.hosts.get(&p.pane.domain_id()) {
                        if !p.pane.is_dead() {
                            add_open_source(&mut result, host);
                        }
                    }
                }
            }
        }
        result
    }
    pub(super) fn forwarding_action(&mut self, c: Command) -> anyhow::Result<()> {
        if matches!(
            c,
            Command::Open | Command::Add | Command::ChangeSource | Command::FromTab(_)
        ) {
            self.termviai_ui.reload();
            self.termviai_ui.forward.sources = self.forwarding_sources();
        }
        if let Command::Copy(id) = &c {
            if let Some(v) = self
                .termviai_ui
                .forward
                .views
                .iter()
                .find(|v| &v.rule.id == id)
            {
                if let Some(w) = &self.window {
                    w.set_clipboard(Clipboard::Clipboard, v.rule.access_address());
                }
            }
            return Ok(());
        }
        if let Command::FromTab(id) = c {
            self.termviai_ui.discard_form();
            self.termviai_ui.tab_menu = None;
            self.termviai_ui.page = Page::Forwarding;
            self.termviai_ui.search_focus = false;
            let host = Mux::get()
                .get_tab(id)
                .and_then(|t| t.get_active_pane())
                .filter(|p| !p.is_dead())
                .and_then(|p| {
                    self.termviai_ui
                        .workspace
                        .hosts
                        .get(&p.domain_id())
                        .cloned()
                })
                .context("Select a TermViAI SSH terminal first")?;
            self.termviai_ui.forward.edit(Rule::new(host), false, true);
            return Ok(());
        }
        if matches!(c, Command::Open | Command::Add) {
            self.termviai_ui.discard_form();
            self.termviai_ui.page = Page::Forwarding;
            self.termviai_ui.search_focus = false;
        }
        let ui = &mut self.termviai_ui;
        ui.forward.handle(c, &ui.store)
    }
    pub(super) fn forwarding_key(&mut self, event: &KeyEvent, context: &dyn WindowOps) -> bool {
        if !event.key_is_down {
            return true;
        }
        let mut action = None;
        match event.key {
            KeyCode::Char('\u{1b}') => action = Some(Command::Close),
            KeyCode::Char('\t') => {
                if self.termviai_ui.forward.panel == Some(Panel::Form) {
                    let m = &mut self.termviai_ui.forward;
                    let mut choices = vec![
                        Command::Field(0),
                        Command::Network,
                        Command::Field(1),
                        Command::Field(2),
                        Command::Retry,
                    ];
                    if m.draft.as_ref().is_some_and(|d| !d.editing) {
                        choices.push(Command::StartSaved)
                    }
                    choices.extend([Command::Close, Command::Save]);
                    let current = m
                        .focus
                        .as_ref()
                        .and_then(|c| choices.iter().position(|v| v == c))
                        .unwrap_or(0);
                    let next = if event.modifiers.contains(Modifiers::SHIFT) {
                        (current + choices.len() - 1) % choices.len()
                    } else {
                        (current + 1) % choices.len()
                    };
                    m.focus = Some(choices[next].clone());
                    m.reveal_focus = true;
                    context.invalidate();
                    return true;
                }

                let mut seen = HashSet::new();
                let choices: Vec<_> = self
                    .termviai_ui
                    .hits
                    .iter()
                    .filter_map(|h| {
                        if let Action::Forward(c) = &h.action {
                            (!matches!(c, Command::Noop | Command::Open)).then_some(c.clone())
                        } else {
                            None
                        }
                    })
                    .filter(|c| seen.insert(c.clone()))
                    .collect();
                if !choices.is_empty() {
                    let old = self
                        .termviai_ui
                        .forward
                        .focus
                        .as_ref()
                        .and_then(|c| choices.iter().position(|v| v == c));
                    let n = if event.modifiers.contains(Modifiers::SHIFT) {
                        old.map(|i| (i + choices.len() - 1) % choices.len())
                            .unwrap_or(choices.len() - 1)
                    } else {
                        old.map(|i| (i + 1) % choices.len()).unwrap_or(0)
                    };
                    self.termviai_ui.forward.focus = Some(choices[n].clone());
                }
            }
            KeyCode::Char('\r') => {
                action = if self.termviai_ui.forward.panel == Some(Panel::Picker)
                    && self.termviai_ui.forward.input().is_some()
                {
                    let m = &self.termviai_ui.forward;
                    source_candidates(
                        &m.sources,
                        &self.termviai_ui.store,
                        &m.query.value.trim().to_lowercase(),
                    )
                    .first()
                    .map(|(i, _)| Command::Pick(*i))
                } else if self.termviai_ui.forward.input().is_some() {
                    Some(Command::Save)
                } else {
                    self.termviai_ui.forward.focus.clone()
                }
            }
            KeyCode::Char(' ') if self.termviai_ui.forward.input().is_none() => {
                action = self.termviai_ui.forward.focus.clone()
            }
            KeyCode::PageDown => self.termviai_ui.forward.scroll_by(240.),
            KeyCode::PageUp => self.termviai_ui.forward.scroll_by(-240.),
            _ => return self.termviai_ui.forward.input().is_none(),
        }
        if let Some(c) = action {
            if let Err(e) = self.forwarding_action(c) {
                self.termviai_ui.forward.error = format!("{e:#}")
            }
        }
        context.invalidate();
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> TermviaiUi {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.page = Page::Forwarding;
        for i in 0..12 {
            let mut rule = Rule::new(Host {
                label: format!("SSH {i}"),
                address: "127.0.0.1".into(),
                username: "fixture".into(),
                port: 22,
                ..Host::default()
            });
            rule.name = format!("Service {i}");
            ui.forward.sources.push(rule.host.clone());
            ui.forward.views.push(View {
                rule,
                state: State::Stopped,
                error: String::new(),
                attempt: 0,
                service: None,
                inspection: None,
                busy: false,
            });
        }
        ui.forward.selected = ui.forward.views[0].rule.id.clone();
        ui
    }
    #[test]
    fn saved_host_picker_filters_group_paths_without_terminal_sessions() {
        let mut ui = fixture();
        ui.store.groups = vec![
            crate::hosts::HostGroup {
                id: "parent".into(),
                name: "Production".into(),
                parent_id: String::new(),
            },
            crate::hosts::HostGroup {
                id: "child".into(),
                name: "NPU".into(),
                parent_id: "parent".into(),
            },
        ];
        ui.store.hosts = ui.forward.sources.clone();
        ui.store.hosts[7].group_id = "child".into();
        ui.forward.sources = ui.store.hosts.clone();
        assert_eq!(
            source_candidates(&ui.forward.sources, &ui.store, "").len(),
            12
        );
        assert_eq!(
            source_candidates(&ui.forward.sources, &ui.store, "production"),
            vec![(7, "Production / NPU".into())]
        );
        assert!(source_candidates(&ui.forward.sources, &ui.store, "missing").is_empty());
        ui.forward.open_picker();
        ui.forward.input().unwrap().value = "SSH 7".into();
        let fonts =
            wezterm_font::FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96)
                .unwrap();
        let mut tiles = vec![];
        let mut hits = vec![];
        draw(
            &mut ui, &mut tiles, &mut hits, 176., 1024., 640., &None, &fonts, 1.,
        )
        .unwrap();
        let picks: Vec<_> = hits
            .iter()
            .filter_map(|h| match &h.action {
                Action::Forward(Command::Pick(i)) => Some(*i),
                _ => None,
            })
            .collect();
        assert_eq!(picks, vec![7]);
        assert!(hits
            .iter()
            .any(|h| h.action == Action::Forward(Command::Field(3))));
        assert!(tiles.iter().any(|t| t.text == "Production / NPU"));
    }
    #[test]
    fn saved_sources_keep_live_credentials_without_duplicate_rows() {
        let saved = Host {
            label: "209".into(),
            address: "192.0.2.9".into(),
            port: 22,
            ..Host::default()
        };
        let mut open = saved.clone();
        open.session_credential = "memory-only".into();
        open.group = "old group".into();
        let mut sources = vec![saved];
        add_open_source(&mut sources, &open);
        add_open_source(&mut sources, &open);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].session_credential, "memory-only");
        assert!(sources[0].group.is_empty());
        open.username = "another-user".into();
        add_open_source(&mut sources, &open);
        assert_eq!(sources.len(), 2);
    }
    #[test]
    fn responsive_cards_keep_readable_minimum() {
        for (w, n) in [(804., 3), (574., 2), (406., 1), (286., 1)] {
            let (cols, cw) = grid(w);
            assert_eq!(cols, n);
            assert!(cw >= 236.);
            assert!((cw * cols as f32 + 10. * cols.saturating_sub(1) as f32 - w).abs() < 0.01);
        }
    }
    #[test]
    fn drawers_preserve_titlebar_and_keep_footer_and_hits_inside_small_windows() {
        let fonts =
            wezterm_font::FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96)
                .unwrap();
        for (w, h) in [(720., 480.), (1024., 640.), (1417., 872.)] {
            for panel in [
                Panel::Picker,
                Panel::Form,
                Panel::Details,
                Panel::Delete,
                Panel::StopAll,
                Panel::Force,
            ] {
                let mut ui = fixture();
                let rule = ui.forward.views[0].rule.clone();
                ui.forward.edit(rule.clone(), false, true);
                ui.forward.panel = Some(panel);
                ui.forward.confirmation = Some((
                    rule,
                    crate::forwarding::Owner {
                        pid: 42,
                        token: "boot:123".into(),
                        command: "sshd: fixture@notty".into(),
                        eligible: true,
                    },
                ));
                let mut tiles = vec![];
                let mut hits = vec![
                    Hit {
                        x: 200.,
                        y: 0.,
                        w: 100.,
                        h: 52.,
                        action: Action::Drag,
                    },
                    Hit {
                        x: 200.,
                        y: 260.,
                        w: 100.,
                        h: 100.,
                        action: Action::Connect(0),
                    },
                ];
                draw(
                    &mut ui, &mut tiles, &mut hits, 176., w, h, &None, &fonts, 1.,
                )
                .unwrap();
                assert!(hits.iter().any(|hit| hit.action == Action::Drag));
                assert!(!hits.iter().any(|hit| hit.action == Action::Connect(0)));
                assert!(hits.iter().all(|hit| hit.x >= 0.
                    && hit.x + hit.w <= w
                    && hit.y >= 0.
                    && hit.y + hit.h <= h));
                let footer: Vec<_> = hits.iter().filter(|hit| hit.y >= h - 64.).collect();
                for (i, a) in footer.iter().enumerate() {
                    for b in &footer[i + 1..] {
                        assert!(
                            a.x + a.w <= b.x || b.x + b.w <= a.x,
                            "footer buttons overlap"
                        );
                    }
                }
                if panel == Panel::Form {
                    ui.forward.focus = Some(Command::Field(2));
                    ui.forward.reveal_focus = true;
                    tiles.clear();
                    hits.clear();
                    draw(
                        &mut ui, &mut tiles, &mut hits, 176., w, h, &None, &fonts, 1.,
                    )
                    .unwrap();
                    assert!(hits
                        .iter()
                        .any(|hit| hit.action == Action::Forward(Command::Field(2))));
                    assert!(hits
                        .iter()
                        .any(|hit| hit.action == Action::Forward(Command::Save)));
                }
            }
        }
    }
    #[test]
    fn scrolling_preserves_partial_cards_but_never_their_offscreen_hitboxes() {
        let fonts =
            wezterm_font::FontConfiguration::new(Some(config::ConfigHandle::default_config()), 96)
                .unwrap();
        let mut ui = fixture();
        let (mut tiles, mut hits) = (vec![], vec![]);
        draw(
            &mut ui, &mut tiles, &mut hits, 176., 1024., 480., &None, &fonts, 1.,
        )
        .unwrap();
        ui.forward.scroll.jump_to(80., Instant::now());
        tiles.clear();
        hits.clear();
        draw(
            &mut ui, &mut tiles, &mut hits, 176., 1024., 480., &None, &fonts, 1.,
        )
        .unwrap();
        assert!(tiles
            .iter()
            .any(|v| v.zindex == 31 && v.y < 207. && v.y + v.h > 207.));
        assert!(tiles.iter().any(|v| v.zindex == 32 && v.y + v.h == 207.));
        assert!(hits
            .iter()
            .filter(|hit| matches!(
                hit.action,
                Action::Forward(Command::Details(_) | Command::Menu(_) | Command::Start(_))
            ))
            .all(|hit| hit.y >= 207. && hit.y + hit.h <= 452.));
    }
}
