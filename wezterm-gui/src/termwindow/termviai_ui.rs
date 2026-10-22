//! Native TermViAI application chrome and local SSH library.
use super::box_model::*;
use super::render::corners::*;
use super::termviai_broadcast::{group_broadcasting, pane_broadcasting};
use super::termviai_icons::Icon;
use super::termviai_layout::{BROADCAST_SURFACE_TOP_INSET, BROADCAST_TRAY_HEIGHT};
use super::termviai_motion::{frame_interval, Tween};
use super::termviai_workspace::WorkspaceUi;
use super::{TermWindow, TermWindowNotif};
use crate::hosts::{new_credential_id, Host, HostStore, Identity, KeyFile};
use crate::utilsprites::RenderMetrics;
use crate::workspaces::{SavedLayout, WorkspaceStore};
use anyhow::Context;
use config::{Dimension, DimensionContext};
use mux::localpane::LocalPane;
use mux::Mux;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use window::{
    Clipboard, CursorIcon, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseEventKind, MousePress,
    WindowOps, WindowState,
};

const BG: &str = "#1c1d2b";
const SIDE: &str = "#232435";
const TOP: &str = "#171823";
const CARD: &str = "#292a3c";
const FIELD: &str = "#37394f";
const INK: &str = "#f1f2fa";
const MUTED: &str = "#9ca2ba";
const ACCENT: &str = "#aaa7cb";
const SIGNAL_BG: &str = "#414256";
const SIGNAL_INK: &str = "#b5a2d8";
const RECONNECT: &str = "#89b4fa";
const SIDEBAR_WIDTH: f32 = 220.;
const MIN_WINDOW_WIDTH: usize = 720;
const MIN_WINDOW_HEIGHT: usize = 480;

pub(super) fn minimum_window_size() -> (usize, usize) {
    (MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)
}

pub fn sidebar_width(config: &config::ConfigHandle, dpi: f32) -> f32 {
    if config.termviai_ui {
        SIDEBAR_WIDTH * dpi / 96.
    } else {
        0.
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Hosts,
    Keys,
    NewTab,
    Terminal,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Action {
    ToggleSidebar,
    Hosts,
    Keys,
    Settings,
    NewTab,
    CloseNewTab,
    TabBroadcast(usize),
    ToggleTray,
    PrevMember,
    NextMember,
    RenameTab(usize),
    SaveWorkspace(usize),
    OpenSaved(String),
    ConnectRecent(usize),
    DismissMenu,
    ConfirmDismiss,
    ConfirmCard,
    ConfirmCancel,
    ConfirmAccept,
    AddHost,
    AddKey,
    AddIdentity,
    EditIdentity(usize),
    DeleteIdentity,
    ChooseAuth,
    ManualAuth,
    UseIdentity(usize),
    RememberPassword,
    RevealPassword,
    Cancel,
    Save,
    Connect(usize),
    QuickConnect,
    HostMenu(usize),
    Field(usize),
    Search,
    Group(String),
    UseKey(usize),
    Tab(usize),
    CloseTab(usize),
    PrevTab,
    NextTab,
    PrevGroup,
    NextGroup,
    Minimize,
    Maximize,
    Close,
    Drag,
    Drawer,
    Broadcast,
    BroadcastAll,
    BroadcastNone,
    Member(usize),
    // Separate interaction identity from the tray member button for the same pane.
    PaneBroadcast(usize),
    ReconnectPane(usize),
    ReconnectGroup(usize),
    RemovePane(usize),
    FocusPane(usize),
    PaneError(usize),
    EditMenuHost,
    DuplicateHost,
}
#[derive(Clone, Default)]
struct Input {
    value: String,
    cursor: usize,
    anchor: Option<usize>,
}
impl Input {
    fn new(value: String) -> Self {
        Self {
            cursor: value.chars().count(),
            value,
            anchor: None,
        }
    }
    fn selection(&self) -> Option<std::ops::Range<usize>> {
        self.anchor
            .filter(|a| *a != self.cursor)
            .map(|a| a.min(self.cursor)..a.max(self.cursor))
    }
    fn selected_text(&self) -> Option<String> {
        self.selection()
            .map(|r| self.value.chars().skip(r.start).take(r.len()).collect())
    }
    fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.value.chars().count();
    }
    fn move_to(&mut self, position: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = position.min(self.value.chars().count());
    }
    fn remove_selection(&mut self) -> bool {
        let range = self.selection();
        self.anchor = None;
        if let Some(range) = range {
            let mut chars: Vec<char> = self.value.chars().collect();
            self.cursor = range.start;
            chars.drain(range);
            self.value = chars.into_iter().collect();
            true
        } else {
            false
        }
    }
    fn insert(&mut self, text: &str) {
        self.remove_selection();
        let mut chars: Vec<char> = self.value.chars().collect();
        let added: Vec<char> = text
            .chars()
            .filter(|c| !c.is_control())
            .take(4096usize.saturating_sub(chars.len()))
            .collect();
        let n = added.len();
        chars.splice(self.cursor..self.cursor, added);
        self.cursor += n;
        self.value = chars.into_iter().collect();
    }
    fn delete(&mut self, back: bool) {
        if self.remove_selection() {
            return;
        }
        let mut chars: Vec<char> = self.value.chars().collect();
        if back && self.cursor > 0 {
            self.cursor -= 1;
            chars.remove(self.cursor);
        } else if !back && self.cursor < chars.len() {
            chars.remove(self.cursor);
        }
        self.value = chars.into_iter().collect();
    }
    fn display(&self, _focused: bool, capacity: usize) -> String {
        self.value.chars().take(capacity).collect()
    }
}

#[derive(Clone)]
struct InputPaint {
    masked: bool,
    input: Input,
    focused: bool,
}
impl InputPaint {
    fn rendered_value(&self) -> String {
        if self.masked {
            "•".repeat(self.input.value.chars().count())
        } else {
            self.input.value.clone()
        }
    }
}
struct InputHit {
    action: Action,
    x: f32,
    start: usize,
    positions: Vec<f32>,
}
impl InputHit {
    fn position(&self, x: f32) -> usize {
        let offset = x - self.x;
        self.start
            + self
                .positions
                .windows(2)
                .position(|pair| offset < (pair[0] + pair[1]) / 2.)
                .unwrap_or(self.positions.len().saturating_sub(1))
    }
}

// Shape with the same font and advances as the field renderer; no guessed
// eight-pixel character widths and no visible cursor glyph in the text.
fn text_positions(
    font: &wezterm_font::LoadedFont,
    text: &str,
    scale: f32,
) -> anyhow::Result<Vec<f32>> {
    let chars: Vec<usize> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    let glyphs = font.shape(
        text,
        || {},
        crate::customglyph::BlockKey::filter_out_synthetic,
        None,
        wezterm_bidi::Direction::LeftToRight,
        None,
        None,
    )?;
    let mut advances = std::collections::BTreeMap::<usize, f32>::new();
    for glyph in glyphs {
        *advances.entry(glyph.cluster as usize).or_default() +=
            glyph.x_advance.get() as f32 / scale;
    }
    let clusters: Vec<_> = advances.into_iter().collect();
    let mut positions = vec![0.; chars.len()];
    let mut x = 0.;
    for (index, (byte, advance)) in clusters.iter().enumerate() {
        let end = clusters.get(index + 1).map(|c| c.0).unwrap_or(text.len());
        let first = chars.partition_point(|b| b < byte);
        let last = chars.partition_point(|b| *b < end);
        for n in first..=last {
            positions[n] = x + advance * (n - first) as f32 / (last - first).max(1) as f32;
        }
        x += advance;
    }
    Ok(positions)
}
#[derive(Default)]
struct AuthForm {
    editing_identity: bool,
    original_identity: Option<Identity>,
    selected_identity: String,
    remember: bool,
    reveal: bool,
    picker: bool,
    picker_scroll: usize,
    confirm_delete: bool,
    reset_password: bool,
}

struct Form {
    auth: AuthForm,
    fields: Vec<Input>,
    focus: usize,
    key: bool,
    original: Option<Host>,
    rename_tab: Option<usize>,
    settings: bool,
    first_row: usize,
    reveal_focus: bool,
}
// Use logical pixels so drawing and hit testing agree at every Windows DPI.
struct DrawerLayout {
    x: f32,
    width: f32,
    height: f32,
    rows: usize,
    footer: f32,
}
impl DrawerLayout {
    fn new(width: f32, height: f32) -> Self {
        let drawer_width = 400_f32.min(width.max(0.));
        let footer = (height - 120.).max(210.);
        Self {
            x: width - drawer_width,
            width: drawer_width,
            height,
            rows: ((footer - 148.) / 70.).floor().max(1.) as usize,
            footer,
        }
    }
    fn capture_hits(&self, hits: &mut Vec<Hit>, width: f32) {
        // One outside click only dismisses the form, never activates a host below it.
        hits.retain(|hit| hit.y < 52.);
        hits.push(Hit {
            x: 0.,
            y: 52.,
            w: width,
            h: (self.height - 52.).max(0.),
            action: Action::Cancel,
        });
        hits.push(Hit {
            x: self.x,
            y: 52.,
            w: self.width,
            h: (self.height - 52.).max(0.),
            action: Action::Drawer,
        });
    }
}
impl Form {
    fn is_host(&self) -> bool {
        !self.key && !self.settings && self.rename_tab.is_none() && !self.auth.editing_identity
    }
    fn password_index(&self) -> Option<usize> {
        if self.auth.editing_identity {
            Some(2)
        } else if self.is_host() && self.auth.selected_identity.is_empty() {
            Some(4)
        } else {
            None
        }
    }
    fn masked_password(&self) -> bool {
        self.password_index() == Some(self.focus) && !self.auth.reveal
    }

    fn visible_rows(&mut self, count: usize) -> std::ops::Range<usize> {
        let count = count.max(1);
        let max = self.fields.len().saturating_sub(count);
        if self.reveal_focus {
            if self.focus < self.first_row {
                self.first_row = self.focus;
            } else if self.focus >= self.first_row + count {
                self.first_row = self.focus + 1 - count;
            }
            self.reveal_focus = false;
        }
        self.first_row = self.first_row.min(max);
        self.first_row..(self.first_row + count).min(self.fields.len())
    }
}
fn hit_action(hits: &[Hit], x: f32, y: f32) -> Option<Action> {
    hits.iter()
        .rev()
        .find(|h| x >= h.x && x < h.x + h.w && y >= h.y && y < h.y + h.h)
        .map(|h| h.action.clone())
}
struct Hit {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    action: Action,
}
struct Tile {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: String,
    bg: &'static str,
    fg: &'static str,
    rounded: bool,
    icon: Option<Icon>,
    icon_size: f32,
    corner_radius: f32,
    outline_width: f32,
    action: Option<Action>,
    right_inset: f32,
    opacity: f32,
    transparent: bool,
    zindex: i8,
    input: Option<InputPaint>,
}

pub struct TermviaiUi {
    pub page: Page,
    store: HostStore,
    query: Input,
    search_focus: bool,
    group: String,
    form: Option<Form>,
    closing_form: Option<Form>,
    drawer_motion: Tween,
    page_motion: Tween,
    sidebar_expanded: bool,
    sidebar_motion: Tween,
    sidebar_layout_pixels: usize,
    dock_motion: Tween,
    dock_preview: Option<super::termviai_dock::Rect>,
    painted_page: Page,
    hover_motion: HashMap<Action, Tween>,
    pub(super) error: String,
    pub(super) terminal_errors: HashMap<usize, String>,
    pub(super) tab_drag: Option<super::termviai_tab_drag::TabDrag>,
    hits: Vec<Hit>,
    input_hits: Vec<InputHit>,
    input_drag: Option<Action>,
    host_menu: Option<(Host, f32, f32)>,
    scroll: usize,
    max_scroll: usize,
    tab_scroll: usize,
    group_scroll: usize,
    generation: u64,
    hover: Option<Action>,
    hover_started: Instant,
    pressed: Option<Action>,
    pub(super) connecting: bool,
    pub(super) workspace: WorkspaceUi,
    pub(super) new_tab_open: bool,
    tab_menu: Option<(usize, f32, f32)>,
    tray_expanded: bool,
    tray_motion: Tween,
    broadcast_motion: Tween,
    broadcast_motion_tab: Option<usize>,
    notice: Option<(String, Instant)>,
    pane_scroll: usize,
    confirm_keys: CloseKeyLatch,
}
impl TermviaiUi {
    pub fn new() -> Self {
        let (store, error) = match HostStore::load() {
            Ok(s) => (s, String::new()),
            Err(e) => (HostStore::default(), format!("{e:#}")),
        };
        Self::with_store(store, error)
    }
    fn with_store(store: HostStore, error: String) -> Self {
        Self {
            page: Page::Hosts,
            store,
            query: Input::default(),
            search_focus: true,
            group: String::new(),
            form: None,
            closing_form: None,
            drawer_motion: Tween::new(0.),
            page_motion: Tween::new(1.),
            sidebar_expanded: true,
            sidebar_motion: Tween::new(1.),
            sidebar_layout_pixels: usize::MAX,
            dock_motion: Tween::new(1.),
            dock_preview: None,
            painted_page: Page::Hosts,
            hover_motion: HashMap::new(),
            error,
            terminal_errors: HashMap::new(),
            tab_drag: None,
            hits: vec![],
            input_hits: vec![],
            input_drag: None,
            host_menu: None,
            scroll: 0,
            max_scroll: 0,
            tab_scroll: 0,
            group_scroll: 0,
            generation: 0,
            hover: None,
            hover_started: Instant::now(),
            pressed: None,
            connecting: false,
            workspace: WorkspaceUi::default(),
            new_tab_open: false,
            tab_menu: None,
            tray_expanded: true,
            tray_motion: Tween::new(1.),
            broadcast_motion: Tween::new(0.),
            broadcast_motion_tab: None,
            notice: None,
            pane_scroll: 0,
            confirm_keys: CloseKeyLatch::default(),
        }
    }
    pub(super) fn broadcast_tray_animating(&self, now: Instant) -> bool {
        self.page == Page::Terminal && self.tray_motion.is_active(now)
    }
    fn sidebar_width(&self, now: Instant) -> f32 {
        SIDEBAR_WIDTH * self.sidebar_motion.value(now)
    }
    fn toggle_sidebar(&mut self, now: Instant) {
        self.sidebar_expanded = !self.sidebar_expanded;
        self.sidebar_motion.set_target(
            if self.sidebar_expanded { 1. } else { 0. },
            now,
            Duration::from_millis(220),
        );
    }
    pub(super) fn broadcast_tray_height(&self) -> f32 {
        if self.page == Page::Terminal {
            BROADCAST_TRAY_HEIGHT * self.tray_motion.value(Instant::now())
        } else {
            0.
        }
    }
    pub(super) fn show_notice(&mut self, text: String) {
        self.notice = Some((text, Instant::now() + Duration::from_secs(3)));
    }
    fn open_rename(&mut self, tab_id: usize, name: String) {
        self.discard_form();
        self.form = Some(Form {
            fields: vec![Input::new(name)],
            focus: 0,
            key: false,
            original: None,
            rename_tab: Some(tab_id),
            settings: false,
            first_row: 0,
            reveal_focus: true,
            auth: AuthForm::default(),
        });
        self.form.as_mut().unwrap().fields[0].select_all();
        self.drawer_motion
            .set_target(1., Instant::now(), Duration::from_millis(220));
        self.tab_menu = None;
        self.error.clear();
    }
    fn open_settings(&mut self, size: f64, ssh_keepalive_interval: u64) {
        self.discard_form();
        self.form = Some(Form {
            fields: vec![
                Input::new(format!("{size:.1}")),
                Input::new(ssh_keepalive_interval.to_string()),
            ],
            focus: 0,
            key: false,
            original: None,
            rename_tab: None,
            settings: true,
            first_row: 0,
            reveal_focus: true,
            auth: AuthForm::default(),
        });
        self.form.as_mut().unwrap().fields[0].select_all();
        self.drawer_motion
            .set_target(1., Instant::now(), Duration::from_millis(220));
        self.tab_menu = None;
        self.error.clear();
    }
    fn reload(&mut self) {
        match HostStore::load() {
            Ok(s) => {
                self.store = s;
                self.error.clear();
            }
            Err(e) => self.error = format!("{e:#}"),
        }
    }
    fn discard_form(&mut self) {
        self.input_drag = None;
        self.input_hits.clear();
        self.form = None;
        self.closing_form = None;
        self.drawer_motion = Tween::new(0.);
        self.generation += 1;
    }
    fn close_form(&mut self) {
        self.input_drag = None;
        if let Some(form) = self.form.take() {
            self.closing_form = Some(form);
            self.drawer_motion
                .set_target(0., Instant::now(), Duration::from_millis(180));
        }
        self.generation += 1;
        self.error.clear();
    }
    fn input(&mut self) -> Option<&mut Input> {
        if self.closing_form.is_some() {
            return None;
        }
        if let Some(f) = &mut self.form {
            if f.auth.picker
                || (!f.auth.selected_identity.is_empty() && matches!(f.focus, 3 | 4 | 5))
            {
                return None;
            }
            f.reveal_focus = true;
            Some(&mut f.fields[f.focus])
        } else if self.search_focus {
            Some(&mut self.query)
        } else {
            None
        }
    }
    fn open_host_menu(&mut self, index: usize) -> anyhow::Result<()> {
        let host = self
            .store
            .hosts
            .get(index)
            .context("Host no longer exists")?
            .clone();
        let anchor = self
            .hits
            .iter()
            .find(|hit| hit.action == Action::HostMenu(index))
            .context("Host actions button is no longer visible")?;
        self.host_menu = Some((host, anchor.x, anchor.y + anchor.h + 6.));
        self.tab_menu = None;
        self.hover = None;
        self.pressed = None;
        Ok(())
    }

    fn open_form(&mut self, key: bool, host: Option<Host>) {
        self.input_drag = None;
        self.input_hits.clear();
        self.host_menu = None;
        let h = host.clone().unwrap_or(Host {
            port: 22,
            ..Default::default()
        });
        let values = if key {
            vec![String::new(), String::new()]
        } else {
            vec![
                h.label,
                h.address,
                h.port.to_string(),
                h.username,
                String::new(),
                h.identity_file,
                h.group,
            ]
        };
        self.closing_form = None;
        self.drawer_motion
            .set_target(1., Instant::now(), Duration::from_millis(220));
        self.form = Some(Form {
            fields: values.into_iter().map(Input::new).collect(),
            focus: 0,
            key,
            original: host,
            rename_tab: None,
            settings: false,
            first_row: 0,
            reveal_focus: true,
            auth: AuthForm::default(),
        });
        if let Some(form) = &mut self.form {
            if let Some(host) = &form.original {
                form.auth.selected_identity = host.identity_id.clone();
                form.auth.remember = !host.credential_id.is_empty();
            }
        }
        self.error.clear();
        self.generation += 1;
    }
    fn open_identity(&mut self, identity: Option<Identity>) {
        self.open_form(true, None);
        let f = self.form.as_mut().unwrap();
        f.key = false;
        f.auth.editing_identity = true;
        f.fields = vec![
            Input::new(
                identity
                    .as_ref()
                    .map(|i| i.label.clone())
                    .unwrap_or_default(),
            ),
            Input::new(
                identity
                    .as_ref()
                    .map(|i| i.username.clone())
                    .unwrap_or_default(),
            ),
            Input::default(),
        ];
        f.auth.original_identity = identity;
    }
    fn save_form(&mut self) -> anyhow::Result<()> {
        self.save_form_at(&HostStore::path())
    }
    fn save_form_at(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let f = self.form.as_ref().context("No form")?;
        anyhow::ensure!(
            f.rename_tab.is_none() && !f.settings,
            "Use the form-specific save action."
        );
        // Validate a fresh snapshot before touching credentials or the library.
        let mut store = HostStore::load_from(path)?;
        let mut pending_password: Option<(String, String, bool)> = None;
        if f.auth.editing_identity {
            let label = f.fields[0].value.trim().to_string();
            let username = f.fields[1].value.trim().to_string();
            anyhow::ensure!(
                !label.is_empty() && !username.is_empty() && !label.chars().any(char::is_control),
                "Enter an identity name and username."
            );
            anyhow::ensure!(
                !username
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control()),
                "Username cannot contain spaces or control characters."
            );
            let id = f
                .auth
                .original_identity
                .as_ref()
                .map(|i| i.id.clone())
                .unwrap_or_else(new_credential_id);
            anyhow::ensure!(
                !store
                    .identities
                    .iter()
                    .any(|i| i.label == label && i.id != id),
                "An identity with this name already exists."
            );
            let identity = Identity {
                id: id.clone(),
                label,
                username,
            };
            if let Some(original) = &f.auth.original_identity {
                let index = store
                    .identities
                    .iter()
                    .position(|i| i == original)
                    .context("This identity changed. Reopen it and try again.")?;
                store.identities[index] = identity;
            } else {
                anyhow::ensure!(
                    !f.fields[2].value.is_empty(),
                    "Enter a password for this identity."
                );
                store.identities.push(identity);
            }
            if !f.fields[2].value.is_empty() {
                pending_password = Some((id, f.fields[2].value.clone(), true));
            }
        } else if f.key {
            let label = f.fields[0].value.trim();
            let path = f.fields[1].value.trim();
            anyhow::ensure!(!label.is_empty(), "Enter a key name.");
            anyhow::ensure!(
                std::path::Path::new(path).is_file(),
                "Enter an existing private key file path."
            );
            anyhow::ensure!(
                !store.keys.iter().any(|k| k.label == label),
                "A key with this name already exists."
            );
            store.keys.push(KeyFile {
                label: label.into(),
                path: path.into(),
            });
        } else {
            let v: Vec<String> = f.fields.iter().map(|v| v.value.clone()).collect();
            let mut host = Host {
                label: v[0].clone(),
                address: v[1].clone(),
                port: v[2]
                    .trim()
                    .parse()
                    .context("Port must be between 1 and 65535.")?,
                username: v[3].clone(),
                group: v[6].clone(),
                identity_file: v[5].clone(),
                identity_id: f.auth.selected_identity.clone(),
                ..Default::default()
            };
            if !host.identity_id.is_empty() {
                host.resolve_identity(&store)?;
            } else if !v[4].is_empty() {
                let id = if f.auth.remember {
                    new_credential_id()
                } else {
                    format!("session-{}", new_credential_id())
                };
                if f.auth.remember {
                    host.credential_id = id.clone();
                } else {
                    host.session_credential = id.clone();
                }
                pending_password = Some((id, v[4].clone(), f.auth.remember));
            } else if let Some(original) = &f.original {
                if original.identity_id.is_empty() && !f.auth.reset_password {
                    if f.auth.remember == !original.credential_id.is_empty() {
                        host.credential_id = original.credential_id.clone();
                        host.session_credential = original.session_credential.clone();
                    } else if let Some(password) =
                        wezterm_ssh::credentials::load(original.credential_ref())?
                    {
                        let id = if f.auth.remember {
                            new_credential_id()
                        } else {
                            format!("session-{}", new_credential_id())
                        };
                        if f.auth.remember {
                            host.credential_id = id.clone();
                        } else {
                            host.session_credential = id.clone();
                        }
                        pending_password = Some((id, password, f.auth.remember));
                    } else if f.auth.remember {
                        anyhow::bail!("The temporary password is no longer available. Enter it again to remember it.");
                    }
                }
            }
            host.validate()?;
            if let Some(original) = &f.original {
                let idx = store
                    .hosts
                    .iter()
                    .position(|h| h == original)
                    .context("This host changed in another window. Reopen it and try again.")?;
                store.hosts[idx] = host;
            } else {
                store.hosts.push(host);
            }
        }
        // Roll back the credential if metadata could not be committed.
        let previous = if let Some((id, password, persistent)) = &pending_password {
            let old = wezterm_ssh::credentials::load(id)?;
            wezterm_ssh::credentials::save(id, password, *persistent)?;
            old
        } else {
            None
        };
        if let Err(error) = store.save_to(path) {
            if let Some((id, _, persistent)) = &pending_password {
                if let Some(old) = previous {
                    let _ = wezterm_ssh::credentials::save(id, &old, *persistent);
                } else {
                    let _ = wezterm_ssh::credentials::delete(id);
                }
            }
            return Err(error);
        }
        if let Some(original) = &f.original {
            let old = original.credential_ref();
            if original.identity_id.is_empty()
                && !old.is_empty()
                && !store.hosts.iter().any(|h| h.credential_ref() == old)
            {
                // No saved host owns the replaced credential any longer.
                wezterm_ssh::credentials::delete(old)?;
            }
        }
        self.store = store;
        self.query = Input::default();
        self.group.clear();
        self.scroll = 0;
        self.close_form();
        self.error.clear();
        self.generation += 1;
        Ok(())
    }
}

fn tile(
    tiles: &mut Vec<Tile>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: impl Into<String>,
    bg: &'static str,
    fg: &'static str,
    rounded: bool,
) {
    if w > 0. && h > 0. {
        tiles.push(Tile {
            x,
            y,
            w,
            h,
            text: text.into(),
            bg,
            fg,
            rounded,
            icon: None,
            icon_size: 18.,
            corner_radius: 9.,
            outline_width: 0.,
            action: None,
            right_inset: 0.,
            opacity: 1.,
            transparent: false,
            zindex: 30,
            input: None,
        });
    }
}
fn button(
    tiles: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: impl Into<String>,
    bg: &'static str,
    action: Action,
    hover: &Option<Action>,
) {
    let text = text.into();
    let icon = match &action {
        Action::ToggleSidebar => Some(Icon::Menu),
        Action::Hosts => Some(Icon::Hosts),
        Action::Keys | Action::UseKey(_) => Some(Icon::Key),
        Action::Settings => Some(Icon::Settings),
        Action::ChooseAuth => Some(Icon::Key),
        Action::RememberPassword => Some(Icon::Check),
        Action::EditIdentity(_) | Action::UseIdentity(_) => Some(Icon::Identity),
        Action::NewTab if text.is_empty() => Some(Icon::Plus),
        Action::NewTab | Action::Tab(_) | Action::FocusPane(_) => Some(Icon::Terminal),
        Action::AddHost | Action::AddKey | Action::AddIdentity => Some(Icon::Plus),
        Action::CloseTab(_) | Action::CloseNewTab | Action::Close | Action::RemovePane(_) => {
            Some(Icon::Close)
        }
        Action::PrevTab | Action::PrevGroup => Some(Icon::ChevronLeft),
        Action::NextTab | Action::NextGroup => Some(Icon::ChevronRight),
        Action::Minimize => Some(Icon::Minimize),
        Action::Maximize => Some(Icon::Maximize),
        Action::HostMenu(_) => Some(Icon::More),
        Action::Search => Some(Icon::Search),
        Action::QuickConnect => Some(Icon::ArrowRight),
        Action::Save => Some(Icon::Check),
        Action::Cancel if text.is_empty() => Some(Icon::ArrowRight),
        Action::Broadcast | Action::TabBroadcast(_) => Some(Icon::Broadcast),
        Action::ReconnectPane(_) | Action::ReconnectGroup(_) => Some(Icon::Refresh),
        Action::ToggleTray => Some(Icon::ChevronDown),
        Action::RenameTab(_) => Some(Icon::Edit),
        Action::SaveWorkspace(_) => Some(Icon::Check),
        Action::OpenSaved(_) => Some(Icon::Grid),
        Action::ConnectRecent(_) => Some(Icon::Hosts),
        Action::PrevMember => Some(Icon::ChevronLeft),
        Action::NextMember => Some(Icon::ChevronRight),
        Action::Member(_) | Action::PaneBroadcast(_) => Some(if text == "member" {
            Icon::CheckboxChecked
        } else {
            Icon::Checkbox
        }),
        _ => None,
    };
    let text = if matches!(action, Action::Member(_) | Action::PaneBroadcast(_)) {
        String::new()
    } else {
        text
    };
    tile(tiles, x, y, w, h, text, bg, INK, true);
    if w > 0. && h > 0. {
        let t = tiles.last_mut().unwrap();
        t.icon = icon;
        t.action = Some(action.clone());
        if matches!(action, Action::Tab(_)) {
            t.right_inset = 32.;
        }
    }
    let _ = hover; // Motion is sampled once per frame for all matching controls.
    hits.push(Hit { x, y, w, h, action });
}
fn action_hint(
    action: &Action,
    broadcast: Option<&termviai_broadcast::BroadcastState<usize>>,
) -> Option<&'static str> {
    Some(match action {
        Action::Member(id) | Action::PaneBroadcast(id) => match broadcast {
            Some(state) if pane_broadcasting(state, *id) => {
                "Broadcast ON · Click to use independent input"
            }
            Some(state) if state.enabled() => "Independent input · Click to join broadcast",
            Some(state) if state.is_member(*id) => {
                "Broadcast OFF · Selected for the next broadcast"
            }
            _ => "Broadcast OFF · Click to select this session",
        },
        Action::TabBroadcast(_) => {
            "Select all sessions for broadcast / Turn all off when fully selected"
        }
        Action::ReconnectPane(_) => "Reconnect this SSH session",
        Action::ReconnectGroup(_) => "Reconnect offline SSH sessions in this tab",
        Action::FocusPane(_) => "Drag onto another session to swap positions",
        Action::Tab(_) => "Drag to reorder tabs",
        _ => return None,
    })
}

fn wrap_hint(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let chars: Vec<char> = paragraph.chars().collect();
        for chunk in chars.chunks(columns.max(1)) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PaneHeaderControls {
    broadcast_x: f32,
    remove_x: Option<f32>,
    label_right_inset: f32,
    error_x: Option<f32>,
}

fn pane_header_controls(x: f32, width: f32, grouped: bool, has_error: bool) -> PaneHeaderControls {
    // On narrow panes prioritize the error and reconnect controls. Closing is
    // still available through the group when there isn't room for three buttons.
    let show_remove = grouped && (!has_error || width >= 104.);
    let broadcast_x = x + width - if show_remove { 66. } else { 34. };
    PaneHeaderControls {
        broadcast_x,
        remove_x: show_remove.then_some(x + width - 34.),
        label_right_inset: (if show_remove { 72. } else { 40. }) + if has_error { 28. } else { 0. },
        error_x: has_error.then_some(broadcast_x - 28.),
    }
}

/// Lay out the three toolbar clusters together so none can overlap at a
/// responsive breakpoint. The switch and collapse control always have priority.
struct TrayLayout {
    title_width: f32,
    members_x: f32,
    members_width: f32,
    selection_x: Option<f32>,
    switch_x: f32,
    switch_width: f32,
    collapse_x: f32,
}

impl TrayLayout {
    fn new(width: f32, content_left: f32) -> Self {
        let collapse_x = width - 48.;
        let switch_width = if width >= 560. { 96. } else { 40. };
        let switch_x = collapse_x - 8. - switch_width;
        let selection_x = (width >= 900.).then_some(switch_x - 16. - 108.);
        let right = selection_x.unwrap_or(switch_x) - 16.;
        let content_start = content_left + 24.;
        let available = (right - content_start).max(0.);
        let title_width = if available >= 340. {
            166.
        } else if available >= 286. {
            132.
        } else if available >= 190. {
            36.
        } else {
            available.min(166.)
        };
        let members_x = content_start + title_width + 18.;
        Self {
            title_width,
            members_x,
            members_width: (right - members_x).max(0.),
            selection_x,
            switch_x,
            switch_width,
            collapse_x,
        }
    }
}

struct TrayMembers {
    start: usize,
    chips: Vec<(usize, f32, f32)>,
    pager_x: Option<f32>,
    has_next: bool,
}

impl TrayMembers {
    fn new(widths: &[f32], x: f32, width: f32, requested: usize) -> Self {
        let total = widths.iter().sum::<f32>() + widths.len().saturating_sub(1) as f32 * 6.;
        let overflow = widths.len() > 1 && total > width;
        let budget = width - if overflow { 68. } else { 0. };
        let mut result = Self {
            start: 0,
            chips: vec![],
            pager_x: None,
            has_next: false,
        };
        if widths.is_empty() || budget < 68. {
            return result;
        }
        // Clamp to a full final page after a resize, tab change or pane close.
        let mut last_start = widths.len() - 1;
        let mut used = widths[last_start].min(budget);
        while last_start > 0 && used + 6. + widths[last_start - 1] <= budget {
            last_start -= 1;
            used += 6. + widths[last_start];
        }
        result.start = if overflow {
            requested.min(last_start)
        } else {
            0
        };
        let mut offset = 0.;
        for (index, desired) in widths.iter().enumerate().skip(result.start) {
            let chip_width = desired.min(budget);
            if offset + chip_width > budget {
                break;
            }
            result.chips.push((index, x + offset, chip_width));
            offset += chip_width + 6.;
        }
        if overflow {
            result.pager_x = Some(x + width - 60.);
            result.has_next = result.start + result.chips.len() < widths.len();
        }
        result
    }
}

fn tray_label_width(label: &str) -> f32 {
    label
        .chars()
        .map(|c| if c.is_ascii() { 7. } else { 14. })
        .sum()
}

fn tray_label(label: &str, width: f32) -> String {
    if tray_label_width(label) <= width {
        return label.into();
    }
    let mut text = String::new();
    for ch in label.chars() {
        if tray_label_width(&text) + tray_label_width(&ch.to_string()) + 14. > width {
            break;
        }
        text.push(ch);
    }
    text.push('…');
    text
}

fn shield_tray(hits: &mut Vec<Hit>, width: f32, height: f32, reserved: f32, content_left: f32) {
    if reserved > 0. && width > content_left {
        hits.push(Hit {
            x: content_left,
            y: height - reserved,
            w: width - content_left,
            h: reserved,
            action: Action::Drawer,
        });
    }
}

#[derive(Debug, PartialEq)]
enum CloseKeyAction {
    Cancel,
    Accept,
    Focus(bool),
}

#[derive(Default)]
struct CloseKeyLatch {
    pending: Option<(KeyCode, CloseKeyAction)>,
}

impl CloseKeyLatch {
    fn event(
        &mut self,
        key: &KeyCode,
        mods: Modifiers,
        down: bool,
        focus: bool,
    ) -> Option<CloseKeyAction> {
        if down {
            if self.pending.is_none() {
                match close_key_action(key, mods, focus) {
                    Some(action @ (CloseKeyAction::Accept | CloseKeyAction::Cancel)) => {
                        self.pending = Some((key.clone(), action));
                    }
                    action => return action,
                }
            }
            None
        } else if self
            .pending
            .as_ref()
            .map(|(held, _)| held == key)
            .unwrap_or(false)
        {
            self.pending.take().map(|(_, action)| action)
        } else {
            None
        }
    }
}

fn close_key_action(
    key: &KeyCode,
    mods: Modifiers,
    focused_confirm: bool,
) -> Option<CloseKeyAction> {
    match key {
        KeyCode::Char('\u{1b}') => Some(CloseKeyAction::Cancel),
        KeyCode::Char('\t')
            if !mods.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::SUPER) =>
        {
            Some(CloseKeyAction::Focus(!focused_confirm))
        }
        KeyCode::LeftArrow if mods.is_empty() => Some(CloseKeyAction::Focus(false)),
        KeyCode::RightArrow if mods.is_empty() => Some(CloseKeyAction::Focus(true)),
        KeyCode::Char('\r' | ' ')
            if !mods.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::SUPER) =>
        {
            Some(if focused_confirm {
                CloseKeyAction::Accept
            } else {
                CloseKeyAction::Cancel
            })
        }
        _ => None,
    }
}

struct CloseDialogLayout {
    card: super::termviai_dock::Rect,
    cancel: super::termviai_dock::Rect,
    accept: super::termviai_dock::Rect,
    padding: f32,
}

impl CloseDialogLayout {
    fn new(width: f32, height: f32, progress: f32) -> Self {
        use super::termviai_dock::Rect;
        let w = (width - 32.).clamp(1., 480.);
        let h = (height - 32.).clamp(1., 286.);
        let x = (width - w) / 2.;
        let y = (height - h) / 2. + 10. * (1. - progress);
        let padding = if h < 180. {
            12.
        } else if w >= 380. {
            24.
        } else {
            16.
        };
        let inner = (w - padding * 2. - 12.).max(2.);
        let accept_width = (inner * 0.58).min(152.);
        let cancel_width = (inner - accept_width).min(112.);
        let button_height = if h < 180. { 32. } else { 38. };
        let button_y = y + h - padding - button_height;
        Self {
            card: Rect { x, y, w, h },
            cancel: Rect {
                x: x + w - padding - accept_width - 12. - cancel_width,
                y: button_y,
                w: cancel_width,
                h: button_height,
            },
            accept: Rect {
                x: x + w - padding - accept_width,
                y: button_y,
                w: accept_width,
                h: button_height,
            },
            padding,
        }
    }

    fn hits(&self, width: f32, height: f32) -> Vec<Hit> {
        let mut hits = vec![Hit {
            x: 0.,
            y: 0.,
            w: width,
            h: height,
            action: Action::ConfirmDismiss,
        }];
        for (rect, action) in [
            (self.card, Action::ConfirmCard),
            (self.cancel, Action::ConfirmCancel),
            (self.accept, Action::ConfirmAccept),
        ] {
            hits.push(Hit {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
                action,
            });
        }
        hits
    }
}

fn dialog_lines(text: &str, columns: usize) -> Vec<String> {
    let mut lines = vec![];
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + word.chars().count() + 1 > columns {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn paint_close_dialog(
    tiles: &mut Vec<Tile>,
    hits: &mut Vec<Hit>,
    width: f32,
    height: f32,
    prompt: &super::termviai_confirm::ClosePrompt,
    progress: f32,
    focused_confirm: bool,
    hover: &Option<Action>,
) {
    let layout = CloseDialogLayout::new(width, height, progress);
    let r = layout.card;
    let p = layout.padding;
    tile(tiles, 0., 0., width, height, "", "#090a12", INK, false);
    let dim = tiles.last_mut().unwrap();
    dim.opacity = 0.64 * progress;
    dim.zindex = 70;
    // Soft inset shadow and a quiet outline separate the card from live sessions.
    for (spread, opacity) in [(10., 0.12), (5., 0.18)] {
        tile(
            tiles,
            r.x - spread,
            r.y + 6. - spread,
            r.w + spread * 2.,
            r.h + spread * 2.,
            "",
            "#080911",
            INK,
            true,
        );
        let t = tiles.last_mut().unwrap();
        t.opacity = opacity * progress;
        t.corner_radius = 20.;
        t.zindex = 71;
    }
    let start = tiles.len();
    tile(tiles, r.x, r.y, r.w, r.h, "", "#45445c", INK, true);
    tiles.last_mut().unwrap().corner_radius = 16.;
    tile(
        tiles,
        r.x + 1.,
        r.y + 1.,
        r.w - 2.,
        r.h - 2.,
        "",
        SIDE,
        INK,
        true,
    );
    tiles.last_mut().unwrap().corner_radius = 15.;
    if r.h >= 180. {
        tile(
            tiles,
            r.x + p,
            r.y + p,
            44.,
            44.,
            "",
            "#3b2b3c",
            "#f5a6b8",
            true,
        );
        let icon = tiles.last_mut().unwrap();
        icon.icon = Some(Icon::Power);
        icon.icon_size = 24.;
        icon.corner_radius = 12.;
        tile(
            tiles,
            r.x + p + 56.,
            r.y + p - 2.,
            r.w - p * 2. - 56.,
            28.,
            &prompt.title,
            SIDE,
            INK,
            false,
        );
    } else if r.h >= 90. {
        tile(
            tiles,
            r.x + p - 12.,
            r.y + p,
            r.w - p * 2. + 24.,
            24.,
            &prompt.title,
            SIDE,
            INK,
            false,
        );
    }
    if r.h >= 180. {
        tile(
            tiles,
            r.x + p + 56.,
            r.y + p + 25.,
            r.w - p * 2. - 56.,
            20.,
            "CONFIRM DISCONNECT",
            SIDE,
            MUTED,
            false,
        );
    }
    if r.h >= 210. {
        for (line, text) in
            dialog_lines(&prompt.detail, ((r.w - p * 2. - 24.) / 8.).max(1.) as usize)
                .into_iter()
                .take(2)
                .enumerate()
        {
            tile(
                tiles,
                r.x + p - 12.,
                r.y + 80. + line as f32 * 22.,
                r.w - p * 2. + 24.,
                22.,
                text,
                SIDE,
                "#b8bdd2",
                false,
            );
        }
    }
    if r.h >= 260. {
        tile(
            tiles,
            r.x + p,
            r.y + 138.,
            r.w - p * 2.,
            42.,
            &prompt.summary,
            "#2c2d41",
            INK,
            true,
        );
        tiles.last_mut().unwrap().icon = Some(Icon::Hosts);
    }
    if r.h >= 180. {
        tile(
            tiles,
            r.x + p,
            layout.cancel.y - 16.,
            r.w - p * 2.,
            1.,
            "",
            "#36374a",
            INK,
            false,
        );
    }
    let focus = if focused_confirm {
        layout.accept
    } else {
        layout.cancel
    };
    tile(
        tiles,
        focus.x - 2.,
        focus.y - 2.,
        focus.w + 4.,
        focus.h + 4.,
        "",
        ACCENT,
        INK,
        true,
    );
    button(
        tiles,
        hits,
        layout.cancel.x,
        layout.cancel.y,
        layout.cancel.w,
        layout.cancel.h,
        "Cancel",
        FIELD,
        Action::ConfirmCancel,
        hover,
    );
    button(
        tiles,
        hits,
        layout.accept.x,
        layout.accept.y,
        layout.accept.w,
        layout.accept.h,
        if r.w < 260. && prompt.confirm_label != "Closing…" {
            "Close"
        } else {
            &prompt.confirm_label
        },
        "#b65370",
        Action::ConfirmAccept,
        hover,
    );
    for t in &mut tiles[start..] {
        t.zindex = 72;
        t.opacity = progress;
    }
    *hits = layout.hits(width, height);
}

fn mix_color(
    a: window::color::LinearRgba,
    b: window::color::LinearRgba,
    amount: f32,
) -> window::color::LinearRgba {
    window::color::LinearRgba(
        a.0 + (b.0 - a.0) * amount,
        a.1 + (b.1 - a.1) * amount,
        a.2 + (b.2 - a.2) * amount,
        a.3 + (b.3 - a.3) * amount,
    )
}

fn color(s: &str) -> window::color::LinearRgba {
    s.parse::<window::color::SrgbaTuple>()
        .expect("valid TermViAI palette color")
        .to_linear()
}

impl TermWindow {
    pub(super) fn termviai_tab_strip(&self) -> Vec<(usize, super::termviai_dock::Rect)> {
        self.termviai_ui
            .hits
            .iter()
            .filter_map(|hit| {
                if let Action::Tab(id) = hit.action {
                    Some((
                        id,
                        super::termviai_dock::Rect {
                            x: hit.x,
                            y: hit.y,
                            w: hit.w,
                            h: hit.h,
                        },
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn termviai_sidebar_pixel_width(&self, dpi: f32) -> f32 {
        if self.config.termviai_ui {
            self.termviai_ui.sidebar_width(Instant::now()) * dpi / 96.
        } else {
            0.
        }
    }

    fn termviai_sync_sidebar_layout(&mut self, now: Instant) {
        if !self.config.termviai_ui {
            return;
        }
        let pixels = (self.termviai_ui.sidebar_width(now) * self.dimensions.dpi as f32 / 96.)
            .round() as usize;
        if self.termviai_ui.sidebar_layout_pixels != pixels {
            self.termviai_ui.sidebar_layout_pixels = pixels;
            if let Some(window) = self.window.clone() {
                self.apply_dimensions(&self.dimensions.clone(), None, &window, false);
            }
        }
    }

    pub(super) fn termviai_confirmation_input_active(&self) -> bool {
        self.config.termviai_ui
            && (self.termviai_confirm.is_active()
                || self.termviai_ui.confirm_keys.pending.is_some())
    }
    pub(super) fn termviai_input_generation(&self) -> u64 {
        self.termviai_ui.generation
    }
    pub fn termviai_library_active(&self) -> bool {
        self.config.termviai_ui && self.termviai_ui.page != Page::Terminal
    }

    pub(super) fn termviai_ui_input_active(&self) -> bool {
        self.config.termviai_ui
            && (self.termviai_confirmation_input_active()
                || self.termviai_library_active()
                || self.termviai_ui.form.is_some()
                || self.termviai_ui.closing_form.is_some()
                || self.termviai_ui.tab_menu.is_some()
                || self.termviai_ui.host_menu.is_some())
    }

    fn termviai_save_form(&mut self) -> anyhow::Result<()> {
        if let Some((size_value, keepalive_value)) = self
            .termviai_ui
            .form
            .as_ref()
            .filter(|form| form.settings)
            .map(|form| (form.fields[0].value.clone(), form.fields[1].value.clone()))
        {
            let size: f64 = size_value
                .trim()
                .parse()
                .context("Enter a font size in points.")?;
            let keepalive: u64 = keepalive_value
                .trim()
                .parse()
                .context("Enter the SSH keepalive interval in seconds.")?;
            self.termviai_set_preferences(size, keepalive)?;
            self.termviai_ui.close_form();
            return Ok(());
        }
        if let Some((id, name)) = self
            .termviai_ui
            .form
            .as_ref()
            .and_then(|f| f.rename_tab.map(|id| (id, f.fields[0].value.clone())))
        {
            self.termviai_rename_tab(id, &name)?;
            self.termviai_ui.close_form();
            Ok(())
        } else {
            self.termviai_ui.save_form()
        }
    }

    fn termviai_connect(&mut self, mut host: Host) -> anyhow::Result<()> {
        host.validate()?;
        self.termviai_open_workspace(SavedLayout::Pane { host }, None, None, None)?;
        self.termviai_ui.new_tab_open = false;
        self.termviai_ui.discard_form();
        Ok(())
    }

    pub(super) fn reset_termviai_interaction(&mut self) {
        self.cancel_termviai_tab_drag();
        self.termviai_ui.input_drag = None;
        self.termviai_ui.host_menu = None;
        self.termviai_ui.pressed = None;
        self.termviai_ui.hover = None;
        self.termviai_ui.tab_menu = None;
        self.termviai_ui.confirm_keys.pending = None;
    }

    pub(super) fn termviai_prepare_close_confirmation(&mut self) {
        self.reset_termviai_interaction();
        self.termviai_ui.generation += 1;
        self.window_drag_position = None;
        self.current_mouse_capture = None;
        self.current_mouse_buttons.clear();
        self.dragging = None;
        self.last_mouse_click = None;
        self.dead_key_status = window::DeadKeyStatus::None;
    }

    fn termviai_action(&mut self, action: Action) -> anyhow::Result<()> {
        self.termviai_ui.generation += 1;
        match action {
            Action::ToggleSidebar => {
                let now = Instant::now();
                self.termviai_ui.toggle_sidebar(now);
                self.termviai_sync_sidebar_layout(now);
            }
            Action::ConfirmAccept => self.termviai_accept_close()?,
            Action::ConfirmCancel | Action::ConfirmDismiss => self.termviai_cancel_close(),
            Action::ConfirmCard => {}
            Action::Hosts | Action::Keys => {
                self.termviai_ui.page = if action == Action::Hosts {
                    Page::Hosts
                } else {
                    Page::Keys
                };
                self.termviai_ui.discard_form();
                self.termviai_ui.group.clear();
                self.termviai_ui.scroll = 0;
                self.termviai_ui.query = Input::default();
                self.termviai_ui.search_focus = true;
                self.termviai_ui.reload();
            }
            Action::NewTab => {
                self.termviai_ui.discard_form();
                self.termviai_ui.new_tab_open = true;
                self.termviai_ui.page = Page::NewTab;
                self.termviai_ui.query = Input::default();
                self.termviai_ui.search_focus = true;
                self.termviai_ui.scroll = 0;
                self.termviai_ui.workspace.data = WorkspaceStore::load()?;
                self.termviai_ui.tab_scroll = usize::MAX;
            }
            Action::CloseNewTab => {
                self.termviai_ui.new_tab_open = false;
                self.termviai_ui.page = if Mux::get()
                    .get_active_tab_for_window(self.mux_window_id)
                    .is_some()
                {
                    Page::Terminal
                } else {
                    Page::Hosts
                };
            }
            Action::RenameTab(id) => {
                let tab = Mux::get().get_tab(id).context("This tab closed.")?;
                let name = self.termviai_tab_label(&tab);
                self.termviai_ui.open_rename(id, name);
            }
            Action::SaveWorkspace(id) => {
                self.termviai_ui.tab_menu = None;
                self.termviai_save_workspace(id)?;
            }
            Action::DismissMenu => {
                self.termviai_ui.tab_menu = None;
                self.termviai_ui.host_menu = None;
            }
            Action::EditMenuHost => {
                if let Some((host, _, _)) = self.termviai_ui.host_menu.take() {
                    self.termviai_ui.open_form(false, Some(host));
                }
            }
            Action::DuplicateHost => {
                if let Some((host, _, _)) = self.termviai_ui.host_menu.take() {
                    let mut store = HostStore::load()?;
                    store.duplicate_host(&host)?;
                    store.save()?;
                    self.termviai_ui.store = store;
                }
            }
            Action::OpenSaved(id) => {
                let group = self
                    .termviai_ui
                    .workspace
                    .data
                    .saved
                    .iter()
                    .find(|g| g.id == id)
                    .context("This workspace changed. Reopen New Tab.")?
                    .clone();
                self.termviai_open_workspace(group.layout, Some(group.name), Some(id), None)?;
                self.termviai_ui.new_tab_open = false;
            }
            Action::ConnectRecent(index) => {
                let host = self
                    .termviai_ui
                    .workspace
                    .data
                    .recent
                    .get(index)
                    .context("This recent connection changed.")?
                    .host
                    .clone();
                self.termviai_connect(host)?;
            }
            Action::ToggleTray => {
                self.termviai_ui.tray_expanded = !self.termviai_ui.tray_expanded;
                self.termviai_ui.tray_motion.set_target(
                    if self.termviai_ui.tray_expanded {
                        1.
                    } else {
                        0.
                    },
                    Instant::now(),
                    Duration::from_millis(200),
                );
                self.termviai_sync_terminal_layout();
            }
            Action::PrevMember => {
                self.termviai_ui.pane_scroll = self.termviai_ui.pane_scroll.saturating_sub(1)
            }
            Action::NextMember => self.termviai_ui.pane_scroll += 1,
            Action::TabBroadcast(id) => {
                self.termviai_sync_broadcast();
                self.termviai_broadcast.toggle_group(id);
                self.termviai_ui.error.clear();
            }
            Action::Settings => {
                let size = self.termviai_global_font_size();
                let keepalive = self.termviai_ssh_keepalive_interval();
                self.termviai_ui.open_settings(size, keepalive);
            }
            Action::AddHost => self.termviai_ui.open_form(false, None),
            Action::AddKey => self.termviai_ui.open_form(true, None),
            Action::AddIdentity => self.termviai_ui.open_identity(None),
            Action::EditIdentity(index) => {
                let identity = self
                    .termviai_ui
                    .store
                    .identities
                    .get(index)
                    .context("Identity no longer exists")?
                    .clone();
                self.termviai_ui.open_identity(Some(identity));
            }
            Action::ChooseAuth => {
                if let Some(f) = &mut self.termviai_ui.form {
                    f.auth.picker = !f.auth.picker;
                    f.auth.picker_scroll = 0;
                }
            }
            Action::ManualAuth => {
                if let Some(f) = &mut self.termviai_ui.form {
                    f.auth.selected_identity.clear();
                    f.auth.picker = false;
                }
            }
            Action::UseIdentity(index) => {
                let identity = self
                    .termviai_ui
                    .store
                    .identities
                    .get(index)
                    .context("Identity no longer exists")?
                    .clone();
                if let Some(f) = &mut self.termviai_ui.form {
                    f.auth.selected_identity = identity.id;
                    f.fields[3] = Input::new(identity.username);
                    f.fields[5] = Input::default();
                    f.fields[4] = Input::default();
                    f.auth.picker = false;
                    f.auth.reveal = false;
                }
            }
            Action::RememberPassword => {
                if let Some(f) = &mut self.termviai_ui.form {
                    f.auth.remember = !f.auth.remember;
                }
            }
            Action::RevealPassword => {
                if let Some(f) = &mut self.termviai_ui.form {
                    f.auth.reveal = !f.auth.reveal;
                }
            }
            Action::DeleteIdentity => {
                let f = self.termviai_ui.form.as_mut().context("No identity open")?;
                if !f.auth.confirm_delete {
                    f.auth.confirm_delete = true;
                } else if let Some(original) = &f.auth.original_identity {
                    let mut store = HostStore::load()?;
                    anyhow::ensure!(
                        !store.hosts.iter().any(|h| h.identity_id == original.id),
                        "This identity is used by a Host. Change that host's authentication first."
                    );
                    let index = store
                        .identities
                        .iter()
                        .position(|i| i == original)
                        .context("This identity changed. Reopen it.")?;
                    store.identities.remove(index);
                    store.save()?;
                    wezterm_ssh::credentials::delete(&original.id)?;
                    self.termviai_ui.store = store;
                    self.termviai_ui.close_form();
                }
            }

            Action::Cancel => {
                if let Some(f) = &mut self.termviai_ui.form {
                    if f.auth.picker {
                        f.auth.picker = false;
                        return Ok(());
                    }
                }
                self.termviai_ui.close_form();
                self.termviai_ui.error.clear();
            }
            Action::Save => {
                if self
                    .termviai_ui
                    .form
                    .as_ref()
                    .is_some_and(|f| f.auth.picker)
                {
                    self.termviai_ui.form.as_mut().unwrap().auth.picker = false;
                } else {
                    self.termviai_save_form()?;
                }
            }
            Action::HostMenu(i) => self.termviai_ui.open_host_menu(i)?,
            Action::Connect(i) => {
                let h = self
                    .termviai_ui
                    .store
                    .hosts
                    .get(i)
                    .context("Host no longer exists")?
                    .clone();
                self.termviai_connect(h)?;
            }
            Action::QuickConnect => {
                let h = Host::quick_connect(&self.termviai_ui.query.value)?;
                self.termviai_connect(h)?;
            }
            Action::Field(i) => {
                if let Some(f) = &mut self.termviai_ui.form {
                    f.focus = i;
                    f.reveal_focus = true;
                }
            }
            Action::Search => self.termviai_ui.search_focus = true,
            Action::Group(g) => {
                self.termviai_ui.group = g;
                self.termviai_ui.scroll = 0;
            }
            Action::UseKey(i) => {
                if let Some(f) = &mut self.termviai_ui.form {
                    if f.is_host() {
                        f.auth.selected_identity.clear();
                        f.auth.picker = false;
                        f.fields[4] = Input::default();
                        f.auth.remember = false;
                        f.auth.reset_password = true;
                        f.fields[5] = Input::new(self.termviai_ui.store.keys[i].path.clone());
                        f.focus = 5;
                        f.reveal_focus = true;
                    }
                }
            }
            Action::Tab(id) | Action::CloseTab(id) => {
                let mux = Mux::get();
                let index = mux
                    .get_window(self.mux_window_id)
                    .and_then(|w| w.iter_tabs().position(|t| t.tab_id() == id));
                if let Some(i) = index {
                    if matches!(action, Action::CloseTab(_)) {
                        self.termviai_ui.tab_menu = None;
                        self.close_specific_tab(i, true);
                    } else {
                        self.termviai_ui.page = Page::Terminal;
                        self.termviai_ui.discard_form();
                        self.activate_tab(i as isize)?;
                    }
                }
            }
            Action::PrevTab => {
                self.termviai_ui.tab_scroll = self.termviai_ui.tab_scroll.saturating_sub(1)
            }
            Action::NextTab => self.termviai_ui.tab_scroll += 1,
            Action::PrevGroup => {
                self.termviai_ui.group_scroll = self.termviai_ui.group_scroll.saturating_sub(1)
            }
            Action::NextGroup => self.termviai_ui.group_scroll += 1,
            Action::Minimize => {
                if let Some(w) = &self.window {
                    w.hide();
                }
            }
            Action::Maximize => {
                if let Some(w) = &self.window {
                    if self.window_state.contains(WindowState::MAXIMIZED) {
                        w.restore();
                    } else {
                        w.maximize();
                    }
                }
            }
            Action::Close => {
                if let Some(w) = self.window.clone() {
                    self.close_requested(&w);
                }
            }
            Action::Broadcast
            | Action::BroadcastAll
            | Action::BroadcastNone
            | Action::Member(_)
            | Action::PaneBroadcast(_) => {
                self.termviai_sync_broadcast();
                let mux = Mux::get();
                let tab = mux
                    .get_active_tab_for_window(self.mux_window_id)
                    .context("No active tab")?;
                if let Some(state) = self.termviai_broadcast.tabs.get_mut(&tab.tab_id()) {
                    match action {
                        Action::Broadcast => state.toggle_enabled(),
                        Action::BroadcastAll => state.select_all(),
                        Action::BroadcastNone => state.select_none(),
                        Action::Member(id) | Action::PaneBroadcast(id) => {
                            state.toggle_member(id)?
                        }
                        _ => unreachable!(),
                    }
                }
                self.termviai_ui.error.clear();
            }
            Action::FocusPane(id) => {
                let mux = Mux::get();
                let tab = mux
                    .get_active_tab_for_window(self.mux_window_id)
                    .context("No active tab")?;
                if tab.contains_pane(id) {
                    if let Some(pane) = mux.get_pane(id) {
                        tab.set_zoomed(false);
                        tab.set_active_pane(&pane);
                    }
                }
            }
            Action::ReconnectPane(id) => self.termviai_reconnect_pane(id),
            Action::ReconnectGroup(id) => self.termviai_reconnect_group(id),
            Action::RemovePane(id) => self.close_specific_pane(id, true),
            Action::PaneError(id) => {
                self.termviai_broadcast.pane_errors.remove(&id);
                self.termviai_ui.workspace.reconnect_errors.remove(&id);
                if let Some((_, _, tab)) = Mux::get().resolve_pane_id(id) {
                    self.termviai_ui.terminal_errors.remove(&tab);
                }
            }
            Action::Drag | Action::Drawer => {}
        }
        self.update_title();
        Ok(())
    }

    pub fn termviai_mouse(&mut self, event: &MouseEvent, context: &dyn WindowOps) -> bool {
        if self.config.termviai_ui && self.termviai_confirm.is_active() {
            context.set_cursor(Some(CursorIcon::Default));
            if self.termviai_confirm.closing {
                return true;
            }
            let scale = self.dimensions.dpi as f32 / 96.;
            let width = self.dimensions.pixel_width as f32 / scale;
            let height = (self.dimensions.pixel_height as f32
                - self.get_os_border().top.get() as f32)
                / scale;
            let x = event.coords.x as f32 / scale;
            let y = (event.coords.y as f32 - self.get_os_border().top.get() as f32) / scale;
            // Recompute instead of trusting hits from the previous animation frame.
            let layout = CloseDialogLayout::new(
                width,
                height,
                self.termviai_confirm.motion.value(Instant::now()),
            );
            let over = hit_action(&layout.hits(width, height), x, y);
            match event.kind {
                MouseEventKind::Move => {
                    if self.termviai_ui.hover != over {
                        self.termviai_ui.hover = over.clone();
                        context.invalidate();
                    }
                    if matches!(over, Some(Action::ConfirmCancel | Action::ConfirmAccept)) {
                        context.set_cursor(Some(CursorIcon::Pointer));
                    }
                }
                MouseEventKind::Press(MousePress::Left) => {
                    if matches!(over, Some(Action::ConfirmCancel | Action::ConfirmAccept)) {
                        self.termviai_confirm.focused_confirm = over == Some(Action::ConfirmAccept);
                    }
                    self.termviai_ui.pressed = over;
                    context.invalidate();
                }
                MouseEventKind::Release(MousePress::Left) => {
                    let pressed = self.termviai_ui.pressed.take();
                    if pressed == over {
                        if let Some(action) = over {
                            if let Err(error) = self.termviai_action(action) {
                                self.termviai_ui.error = format!("{error:#}");
                            }
                        }
                    }
                    context.invalidate();
                }
                _ => {}
            }
            return true;
        }
        // An existing terminal selection or UI drag owns its release event,
        // even when the pointer moves across the sidebar or application tabs.
        if self.window_drag_position.is_some()
            || (!self.termviai_library_active()
                && (self.current_mouse_capture.is_some() || self.dragging.is_some()))
        {
            return false;
        }
        if !self.config.termviai_ui || self.get_modal().is_some() {
            if self.termviai_ui.tab_drag.is_some() {
                self.reset_termviai_interaction();
            }
            return false;
        }
        let scale = self.dimensions.dpi as f32 / 96.;
        let x = event.coords.x as f32 / scale;
        let y = (event.coords.y as f32 - self.get_os_border().top.get() as f32) / scale;
        let over = hit_action(&self.termviai_ui.hits, x, y);
        let capture = self.termviai_ui_input_active()
            || over.is_some()
            || x < self.termviai_ui.sidebar_width(Instant::now())
            || y < 52.
            || self.termviai_ui.pressed.is_some()
            || self.termviai_ui.input_drag.is_some()
            || self.termviai_ui.tab_drag.is_some();
        if !capture {
            if matches!(event.kind, MouseEventKind::Move) && self.termviai_ui.hover.take().is_some()
            {
                context.invalidate();
            }
            return false;
        }
        self.current_mouse_event = Some(event.clone());
        if matches!(
            event.kind,
            MouseEventKind::Move | MouseEventKind::Release(MousePress::Left)
        ) {
            if let Some(action) = self.termviai_ui.input_drag.clone() {
                if event.mouse_buttons.contains(window::MouseButtons::LEFT)
                    || matches!(event.kind, MouseEventKind::Release(MousePress::Left))
                {
                    let position = self
                        .termviai_ui
                        .input_hits
                        .iter()
                        .find(|hit| hit.action == action)
                        .map(|hit| {
                            let position = hit.position(x);
                            if x > hit.x + hit.positions.last().copied().unwrap_or(0.) + 4. {
                                position + 1
                            } else if x < hit.x - 4. {
                                position.saturating_sub(1)
                            } else {
                                position
                            }
                        });
                    if let (Some(position), Some(input)) = (position, self.termviai_ui.input()) {
                        input.move_to(position, true);
                        self.termviai_ui.generation += 1;
                    }
                }
                if !event.mouse_buttons.contains(window::MouseButtons::LEFT)
                    || matches!(event.kind, MouseEventKind::Release(MousePress::Left))
                {
                    self.termviai_ui.input_drag = None;
                }
                context.set_cursor(Some(CursorIcon::Text));
                context.invalidate();
                return true;
            }
        }
        if matches!(event.kind, MouseEventKind::Press(MousePress::Left))
            && matches!(over, Some(Action::Field(_) | Action::Search))
        {
            let action = over.clone().unwrap();
            let position = self
                .termviai_ui
                .input_hits
                .iter()
                .find(|hit| hit.action == action)
                .map(|hit| hit.position(x))
                .unwrap_or(0);
            let _ = self.termviai_action(action.clone());
            if let Some(input) = self.termviai_ui.input() {
                input.move_to(position, event.modifiers.contains(Modifiers::SHIFT));
                input.anchor.get_or_insert(input.cursor);
            }
            self.termviai_ui.input_drag = Some(action);
            self.termviai_ui.pressed = None;
            context.set_cursor(Some(CursorIcon::Text));
            context.invalidate();
            return true;
        }
        match event.kind {
            MouseEventKind::Move => {
                if self.termviai_ui.tab_drag.is_some() {
                    if !event.mouse_buttons.contains(window::MouseButtons::LEFT) {
                        self.reset_termviai_interaction();
                        context.set_cursor(Some(CursorIcon::Default));
                    } else {
                        self.update_termviai_tab_drag(x, y);
                        context.set_cursor(Some(CursorIcon::Grabbing));
                    }
                    return true;
                }
                // Windows performs caption hit testing before the next button press.
                if over == Some(Action::Drag) {
                    context.set_window_drag_position(event.screen_coords);
                }
                if self.termviai_ui.hover != over {
                    self.termviai_ui.hover = over.clone();
                    self.termviai_ui.hover_started = Instant::now();
                    context.invalidate();
                }
                context.set_cursor(Some(
                    if matches!(over, Some(Action::Field(_) | Action::Search)) {
                        CursorIcon::Text
                    } else if over.is_some()
                        && !matches!(over, Some(Action::Drag | Action::Drawer | Action::Cancel))
                    {
                        CursorIcon::Pointer
                    } else {
                        CursorIcon::Default
                    },
                ));
            }
            MouseEventKind::Press(MousePress::Right) => {
                if let Some(
                    Action::Tab(id)
                    | Action::CloseTab(id)
                    | Action::TabBroadcast(id)
                    | Action::ReconnectGroup(id),
                ) = over
                {
                    self.termviai_ui.tab_menu = Some((id, x, y));
                    self.termviai_ui.pressed = None;
                    self.termviai_ui.generation += 1;
                    context.invalidate();
                }
            }
            MouseEventKind::Press(MousePress::Left) => {
                if over == Some(Action::Drag) {
                    if !self
                        .window_state
                        .intersects(WindowState::MAXIMIZED | WindowState::FULL_SCREEN)
                    {
                        self.window_drag_position = Some(event.clone());
                    }
                    context.request_drag_move();
                } else {
                    if let Some(Action::Tab(id)) = &over {
                        self.begin_termviai_tab_drag(*id, x, y);
                    } else if let Some(Action::FocusPane(id)) = &over {
                        self.begin_termviai_pane_drag(*id, x, y);
                    }
                    self.termviai_ui.pressed = over;
                    context.invalidate();
                }
            }
            MouseEventKind::Release(MousePress::Left) => {
                if self.termviai_ui.tab_drag.is_some() {
                    context.set_cursor(Some(if matches!(over, Some(Action::Tab(_))) {
                        CursorIcon::Pointer
                    } else {
                        CursorIcon::Default
                    }));
                    self.update_termviai_tab_drag(x, y);
                    match self.finish_termviai_tab_drag() {
                        Ok(false) => {}
                        result => {
                            if let Err(error) = result {
                                self.termviai_ui.error = format!("{error:#}");
                            }
                            self.termviai_ui.pressed = None;
                            self.termviai_ui.hover = None;
                            context.invalidate();
                            return true;
                        }
                    }
                }
                let pressed = self.termviai_ui.pressed.take();
                if pressed == over {
                    if let Some(a) = over {
                        if let Err(e) = self.termviai_action(a) {
                            self.termviai_ui.error = format!("{e:#}");
                        }
                    }
                }
                context.invalidate();
            }
            MouseEventKind::VertWheel(amount) => {
                if self.termviai_ui.tab_drag.is_some() && y < 52. {
                    let step = amount.unsigned_abs().max(1) as usize;
                    self.termviai_ui.tab_scroll = if amount > 0 {
                        self.termviai_ui.tab_scroll.saturating_sub(step)
                    } else {
                        self.termviai_ui.tab_scroll.saturating_add(step)
                    };
                    context.invalidate();
                    return true;
                }
                if self.termviai_ui.closing_form.is_some() {
                    return true;
                }
                if self.termviai_ui.page == Page::Terminal
                    && self.termviai_ui.form.is_none()
                    && self.termviai_ui.broadcast_tray_height() > 0.
                    && y >= (self.dimensions.pixel_height as f32 / scale
                        - self.termviai_ui.broadcast_tray_height())
                {
                    let step = amount.unsigned_abs().max(1) as usize;
                    self.termviai_ui.pane_scroll = if amount > 0 {
                        self.termviai_ui.pane_scroll.saturating_sub(step)
                    } else {
                        self.termviai_ui.pane_scroll.saturating_add(step)
                    };
                    context.invalidate();
                    return true;
                }
                let layout = DrawerLayout::new(
                    self.dimensions.pixel_width as f32 / scale,
                    (self.dimensions.pixel_height as f32 - self.get_os_border().top.get() as f32)
                        / scale,
                );
                if let Some(form) = &mut self.termviai_ui.form {
                    if form.auth.picker {
                        form.auth.picker_scroll = if amount > 0 {
                            form.auth
                                .picker_scroll
                                .saturating_sub(amount.unsigned_abs() as usize)
                        } else {
                            form.auth
                                .picker_scroll
                                .saturating_add(amount.unsigned_abs() as usize)
                        };
                        context.invalidate();
                        return true;
                    }

                    if x >= layout.x
                        + (1. - self.termviai_ui.drawer_motion.value(Instant::now())) * layout.width
                        && y >= 148.
                        && y < layout.footer
                    {
                        let step = amount.unsigned_abs().max(1) as usize;
                        form.first_row = if amount > 0 {
                            form.first_row.saturating_sub(step)
                        } else {
                            (form.first_row + step)
                                .min(form.fields.len().saturating_sub(layout.rows))
                        };
                        form.reveal_focus = false;
                        context.invalidate();
                    }
                } else if self.termviai_library_active() {
                    let step = amount.unsigned_abs().max(1) as usize;
                    self.termviai_ui.scroll = if amount > 0 {
                        self.termviai_ui.scroll.saturating_sub(step)
                    } else {
                        (self.termviai_ui.scroll + step).min(self.termviai_ui.max_scroll)
                    };
                    context.invalidate();
                }
            }
            _ => {}
        }
        true
    }

    pub fn termviai_key(&mut self, event: &KeyEvent, context: &dyn WindowOps) -> bool {
        if self.termviai_confirmation_input_active() {
            let action = self.termviai_ui.confirm_keys.event(
                &event.key,
                event.modifiers,
                event.key_is_down,
                self.termviai_confirm.focused_confirm,
            );
            // Execute on release: key repeats and the matching release never
            // reach a restored SSH session or an editor behind the dialog.
            if self.termviai_confirm.closing || !self.termviai_confirm.is_active() {
                return true;
            }
            match action {
                Some(CloseKeyAction::Cancel) => self.termviai_cancel_close(),
                Some(CloseKeyAction::Focus(focus)) => self.termviai_confirm.focused_confirm = focus,
                Some(CloseKeyAction::Accept) => {
                    if let Err(error) = self.termviai_accept_close() {
                        self.termviai_ui.error = format!("{error:#}");
                    }
                }
                None => {}
            }
            context.invalidate();
            return true;
        }
        if !self.config.termviai_ui || self.get_modal().is_some() {
            if self.termviai_ui.tab_drag.is_some() {
                self.reset_termviai_interaction();
            }
            return false;
        }
        if self.termviai_ui.tab_drag.is_some() {
            if event.key_is_down && event.key == KeyCode::Char('\u{1b}') {
                self.reset_termviai_interaction();
                context.invalidate();
            }
            return true;
        }
        if event.key_is_down
            && event.modifiers.contains(Modifiers::CTRL | Modifiers::SHIFT)
            && matches!(event.key, KeyCode::Char('h' | 'H'))
        {
            if let Err(e) = self.termviai_action(Action::Hosts) {
                self.termviai_ui.error = e.to_string();
            }
            context.invalidate();
            return true;
        }
        if self.termviai_ui.tab_menu.is_some() || self.termviai_ui.host_menu.is_some() {
            if event.key_is_down && event.key == KeyCode::Char('\u{1b}') {
                self.termviai_ui.tab_menu = None;
                self.termviai_ui.host_menu = None;
                context.invalidate();
            }
            return true;
        }
        if self.termviai_ui.page == Page::Terminal
            && self.termviai_ui.form.is_none()
            && event.modifiers.contains(Modifiers::CTRL)
            && matches!(event.key, KeyCode::Char('s' | 'S'))
        {
            if event.key_is_down {
                if let Some(tab) = Mux::get().get_active_tab_for_window(self.mux_window_id) {
                    if let Err(error) = self.termviai_save_workspace(tab.tab_id()) {
                        self.termviai_ui.error = format!("{error:#}");
                    }
                }
                context.invalidate();
            }
            return true;
        }
        if !self.termviai_ui_input_active() {
            return false;
        }
        if !event.key_is_down || self.termviai_ui.closing_form.is_some() {
            return true;
        }
        let ctrl = event.modifiers.contains(Modifiers::CTRL);
        if ctrl && matches!(event.key, KeyCode::Char('c' | 'C' | 'x' | 'X')) {
            if self
                .termviai_ui
                .form
                .as_ref()
                .is_some_and(Form::masked_password)
            {
                return true;
            }
            if let Some(text) = self
                .termviai_ui
                .input()
                .and_then(|input| input.selected_text())
            {
                if let Some(window) = &self.window {
                    window.set_clipboard(Clipboard::Clipboard, text);
                    if matches!(event.key, KeyCode::Char('x' | 'X')) {
                        if let Some(input) = self.termviai_ui.input() {
                            input.remove_selection();
                        }
                        self.termviai_ui.generation += 1;
                    }
                }
            }
            context.invalidate();
            return true;
        }
        if ctrl && matches!(event.key, KeyCode::Char('v' | 'V')) {
            let generation = self.termviai_ui.generation;
            if let Some(w) = self.window.clone() {
                let future = w.get_clipboard(Clipboard::Clipboard);
                promise::spawn::spawn(async move {
                    if let Ok(text) = future.await {
                        w.notify(TermWindowNotif::Apply(Box::new(move |tw| {
                            if tw.termviai_ui_input_active()
                                && tw.termviai_ui.generation == generation
                            {
                                if let Some(input) = tw.termviai_ui.input() {
                                    input.insert(&text);
                                }
                                tw.termviai_ui.scroll = 0;
                                if let Some(w) = &tw.window {
                                    w.invalidate();
                                }
                            }
                        })));
                    }
                })
                .detach();
            }
            return true;
        }
        if let Some(f) = &mut self.termviai_ui.form {
            if f.auth.picker {
                if event.key == KeyCode::Char('\u{1b}') {
                    f.auth.picker = false;
                    context.invalidate();
                }
                return true;
            }
        }
        self.termviai_ui.generation += 1;
        match &event.key {
            KeyCode::Char('\u{1b}') => {
                if self.termviai_ui.form.is_some() {
                    self.termviai_ui.close_form();
                    self.termviai_ui.error.clear();
                } else {
                    self.termviai_ui.page = if Mux::get()
                        .get_active_tab_for_window(self.mux_window_id)
                        .is_some()
                    {
                        Page::Terminal
                    } else {
                        Page::Hosts
                    };
                }
            }
            KeyCode::Char('\r') => {
                let result = if self.termviai_ui.form.is_some() {
                    self.termviai_save_form()
                } else if self.termviai_ui.page == Page::NewTab {
                    let action = self
                        .termviai_ui
                        .hits
                        .iter()
                        .find(|h| {
                            matches!(h.action, Action::OpenSaved(_) | Action::ConnectRecent(_))
                        })
                        .map(|h| h.action.clone())
                        .unwrap_or(Action::QuickConnect);
                    self.termviai_action(action)
                } else if self.termviai_ui.page == Page::Hosts {
                    self.termviai_action(Action::QuickConnect)
                } else {
                    Ok(())
                };
                if let Err(e) = result {
                    self.termviai_ui.error = format!("{e:#}");
                }
            }
            KeyCode::Char('\t') => {
                if let Some(f) = &mut self.termviai_ui.form {
                    let n = f.fields.len();
                    f.focus = if event.modifiers.contains(Modifiers::SHIFT) {
                        (f.focus + n - 1) % n
                    } else {
                        (f.focus + 1) % n
                    };
                    if !f.auth.selected_identity.is_empty() {
                        while matches!(f.focus, 3 | 4 | 5) {
                            f.focus = if event.modifiers.contains(Modifiers::SHIFT) {
                                (f.focus + n - 1) % n
                            } else {
                                (f.focus + 1) % n
                            };
                        }
                    }
                    f.reveal_focus = true;
                }
            }
            KeyCode::Char('f' | 'F') if ctrl && self.termviai_ui.form.is_none() => {
                self.termviai_ui.search_focus = true;
                self.termviai_ui.query.select_all();
            }
            KeyCode::Char('a' | 'A') if ctrl => {
                if let Some(i) = self.termviai_ui.input() {
                    i.select_all();
                }
            }
            KeyCode::Char('\u{8}') => {
                if let Some(i) = self.termviai_ui.input() {
                    i.delete(true);
                }
            }
            KeyCode::Char('\u{7f}') => {
                if let Some(i) = self.termviai_ui.input() {
                    i.delete(false);
                }
            }
            KeyCode::LeftArrow => {
                if let Some(i) = self.termviai_ui.input() {
                    let extend = event.modifiers.contains(Modifiers::SHIFT);
                    let position = if !extend {
                        i.selection()
                            .map(|r| r.start)
                            .unwrap_or(i.cursor.saturating_sub(1))
                    } else {
                        i.cursor.saturating_sub(1)
                    };
                    i.move_to(position, extend);
                }
            }
            KeyCode::RightArrow => {
                if let Some(i) = self.termviai_ui.input() {
                    let extend = event.modifiers.contains(Modifiers::SHIFT);
                    let position = if !extend {
                        i.selection().map(|r| r.end).unwrap_or(i.cursor + 1)
                    } else {
                        i.cursor + 1
                    };
                    i.move_to(position, extend);
                }
            }
            KeyCode::Home => {
                if let Some(i) = self.termviai_ui.input() {
                    i.move_to(0, event.modifiers.contains(Modifiers::SHIFT));
                }
            }
            KeyCode::End => {
                if let Some(i) = self.termviai_ui.input() {
                    i.move_to(
                        i.value.chars().count(),
                        event.modifiers.contains(Modifiers::SHIFT),
                    );
                }
            }
            KeyCode::Char(c)
                if !ctrl && !event.modifiers.contains(Modifiers::ALT) && !c.is_control() =>
            {
                if let Some(i) = self.termviai_ui.input() {
                    i.insert(&c.to_string());
                }
            }
            KeyCode::Composed(text) => {
                if let Some(i) = self.termviai_ui.input() {
                    i.insert(text);
                }
            }
            _ => {}
        }
        self.termviai_ui.scroll = 0;
        context.invalidate();
        true
    }

    pub fn paint_termviai(&mut self) -> anyhow::Result<()> {
        if !self.config.termviai_ui {
            return Ok(());
        }
        let now = Instant::now();
        self.termviai_sync_sidebar_layout(now);
        let was_confirming = self.termviai_confirm.is_active();
        self.termviai_confirm.settle(now);
        if was_confirming && !self.termviai_confirm.is_active() {
            self.termviai_ui.pressed = None;
            self.termviai_ui.hover = None;
        }
        let confirmation = self.termviai_confirm.prompt.clone();
        let confirmation_progress = self.termviai_confirm.motion.value(now);
        let confirmation_focus = self.termviai_confirm.focused_confirm;
        let scale = self.dimensions.dpi as f32 / 96.;
        let ui_font = self.fonts.title_font()?;
        let w = self.dimensions.pixel_width as f32 / scale;
        let border = self.get_os_border();
        let top = border.top.get() as f32;
        let h = (self.dimensions.pixel_height as f32 - top) / scale;
        self.termviai_sync_broadcast();
        let active_tab = Mux::get().get_active_tab_for_window(self.mux_window_id);
        let pane_list = active_tab
            .as_ref()
            .map(|tab| tab.iter_panes_ignoring_zoom())
            .unwrap_or_default();
        let disconnected_panes: HashSet<usize> = pane_list
            .iter()
            .filter_map(|positioned| {
                positioned
                    .pane
                    .downcast_ref::<LocalPane>()
                    .is_some_and(LocalPane::is_reconnectable_ssh)
                    .then_some(positioned.pane.pane_id())
            })
            .collect();
        if active_tab.is_none()
            && self.termviai_ui.page == Page::Terminal
            && !self.termviai_ui.connecting
        {
            self.termviai_ui.page = Page::Hosts;
        }
        let headers = self.termviai_pane_headers();
        let labels: HashMap<usize, String> = pane_list
            .iter()
            .map(|p| (p.pane.pane_id(), self.termviai_pane_label(p.pane.pane_id())))
            .collect();
        let mux = Mux::get();
        let current_tabs = mux
            .get_window(self.mux_window_id)
            .map(|win| win.iter_tabs().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let active_id = active_tab.as_ref().map(|t| t.tab_id());
        let disconnected_groups: HashSet<usize> = current_tabs
            .iter()
            .filter_map(|tab| {
                super::termviai_workspace::group_needs_reconnect(
                    tab.iter_panes_ignoring_zoom().iter().map(|p| {
                        p.pane
                            .downcast_ref::<LocalPane>()
                            .is_some_and(LocalPane::is_reconnectable_ssh)
                    }),
                )
                .then_some(tab.tab_id())
            })
            .collect();
        self.termviai_ui
            .workspace
            .reconnect_errors
            .retain(|id, _| mux.get_pane(*id).is_some());
        let mut tabs: Vec<(usize, String, bool, usize)> = current_tabs
            .iter()
            .map(|t| {
                (
                    t.tab_id(),
                    self.termviai_tab_label(t),
                    active_id == Some(t.tab_id()),
                    t.iter_panes_ignoring_zoom().len(),
                )
            })
            .collect();
        if self.termviai_ui.new_tab_open {
            tabs.push((
                usize::MAX,
                "New Tab".into(),
                self.termviai_ui.page == Page::NewTab,
                0,
            ));
        }
        let group_states: HashMap<usize, bool> = self
            .termviai_broadcast
            .tabs
            .iter()
            .map(|(id, state)| (*id, group_broadcasting(state)))
            .collect();
        let broadcast = active_tab
            .as_ref()
            .and_then(|tab| self.termviai_broadcast.tabs.get(&tab.tab_id()))
            .cloned();
        // Store terminal notices with their owning tab, never in a global banner.
        self.termviai_ui
            .terminal_errors
            .retain(|id, _| current_tabs.iter().any(|t| t.tab_id() == *id));
        if self.termviai_ui.page == Page::Terminal
            && self.termviai_ui.form.is_none()
            && !self.termviai_ui.error.is_empty()
        {
            if let Some(id) = active_id {
                let error = std::mem::take(&mut self.termviai_ui.error);
                self.termviai_ui.terminal_errors.insert(id, error);
            }
        }
        let mut pane_errors = self.termviai_broadcast.pane_errors.clone();
        for (id, error) in &self.termviai_ui.workspace.reconnect_errors {
            pane_errors
                .entry(*id)
                .and_modify(|message| {
                    message.push('\n');
                    message.push_str(error);
                })
                .or_insert_with(|| error.clone());
        }
        if let Some(tab) = &active_tab {
            if let (Some(pane), Some(error)) = (
                tab.get_active_pane(),
                self.termviai_ui.terminal_errors.get(&tab.tab_id()),
            ) {
                pane_errors
                    .entry(pane.pane_id())
                    .and_modify(|message| {
                        message.push('\n');
                        message.push_str(error);
                    })
                    .or_insert_with(|| error.clone());
            }
        }
        let drag_visuals = self.termviai_tab_drag_visuals();
        let mut tiles = vec![];
        let mut hits = vec![];
        let ui = &mut self.termviai_ui;
        let content_left = ui.sidebar_width(now);
        let sidebar_shift = content_left - SIDEBAR_WIDTH;
        if ui.painted_page != ui.page {
            ui.painted_page = ui.page;
            ui.page_motion = Tween::new(0.);
            ui.page_motion
                .set_target(1., now, Duration::from_millis(160));
        }
        if !ui.drawer_motion.is_active(now) && ui.form.is_none() {
            ui.closing_form = None;
        }
        let drawer_progress = ui.drawer_motion.value(now);
        let page_progress = ui.page_motion.value(now);
        let hover = ui.hover.clone();
        tile(&mut tiles, 0., 0., w, 52., "", TOP, INK, false);
        hits.push(Hit {
            x: 0.,
            y: 0.,
            w,
            h: 52.,
            action: Action::Drag,
        });
        button(
            &mut tiles,
            &mut hits,
            14.,
            9.,
            38.,
            34.,
            "",
            TOP,
            Action::ToggleSidebar,
            &hover,
        );
        tile(&mut tiles, 58., 9., 146., 34., "TermViAI", TOP, INK, false);
        let available = (w - 220. - 170.).max(100.);
        let visible = ((available - 66.) / 190.).floor().max(1.) as usize;
        ui.tab_scroll = ui.tab_scroll.min(tabs.len().saturating_sub(visible));
        let mut tx = 220.;
        for (id, title, active, pane_count) in tabs.iter().skip(ui.tab_scroll).take(visible) {
            let tw = ((available - 66.) / visible as f32).min(210.).max(70.);
            let label = title.clone();
            button(
                &mut tiles,
                &mut hits,
                tx,
                8.,
                tw - 8.,
                36.,
                label,
                if *active && (ui.page == Page::Terminal || *id == usize::MAX) {
                    FIELD
                } else {
                    TOP
                },
                if *id == usize::MAX {
                    Action::NewTab
                } else {
                    Action::Tab(*id)
                },
                &hover,
            );
            if *pane_count > 1 {
                let reconnect = disconnected_groups.contains(id);
                tiles.last_mut().unwrap().right_inset = 62.;
                let t = tiles.last_mut().unwrap();
                t.icon = (tw >= 120.).then_some(Icon::Grid);
                if tw < 120. {
                    t.text.clear();
                }
                button(
                    &mut tiles,
                    &mut hits,
                    tx + tw - 66.,
                    12.,
                    26.,
                    28.,
                    "",
                    TOP,
                    if reconnect {
                        Action::ReconnectGroup(*id)
                    } else {
                        Action::TabBroadcast(*id)
                    },
                    &hover,
                );
                let t = tiles.last_mut().unwrap();
                let broadcasting = !reconnect && group_states.get(id) == Some(&true);
                t.transparent = true;
                t.fg = if reconnect {
                    RECONNECT
                } else if broadcasting {
                    SIGNAL_INK
                } else {
                    MUTED
                };
                // Match pane-header icons: 18 px also keeps a clear 2 px stroke at 100% DPI.
                t.icon_size = if reconnect { 16. } else { 18. };
                t.corner_radius = 7.;
            }
            button(
                &mut tiles,
                &mut hits,
                tx + tw - 36.,
                12.,
                24.,
                28.,
                "",
                TOP,
                if *id == usize::MAX {
                    Action::CloseNewTab
                } else {
                    Action::CloseTab(*id)
                },
                &hover,
            );
            let close = tiles.last_mut().unwrap();
            close.transparent = true;
            close.fg = MUTED;
            close.icon_size = 18.;
            tx += tw;
        }
        button(
            &mut tiles,
            &mut hits,
            tx,
            10.,
            32.,
            32.,
            "",
            TOP,
            Action::NewTab,
            &hover,
        );
        if tabs.len() > visible {
            button(
                &mut tiles,
                &mut hits,
                tx + 34.,
                10.,
                22.,
                32.,
                "",
                TOP,
                Action::PrevTab,
                &hover,
            );
            button(
                &mut tiles,
                &mut hits,
                tx + 58.,
                10.,
                22.,
                32.,
                "",
                TOP,
                Action::NextTab,
                &hover,
            );
        }
        for (x, label, a) in [
            (w - 126., "", Action::Minimize),
            (w - 84., "", Action::Maximize),
            (w - 42., "", Action::Close),
        ] {
            button(
                &mut tiles, &mut hits, x, 0., 42., 52., label, TOP, a, &hover,
            );
        }
        let sidebar_tile_start = tiles.len();
        let sidebar_hit_start = hits.len();
        tile(
            &mut tiles,
            0.,
            52.,
            SIDEBAR_WIDTH,
            h - 52.,
            "",
            SIDE,
            INK,
            false,
        );
        tile(
            &mut tiles,
            SIDEBAR_WIDTH - 1.,
            52.,
            1.,
            h - 52.,
            "",
            FIELD,
            INK,
            false,
        );
        button(
            &mut tiles,
            &mut hits,
            12.,
            66.,
            196.,
            40.,
            "Hosts",
            if ui.page == Page::Hosts { FIELD } else { SIDE },
            Action::Hosts,
            &hover,
        );
        button(
            &mut tiles,
            &mut hits,
            12.,
            114.,
            196.,
            40.,
            "Keychain",
            if ui.page == Page::Keys { FIELD } else { SIDE },
            Action::Keys,
            &hover,
        );
        button(
            &mut tiles,
            &mut hits,
            12.,
            162.,
            196.,
            40.,
            "Settings",
            SIDE,
            Action::Settings,
            &hover,
        );
        if ui.page != Page::Terminal {
            tile(
                &mut tiles,
                22.,
                h - 80.,
                180.,
                24.,
                "LOCAL WORKSPACE",
                SIDE,
                MUTED,
                false,
            );
            tile(
                &mut tiles,
                22.,
                h - 52.,
                180.,
                24.,
                "SSH · stored on this device",
                SIDE,
                MUTED,
                false,
            );
        }
        for tile in &mut tiles[sidebar_tile_start..] {
            tile.x += sidebar_shift;
        }
        for hit in &mut hits[sidebar_hit_start..] {
            hit.x += sidebar_shift;
        }
        if ui.page == Page::Terminal {
            if ui.connecting {
                tile(
                    &mut tiles,
                    content_left + 16.,
                    58.,
                    (w - content_left - 32.).max(1.),
                    32.,
                    "Opening SSH connections…",
                    CARD,
                    INK,
                    true,
                );
            }
            for header in &headers {
                let frame = header.frame;
                tile(
                    &mut tiles,
                    frame.x,
                    frame.y,
                    frame.w,
                    frame.h,
                    "",
                    if header.is_active {
                        "#8d86d7"
                    } else {
                        "#3b3e52"
                    },
                    INK,
                    true,
                );
                let t = tiles.last_mut().unwrap();
                t.outline_width = if header.is_active { 1.5 } else { 1. };
                t.corner_radius = 10.;
                t.zindex = 29;
                let rect = header.rect;
                button(
                    &mut tiles,
                    &mut hits,
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    labels
                        .get(&header.pane_id)
                        .cloned()
                        .unwrap_or_else(|| "Terminal".into()),
                    BG,
                    Action::FocusPane(header.pane_id),
                    &hover,
                );
                let t = tiles.last_mut().unwrap();
                let grouped = pane_list.len() > 1;
                let has_error = pane_errors.contains_key(&header.pane_id);
                let controls = pane_header_controls(rect.x, rect.w, grouped, has_error);
                t.right_inset = controls.label_right_inset;
                t.transparent = true;
                t.fg = if header.is_active { ACCENT } else { MUTED };
                t.icon = Some(Icon::Hosts);
                t.rounded = false;
                if rect.w < controls.label_right_inset + 52. || rect.h < 24. {
                    t.text.clear();
                    t.icon = None;
                }
                if rect.h >= 20. && rect.w >= 76. {
                    let disconnected = disconnected_panes.contains(&header.pane_id);
                    let member = broadcast
                        .as_ref()
                        .map(|s| s.is_member(header.pane_id))
                        .unwrap_or(false);
                    let broadcasting = !disconnected
                        && broadcast
                            .as_ref()
                            .map(|s| pane_broadcasting(s, header.pane_id))
                            .unwrap_or(false);
                    if disconnected {
                        button(
                            &mut tiles,
                            &mut hits,
                            controls.broadcast_x,
                            rect.y + (rect.h - 28.).max(0.) / 2.,
                            28.,
                            rect.h.min(28.),
                            "",
                            BG,
                            Action::ReconnectPane(header.pane_id),
                            &hover,
                        );
                        let t = tiles.last_mut().unwrap();
                        t.icon = Some(Icon::Refresh);
                        t.icon_size = 16.;
                        t.transparent = true;
                        t.fg = RECONNECT;
                        if ui.workspace.reconnecting.contains(&header.pane_id) {
                            t.opacity = 0.55;
                        }
                    } else {
                        button(
                            &mut tiles,
                            &mut hits,
                            controls.broadcast_x,
                            rect.y + (rect.h - 28.).max(0.) / 2.,
                            28.,
                            rect.h.min(28.),
                            if member { "member" } else { "solo" },
                            BG,
                            Action::PaneBroadcast(header.pane_id),
                            &hover,
                        );
                        let t = tiles.last_mut().unwrap();
                        t.icon = Some(Icon::Broadcast);
                        t.corner_radius = 7.;
                        t.transparent = true;
                        t.fg = if broadcasting { SIGNAL_INK } else { MUTED };
                    }
                    if let Some(remove_x) = controls.remove_x {
                        button(
                            &mut tiles,
                            &mut hits,
                            remove_x,
                            rect.y + (rect.h - 28.).max(0.) / 2.,
                            28.,
                            rect.h.min(28.),
                            "",
                            BG,
                            Action::RemovePane(header.pane_id),
                            &hover,
                        );
                        let t = tiles.last_mut().unwrap();
                        t.fg = MUTED;
                        t.transparent = true;
                    }
                }
                if has_error && rect.w >= 76. && rect.h >= 20. {
                    // Keep the alert in the pane chrome, clear of terminal text and controls.
                    let alert_x = controls.error_x.unwrap();
                    button(
                        &mut tiles,
                        &mut hits,
                        alert_x,
                        rect.y + (rect.h - 24.).max(0.) / 2.,
                        24.,
                        24.,
                        "",
                        "#ad4f60",
                        Action::PaneError(header.pane_id),
                        &hover,
                    );
                    let t = tiles.last_mut().unwrap();
                    t.icon = Some(Icon::Alert);
                    t.icon_size = 16.;
                    t.corner_radius = 12.;
                    t.fg = "#fff2f3";
                    t.zindex = 34;
                }
            }
            let enabled = broadcast.as_ref().map(|s| s.enabled()).unwrap_or(false);
            let target = if enabled { 1. } else { 0. };
            if ui.broadcast_motion_tab != active_id {
                ui.broadcast_motion_tab = active_id;
                ui.broadcast_motion = Tween::new(target);
                ui.pane_scroll = 0;
            }
            ui.broadcast_motion
                .set_target(target, now, Duration::from_millis(160));
            let switch_progress = ui.broadcast_motion.value(now);
            let progress = ui.tray_motion.value(now);
            let tray_top = h - BROADCAST_TRAY_HEIGHT * progress;
            let layout = TrayLayout::new(w, content_left);
            shield_tray(&mut hits, w, h, ui.broadcast_tray_height(), content_left);
            if progress > 0. {
                tile(
                    &mut tiles,
                    content_left,
                    tray_top,
                    (w - content_left).max(1.),
                    BROADCAST_TRAY_HEIGHT,
                    "",
                    BG,
                    INK,
                    false,
                );
                tiles.last_mut().unwrap().zindex = 31;
                let start = tiles.len();
                let body_x = if w - content_left < 130. {
                    content_left
                } else {
                    content_left + 12.
                };
                let body_width = (w - body_x - 12.).max(1.);
                // Subtle outline and inset surface keep all controls in one bar.
                tile(
                    &mut tiles,
                    body_x,
                    tray_top + 10.,
                    body_width,
                    58.,
                    "",
                    "#12131e",
                    INK,
                    true,
                );
                tiles.last_mut().unwrap().corner_radius = 14.;
                tile(
                    &mut tiles,
                    body_x,
                    tray_top + BROADCAST_SURFACE_TOP_INSET,
                    body_width,
                    58.,
                    "",
                    "#353648",
                    INK,
                    true,
                );
                tiles.last_mut().unwrap().corner_radius = 14.;
                tile(
                    &mut tiles,
                    body_x + 1.,
                    tray_top + 9.,
                    body_width - 2.,
                    56.,
                    "",
                    SIDE,
                    INK,
                    true,
                );
                tiles.last_mut().unwrap().corner_radius = 13.;
                if layout.title_width >= 32. {
                    tile(
                        &mut tiles,
                        content_left + 24.,
                        tray_top + 22.,
                        32.,
                        32.,
                        "",
                        if enabled { "#34314f" } else { CARD },
                        if enabled { ACCENT } else { MUTED },
                        true,
                    );
                    tiles.last_mut().unwrap().icon = Some(Icon::Broadcast);
                }
                if let Some(state) = &broadcast {
                    if layout.title_width >= 120. {
                        let title_text_x = content_left + 60.;
                        tile(
                            &mut tiles,
                            title_text_x,
                            tray_top + 17.,
                            layout.title_width - 36.,
                            22.,
                            "Broadcast",
                            SIDE,
                            INK,
                            false,
                        );
                        let status = if layout.title_width >= 166. {
                            format!(
                                "{}/{} {}",
                                state.member_count(),
                                state.pane_count(),
                                if enabled { "active" } else { "selected" }
                            )
                        } else {
                            format!(
                                "{} {}",
                                state.member_count(),
                                if enabled { "active" } else { "selected" }
                            )
                        };
                        tile(
                            &mut tiles,
                            title_text_x,
                            tray_top + 38.,
                            layout.title_width - 36.,
                            18.,
                            status,
                            SIDE,
                            MUTED,
                            false,
                        );
                    }
                    let member_labels: Vec<_> = pane_list
                        .iter()
                        .map(|p| {
                            labels
                                .get(&p.pane.pane_id())
                                .cloned()
                                .unwrap_or_else(|| "Terminal".into())
                        })
                        .collect();
                    let widths: Vec<_> = member_labels
                        .iter()
                        .map(|label| (tray_label_width(label) + 54.).clamp(68., 164.))
                        .collect();
                    let members = TrayMembers::new(
                        &widths,
                        layout.members_x,
                        layout.members_width,
                        ui.pane_scroll,
                    );
                    ui.pane_scroll = members.start;
                    if !members.chips.is_empty() {
                        tile(
                            &mut tiles,
                            layout.members_x - 10.,
                            tray_top + 24.,
                            1.,
                            28.,
                            "",
                            "#38394c",
                            INK,
                            false,
                        );
                    }
                    for (index, x, width) in &members.chips {
                        let id = pane_list[*index].pane.pane_id();
                        let member = state.is_member(id);
                        let broadcasting =
                            !disconnected_panes.contains(&id) && pane_broadcasting(state, id);
                        tile(
                            &mut tiles,
                            *x,
                            tray_top + 22.,
                            *width,
                            32.,
                            "",
                            if broadcasting {
                                "#4a436c"
                            } else if member {
                                "#414357"
                            } else {
                                "#343648"
                            },
                            INK,
                            true,
                        );
                        button(
                            &mut tiles,
                            &mut hits,
                            *x + 1.,
                            tray_top + 23.,
                            *width - 2.,
                            30.,
                            "",
                            if broadcasting {
                                "#302d47"
                            } else if member {
                                CARD
                            } else {
                                SIDE
                            },
                            Action::Member(id),
                            &hover,
                        );
                        let t = tiles.last_mut().unwrap();
                        t.corner_radius = 8.;
                        t.icon_size = 16.;
                        t.fg = if broadcasting {
                            "#b8b2ff"
                        } else if member {
                            INK
                        } else {
                            MUTED
                        };
                        t.text = tray_label(&member_labels[*index], *width - 52.);
                        t.icon = Some(if member { Icon::Check } else { Icon::Checkbox });
                    }
                    if let Some(x) = members.pager_x {
                        for (x, action, icon, available) in [
                            (x, Action::PrevMember, Icon::ChevronLeft, members.start > 0),
                            (
                                x + 32.,
                                Action::NextMember,
                                Icon::ChevronRight,
                                members.has_next,
                            ),
                        ] {
                            if available {
                                button(
                                    &mut tiles,
                                    &mut hits,
                                    x,
                                    tray_top + 22.,
                                    28.,
                                    32.,
                                    "",
                                    SIDE,
                                    action,
                                    &hover,
                                );
                            } else {
                                tile(
                                    &mut tiles,
                                    x,
                                    tray_top + 22.,
                                    28.,
                                    32.,
                                    "",
                                    SIDE,
                                    "#53566c",
                                    true,
                                );
                            }
                            tiles.last_mut().unwrap().icon = Some(icon);
                        }
                    }
                    if let Some(x) = layout.selection_x {
                        tile(
                            &mut tiles,
                            x,
                            tray_top + 22.,
                            108.,
                            32.,
                            "",
                            CARD,
                            INK,
                            true,
                        );
                        for (bx, width, label, action, selected) in [
                            (
                                x + 1.,
                                48.,
                                "All",
                                Action::BroadcastAll,
                                state.member_count() == state.pane_count(),
                            ),
                            (
                                x + 50.,
                                57.,
                                "None",
                                Action::BroadcastNone,
                                state.member_count() == 0,
                            ),
                        ] {
                            button(
                                &mut tiles,
                                &mut hits,
                                bx,
                                tray_top + 23.,
                                width,
                                30.,
                                label,
                                if selected { "#514b73" } else { CARD },
                                action,
                                &hover,
                            );
                            let t = tiles.last_mut().unwrap();
                            t.corner_radius = 8.;
                            t.transparent = !selected;
                            t.fg = if selected { INK } else { MUTED };
                        }
                        tile(
                            &mut tiles,
                            layout.switch_x - 8.,
                            tray_top + 26.,
                            1.,
                            24.,
                            "",
                            "#38394c",
                            INK,
                            false,
                        );
                    }
                    // One hit target covers the label, track and thumb.
                    button(
                        &mut tiles,
                        &mut hits,
                        layout.switch_x,
                        tray_top + 22.,
                        layout.switch_width,
                        32.,
                        if layout.switch_width > 40. {
                            if enabled {
                                "On"
                            } else {
                                "Off"
                            }
                        } else {
                            ""
                        },
                        SIDE,
                        Action::Broadcast,
                        &hover,
                    );
                    let t = tiles.last_mut().unwrap();
                    t.icon = None;
                    t.right_inset = 42.;
                    t.fg = if enabled { INK } else { MUTED };
                    let track_x = layout.switch_x + layout.switch_width - 39.;
                    tile(
                        &mut tiles,
                        track_x,
                        tray_top + 27.,
                        38.,
                        22.,
                        "",
                        if enabled { "#7568df" } else { "#45485e" },
                        INK,
                        true,
                    );
                    tiles.last_mut().unwrap().corner_radius = 11.;
                    tile(
                        &mut tiles,
                        track_x + 3. + 16. * switch_progress,
                        tray_top + 30.,
                        16.,
                        16.,
                        "",
                        if enabled { "#f1efff" } else { "#b5b8cc" },
                        INK,
                        true,
                    );
                    tiles.last_mut().unwrap().corner_radius = 8.;
                }
                for t in &mut tiles[start..] {
                    t.zindex = 32;
                }
            }
            let arrow_y = if progress > 0. {
                (tray_top + 22.).min(h - 34.)
            } else {
                h - 34.
            };
            button(
                &mut tiles,
                &mut hits,
                layout.collapse_x,
                arrow_y,
                28.,
                32.,
                "",
                SIDE,
                Action::ToggleTray,
                &hover,
            );
            let t = tiles.last_mut().unwrap();
            t.zindex = 33;
            t.fg = MUTED;
            t.icon = Some(if ui.tray_expanded {
                Icon::ChevronDown
            } else {
                Icon::ChevronUp
            });
        }
        if ui.page == Page::NewTab {
            tile(
                &mut tiles,
                content_left,
                52.,
                w - content_left,
                h - 52.,
                "",
                BG,
                INK,
                false,
            );
            let cw = (w - content_left - 48.).clamp(1., 720.);
            let cx = content_left + ((w - content_left - cw) / 2.).max(12.);
            tile(&mut tiles, cx, 72., cw, 34., "New Tab", BG, INK, false);
            let query = if ui.query.value.is_empty() {
                "Search hosts or workspaces…".into()
            } else {
                ui.query
                    .display(ui.search_focus, ((cw - 52.) / 8.).max(1.) as usize)
            };
            button(
                &mut tiles,
                &mut hits,
                cx,
                118.,
                cw,
                42.,
                query,
                FIELD,
                Action::Search,
                &hover,
            );
            tiles.last_mut().unwrap().input = Some(InputPaint {
                masked: false,
                input: ui.query.clone(),
                focused: ui.search_focus && ui.form.is_none(),
            });
            let q = ui.query.value.to_lowercase();
            let mut entries: Vec<(String, String, Option<Action>)> = vec![];
            let matches = |name: &str, layout: &SavedLayout| {
                name.to_lowercase().contains(&q)
                    || layout.hosts().iter().any(|h| {
                        format!("{} {}", h.label, h.address)
                            .to_lowercase()
                            .contains(&q)
                    })
            };
            let groups: Vec<_> = ui
                .workspace
                .data
                .saved
                .iter()
                .filter(|g| matches(&g.name, &g.layout))
                .collect();
            if !groups.is_empty() {
                entries.push(("Saved workspaces".into(), String::new(), None));
            }
            for g in groups {
                entries.push((
                    g.name.clone(),
                    format!("{} connections", g.layout.pane_count()),
                    Some(Action::OpenSaved(g.id.clone())),
                ));
            }
            let recents: Vec<_> = ui
                .workspace
                .data
                .recent
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    format!("{} {} {}", r.host.label, r.host.address, r.host.username)
                        .to_lowercase()
                        .contains(&q)
                })
                .collect();
            if !recents.is_empty() {
                entries.push(("Recent connections".into(), String::new(), None));
            }
            for (i, recent) in recents {
                entries.push((
                    recent.host.label.clone(),
                    recent.host.endpoint(),
                    Some(Action::ConnectRecent(i)),
                ));
            }
            let visible = ((h - 206.) / 48.).floor().max(1.) as usize;
            ui.max_scroll = entries.len().saturating_sub(visible);
            ui.scroll = ui.scroll.min(ui.max_scroll);
            if entries.is_empty() {
                tile(
                    &mut tiles,
                    cx,
                    202.,
                    cw,
                    36.,
                    "Your next workspace starts here",
                    BG,
                    INK,
                    false,
                );
                tile(
                    &mut tiles,
                    cx,
                    246.,
                    cw,
                    28.,
                    "Connect to a host, or save a group with Ctrl+S.",
                    BG,
                    MUTED,
                    false,
                );
                button(
                    &mut tiles,
                    &mut hits,
                    cx,
                    298.,
                    156.,
                    40.,
                    "Browse Hosts",
                    SIGNAL_BG,
                    Action::Hosts,
                    &hover,
                );
            }
            for (row, (label, detail, action)) in entries
                .into_iter()
                .skip(ui.scroll)
                .take(visible)
                .enumerate()
            {
                let y = 184. + row as f32 * 48.;
                if let Some(action) = action {
                    button(
                        &mut tiles, &mut hits, cx, y, cw, 40., label, CARD, action, &hover,
                    );
                    if cw > 400. {
                        tiles.last_mut().unwrap().right_inset = 170.;
                        tile(
                            &mut tiles,
                            cx + cw - 182.,
                            y,
                            174.,
                            40.,
                            detail,
                            CARD,
                            MUTED,
                            false,
                        );
                        tiles.last_mut().unwrap().transparent = true;
                    }
                } else {
                    tile(&mut tiles, cx, y, cw, 40., label, BG, MUTED, false);
                }
            }
            if !ui.error.is_empty() {
                tile(
                    &mut tiles,
                    cx,
                    h - 44.,
                    cw,
                    32.,
                    ui.error.clone(),
                    "#59333f",
                    INK,
                    true,
                );
            }
        }
        if matches!(ui.page, Page::Hosts | Page::Keys) {
            tile(
                &mut tiles,
                content_left,
                52.,
                w - content_left,
                h - 52.,
                "",
                BG,
                INK,
                false,
            );
            let cx = content_left + 24.;
            let cw = (w - cx - 24.).max(1.);
            tile(
                &mut tiles,
                content_left,
                52.,
                w - content_left,
                124.,
                "",
                SIDE,
                INK,
                false,
            );
            let query = if ui.query.value.is_empty() {
                if ui.page == Page::Hosts {
                    "Find a host or ssh user@hostname…".into()
                } else {
                    "Find a key or identity…".into()
                }
            } else {
                ui.query.display(
                    ui.search_focus && ui.form.is_none(),
                    ((cw - 160.) / 8.).max(4.) as usize,
                )
            };
            button(
                &mut tiles,
                &mut hits,
                cx,
                68.,
                cw,
                42.,
                query,
                FIELD,
                Action::Search,
                &hover,
            );
            tiles.last_mut().unwrap().input = Some(InputPaint {
                masked: false,
                input: ui.query.clone(),
                focused: ui.search_focus && ui.form.is_none(),
            });
            if ui.query.value.is_empty() {
                tiles.last_mut().unwrap().fg = MUTED;
            }
            if ui.page == Page::Hosts && cw >= 240. {
                tiles.last_mut().unwrap().right_inset = 116.;
                button(
                    &mut tiles,
                    &mut hits,
                    cx + cw - 112.,
                    74.,
                    104.,
                    30.,
                    "Connect",
                    CARD,
                    Action::QuickConnect,
                    &hover,
                );
            }
            button(
                &mut tiles,
                &mut hits,
                cx,
                130.,
                134_f32.min(cw),
                34.,
                if ui.page == Page::Hosts {
                    "Add Host"
                } else {
                    "Add Key"
                },
                SIGNAL_BG,
                if ui.page == Page::Hosts {
                    Action::AddHost
                } else {
                    Action::AddKey
                },
                &hover,
            );
            if ui.page == Page::Keys {
                button(
                    &mut tiles,
                    &mut hits,
                    cx + 144.,
                    130.,
                    (cw - 144.).min(154.).max(1.),
                    34.,
                    "Add Identity",
                    CARD,
                    Action::AddIdentity,
                    &hover,
                );
                tiles.last_mut().unwrap().icon = Some(Icon::Identity);
            }
            let query = ui.query.value.to_lowercase();
            let mut y = 194.;
            if !ui.error.is_empty() && ui.form.is_none() {
                tile(
                    &mut tiles,
                    cx,
                    y,
                    cw,
                    44.,
                    ui.error.clone(),
                    "#59333f",
                    INK,
                    true,
                );
                y += 56.;
            }
            let columns = ((cw + 16.) / 270.).floor().max(1.) as usize;
            let cardw = (cw - 16. * (columns - 1) as f32) / columns as f32;
            if ui.page == Page::Hosts {
                let mut groups: Vec<String> = ui
                    .store
                    .hosts
                    .iter()
                    .map(|h| h.group.clone())
                    .filter(|g| !g.is_empty())
                    .collect();
                groups.sort();
                groups.dedup();
                if !groups.is_empty() {
                    tile(&mut tiles, cx, y, cw, 26., "Groups", BG, INK, false);
                    button(
                        &mut tiles,
                        &mut hits,
                        cx + cw - 168.,
                        y - 3.,
                        102.,
                        28.,
                        "All hosts",
                        CARD,
                        Action::Group(String::new()),
                        &hover,
                    );
                    ui.group_scroll = ui.group_scroll.min(groups.len().saturating_sub(columns));
                    if groups.len() > columns {
                        button(
                            &mut tiles,
                            &mut hits,
                            cx + cw - 60.,
                            y - 3.,
                            26.,
                            28.,
                            "",
                            CARD,
                            Action::PrevGroup,
                            &hover,
                        );
                        button(
                            &mut tiles,
                            &mut hits,
                            cx + cw - 28.,
                            y - 3.,
                            26.,
                            28.,
                            "",
                            CARD,
                            Action::NextGroup,
                            &hover,
                        );
                    }
                    y += 36.;
                    for (index, group) in groups
                        .iter()
                        .skip(ui.group_scroll)
                        .take(columns)
                        .enumerate()
                    {
                        let gx = cx + index as f32 * (cardw + 16.);
                        let count = ui
                            .store
                            .hosts
                            .iter()
                            .filter(|host| &host.group == group)
                            .count();
                        let bg = if &ui.group == group { FIELD } else { CARD };
                        button(
                            &mut tiles,
                            &mut hits,
                            gx,
                            y,
                            cardw,
                            70.,
                            "",
                            bg,
                            Action::Group(group.clone()),
                            &hover,
                        );
                        tile(
                            &mut tiles,
                            gx + 12.,
                            y + 12.,
                            46.,
                            46.,
                            "",
                            SIGNAL_BG,
                            INK,
                            true,
                        );
                        tiles.last_mut().unwrap().icon = Some(Icon::Folder);
                        tile(
                            &mut tiles,
                            gx + 56.,
                            y + 10.,
                            (cardw - 68.).max(20.),
                            24.,
                            group.clone(),
                            bg,
                            INK,
                            false,
                        );
                        tiles.last_mut().unwrap().transparent = true;
                        tile(
                            &mut tiles,
                            gx + 56.,
                            y + 38.,
                            (cardw - 68.).max(20.),
                            22.,
                            format!("{} Hosts", count),
                            bg,
                            MUTED,
                            false,
                        );
                        tiles.last_mut().unwrap().transparent = true;
                    }
                    y += 94.;
                }
                let filtered: Vec<(usize, Host)> = ui
                    .store
                    .hosts
                    .iter()
                    .enumerate()
                    .filter(|(_, host)| {
                        (ui.group.is_empty() || host.group == ui.group)
                            && format!(
                                "{} {} {} {}",
                                host.label, host.address, host.username, host.group
                            )
                            .to_lowercase()
                            .contains(&query)
                    })
                    .map(|(i, h)| (i, h.clone()))
                    .collect();
                tile(
                    &mut tiles,
                    cx,
                    y,
                    cw,
                    28.,
                    format!("Hosts   {}", filtered.len()),
                    BG,
                    INK,
                    false,
                );
                y += 42.;
                let rows = ((h - y - 28.) / 88.).floor().max(1.) as usize;
                ui.max_scroll = filtered.len().div_ceil(columns).saturating_sub(rows);
                ui.scroll = ui.scroll.min(ui.max_scroll);
                if filtered.is_empty() {
                    let empty = if ui.store.hosts.is_empty() {
                        "Your servers, one workspace"
                    } else {
                        "No matching hosts"
                    };
                    tile(&mut tiles, cx, y + 34., cw, 46., empty, BG, INK, false);
                    tile(
                        &mut tiles,
                        cx,
                        y + 84.,
                        cw,
                        36.,
                        if ui.store.hosts.is_empty() {
                            "Add your first SSH host to get started."
                        } else {
                            "Try another search or choose All hosts."
                        },
                        BG,
                        MUTED,
                        false,
                    );
                    button(
                        &mut tiles,
                        &mut hits,
                        cx,
                        y + 142.,
                        150.,
                        40.,
                        "Add Host",
                        FIELD,
                        Action::AddHost,
                        &hover,
                    );
                }
                for (index, (id, host)) in filtered
                    .iter()
                    .skip(ui.scroll * columns)
                    .take(rows * columns)
                    .enumerate()
                {
                    let x = cx + (index % columns) as f32 * (cardw + 16.);
                    let cy = y + (index / columns) as f32 * 88.;
                    button(
                        &mut tiles,
                        &mut hits,
                        x,
                        cy,
                        cardw,
                        72.,
                        "",
                        CARD,
                        Action::Connect(*id),
                        &hover,
                    );
                    tile(
                        &mut tiles,
                        x + 12.,
                        cy + 13.,
                        46.,
                        46.,
                        "",
                        SIGNAL_BG,
                        INK,
                        true,
                    );
                    tiles.last_mut().unwrap().icon = Some(Icon::Hosts);
                    tile(
                        &mut tiles,
                        x + 56.,
                        cy + 9.,
                        (cardw - 96.).max(24.),
                        25.,
                        host.label.clone(),
                        CARD,
                        INK,
                        false,
                    );
                    tiles.last_mut().unwrap().transparent = true;
                    tile(
                        &mut tiles,
                        x + 56.,
                        cy + 37.,
                        (cardw - 70.).max(24.),
                        22.,
                        format!(
                            "ssh, {}",
                            if host.username.is_empty() {
                                "default user"
                            } else {
                                &host.username
                            }
                        ),
                        CARD,
                        MUTED,
                        false,
                    );
                    tiles.last_mut().unwrap().transparent = true;
                    button(
                        &mut tiles,
                        &mut hits,
                        x + cardw - 35.,
                        cy + 8.,
                        26.,
                        26.,
                        "",
                        CARD,
                        Action::HostMenu(*id),
                        &hover,
                    );
                }
            } else {
                let keys: Vec<_> = ui
                    .store
                    .keys
                    .iter()
                    .filter(|k| {
                        format!("{} {}", k.label, k.path)
                            .to_lowercase()
                            .contains(&query)
                    })
                    .collect();
                let identities: Vec<_> = ui
                    .store
                    .identities
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| {
                        format!("{} {}", i.label, i.username)
                            .to_lowercase()
                            .contains(&query)
                    })
                    .collect();
                let key_rows = ((keys.len() + columns - 1) / columns).max(1);
                let identity_rows = ((identities.len() + columns - 1) / columns).max(1);
                let total_rows = key_rows + identity_rows + 2;
                let visible_rows = ((h - y - 20.) / 88.).floor().max(1.) as usize;
                ui.max_scroll = total_rows.saturating_sub(visible_rows);
                ui.scroll = ui.scroll.min(ui.max_scroll);
                for row in ui.scroll..(ui.scroll + visible_rows).min(total_rows) {
                    let cy = y + (row - ui.scroll) as f32 * 88.;
                    if row == 0 || row == key_rows + 1 {
                        tile(
                            &mut tiles,
                            cx,
                            cy + 20.,
                            cw,
                            32.,
                            if row == 0 { "Keys" } else { "Identities" },
                            BG,
                            INK,
                            false,
                        );
                        continue;
                    }
                    let is_key = row <= key_rows;
                    let item_row = if is_key { row - 1 } else { row - key_rows - 2 };
                    if (is_key && keys.is_empty()) || (!is_key && identities.is_empty()) {
                        tile(
                            &mut tiles,
                            cx,
                            cy,
                            cw,
                            48.,
                            if is_key {
                                "Add a private key file to use with your hosts."
                            } else {
                                "Save a username and password to reuse across hosts."
                            },
                            BG,
                            MUTED,
                            false,
                        );
                        continue;
                    }
                    for col in 0..columns {
                        let index = item_row * columns + col;
                        let (label, detail, icon, action) = if is_key {
                            match keys.get(index) {
                                Some(k) => (k.label.clone(), k.path.clone(), Icon::Key, None),
                                None => continue,
                            }
                        } else {
                            match identities.get(index) {
                                Some((id, i)) => (
                                    i.label.clone(),
                                    format!("{} · Password", i.username),
                                    Icon::Identity,
                                    Some(Action::EditIdentity(*id)),
                                ),
                                None => continue,
                            }
                        };
                        let x = cx + col as f32 * (cardw + 16.);
                        if let Some(action) = action {
                            button(
                                &mut tiles, &mut hits, x, cy, cardw, 72., "", CARD, action, &hover,
                            );
                        } else {
                            tile(&mut tiles, x, cy, cardw, 72., "", CARD, INK, true);
                        }
                        tile(
                            &mut tiles,
                            x + 12.,
                            cy + 13.,
                            46.,
                            46.,
                            "",
                            SIGNAL_BG,
                            INK,
                            true,
                        );
                        tiles.last_mut().unwrap().icon = Some(icon);
                        tile(
                            &mut tiles,
                            x + 60.,
                            cy + 9.,
                            cardw - 72.,
                            25.,
                            label,
                            CARD,
                            INK,
                            false,
                        );
                        tiles.last_mut().unwrap().transparent = true;
                        tile(
                            &mut tiles,
                            x + 60.,
                            cy + 37.,
                            cardw - 72.,
                            22.,
                            detail,
                            CARD,
                            MUTED,
                            false,
                        );
                        tiles.last_mut().unwrap().transparent = true;
                    }
                }
            }
            if ui.max_scroll > 0 {
                tile(
                    &mut tiles,
                    w - 7.,
                    184.,
                    4.,
                    (h - 208.).max(20.),
                    "",
                    SIDE,
                    MUTED,
                    true,
                );
                let sy = 184. + (h - 268.).max(0.) * (ui.scroll as f32 / ui.max_scroll as f32);
                tile(&mut tiles, w - 7., sy, 4., 60., "", MUTED, MUTED, true);
            }
        }
        // A separate renderer layer is required: backgrounds and glyphs are
        // batched independently within each layer, so draw order alone cannot
        // cover the library text behind the drawer.
        if ui.page != Page::Terminal {
            for t in &mut tiles {
                if t.x >= content_left && t.y >= 184. {
                    t.opacity *= page_progress;
                    t.y += 8. * (1. - page_progress);
                }
            }
            for hit in &mut hits {
                if hit.x >= content_left && hit.y >= 184. {
                    hit.y += 8. * (1. - page_progress);
                }
            }
        }
        if ui.form.is_some() || ui.closing_form.is_some() {
            tile(&mut tiles, 0., 52., w, h - 52., "", "#080911", INK, false);
            let dim = tiles.last_mut().unwrap();
            dim.opacity = 0.4 * drawer_progress;
            dim.zindex = 35;
        }
        let drawer_start = tiles.len();
        let interactive_drawer = ui.form.is_some();
        let drawer_offset = (1. - drawer_progress) * DrawerLayout::new(w, h).width;
        if let Some(form) = ui.form.as_mut().or(ui.closing_form.as_mut()) {
            let layout = DrawerLayout::new(w, h);
            let animated = DrawerLayout {
                x: layout.x + drawer_offset,
                ..DrawerLayout::new(w, h)
            };
            animated.capture_hits(&mut hits, w);
            let drawer_hit_start = hits.len();
            tile(
                &mut tiles,
                layout.x - 2.,
                52.,
                2.,
                h - 52.,
                "",
                TOP,
                INK,
                false,
            );
            tile(
                &mut tiles,
                layout.x,
                52.,
                layout.width,
                h - 52.,
                "",
                SIDE,
                INK,
                false,
            );
            let fx = layout.x + 20.;
            let fw = (layout.width - 40.).max(1.);
            let title = if form.auth.picker {
                "Choose authentication"
            } else if form.auth.editing_identity {
                if form.auth.original_identity.is_some() {
                    "Edit Identity"
                } else {
                    "Add Identity"
                }
            } else if form.settings {
                "Terminal settings"
            } else if form.rename_tab.is_some() {
                "Rename tab"
            } else if form.key {
                "Add Key"
            } else if form.original.is_some() {
                "Edit Host"
            } else {
                "Add Host"
            };
            tile(&mut tiles, fx, 70., fw - 44., 38., title, SIDE, INK, false);
            button(
                &mut tiles,
                &mut hits,
                fx + fw - 36.,
                72.,
                36.,
                34.,
                "",
                SIDE,
                Action::Cancel,
                &hover,
            );
            tile(
                &mut tiles,
                fx,
                110.,
                fw,
                26.,
                if form.auth.picker {
                    "Use manual credentials or choose from Keychain"
                } else if form.auth.editing_identity {
                    "Username & password · Windows protected"
                } else if form.settings {
                    "Terminal & SSH · saved on this device"
                } else if form.rename_tab.is_some() {
                    "A stable name for this tab or group"
                } else if form.key {
                    "Use a local SSH private key"
                } else {
                    "SSH connection · saved locally"
                },
                SIDE,
                MUTED,
                false,
            );
            let labels = if form.auth.editing_identity {
                vec!["Label", "Username", "Password"]
            } else if form.settings {
                vec![
                    "Global font size · 6–48 pt",
                    "SSH keepalive interval · seconds · 0 disables",
                ]
            } else if form.rename_tab.is_some() {
                vec!["Tab name"]
            } else if form.key {
                vec!["Name", "Private key file path"]
            } else {
                vec![
                    "Label",
                    "Address",
                    "Port",
                    "Username (optional)",
                    "Password (optional)",
                    "Private key file path (optional)",
                    "Group (optional)",
                ]
            };
            let visible = if form.auth.picker {
                0..0
            } else {
                form.visible_rows(layout.rows)
            };
            for (row, i) in visible.enumerate() {
                let y = 148. + row as f32 * 70.;
                tile(&mut tiles, fx, y, fw, 20., labels[i], SIDE, MUTED, false);
                let focused = form.focus == i;
                let password = form.password_index() == Some(i);
                let linked = !form.auth.selected_identity.is_empty() && matches!(i, 3 | 4 | 5);
                let text = if password && !form.auth.reveal {
                    "•".repeat(form.fields[i].value.chars().count())
                } else {
                    form.fields[i].display(focused, ((fw - 30.) / 8.).max(1.) as usize)
                };
                if focused {
                    tile(
                        &mut tiles,
                        fx - 1.,
                        y + 23.,
                        fw + 2.,
                        36.,
                        "",
                        ACCENT,
                        INK,
                        true,
                    );
                }
                button(
                    &mut tiles,
                    &mut hits,
                    fx,
                    y + 24.,
                    fw,
                    34.,
                    text,
                    FIELD,
                    if linked {
                        Action::ChooseAuth
                    } else {
                        Action::Field(i)
                    },
                    &hover,
                );
                if linked {
                    let t = tiles.last_mut().unwrap();
                    t.text = if i == 3 {
                        form.fields[3].value.clone()
                    } else {
                        "Managed by Identity".into()
                    };
                    t.fg = MUTED;
                } else {
                    tiles.last_mut().unwrap().input = Some(InputPaint {
                        masked: password && !form.auth.reveal,
                        input: form.fields[i].clone(),
                        focused: focused && interactive_drawer,
                    });
                }
                if password {
                    tiles.last_mut().unwrap().right_inset = 58.;
                    if form.fields[i].value.is_empty() {
                        tiles.last_mut().unwrap().text = if form.auth.original_identity.is_some()
                            || form
                                .original
                                .as_ref()
                                .is_some_and(|h| !h.credential_ref().is_empty())
                        {
                            "Unchanged · enter to replace".into()
                        } else {
                            "Enter password".into()
                        };
                    }
                    button(
                        &mut tiles,
                        &mut hits,
                        fx + fw - 60.,
                        y + 26.,
                        58.,
                        30.,
                        if form.auth.reveal { "Hide" } else { "Show" },
                        FIELD,
                        Action::RevealPassword,
                        &hover,
                    );
                    if form.is_host() {
                        button(
                            &mut tiles,
                            &mut hits,
                            fx + fw - 168.,
                            y - 2.,
                            168.,
                            22.,
                            "Remember password",
                            if form.auth.remember { SIGNAL_BG } else { SIDE },
                            Action::RememberPassword,
                            &hover,
                        );
                        tiles.last_mut().unwrap().icon = Some(if form.auth.remember {
                            Icon::CheckboxChecked
                        } else {
                            Icon::Checkbox
                        });
                        tiles.last_mut().unwrap().icon_size = 14.;
                    }
                }
            }
            let max_scroll = form.fields.len().saturating_sub(layout.rows);
            if max_scroll > 0 && !form.auth.picker {
                let track = layout.rows as f32 * 70. - 12.;
                tile(&mut tiles, w - 7., 148., 3., track, "", FIELD, MUTED, true);
                let thumb = (track * layout.rows as f32 / form.fields.len() as f32).max(16.);
                let sy = 148. + (track - thumb) * form.first_row as f32 / max_scroll as f32;
                tile(&mut tiles, w - 7., sy, 3., thumb, "", MUTED, MUTED, true);
            }
            if form.settings {
                tile(
                    &mut tiles,
                    fx,
                    300.,
                    fw,
                    28.,
                    "Keepalive changes apply to new SSH connections.",
                    SIDE,
                    MUTED,
                    false,
                );
                tile(
                    &mut tiles,
                    fx,
                    328.,
                    fw,
                    28.,
                    "Ctrl + wheel session font changes stay temporary.",
                    SIDE,
                    MUTED,
                    false,
                );
            }
            if form.is_host() && !form.auth.picker {
                let label = ui
                    .store
                    .identities
                    .iter()
                    .find(|i| i.id == form.auth.selected_identity)
                    .map(|i| format!("Identity · {}", i.label))
                    .unwrap_or({
                        ui.store
                            .keys
                            .iter()
                            .find(|k| k.path == form.fields[5].value)
                            .map(|k| format!("Key · {}", k.label))
                            .unwrap_or_else(|| "Authentication · Manual / Keychain".into())
                    });
                button(
                    &mut tiles,
                    &mut hits,
                    fx,
                    layout.footer,
                    fw,
                    32.,
                    label,
                    CARD,
                    Action::ChooseAuth,
                    &hover,
                );
                tiles.last_mut().unwrap().icon = Some(Icon::Key);
            }
            if form.auth.picker {
                let mut choices = vec![(
                    "Manual credentials".to_string(),
                    Action::ManualAuth,
                    Icon::Terminal,
                )];
                choices.extend(
                    ui.store
                        .keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| (format!("Key · {}", k.label), Action::UseKey(i), Icon::Key)),
                );
                choices.extend(ui.store.identities.iter().enumerate().map(|(i, k)| {
                    (
                        format!("Identity · {} · {}", k.label, k.username),
                        Action::UseIdentity(i),
                        Icon::Identity,
                    )
                }));
                let count = ((layout.footer - 148.) / 52.).floor().max(1.) as usize;
                form.auth.picker_scroll = form
                    .auth
                    .picker_scroll
                    .min(choices.len().saturating_sub(count));
                for (row, (label, action, icon)) in choices
                    .into_iter()
                    .skip(form.auth.picker_scroll)
                    .take(count)
                    .enumerate()
                {
                    button(
                        &mut tiles,
                        &mut hits,
                        fx,
                        148. + row as f32 * 52.,
                        fw,
                        42.,
                        label,
                        CARD,
                        action,
                        &hover,
                    );
                    tiles.last_mut().unwrap().icon = Some(icon);
                }
                tile(
                    &mut tiles,
                    fx,
                    layout.footer,
                    fw,
                    28.,
                    "Scroll to browse · Esc to go back",
                    SIDE,
                    MUTED,
                    false,
                );
            }
            if form.auth.editing_identity && form.auth.original_identity.is_some() {
                button(
                    &mut tiles,
                    &mut hits,
                    fx,
                    layout.footer,
                    fw,
                    30.,
                    if form.auth.confirm_delete {
                        "Confirm delete identity"
                    } else {
                        "Delete identity"
                    },
                    CARD,
                    Action::DeleteIdentity,
                    &hover,
                );
                tiles.last_mut().unwrap().fg = "#ed8193";
            }
            if !ui.error.is_empty() {
                tile(
                    &mut tiles,
                    fx,
                    h - 88.,
                    fw,
                    30.,
                    ui.error.clone(),
                    "#59333f",
                    INK,
                    true,
                );
            }
            let bw = ((fw - 12.) / 2.).max(1.);
            button(
                &mut tiles,
                &mut hits,
                fx,
                h - 52.,
                bw,
                36.,
                "Cancel",
                CARD,
                Action::Cancel,
                &hover,
            );
            button(
                &mut tiles,
                &mut hits,
                fx + bw + 12.,
                h - 52.,
                bw,
                36.,
                if form.auth.picker { "Back" } else { "Save" },
                SIGNAL_BG,
                Action::Save,
                &hover,
            );
            for t in &mut tiles[drawer_start..] {
                t.x += drawer_offset;
                t.zindex = 40;
            }
            for hit in &mut hits[drawer_hit_start..] {
                hit.x += drawer_offset;
            }
            if !interactive_drawer {
                // Closing visuals own the click until the next settled frame.
                hits.truncate(drawer_hit_start);
            }
        }
        if let Some((_, mx, my)) = &ui.host_menu {
            let x = mx.min(w - 208.).max(8.);
            let y = my.min(h - 96.).max(52.);
            hits.push(Hit {
                x: 0.,
                y: 0.,
                w,
                h,
                action: Action::DismissMenu,
            });
            let start = tiles.len();
            tile(&mut tiles, x, y, 200., 88., "", SIDE, INK, true);
            button(
                &mut tiles,
                &mut hits,
                x + 6.,
                y + 6.,
                188.,
                36.,
                "Edit Host",
                SIDE,
                Action::EditMenuHost,
                &hover,
            );
            button(
                &mut tiles,
                &mut hits,
                x + 6.,
                y + 46.,
                188.,
                36.,
                "Duplicate Host",
                SIDE,
                Action::DuplicateHost,
                &hover,
            );
            for tile in &mut tiles[start..] {
                tile.zindex = 60;
            }
        }
        if let Some((id, mx, my)) = ui.tab_menu {
            let x = mx.min(w - 196.).max(8.);
            let y = my.min(h - 132.).max(52.);
            hits.clear();
            hits.push(Hit {
                x: 0.,
                y: 0.,
                w,
                h,
                action: Action::DismissMenu,
            });
            let start = tiles.len();
            tile(&mut tiles, x, y, 188., 124., "", SIDE, INK, true);
            button(
                &mut tiles,
                &mut hits,
                x + 6.,
                y + 6.,
                176.,
                34.,
                "Rename tab",
                SIDE,
                Action::RenameTab(id),
                &hover,
            );
            if tabs
                .iter()
                .any(|(tab, _, _, panes)| *tab == id && *panes > 1)
            {
                button(
                    &mut tiles,
                    &mut hits,
                    x + 6.,
                    y + 44.,
                    176.,
                    34.,
                    "Save group · Ctrl+S",
                    SIDE,
                    Action::SaveWorkspace(id),
                    &hover,
                );
            }
            button(
                &mut tiles,
                &mut hits,
                x + 6.,
                y + 82.,
                176.,
                34.,
                "Close tab",
                SIDE,
                Action::CloseTab(id),
                &hover,
            );
            for t in &mut tiles[start..] {
                t.zindex = 60;
            }
        }
        let mut notice_due = None;
        if let Some((text, until)) = &ui.notice {
            if now < *until {
                let width =
                    (text.chars().count() as f32 * 8. + 30.).min((w - content_left - 30.).max(1.));
                tile(
                    &mut tiles,
                    content_left + 16.,
                    64.,
                    width,
                    34.,
                    text.clone(),
                    "#285d53",
                    INK,
                    true,
                );
                tiles.last_mut().unwrap().zindex = 65;
                notice_due = Some(*until);
            } else {
                ui.notice = None;
            }
        }
        let preview = drag_visuals.as_ref().and_then(|d| d.preview);
        if ui.dock_preview != preview {
            ui.dock_preview = preview;
            ui.dock_motion = Tween::new(0.);
            ui.dock_motion
                .set_target(1., now, Duration::from_millis(100));
        }
        if let Some(drag) = drag_visuals {
            if let Some(x) = drag.insertion_x {
                tile(&mut tiles, x - 2., 7., 3., 38., "", "#a6a0ff", INK, false);
                if let Some(t) = tiles.last_mut() {
                    t.zindex = 52;
                }
            }
            if let Some(rect) = drag.preview {
                tile(
                    &mut tiles,
                    rect.x + 5.,
                    rect.y + 5.,
                    (rect.w - 10.).max(1.),
                    (rect.h - 10.).max(1.),
                    "",
                    "#59b79e",
                    INK,
                    true,
                );
                if let Some(t) = tiles.last_mut() {
                    t.zindex = 50;
                    t.opacity = 0.28 * ui.dock_motion.value(now);
                }
                tile(
                    &mut tiles,
                    rect.x + 12.,
                    rect.y + 12.,
                    (rect.w - 24.).clamp(1., 240.),
                    32.,
                    if drag.swapping {
                        "Release to swap sessions"
                    } else {
                        "Release to place session"
                    },
                    "#285d53",
                    INK,
                    true,
                );
                if let Some(t) = tiles.last_mut() {
                    t.zindex = 51;
                    t.icon = Some(if drag.swapping {
                        Icon::Grid
                    } else {
                        Icon::Plus
                    });
                }
            }
            let label_width = 210_f32.min(w - 16.);
            tile(
                &mut tiles,
                (drag.pointer.0 + 14.).min(w - label_width - 8.).max(8.),
                (drag.pointer.1 + 16.).min(h - 46.).max(8.),
                label_width,
                38.,
                drag.title,
                FIELD,
                INK,
                true,
            );
            let t = tiles.last_mut().unwrap();
            t.icon = Some(Icon::Terminal);
            t.opacity = 0.94;
            t.zindex = 55;
        }
        let mut hint_due = None;
        if confirmation.is_none()
            && ui.pressed.is_none()
            && ui.host_menu.is_none()
            && ui.tab_menu.is_none()
            && ui.form.is_none()
            && ui.closing_form.is_none()
        {
            if let Some((action, hint)) = hover.as_ref().and_then(|action| {
                let hint = match action {
                    Action::PaneError(id) => pane_errors
                        .get(id)
                        .map(|error| format!("{error}\nClick to dismiss")),
                    _ => action_hint(action, broadcast.as_ref()).map(str::to_owned),
                };
                hint.map(|hint| (action, hint))
            }) {
                let due = ui.hover_started + Duration::from_millis(400);
                if now >= due {
                    if let Some(hit) = hits.iter().find(|h| &h.action == action) {
                        let mut natural_width = 0_f32;
                        for line in hint.lines() {
                            natural_width = natural_width
                                .max(*text_positions(&ui_font, line, scale)?.last().unwrap_or(&0.));
                        }
                        let width = (natural_width + 40.).min(480.).min(w - 16.);
                        let mut lines = if natural_width + 40. <= width {
                            hint.lines().map(str::to_owned).collect::<Vec<_>>()
                        } else {
                            wrap_hint(&hint, ((width - 40.) / 8.).max(1.) as usize)
                        };
                        let max_lines = ((h - 48.) / 22.).max(1.) as usize;
                        if lines.len() > max_lines {
                            lines.truncate(max_lines);
                            if let Some(line) = lines.last_mut() {
                                line.push('…');
                            }
                        }
                        let height = lines.len() as f32 * 22. + 16.;
                        let x = hit.x.min(w - width - 8.).max(8.);
                        let y = (hit.y + hit.h + 8.).min(h - height - 8.).max(8.);
                        tile(&mut tiles, x, y, width, height, "", FIELD, INK, true);
                        tiles.last_mut().unwrap().zindex = 60;
                        for (index, line) in lines.iter().enumerate() {
                            tile(
                                &mut tiles,
                                x + 4.,
                                y + 8. + index as f32 * 22.,
                                width - 8.,
                                22.,
                                line,
                                FIELD,
                                INK,
                                false,
                            );
                            tiles.last_mut().unwrap().zindex = 61;
                        }
                    }
                } else {
                    hint_due = Some(due);
                }
            }
        }

        if let Some(prompt) = &confirmation {
            paint_close_dialog(
                &mut tiles,
                &mut hits,
                w,
                h,
                prompt,
                confirmation_progress,
                confirmation_focus,
                &hover,
            );
        }
        ui.hits = hits;
        let mut moving = ui.sidebar_motion.is_active(now)
            || ui.drawer_motion.is_active(now)
            || ui.page_motion.is_active(now)
            || (ui.dock_preview.is_some() && ui.dock_motion.is_active(now))
            || ui.tray_motion.is_active(now)
            || ui.broadcast_motion.is_active(now)
            || self.termviai_confirm.motion.is_active(now);
        let mut visible_actions = std::collections::HashSet::new();
        for t in &mut tiles {
            if let Some(action) = &t.action {
                visible_actions.insert(action.clone());
                let motion = ui
                    .hover_motion
                    .entry(action.clone())
                    .or_insert_with(|| Tween::new(0.));
                motion.set_target(
                    if hover.as_ref() == Some(action) {
                        1.
                    } else {
                        0.
                    },
                    now,
                    Duration::from_millis(120),
                );
                moving |= motion.is_active(now);
                if matches!(action, Action::Maximize)
                    && self.window_state.contains(WindowState::MAXIMIZED)
                {
                    t.icon = Some(Icon::Restore);
                }
            }
        }
        ui.hover_motion
            .retain(|action, _| visible_actions.contains(action));
        let hover_colors: HashMap<_, _> = ui
            .hover_motion
            .iter()
            .map(|(a, t)| (a.clone(), t.value(now)))
            .collect();
        if let Some(due) = notice_due {
            self.update_next_frame_time(Some(due));
        }
        if let Some(due) = hint_due {
            self.update_next_frame_time(Some(due));
        }
        if moving {
            self.update_next_frame_time(Some(now + frame_interval(self.config.max_fps as u64)));
        }
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let gl_state = self
            .render_state
            .as_ref()
            .context("Renderer not initialized")?;
        let mut input_hits = Vec::new();
        for mut t in tiles {
            let hover = t
                .action
                .as_ref()
                .and_then(|a| hover_colors.get(a))
                .copied()
                .unwrap_or(0.);
            let pressed = t.action.is_some() && t.action == self.termviai_ui.pressed;
            let mut bg = mix_color(
                color(t.bg),
                color(
                    if matches!(
                        t.action,
                        Some(Action::Close | Action::ConfirmAccept | Action::RemovePane(_))
                    ) {
                        "#b84e64"
                    } else {
                        "#52546e"
                    },
                ),
                hover * 0.58,
            );
            if pressed {
                bg = mix_color(bg, color(ACCENT), 0.28);
            }
            if t.transparent {
                // Header broadcast controls are bare icons at rest. Only the
                // hovered control gains a subdued surface; color carries state.
                bg = if t.icon == Some(Icon::Broadcast) {
                    color(SIGNAL_BG).mul_alpha(if pressed { 0.95 } else { hover * 0.8 })
                } else {
                    bg.mul_alpha(0.)
                };
            }
            bg = bg.mul_alpha(t.opacity);
            let close_icon = t.text.is_empty()
                && matches!(
                    t.action,
                    Some(Action::CloseTab(_) | Action::CloseNewTab | Action::RemovePane(_))
                );
            let fg = if close_icon {
                mix_color(
                    color(t.fg),
                    color("#ed8193"),
                    if pressed { 1. } else { hover },
                )
            } else {
                color(t.fg)
            }
            .mul_alpha(t.opacity);
            if t.outline_width > 0. {
                let layer = gl_state
                    .layer_for_zindex(t.zindex)
                    .context("pane outline layer")?;
                let mut layers = layer.quad_allocator();
                self.rounded_rectangle_outline(
                    &mut layers,
                    0,
                    euclid::rect(t.x * scale, t.y * scale + top, t.w * scale, t.h * scale),
                    t.corner_radius * scale,
                    t.outline_width * scale,
                    bg,
                )?;
                continue;
            }
            let transparent = color(TOP).mul_alpha(0.);
            let mut parts = vec![(
                t.x,
                t.y,
                t.w,
                t.h,
                ElementContent::Children(vec![]),
                bg,
                t.rounded,
            )];
            if let Some(paint) = &t.input {
                let input = &paint.input;
                let left = if t.icon.is_some() { 38. } else { 12. };
                let available = (t.w - left - 12. - t.right_inset - 2.).max(1.);
                let display_value = paint.rendered_value();
                let positions = text_positions(&font, &display_value, scale)?;
                let cursor = input.cursor.min(positions.len() - 1);
                let start = if paint.focused {
                    positions
                        .iter()
                        .position(|x| positions[cursor] - *x <= available)
                        .unwrap_or(cursor)
                } else {
                    0
                };
                let end = positions
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take_while(|(_, x)| **x - positions[start] <= available)
                    .last()
                    .map(|(index, _)| index)
                    .unwrap_or(start);
                if !input.value.is_empty() || paint.focused {
                    t.text = display_value
                        .chars()
                        .skip(start)
                        .take(end - start)
                        .collect();
                }
                let text_x = t.x + left;
                let text_height = metrics.cell_size.height as f32 / scale;
                let text_y = t.y + (t.h - text_height).max(0.) / 2.;
                if paint.focused {
                    if let Some(selection) = input.selection() {
                        let a = selection.start.clamp(start, end);
                        let b = selection.end.clamp(start, end);
                        if a < b {
                            parts.push((
                                text_x + positions[a] - positions[start],
                                text_y,
                                positions[b] - positions[a],
                                text_height,
                                ElementContent::Children(vec![]),
                                color("#575078"),
                                false,
                            ));
                        }
                    }
                    let caret_x =
                        ((text_x + positions[cursor] - positions[start]) * scale).round() / scale;
                    parts.push((
                        caret_x,
                        text_y,
                        1. / scale,
                        text_height,
                        ElementContent::Children(vec![]),
                        color(INK),
                        false,
                    ));
                    if let Some(window) = &self.window {
                        window.set_text_cursor_position(window::Rect::new(
                            window::Point::new(
                                (caret_x * scale) as isize,
                                ((text_y + text_height) * scale + top) as isize,
                            ),
                            window::Size::new(1, (text_height * scale) as isize),
                        ));
                    }
                }
                if let Some(action) = &t.action {
                    input_hits.push(InputHit {
                        action: action.clone(),
                        x: text_x,
                        start,
                        positions: positions[start..=end]
                            .iter()
                            .map(|x| x - positions[start])
                            .collect(),
                    });
                }
            }
            if let Some(icon) = t.icon {
                let size = t.icon_size.min(t.h - 4.).min(t.w - 4.).max(1.);
                let ix = if t.text.is_empty() && t.input.is_none() {
                    t.x + (t.w - size) / 2.
                } else {
                    t.x + 12.
                };
                parts.push((
                    ix,
                    t.y + (t.h - size) / 2.,
                    size,
                    size,
                    icon.content(size * scale),
                    transparent,
                    false,
                ));
            }
            if !t.text.is_empty() {
                let left = if t.icon.is_some() { 38. } else { 12. };
                let text_width = (t.w - left - 12. - t.right_inset).max(0.);
                if text_width > 0. {
                    let text_height = metrics.cell_size.height as f32 / scale;
                    parts.push((
                        t.x + left,
                        t.y + (t.h - text_height).max(0.) / 2.,
                        text_width,
                        text_height,
                        ElementContent::Text(t.text),
                        transparent,
                        false,
                    ));
                }
            }
            for (x, y, width, height, content, bg, rounded) in parts {
                if x >= w || x + width <= 0. || y >= h || y + height <= 0. {
                    continue;
                }
                let tw = width * scale;
                let th = height * scale;
                let mut e = Element::new(&font, content)
                    .min_width(Some(Dimension::Pixels(tw)))
                    .max_width(Some(Dimension::Pixels(tw)))
                    .min_height(Some(Dimension::Pixels(th)))
                    .colors(ElementColors {
                        border: BorderColor::new(bg.into()),
                        bg: bg.into(),
                        text: fg.into(),
                    });
                if rounded {
                    let corner = (height / 2.).min(width / 2.).min(t.corner_radius) * scale;
                    let p = |poly| SizedPoly {
                        width: Dimension::Pixels(corner),
                        height: Dimension::Pixels(corner),
                        poly,
                    };
                    e = e.border_corners(Some(Corners {
                        top_left: p(TOP_LEFT_ROUNDED_CORNER),
                        top_right: p(TOP_RIGHT_ROUNDED_CORNER),
                        bottom_left: p(BOTTOM_LEFT_ROUNDED_CORNER),
                        bottom_right: p(BOTTOM_RIGHT_ROUNDED_CORNER),
                    }));
                }
                let computed = self.compute_element(
                    &LayoutContext {
                        width: DimensionContext {
                            dpi: self.dimensions.dpi as f32,
                            pixel_max: (self.dimensions.pixel_width as f32)
                                .max((x + width) * scale),
                            pixel_cell: metrics.cell_size.width as f32,
                        },
                        height: DimensionContext {
                            dpi: self.dimensions.dpi as f32,
                            pixel_max: self.dimensions.pixel_height as f32,
                            pixel_cell: metrics.cell_size.height as f32,
                        },
                        bounds: euclid::rect(x * scale, y * scale + top, tw, th),
                        metrics: &metrics,
                        gl_state,
                        zindex: t.zindex,
                    },
                    &e,
                )?;
                self.render_element(&computed, gl_state, None)?;
            }
        }
        self.termviai_ui.input_hits = input_hits;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn host_password_can_switch_between_session_and_remembered_without_retyping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.open_form(false, None);
        let f = ui.form.as_mut().unwrap();
        f.fields[1] = Input::new("example.invalid".into());
        f.fields[4] = Input::new("synthetic-only password".into());
        ui.save_form_at(&path).unwrap();
        let first = ui.store.hosts[0].clone();
        assert!(first.credential_id.is_empty());
        assert_eq!(
            wezterm_ssh::credentials::load(first.credential_ref())
                .unwrap()
                .as_deref(),
            Some("synthetic-only password")
        );
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("synthetic-only password"));
        ui.open_form(false, Some(first.clone()));
        ui.form.as_mut().unwrap().auth.remember = true;
        ui.save_form_at(&path).unwrap();
        let remembered = ui.store.hosts[0].clone();
        assert!(!remembered.credential_id.is_empty());
        assert!(remembered.session_credential.is_empty());
        assert_eq!(
            wezterm_ssh::credentials::load(remembered.credential_ref())
                .unwrap()
                .as_deref(),
            Some("synthetic-only password")
        );
        ui.open_form(false, Some(remembered.clone()));
        ui.form.as_mut().unwrap().auth.remember = false;
        ui.save_form_at(&path).unwrap();
        assert!(wezterm_ssh::credentials::load(&remembered.credential_id)
            .unwrap()
            .is_none());
        let session = ui.store.hosts[0].credential_ref().to_string();
        assert_eq!(
            wezterm_ssh::credentials::load(&session).unwrap().as_deref(),
            Some("synthetic-only password")
        );
        wezterm_ssh::credentials::delete(&session).unwrap();
        // A restored host can retain a reference after the process password expires.
        let before = std::fs::read(&path).unwrap();
        ui.open_form(false, Some(ui.store.hosts[0].clone()));
        ui.form.as_mut().unwrap().auth.remember = true;
        assert!(ui
            .save_form_at(&path)
            .unwrap_err()
            .to_string()
            .contains("Enter it again"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[cfg(windows)]
    #[test]
    fn identity_edit_preserves_blank_password_and_host_uses_reference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts.json");
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.open_identity(None);
        let f = ui.form.as_mut().unwrap();
        f.fields[0] = Input::new("Test identity".into());
        f.fields[1] = Input::new("deploy".into());
        f.fields[2] = Input::new("synthetic identity secret".into());
        ui.save_form_at(&path).unwrap();
        let original = ui.store.identities[0].clone();
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = wezterm_ssh::credentials::delete(&self.0);
            }
        }
        let _cleanup = Cleanup(original.id.clone());
        ui.open_identity(Some(original.clone()));
        ui.form.as_mut().unwrap().fields[0] = Input::new("Renamed identity".into());
        ui.save_form_at(&path).unwrap();
        assert_eq!(
            wezterm_ssh::credentials::load(&original.id)
                .unwrap()
                .as_deref(),
            Some("synthetic identity secret")
        );
        ui.open_form(false, None);
        let f = ui.form.as_mut().unwrap();
        f.fields[1] = Input::new("example.invalid".into());
        f.auth.selected_identity = original.id.clone();
        ui.save_form_at(&path).unwrap();
        assert_eq!(ui.store.hosts[0].username, "deploy");
        assert_eq!(ui.store.hosts[0].identity_id, original.id);
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("synthetic identity secret"));
        let saved = HostStore::load_from(&path).unwrap();
        assert_eq!(saved.hosts[0].credential_ref(), original.id);
    }

    #[test]
    fn password_fields_mask_unicode_and_identity_edit_never_prefills_secrets() {
        let paint = InputPaint {
            masked: true,
            input: Input::new("密碼ab".into()),
            focused: true,
        };
        assert_eq!(paint.rendered_value(), "••••");
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.open_identity(Some(Identity {
            id: "opaque".into(),
            label: "Ops".into(),
            username: "root".into(),
        }));
        let form = ui.form.as_ref().unwrap();
        assert_eq!(form.password_index(), Some(2));
        assert!(form.fields[2].value.is_empty());
        assert!(!form.auth.reveal);
        assert_eq!(form.fields[1].value, "root");
    }

    #[test]
    fn host_actions_open_a_menu_below_the_button_without_editing() {
        let host = Host {
            label: "Production".into(),
            address: "192.0.2.1".into(),
            port: 22,
            ..Default::default()
        };
        let mut ui = TermviaiUi::with_store(
            HostStore {
                hosts: vec![host.clone()],
                ..Default::default()
            },
            String::new(),
        );
        ui.hits.push(Hit {
            x: 300.,
            y: 100.,
            w: 28.,
            h: 28.,
            action: Action::HostMenu(0),
        });
        ui.hover = Some(Action::HostMenu(0));
        ui.open_host_menu(0).unwrap();
        assert_eq!(ui.host_menu, Some((host, 300., 134.)));
        assert!(ui.form.is_none());
        assert!(ui.hover.is_none());
        assert!(ui.open_host_menu(1).is_err());
    }

    #[test]
    fn input_range_selection_copies_replaces_and_deletes_unicode() {
        let mut input = Input::new("ab开发cd".into());
        input.move_to(4, false);
        input.move_to(2, true);
        assert_eq!(input.selected_text().as_deref(), Some("开发"));
        input.insert("xy");
        assert_eq!(input.value, "abxycd");
        assert_eq!(input.cursor, 4);
        assert!(input.selection().is_none());
        input.move_to(2, false);
        input.move_to(4, true);
        input.delete(false);
        assert_eq!(input.value, "abcd");
        assert_eq!(input.cursor, 2);
        input.select_all();
        assert_eq!(input.selected_text().as_deref(), Some("abcd"));
        assert!(input.remove_selection());
        assert_eq!(input.value, "");
        input.insert("123213");
        assert_eq!(input.display(true, 20), "123213");
        assert!(!input.display(true, 20).contains('│'));
    }

    #[test]
    fn proportional_input_hit_testing_uses_glyph_midpoints() {
        let hit = InputHit {
            action: Action::Field(1),
            x: 100.,
            start: 2,
            positions: vec![0., 4., 14., 20.],
        };
        assert_eq!(hit.position(90.), 2);
        assert_eq!(hit.position(101.), 2);
        assert_eq!(hit.position(103.), 3);
        assert_eq!(hit.position(110.), 4);
        assert_eq!(hit.position(121.), 5);
    }

    #[test]
    fn error_hints_wrap_unicode_and_preserve_separate_messages() {
        assert_eq!(
            wrap_hint("离线终端\nRetry", 2),
            vec!["离线", "终端", "Re", "tr", "y"]
        );
        assert_eq!(wrap_hint("ABC", 0), vec!["A", "B", "C"]);
    }

    #[test]
    fn grouped_pane_header_places_remove_after_broadcast() {
        let grouped = pane_header_controls(100., 500., true, false);
        assert_eq!(grouped.broadcast_x, 534.);
        assert_eq!(grouped.remove_x, Some(566.));
        assert_eq!(grouped.label_right_inset, 72.);
        assert_eq!(grouped.remove_x.unwrap() - (grouped.broadcast_x + 28.), 4.);

        let single = pane_header_controls(100., 500., false, false);
        assert_eq!(single.broadcast_x, 566.);
        assert_eq!(single.remove_x, None);
        assert_eq!(single.label_right_inset, 40.);
        for width in [76., 100., 104., 240., 500.] {
            for grouped in [false, true] {
                let c = pane_header_controls(100., width, grouped, true);
                assert!(c.error_x.unwrap() >= 100.);
                assert!(c.error_x.unwrap() + 24. <= c.broadcast_x);
                assert!(c.broadcast_x + 28. <= 100. + width);
                if let Some(remove) = c.remove_x {
                    assert!(c.broadcast_x + 28. <= remove);
                }
            }
        }
    }
    #[test]
    fn close_dialog_shields_background_and_tracks_animated_card() {
        for (width, height) in [
            (240., 120.),
            (240., 180.),
            (320., 240.),
            (900., 600.),
            (1866., 1065.),
        ] {
            for progress in [0., 0.5, 1.] {
                let layout = CloseDialogLayout::new(width, height, progress);
                let r = layout.card;
                assert!(r.x >= 0. && r.y >= 0. && r.x + r.w <= width && r.y + r.h <= height);
                assert!(layout.cancel.x >= r.x && layout.accept.x + layout.accept.w <= r.x + r.w);
                assert!(layout.cancel.x + layout.cancel.w < layout.accept.x);
                let hits = layout.hits(width, height);
                assert_eq!(hit_action(&hits, 2., 2.), Some(Action::ConfirmDismiss));
                assert_eq!(
                    hit_action(&hits, r.x + 5., r.y + 5.),
                    Some(Action::ConfirmCard)
                );
                assert_eq!(
                    hit_action(&hits, layout.cancel.x + 4., layout.cancel.y + 4.),
                    Some(Action::ConfirmCancel)
                );
                assert_eq!(
                    hit_action(&hits, layout.accept.x + 4., layout.accept.y + 4.),
                    Some(Action::ConfirmAccept)
                );
            }
        }
    }

    #[test]
    fn held_confirmation_keys_only_execute_on_the_matching_release() {
        for (key, focus, expected) in [
            (KeyCode::Char('\r'), true, CloseKeyAction::Accept),
            (KeyCode::Char('\u{1b}'), false, CloseKeyAction::Cancel),
        ] {
            let mut latch = CloseKeyLatch::default();
            for _ in 0..8 {
                assert_eq!(latch.event(&key, Modifiers::NONE, true, focus), None);
                assert!(latch.pending.is_some());
            }
            assert_eq!(
                latch.event(&KeyCode::Char('x'), Modifiers::NONE, false, focus),
                None
            );
            assert!(latch.pending.is_some());
            assert_eq!(
                latch.event(&key, Modifiers::NONE, false, focus),
                Some(expected)
            );
            assert!(latch.pending.is_none());
            assert_eq!(latch.event(&key, Modifiers::NONE, false, focus), None);
        }
    }

    #[test]
    fn close_confirmation_requires_explicit_focus_for_destructive_enter() {
        assert_eq!(
            close_key_action(&KeyCode::Char('\r'), Modifiers::NONE, false),
            Some(CloseKeyAction::Cancel)
        );
        assert_eq!(
            close_key_action(&KeyCode::Char('\t'), Modifiers::NONE, false),
            Some(CloseKeyAction::Focus(true))
        );
        assert_eq!(
            close_key_action(&KeyCode::Char('\r'), Modifiers::NONE, true),
            Some(CloseKeyAction::Accept)
        );
        assert_eq!(
            close_key_action(&KeyCode::Char('\u{1b}'), Modifiers::NONE, true),
            Some(CloseKeyAction::Cancel)
        );
        for (key, mods) in [
            (KeyCode::Char('y'), Modifiers::NONE),
            (KeyCode::Char('s'), Modifiers::CTRL),
            (KeyCode::Char('\r'), Modifiers::CTRL),
            (KeyCode::Composed("确认".into()), Modifiers::NONE),
        ] {
            assert_eq!(close_key_action(&key, mods, true), None);
        }
    }

    #[test]
    fn tray_clusters_remain_separate_on_narrow_windows() {
        for content_left in [0., SIDEBAR_WIDTH] {
            for width in (320..=1920).step_by(5) {
                let width = width as f32;
                let layout = TrayLayout::new(width, content_left);
                assert!(layout.switch_x >= content_left);
                assert!(layout.switch_x + layout.switch_width < layout.collapse_x);
                assert!(layout.collapse_x + 28. <= width - 12.);
                if layout.title_width > 0. {
                    assert!(content_left + 24. + layout.title_width < layout.switch_x);
                }
                if let Some(x) = layout.selection_x {
                    assert!(x + 108. < layout.switch_x);
                    assert!(layout.members_x + layout.members_width < x);
                } else if layout.members_width > 0. {
                    assert!(layout.members_x + layout.members_width < layout.switch_x);
                }
                let members = TrayMembers::new(
                    &[72., 164., 88., 72., 120.],
                    layout.members_x,
                    layout.members_width,
                    20,
                );
                for (_, x, width) in &members.chips {
                    assert!(*width >= 68.);
                    assert!(*x >= layout.members_x);
                    assert!(*x + *width <= layout.members_x + layout.members_width);
                    if let Some(pager) = members.pager_x {
                        assert!(*x + *width < pager);
                    }
                }
                assert!(members.pager_x.is_none() || !members.chips.is_empty());
            }
        }
    }

    #[test]
    fn termviai_window_minimum_keeps_the_responsive_toolbar_usable() {
        let (width, height) = minimum_window_size();
        assert_eq!((width, height), (720, 480));
        for content_left in [0., SIDEBAR_WIDTH] {
            let layout = TrayLayout::new(width as f32, content_left);
            assert!(layout.members_width >= 68.);
            assert!(layout.switch_x + layout.switch_width < layout.collapse_x);
        }
    }

    #[test]
    fn tray_member_paging_reaches_every_host_and_clamps_after_resize() {
        let widths = [72., 164., 88., 72., 120.];
        let mut seen = std::collections::HashSet::new();
        for requested in 0..20 {
            let members = TrayMembers::new(&widths, 430., 224., requested);
            seen.extend(members.chips.iter().map(|(index, _, _)| *index));
            assert_eq!(
                members.has_next,
                members.start + members.chips.len() < widths.len()
            );
        }
        assert_eq!(seen.len(), widths.len());
        let wider = TrayMembers::new(&widths, 430., 800., usize::MAX);
        assert_eq!(wider.start, 0);
        assert_eq!(wider.chips.len(), widths.len());
        assert!(wider.pager_x.is_none());
        let closed = TrayMembers::new(&widths[..1], 430., 224., usize::MAX);
        assert_eq!(closed.start, 0);
        assert_eq!(closed.chips.len(), 1);
        let long_single = TrayMembers::new(&[164.], 430., 136., usize::MAX);
        assert_eq!(long_single.chips, vec![(0, 430., 136.)]);
        assert!(long_single.pager_x.is_none());
        assert!(TrayMembers::new(&[], 430., 224., 4).chips.is_empty());
        assert!(TrayMembers::new(&widths, 430., 100., 4).pager_x.is_none());
    }

    #[test]
    fn tray_transition_shields_reserved_space_and_drawer_takes_priority() {
        for reserved in [76., 0.] {
            let mut hits = vec![];
            shield_tray(&mut hits, 1200., 720., reserved, SIDEBAR_WIDTH);
            for y in [645., 680., 719.] {
                assert_eq!(
                    hit_action(&hits, 900., y),
                    if reserved > 0. {
                        Some(Action::Drawer)
                    } else {
                        None
                    }
                );
            }
            let drawer = DrawerLayout::new(1200., 720.);
            drawer.capture_hits(&mut hits, 1200.);
            assert_eq!(hit_action(&hits, 750., 680.), Some(Action::Cancel));
            assert_eq!(hit_action(&hits, 900., 680.), Some(Action::Drawer));
        }
    }

    #[test]
    fn sidebar_toggle_preserves_page_and_animates_both_directions() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.page = Page::Keys;
        let start = Instant::now();

        ui.toggle_sidebar(start);
        assert!(ui.page == Page::Keys);
        assert!(!ui.sidebar_expanded);
        assert!(ui.sidebar_motion.is_active(start));
        assert_eq!(ui.sidebar_width(start + Duration::from_millis(300)), 0.);

        let reopen = start + Duration::from_millis(300);
        ui.toggle_sidebar(reopen);
        assert!(ui.page == Page::Keys);
        assert!(ui.sidebar_expanded);
        assert!(ui.sidebar_motion.is_active(reopen));
        assert_eq!(
            ui.sidebar_width(reopen + Duration::from_millis(300)),
            SIDEBAR_WIDTH
        );
    }

    #[test]
    fn settings_drawer_keeps_session_and_exposes_keepalive() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.page = Page::Terminal;
        ui.open_settings(13.0, 30);
        assert!(ui.page == Page::Terminal);
        let form = ui.form.as_ref().unwrap();
        assert!(form.settings);
        assert_eq!(form.fields.len(), 2);
        assert_eq!(form.fields[1].value, "30");
        ui.input().unwrap().insert("16");
        assert_eq!(ui.form.as_ref().unwrap().fields[0].value, "16");
        ui.close_form();
        assert!(ui.input().is_none());
        assert!(ui.page == Page::Terminal);
    }

    #[test]
    fn rename_drawer_accepts_one_name_without_switching_the_terminal_page() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.page = Page::Terminal;
        ui.open_rename(42, "209 + 212".into());
        assert!(ui.page == Page::Terminal);
        let form = ui.form.as_ref().unwrap();
        assert_eq!(form.rename_tab, Some(42));
        assert_eq!(form.fields.len(), 1);
        ui.input().unwrap().insert("Production");
        assert_eq!(ui.form.as_ref().unwrap().fields[0].value, "Production");
        ui.close_form();
        assert!(ui.input().is_none());
        assert!(ui.page == Page::Terminal);
    }

    #[test]
    fn closing_drawer_discards_input_target_and_can_be_reopened() {
        let mut ui = TermviaiUi::with_store(HostStore::default(), String::new());
        ui.open_form(false, None);
        ui.input().unwrap().insert("unsaved host");
        let generation = ui.generation;
        ui.close_form();
        assert!(ui.form.is_none());
        assert!(ui.closing_form.is_some());
        assert!(ui.input().is_none());
        assert!(ui.generation > generation);
        ui.open_form(true, None);
        assert!(ui.closing_form.is_none());
        assert!(ui.form.as_ref().unwrap().key);
        assert!(ui.input().unwrap().value.is_empty());
        ui.discard_form();
        assert!(ui.closing_form.is_none());
        assert_eq!(ui.drawer_motion.value(Instant::now()), 0.);
    }

    #[test]
    fn moving_drawer_hit_regions_follow_its_visible_edge() {
        for scale in [1., 1.25, 1.5, 2.] {
            let mut layout = DrawerLayout::new(1200., 720.);
            layout.x += layout.width * 0.5;
            let mut hits = vec![];
            layout.capture_hits(&mut hits, 1200.);
            let pointer = (950. * scale / scale, 300. * scale / scale);
            assert_eq!(
                hit_action(&hits, pointer.0, pointer.1),
                Some(Action::Cancel)
            );
            assert_eq!(hit_action(&hits, 1050., 300.), Some(Action::Drawer));
        }
    }

    #[test]
    fn drawer_dismissal_does_not_click_through() {
        let layout = DrawerLayout::new(1200., 720.);
        let mut hits = vec![
            Hit {
                x: 0.,
                y: 0.,
                w: 1200.,
                h: 52.,
                action: Action::Drag,
            },
            Hit {
                x: 1080.,
                y: 8.,
                w: 40.,
                h: 36.,
                action: Action::Minimize,
            },
            Hit {
                x: 250.,
                y: 210.,
                w: 240.,
                h: 100.,
                action: Action::Connect(0),
            },
        ];
        layout.capture_hits(&mut hits, 1200.);
        assert_eq!(hit_action(&hits, 300., 240.), Some(Action::Cancel));
        assert_eq!(hit_action(&hits, 120., 300.), Some(Action::Cancel));
        assert_eq!(hit_action(&hits, 799., 300.), Some(Action::Cancel));
        assert_eq!(hit_action(&hits, 800., 300.), Some(Action::Drawer));
        assert_eq!(hit_action(&hits, 900., 600.), Some(Action::Drawer));
        assert_eq!(hit_action(&hits, 100., 20.), Some(Action::Drag));
        assert_eq!(hit_action(&hits, 1090., 20.), Some(Action::Minimize));
        hits.push(Hit {
            x: 820.,
            y: 172.,
            w: 360.,
            h: 34.,
            action: Action::Field(0),
        });
        assert_eq!(hit_action(&hits, 850., 180.), Some(Action::Field(0)));
        // Once the drawer closes, a later click can reach the library again.
        let library = vec![Hit {
            x: 250.,
            y: 210.,
            w: 240.,
            h: 100.,
            action: Action::Connect(0),
        }];
        assert_eq!(hit_action(&library, 300., 240.), Some(Action::Connect(0)));
    }

    #[test]
    fn drawer_layout_and_keyboard_scrolling() {
        for scale in [1., 1.25, 1.5, 2.] {
            for (width, height) in [(320., 480.), (900., 600.), (1440., 900.)] {
                let layout = DrawerLayout::new(width * scale / scale, height * scale / scale);
                assert_eq!(layout.x + layout.width, width);
                assert!(layout.x >= 0. && layout.width <= 400.);
                assert!(148. + layout.rows as f32 * 70. <= layout.footer);
                assert!(layout.footer + 28. <= height - 88.);
            }
        }
        for key in [false, true] {
            let mut form = Form {
                fields: (0..if key { 2 } else { 6 })
                    .map(|_| Input::default())
                    .collect(),
                focus: 0,
                key,
                original: None,
                rename_tab: None,
                settings: false,
                first_row: 0,
                reveal_focus: true,
                auth: AuthForm::default(),
            };
            let n = form.fields.len();
            assert_eq!(form.visible_rows(2), 0..2);
            form.focus = n - 1;
            form.reveal_focus = true;
            assert_eq!(form.visible_rows(2), n - 2..n);
            // Scroll explicitly without focus snapping the viewport back.
            form.first_row = 0;
            assert_eq!(form.visible_rows(2), 0..2);
            form.reveal_focus = true;
            assert!(form.visible_rows(2).contains(&form.focus));
            // Growing the window exposes all fields and clamps the old offset.
            assert_eq!(form.visible_rows(8), 0..n);
        }
    }

    #[test]
    fn palette_and_unicode_editing() {
        for c in [
            BG, SIDE, TOP, CARD, FIELD, INK, MUTED, ACCENT, SIGNAL_BG, SIGNAL_INK, "#ad4f60",
        ] {
            color(c);
        }
        let mut input = Input::new("开发host".into());
        input.cursor = 2;
        input.insert("服务器");
        assert_eq!(input.value, "开发服务器host");
        input.delete(true);
        assert_eq!(input.value, "开发服务host");
        input.select_all();
        input.insert("new\nname");
        assert_eq!(input.value, "newname");
        input.cursor = 0;
        input.delete(false);
        assert_eq!(input.value, "ewname");
    }
}
