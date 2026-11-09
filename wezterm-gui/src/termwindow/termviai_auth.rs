//! Authentication recovery lives inside its owning terminal, never in Edit Host.
use super::super::termviai_dock::Rect;
use super::super::termviai_motion::SmoothScroll;
use super::*;
use mux::ssh::authentication::{Command as Request, Control, Phase, View};
use mux::ssh::RemoteSshDomain;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Command {
    Noop,
    Field(usize),
    Method(Authentication),
    Source(bool),
    Picker,
    Pick(usize),
    Reveal,
    Logs,
    Continue,
    Close,
    Trust,
    SaveAgain,
}
#[derive(Default)]
pub(super) struct AuthUi {
    pub panes: HashMap<usize, PaneAuth>,
    pub active: Option<usize>,
}
pub(super) struct PaneAuth {
    control: Arc<Control>,
    view: View,
    original: Host,
    saved: bool,
    settings_changed: bool,
    mode: Authentication,
    keychain: bool,
    key: String,
    identity: String,
    fields: Vec<Input>,
    focus: Command,
    picker: bool,
    reveal: bool,
    logs: bool,
    scroll: SmoothScroll,
    pub bounds: Rect,
    error: String,
    dismissed: bool,
    pending: Option<Pending>,
    notice: String,
    notice_until: Option<Instant>,
    challenge_revision: u64,
    tab_order: Vec<Command>,
    reveal_focus: bool,
}
struct Pending {
    host: Host,
    mode: Authentication,
    secret: String,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if !self.secret.is_empty() {
            let _ = wezterm_ssh::credentials::delete(&self.secret);
        }
    }
}
impl PaneAuth {
    fn new(control: Arc<Control>, host: Host, saved: bool) -> Self {
        let view = control.view();
        Self {
            control,
            view,
            fields: vec![
                Input::new(host.username.clone()),
                Input::default(),
                Input::new(host.identity_file.clone()),
            ],
            key: host.key_label.clone(),
            identity: host.identity_id.clone(),
            original: host,
            saved,
            settings_changed: false,
            mode: Authentication::Password,
            keychain: true,
            focus: Command::Field(1),
            picker: false,
            reveal: false,
            logs: false,
            scroll: SmoothScroll::default(),
            bounds: Rect {
                x: 0.,
                y: 0.,
                w: 0.,
                h: 0.,
            },
            error: String::new(),
            dismissed: false,
            pending: None,
            notice: String::new(),
            notice_until: None,
            challenge_revision: 0,
            tab_order: vec![],
            reveal_focus: false,
        }
    }
    fn visible(&self) -> bool {
        !self.dismissed && !matches!(self.view.phase, Phase::Connected | Phase::Closed)
    }
    fn editable(&self) -> bool {
        matches!(self.view.phase, Phase::Waiting | Phase::Challenge)
    }
    fn input(&mut self) -> Option<&mut Input> {
        if !self.visible() || !self.editable() || self.picker {
            return None;
        }
        if let Command::Field(i) = self.focus {
            self.fields.get_mut(i)
        } else {
            None
        }
    }
    fn masked(&self) -> bool {
        if self.view.phase == Phase::Challenge {
            if let Command::Field(i) = self.focus {
                return self.view.prompts.get(i).is_some_and(|(_, echo)| !echo);
            }
        }
        self.focus == Command::Field(1) && !self.reveal
    }
    fn reset_fields(&mut self) {
        self.fields = vec![
            Input::new(
                if self.mode == Authentication::Key && !self.original.key_username.is_empty() {
                    self.original.key_username.clone()
                } else {
                    self.original.username.clone()
                },
            ),
            Input::default(),
            Input::new(self.original.identity_file.clone()),
        ];
    }
}
impl AuthUi {
    pub fn input(&mut self) -> Option<&mut Input> {
        self.panes.get_mut(&self.active?)?.input()
    }
    pub fn active(&self) -> bool {
        self.active
            .and_then(|id| self.panes.get(&id))
            .is_some_and(PaneAuth::visible)
    }
    pub fn masked(&self) -> bool {
        self.active
            .and_then(|id| self.panes.get(&id))
            .is_some_and(PaneAuth::masked)
    }
    pub fn scroll_at(&mut self, x: f32, y: f32, amount: f32) -> bool {
        for p in self.panes.values_mut() {
            let r = p.bounds;
            if p.visible() && x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h {
                p.scroll.move_by(amount, Instant::now());
                return true;
            }
        }
        false
    }
}

// Passwords are staged only in memory until the server accepts them. The saved
// Host is compared with the original snapshot before committing any change.
fn save_success(p: &mut PaneAuth, store: &mut HostStore) -> anyhow::Result<()> {
    save_success_at(p, store, &HostStore::path())
}
fn save_success_at(
    p: &mut PaneAuth,
    store: &mut HostStore,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        p.view.authenticated,
        "Authentication has not succeeded; Host settings were not changed"
    );
    let Some(pending) = p.pending.as_ref() else {
        return Ok(());
    };
    let mut fresh = HostStore::load_from(path)?;
    let mut host = p.original.clone();
    let mut selected = pending.host.clone();
    let mut created = None;
    let index = if p.saved {
        Some(fresh.hosts.iter().position(|h|h==&p.original).context("Connected, but this Host changed or was deleted. Its saved settings were not overwritten.")?)
    } else {
        None
    };
    if !pending.secret.is_empty() {
        let password = wezterm_ssh::credentials::load(&pending.secret)?
            .context("The temporary password expired")?;
        let id = new_credential_id();
        // Even unsaved/quick-connect panes receive a durable opaque reference;
        // saving their workspace must never persist a dead session password.
        wezterm_ssh::credentials::save(&id, &password, true)?;
        selected.credential_id = id.clone();
        selected.session_credential.clear();
        created = Some(id);
    }
    host.apply_authentication(&selected, pending.mode);
    if let Some(index) = index {
        fresh.hosts[index] = host.clone();
        if let Err(error) = fresh.save_to(path) {
            if let Some(id) = created {
                let _ = wezterm_ssh::credentials::delete(&id);
            }
            return Err(error);
        }
    }
    *store = fresh;
    p.original = host;
    p.settings_changed = true;
    p.notice = "Connected · authentication saved for this Host".into();
    p.notice_until = Some(Instant::now() + Duration::from_secs(4));
    p.error.clear();
    p.pending = None;
    Ok(())
}

impl TermWindow {
    pub(super) fn termviai_auth_tick(&mut self) {
        let mux = Mux::get();
        let keepalive = self.termviai_ssh_keepalive_interval();
        let panes = mux
            .get_window(self.mux_window_id)
            .map(|w| {
                w.iter_tabs()
                    .flat_map(|t| t.iter_panes_ignoring_zoom().into_iter().map(|p| p.pane))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let active = mux
            .get_active_tab_for_window(self.mux_window_id)
            .and_then(|t| t.get_active_pane())
            .map(|p| p.pane_id());
        self.termviai_ui.auth.active = active;
        self.termviai_ui
            .auth
            .panes
            .retain(|id, _| panes.iter().any(|p| p.pane_id() == *id));
        let mut polling = false;
        for pane in panes {
            let Some(control) = mux.get_domain(pane.domain_id()).and_then(|d| {
                d.downcast_ref::<RemoteSshDomain>()
                    .and_then(|d| d.authentication(pane.pane_id()))
            }) else {
                continue;
            };
            let Some(host) = self
                .termviai_ui
                .workspace
                .hosts
                .get(&pane.domain_id())
                .cloned()
            else {
                continue;
            };
            let saved = self.termviai_ui.store.hosts.contains(&host);
            let a = self
                .termviai_ui
                .auth
                .panes
                .entry(pane.pane_id())
                .or_insert_with(|| PaneAuth::new(control.clone(), host.clone(), saved));
            if !Arc::ptr_eq(&a.control, &control) {
                *a = PaneAuth::new(control, host, saved);
            }
            a.view = a.control.view();
            if a.view.phase == Phase::Challenge && a.challenge_revision != a.view.revision {
                a.fields = a.view.prompts.iter().map(|_| Input::default()).collect();
                a.focus = Command::Field(0);
                a.picker = false;
                a.challenge_revision = a.view.revision;
            }
            if a.view.authenticated && a.pending.is_some() && a.error.is_empty() {
                if let Err(e) = save_success(a, &mut self.termviai_ui.store) {
                    a.error = format!("{e:#}");
                }
            }
            if a.settings_changed {
                a.settings_changed = false;
                self.termviai_ui
                    .workspace
                    .hosts
                    .insert(pane.domain_id(), a.original.clone());
                if let Some(domain) = mux.get_domain(pane.domain_id()) {
                    if let Some(domain) = domain.downcast_ref::<RemoteSshDomain>() {
                        domain.set_authentication_plan(
                            super::super::termviai_workspace::terminal_authentication_plan(
                                &a.original,
                                &self.termviai_ui.store,
                                keepalive,
                            ),
                        );
                    }
                }
            }
            if a.view.phase == Phase::Waiting && a.challenge_revision != 0 {
                a.reset_fields();
                a.challenge_revision = 0;
            }
            if a.notice_until.is_some_and(|due| Instant::now() >= due) {
                a.notice.clear();
                a.notice_until = None;
            }
            polling |= a.notice_until.is_some();
            polling |= matches!(
                a.view.phase,
                Phase::Trying | Phase::Opening | Phase::Waiting | Phase::Challenge | Phase::Verify
            ) || a.scroll.active();
        }
        if polling {
            self.update_next_frame_time(Some(Instant::now() + Duration::from_millis(80)));
        }
    }
    pub(super) fn termviai_auth_action(&mut self, id: usize, c: Command) -> anyhow::Result<()> {
        // Clicking a recovery page focuses only that pane, not the whole group.
        let mux = Mux::get();
        if let (Some(tab), Some(pane)) = (
            mux.get_active_tab_for_window(self.mux_window_id),
            mux.get_pane(id),
        ) {
            if tab.contains_pane(id) {
                tab.set_active_pane(&pane);
            }
        }
        self.termviai_ui.auth.active = Some(id);
        let keepalive = self.termviai_ssh_keepalive_interval();
        let store = HostStore::load()?;
        let ui = &mut self.termviai_ui;
        let library = &mut ui.store;
        let a = ui
            .auth
            .panes
            .get_mut(&id)
            .context("This authentication page was closed")?;
        if matches!(c, Command::Field(_)) {
            a.focus = c;
            return Ok(());
        }
        let result = (|| -> anyhow::Result<()> {
            match c {
                Command::Noop => {}
                Command::Method(mode) => {
                    if !a.editable() || a.view.phase == Phase::Challenge {
                        return Ok(());
                    }
                    a.mode = mode;
                    a.picker = false;
                    a.reveal = false;
                    a.scroll = SmoothScroll::default();
                    a.error.clear();
                    a.focus = Command::Field(if mode == Authentication::Password {
                        1
                    } else {
                        0
                    });
                }
                Command::Source(keychain) => {
                    a.keychain = keychain;
                    a.picker = false;
                    a.error.clear();
                }
                Command::Picker => {
                    a.picker = !a.picker;
                }
                Command::Pick(i) => {
                    if a.mode == Authentication::Identity {
                        a.identity = store
                            .identities
                            .get(i)
                            .context("Identity no longer exists")?
                            .id
                            .clone();
                    } else {
                        a.key = store
                            .keys
                            .get(i)
                            .context("Key no longer exists")?
                            .label
                            .clone();
                    }
                    a.picker = false;
                    a.error.clear();
                }
                Command::Reveal => a.reveal = !a.reveal,
                Command::Logs => a.logs = !a.logs,
                Command::Trust => {
                    a.control.send(Request::Trust(true))?;
                }
                Command::Close => {
                    a.control.cancel();
                    a.dismissed = true;
                    a.pending = None;
                    a.fields.clear();
                }
                Command::SaveAgain => {
                    a.error.clear();
                    save_success(a, library)?;
                }
                Command::Continue => {
                    if a.view.phase == Phase::Challenge {
                        let answers = a.fields.iter().map(|i| i.value.clone()).collect();
                        a.control.send(Request::Answers(answers))?;
                        for f in &mut a.fields {
                            f.value.clear();
                        }
                        a.view.phase = Phase::Trying;
                        return Ok(());
                    }
                    anyhow::ensure!(
                        a.view.phase == Phase::Waiting,
                        "Wait for the current connection attempt"
                    );
                    let mut chosen = Host {
                        address: a.original.address.clone(),
                        port: a.original.port,
                        label: a.original.label.clone(),
                        authentication: Some(a.mode),
                        ..Default::default()
                    };
                    let mut secret = String::new();
                    match a.mode {
                        Authentication::Password => {
                            chosen.username = a.fields[0].value.trim().into();
                            anyhow::ensure!(!chosen.username.is_empty(), "Enter a username");
                            anyhow::ensure!(!a.fields[1].value.is_empty(), "Enter a password");
                            secret = format!("session-auth-{}", new_credential_id());
                            wezterm_ssh::credentials::save(&secret, &a.fields[1].value, false)?;
                            chosen.session_credential = secret.clone();
                        }
                        Authentication::Key => {
                            chosen.username = a.fields[0].value.trim().into();
                            chosen.key_username = chosen.username.clone();
                            anyhow::ensure!(!chosen.username.is_empty(), "Enter a username");
                            if a.keychain {
                                chosen.key_label = a.key.clone();
                            } else {
                                chosen.identity_file = a.fields[2].value.trim().into();
                            }
                        }
                        Authentication::Identity => chosen.identity_id = a.identity.clone(),
                    }
                    let pending = Pending {
                        host: chosen.clone(),
                        mode: a.mode,
                        secret,
                    };
                    chosen.resolve_identity(&store)?;
                    chosen.validate()?;
                    let candidate = super::super::termviai_workspace::authentication_candidate(
                        format!("{:?}", a.mode),
                        Ok(chosen),
                        keepalive,
                    );
                    if let Some(error) = &candidate.error {
                        anyhow::bail!("{error}");
                    }
                    a.control.send(Request::Retry(candidate))?;
                    a.pending = Some(pending);
                    a.error.clear();
                    a.fields[1].value.clear();
                    a.view.phase = Phase::Trying;
                }
                Command::Field(_) => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            a.error = format!("{e:#}");
        }
        self.termviai_ui.generation += 1;
        Ok(())
    }
    pub(super) fn termviai_auth_key(&mut self, event: &KeyEvent, context: &dyn WindowOps) -> bool {
        self.termviai_ui.auth.active = Mux::get()
            .get_active_tab_for_window(self.mux_window_id)
            .and_then(|t| t.get_active_pane())
            .map(|p| p.pane_id());
        if self.termviai_ui.page != Page::Terminal
            || self.termviai_ui.form.is_some()
            || !self.termviai_ui.auth.active()
        {
            return false;
        }
        if !event.key_is_down {
            return true;
        }
        let id = self.termviai_ui.auth.active.unwrap();
        let mut command = None;
        match event.key {
            KeyCode::Char('\u{1b}') => command = Some(Command::Close),
            KeyCode::Char('\t') => {
                let choices = self.termviai_ui.auth.panes[&id].tab_order.clone();
                let a = self.termviai_ui.auth.panes.get_mut(&id).unwrap();
                if !choices.is_empty() {
                    let old = choices.iter().position(|c| c == &a.focus).unwrap_or(0);
                    a.focus = choices[(old
                        + if event.modifiers.contains(Modifiers::SHIFT) {
                            choices.len() - 1
                        } else {
                            1
                        })
                        % choices.len()]
                    .clone();
                    a.reveal_focus = true;
                }
            }
            KeyCode::Char('\r') => {
                let a = &self.termviai_ui.auth.panes[&id];
                command = Some(if matches!(a.focus, Command::Field(_)) {
                    Command::Continue
                } else {
                    a.focus.clone()
                });
            }
            KeyCode::Char(' ') if self.termviai_ui.auth.input().is_none() => {
                command = Some(self.termviai_ui.auth.panes[&id].focus.clone())
            }
            KeyCode::PageDown => self
                .termviai_ui
                .auth
                .panes
                .get_mut(&id)
                .unwrap()
                .scroll
                .move_by(200., Instant::now()),
            KeyCode::PageUp => self
                .termviai_ui
                .auth
                .panes
                .get_mut(&id)
                .unwrap()
                .scroll
                .move_by(-200., Instant::now()),
            _ => return self.termviai_ui.auth.input().is_none(),
        }
        if let Some(c) = command {
            let _ = self.termviai_auth_action(id, c);
        }
        context.invalidate();
        true
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
) {
    tile(t, x, y, w, h, s, BG, fg, false);
    let v = t.last_mut().unwrap();
    v.transparent = true;
    v.text_padding = 0.;
    v.text_size = Some(size);
}
fn btn(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    id: usize,
    x: f32,
    y: f32,
    w: f32,
    label: impl Into<String>,
    c: Command,
    selected: bool,
    hover: &Option<Action>,
) {
    button(
        t,
        hits,
        x,
        y,
        w,
        36.,
        label,
        if selected { SIGNAL_BG } else { FIELD },
        Action::Authenticate(id, c),
        hover,
    );
    let v = t.last_mut().unwrap();
    v.content_centered = true;
    v.text_size = Some(12);
    v.corner_radius = 7.;
    v.fg = if selected { SIGNAL_INK } else { INK };
}
fn input(
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    a: &PaneAuth,
    id: usize,
    x: f32,
    y: f32,
    w: f32,
    i: usize,
    label: &str,
    masked: bool,
    hover: &Option<Action>,
) {
    text(t, x, y, w, 20., label, MUTED, 12);
    button(
        t,
        hits,
        x,
        y + 25.,
        w,
        40.,
        "",
        FIELD,
        Action::Authenticate(id, Command::Field(i)),
        hover,
    );
    let v = t.last_mut().unwrap();
    v.input = Some(InputPaint {
        masked,
        input: a.fields[i].clone(),
        focused: a.focus == Command::Field(i),
    });
    v.border_color = Some("#353648");
    v.corner_radius = 7.;
}

pub(super) fn draw(
    ui: &mut TermviaiUi,
    t: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    areas: &[(usize, bool, Rect)],
    hover: &Option<Action>,
) {
    for &(id, is_active, r) in areas {
        let Some(a) = ui.auth.panes.get_mut(&id) else {
            continue;
        };
        a.bounds = r;
        if !a.visible() {
            if a.view.authenticated && (!a.error.is_empty() || !a.notice.is_empty()) {
                tile(
                    t,
                    r.x,
                    r.y,
                    r.w,
                    32.,
                    if a.error.is_empty() {
                        &a.notice
                    } else {
                        &a.error
                    },
                    BG,
                    if a.error.is_empty() {
                        "#59b79e"
                    } else {
                        "#ed8193"
                    },
                    false,
                );
                if a.pending.is_some() {
                    t.last_mut().unwrap().right_inset = 116.;
                    btn(
                        t,
                        hits,
                        id,
                        r.x + r.w - 108.,
                        r.y,
                        104.,
                        "Retry saving",
                        Command::SaveAgain,
                        false,
                        hover,
                    );
                }
            }
            continue;
        }
        let start = t.len();
        tile(t, r.x, r.y, r.w, r.h, "", BG, INK, false);
        hits.push(Hit {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            action: Action::Authenticate(id, Command::Noop),
        });
        if r.w < 210. || r.h < 150. {
            text(
                t,
                r.x + 8.,
                r.y + 8.,
                (r.w - 16.).max(1.),
                24_f32.min(r.h - 8.),
                "Enlarge pane to authenticate",
                MUTED,
                12,
            );
            for v in &mut t[start..] {
                v.zindex = 34;
            }
            continue;
        }
        let cw = (r.w - 32.).clamp(1., 430.);
        let x = r.x + (r.w - cw) / 2.;
        let body_top = r.y + 16.;
        let footer = (r.y + r.h - 56.).max(body_top + 36.);
        let mut y = body_top + if r.h > 600. { 30. } else { 0. };
        let mut b = vec![];
        let mut bh = vec![];
        tile(&mut b, x, y, 40., 40., "", SIGNAL_BG, SIGNAL_INK, true);
        b.last_mut().unwrap().icon = Some(Icon::Hosts);
        text(
            &mut b,
            x + 52.,
            y,
            cw - 152.,
            25.,
            &a.original.label,
            INK,
            14,
        );
        text(
            &mut b,
            x + 52.,
            y + 26.,
            cw - 52.,
            20.,
            format!("SSH {}", a.original.endpoint()),
            MUTED,
            12,
        );
        btn(
            &mut b,
            &mut bh,
            id,
            x + cw - 94.,
            y,
            94.,
            if a.logs { "Hide logs" } else { "Show logs" },
            Command::Logs,
            false,
            hover,
        );
        y += 64.;
        let segment = cw / 3.;
        for (i, (label, icon)) in [
            ("Connected", Icon::Check),
            ("Authenticate", Icon::Key),
            ("Terminal", Icon::Terminal),
        ]
        .iter()
        .copied()
        .enumerate()
        {
            text(
                &mut b,
                x + i as f32 * segment,
                y,
                segment,
                25.,
                label,
                if i == 1 { SIGNAL_INK } else { MUTED },
                12,
            );
            let v = b.last_mut().unwrap();
            if cw > 340. {
                v.icon = Some(icon);
            }
            v.icon_size = 14.;
        }
        y += 52.;
        let waiting = matches!(a.view.phase, Phase::Waiting | Phase::Challenge);
        text(
            &mut b,
            x,
            y,
            cw,
            28.,
            match a.view.phase {
                Phase::Waiting => "Authentication required",
                Phase::Challenge => "Additional authentication",
                Phase::Verify => "Verify SSH host",
                Phase::Failed => "Connection unavailable",
                Phase::Opening => "Opening terminal…",
                _ => "Connecting…",
            },
            INK,
            18,
        );
        b.last_mut().unwrap().text_bold = true;
        y += 36.;
        if a.view.phase == Phase::Waiting {
            text(&mut b, x, y, cw, 22., "Choose a way to connect.", MUTED, 12);
            y += 34.;
            for (i, (mode, label, icon)) in [
                (Authentication::Password, "Password", Icon::Lock),
                (Authentication::Key, "SSH Key", Icon::Key),
                (Authentication::Identity, "Identity", Icon::Identity),
            ]
            .iter()
            .copied()
            .enumerate()
            {
                let w = (cw - 8.) / 3.;
                btn(
                    &mut b,
                    &mut bh,
                    id,
                    x + i as f32 * (w + 4.),
                    y,
                    w,
                    label,
                    Command::Method(mode),
                    a.mode == mode,
                    hover,
                );
                if cw > 340. {
                    b.last_mut().unwrap().icon = Some(icon);
                }
            }
            y += 56.;
            match a.mode {
                Authentication::Password => {
                    input(
                        &mut b, &mut bh, a, id, x, y, cw, 0, "Username", false, hover,
                    );
                    y += 80.;
                    input(
                        &mut b, &mut bh, a, id, x, y, cw, 1, "Password", !a.reveal, hover,
                    );
                    b.last_mut().unwrap().right_inset = 40.;
                    btn(
                        &mut b,
                        &mut bh,
                        id,
                        x + cw - 36.,
                        y + 27.,
                        36.,
                        "",
                        Command::Reveal,
                        false,
                        hover,
                    );
                    b.last_mut().unwrap().icon =
                        Some(if a.reveal { Icon::EyeOff } else { Icon::Eye });
                    b.last_mut().unwrap().transparent = true;
                    y += 80.;
                }
                Authentication::Key => {
                    input(
                        &mut b, &mut bh, a, id, x, y, cw, 0, "Username", false, hover,
                    );
                    y += 80.;
                    btn(
                        &mut b,
                        &mut bh,
                        id,
                        x,
                        y,
                        (cw - 8.) / 2.,
                        "Keychain",
                        Command::Source(true),
                        a.keychain,
                        hover,
                    );
                    btn(
                        &mut b,
                        &mut bh,
                        id,
                        x + (cw + 8.) / 2.,
                        y,
                        (cw - 8.) / 2.,
                        "File path",
                        Command::Source(false),
                        !a.keychain,
                        hover,
                    );
                    y += 52.;
                    if a.keychain {
                        btn(
                            &mut b,
                            &mut bh,
                            id,
                            x,
                            y,
                            cw,
                            if a.key.is_empty() {
                                "Select SSH key"
                            } else {
                                &a.key
                            },
                            Command::Picker,
                            false,
                            hover,
                        );
                        b.last_mut().unwrap().icon = Some(Icon::ChevronDown);
                        y += 48.;
                    } else {
                        input(
                            &mut b,
                            &mut bh,
                            a,
                            id,
                            x,
                            y,
                            cw,
                            2,
                            "Private key path",
                            false,
                            hover,
                        );
                        y += 80.;
                    }
                }
                Authentication::Identity => {
                    let label = ui
                        .store
                        .identities
                        .iter()
                        .find(|i| i.id == a.identity)
                        .map(|i| format!("{} · {}", i.label, i.username))
                        .unwrap_or_else(|| "Select Identity".into());
                    btn(
                        &mut b,
                        &mut bh,
                        id,
                        x,
                        y,
                        cw,
                        label,
                        Command::Picker,
                        false,
                        hover,
                    );
                    b.last_mut().unwrap().icon = Some(Icon::ChevronDown);
                    y += 52.;
                }
            }
            if a.picker {
                let labels: Vec<_> = if a.mode == Authentication::Identity {
                    ui.store
                        .identities
                        .iter()
                        .map(|i| format!("{} · {}", i.label, i.username))
                        .collect()
                } else {
                    ui.store.keys.iter().map(|k| k.label.clone()).collect()
                };
                if labels.is_empty() {
                    text(
                        &mut b,
                        x,
                        y,
                        cw,
                        24.,
                        "No saved entries in Keychain",
                        MUTED,
                        12,
                    );
                    y += 30.;
                }
                for (i, label) in labels.into_iter().enumerate() {
                    btn(
                        &mut b,
                        &mut bh,
                        id,
                        x,
                        y,
                        cw,
                        label,
                        Command::Pick(i),
                        false,
                        hover,
                    );
                    y += 40.;
                }
            }
            text(
                &mut b,
                x,
                y,
                cw,
                22.,
                "On success, update only this Host’s",
                MUTED,
                12,
            );
            y += 22.;
            text(
                &mut b,
                x,
                y,
                cw,
                22.,
                match a.mode {
                    Authentication::Password => "manual username and password.",
                    Authentication::Key => "SSH key setting.",
                    Authentication::Identity => "Identity selection.",
                },
                MUTED,
                12,
            );
            y += 32.;
        } else if a.view.phase == Phase::Challenge {
            for (i, (prompt, echo)) in a.view.prompts.iter().enumerate() {
                input(&mut b, &mut bh, a, id, x, y, cw, i, prompt, !echo, hover);
                y += 80.;
            }
        } else if a.view.phase == Phase::Trying {
            text(
                &mut b,
                x,
                y,
                cw,
                25.,
                format!("Trying {}…", a.view.label),
                MUTED,
                12,
            );
            y += 32.;
        }
        let error = if !a.error.is_empty() {
            &a.error
        } else {
            &a.view.error
        };
        if !error.is_empty() {
            for line in textwrap::wrap(error, (cw / 7.).max(10.) as usize) {
                text(
                    &mut b,
                    x,
                    y,
                    cw,
                    22.,
                    line.into_owned(),
                    if a.view.phase == Phase::Verify {
                        INK
                    } else {
                        "#ed8193"
                    },
                    12,
                );
                y += 22.;
            }
            y += 12.;
        }
        if a.logs {
            for log in &a.view.logs {
                for line in textwrap::wrap(log, (cw / 7.).max(10.) as usize) {
                    text(&mut b, x, y, cw, 22., line.into_owned(), MUTED, 12);
                    y += 22.;
                }
            }
        }
        a.scroll.bounds(
            (footer - 12. - body_top).max(0.),
            (y - body_top).max(0.),
            Instant::now(),
        );
        a.tab_order = bh
            .iter()
            .filter_map(|h| {
                if let Action::Authenticate(_, c) = &h.action {
                    Some(c.clone())
                } else {
                    None
                }
            })
            .collect();
        a.tab_order.push(Command::Close);
        if waiting {
            a.tab_order.push(Command::Continue);
        }
        if a.view.phase == Phase::Verify {
            a.tab_order.push(Command::Trust);
        }
        if a.reveal_focus {
            if let Some(hit) = bh
                .iter()
                .find(|h| h.action == Action::Authenticate(id, a.focus.clone()))
            {
                let pos = a.scroll.position;
                if hit.y - pos < body_top {
                    a.scroll.jump_to(hit.y - body_top, Instant::now());
                } else if hit.y + hit.h - pos > footer - 10. {
                    a.scroll
                        .jump_to(hit.y + hit.h - footer + 10., Instant::now());
                }
            }
            a.reveal_focus = false;
        }
        let shift = a.scroll.position;
        // Clip to this pane, including during split resizing. Never let text or
        // hit boxes spill over a neighbor, titlebar, or the fixed action row.
        for mut v in b {
            v.y -= shift;
            if v.y >= body_top && v.y + v.h <= footer - 10. {
                t.push(v);
            }
        }
        for mut v in bh {
            v.y -= shift;
            if v.y >= body_top && v.y + v.h <= footer - 10. {
                hits.push(v);
            }
        }
        btn(
            t,
            hits,
            id,
            x,
            footer,
            70.,
            "Close",
            Command::Close,
            false,
            hover,
        );
        let v = t.last_mut().unwrap();
        v.transparent = true;
        v.bg = BG;
        if waiting || a.view.phase == Phase::Verify {
            btn(
                t,
                hits,
                id,
                x + cw - 160_f32.min(cw - 78.),
                footer,
                160_f32.min(cw - 78.),
                if a.view.phase == Phase::Verify {
                    "Trust & Continue"
                } else if a.view.phase == Phase::Challenge {
                    "Continue"
                } else {
                    "Continue & Save"
                },
                if a.view.phase == Phase::Verify {
                    Command::Trust
                } else {
                    Command::Continue
                },
                false,
                hover,
            );
        }
        if a.scroll.maximum > 0. {
            let height = (footer - body_top).max(1.);
            let thumb = (height * height / (height + a.scroll.maximum))
                .max(18.)
                .min(height);
            tile(
                t,
                r.x + r.w - 5.,
                body_top + (height - thumb) * a.scroll.position / a.scroll.maximum,
                3.,
                thumb,
                "",
                MUTED,
                MUTED,
                true,
            );
        }
        for v in &mut t[start..] {
            v.zindex = 34;
            if v.action.as_ref() == Some(&Action::Authenticate(id, a.focus.clone())) && is_active {
                v.border_color = Some(ACCENT);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_is_clipped_to_the_owning_pane_and_footer_remains_reachable() {
        for (w, h) in [(960., 700.), (430., 650.), (272., 197.), (340., 340.)] {
            let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
            let control = Control::new();
            control.update(Phase::Waiting, String::new());
            let mut p = PaneAuth::new(
                control,
                Host {
                    label: "209".into(),
                    address: "192.168.25.209".into(),
                    port: 22,
                    ..Default::default()
                },
                false,
            );
            p.mode = Authentication::Key;
            p.keychain = false;
            p.logs = true;
            ui.auth.panes.insert(9, p);
            let r = Rect {
                x: 188.,
                y: 84.,
                w,
                h,
            };
            let mut tiles = vec![];
            let mut hits = vec![];
            draw(&mut ui, &mut tiles, &mut hits, &[(9, true, r)], &None);
            for tile in &tiles {
                assert!(
                    tile.x >= r.x
                        && tile.y >= r.y
                        && tile.x + tile.w <= r.x + r.w + 0.1
                        && tile.y + tile.h <= r.y + r.h + 0.1,
                    "tile escapes pane: {} {:?}",
                    tile.text,
                    (tile.x, tile.y, tile.w, tile.h)
                );
            }
            for hit in &hits {
                assert!(
                    hit.x >= r.x
                        && hit.y >= r.y
                        && hit.x + hit.w <= r.x + r.w + 0.1
                        && hit.y + hit.h <= r.y + r.h + 0.1
                );
            }
            assert!(hits
                .iter()
                .any(|h| h.action == Action::Authenticate(9, Command::Continue)));
            assert!(ui.auth.panes[&9].tab_order.contains(&Command::Field(2)));
        }
    }
    #[test]
    fn input_drafts_belong_to_one_pane_and_are_masked() {
        let mut ui = AuthUi::default();
        for id in [1, 2] {
            let c = Control::new();
            c.update(Phase::Waiting, String::new());
            ui.panes
                .insert(id, PaneAuth::new(c, Host::default(), false));
        }
        ui.active = Some(1);
        ui.input().unwrap().insert("one-private-password");
        assert!(ui.masked());
        ui.active = Some(2);
        assert!(ui.input().unwrap().value.is_empty());
        ui.panes.get_mut(&2).unwrap().view.phase = Phase::Trying;
        assert!(ui.input().is_none());
        ui.active = Some(1);
        assert_eq!(ui.input().unwrap().value, "one-private-password");
    }
    #[test]
    fn discarded_attempt_removes_temporary_password() {
        let reference = format!("session-test-{}", new_credential_id());
        wezterm_ssh::credentials::save(&reference, "synthetic-password", false).unwrap();
        let pending = Pending {
            host: Host::default(),
            mode: Authentication::Password,
            secret: reference.clone(),
        };
        drop(pending);
        assert!(wezterm_ssh::credentials::load(&reference)
            .unwrap()
            .is_none());
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn identity_success_updates_only_that_host_reference_and_conflicts_do_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let original = Host {
            label: "209".into(),
            address: "host.invalid".into(),
            port: 22,
            username: "manual".into(),
            credential_id: "old-password".into(),
            key_label: "old-key".into(),
            identity_id: "old-id".into(),
            ..Default::default()
        };
        let mut store = HostStore {
            hosts: vec![original.clone()],
            identities: vec![Identity {
                id: "new-id".into(),
                label: "Shared".into(),
                username: "deploy".into(),
            }],
            ..Default::default()
        };
        store.save_to(&path).unwrap();
        let mut p = PaneAuth::new(Control::new(), original, true);
        p.pending = Some(Pending {
            host: Host {
                identity_id: "new-id".into(),
                ..Default::default()
            },
            mode: Authentication::Identity,
            secret: String::new(),
        });
        let shared = store.identities.clone();
        assert!(save_success_at(&mut p, &mut store, &path).is_err());
        assert_eq!(
            HostStore::load_from(&path).unwrap().hosts[0].identity_id,
            "old-id"
        );
        p.view.authenticated = true;
        save_success_at(&mut p, &mut store, &path).unwrap();
        assert_eq!(store.identities, shared);
        assert_eq!(store.hosts[0].credential_id, "old-password");
        assert_eq!(store.hosts[0].key_label, "old-key");
        assert_eq!(store.hosts[0].username, "manual");
        assert_eq!(store.hosts[0].identity_id, "new-id");
        assert!(p.pending.is_none());
        p.pending = Some(Pending {
            host: Host {
                identity_id: "another-id".into(),
                ..Default::default()
            },
            mode: Authentication::Identity,
            secret: String::new(),
        });
        store.hosts[0].label = "Changed in another window".into();
        store.save_to(&path).unwrap();
        assert!(save_success_at(&mut p, &mut store, &path).is_err());
        let saved = HostStore::load_from(&path).unwrap();
        assert_eq!(saved.hosts[0].identity_id, "new-id");
        assert_eq!(saved.hosts[0].label, "Changed in another window");
    }
}
