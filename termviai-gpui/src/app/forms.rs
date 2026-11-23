use super::*;
use anyhow::{Context as _, ensure};
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    radio::Radio,
};
impl AppView {
    fn validate_form_fields(&self, f: &Form, cx: &Context<Self>) -> BTreeMap<&'static str, String> {
        let mut errors = BTreeMap::new();
        let required: Vec<(&'static str, &'static str)> = match &f.kind {
            FormKind::Host(_) => vec![("address", "Enter an IP address or hostname.")],
            FormKind::Key(_) => vec![
                ("label", "Enter a key name."),
                ("path", "Enter a private key path."),
            ],
            FormKind::Identity(old) => {
                if old.is_none() {
                    vec![
                        ("label", "Enter an identity name."),
                        ("user", "Enter a username."),
                        ("password", "Enter a password."),
                    ]
                } else {
                    vec![
                        ("label", "Enter an identity name."),
                        ("user", "Enter a username."),
                    ]
                }
            }
            FormKind::Group(_)
            | FormKind::RenameWorkspace(_)
            | FormKind::RenameTabGroup(_)
            | FormKind::Rename(_)
            | FormKind::Forward(_) => {
                vec![("label", "Enter a name.")]
            }
            _ => vec![],
        };
        for (key, message) in required {
            let value = self.input(f, key, cx);
            if (key == "password" && value.is_empty())
                || (key != "password" && value.trim().is_empty())
            {
                errors.insert(key, message.into());
            }
        }
        for key in ["port", "remote", "local"] {
            if f.fields.contains_key(key)
                && self
                    .input(f, key, cx)
                    .parse::<u16>()
                    .ok()
                    .filter(|p| *p > 0)
                    .is_none()
            {
                errors.insert(key, "Enter a port from 1 to 65535.".into());
            }
        }
        errors
    }
    fn monitor_host_picker(&self, f: &Form, cx: &mut Context<Self>) -> AnyElement {
        let replacing = match &f.kind {
            FormKind::MonitorReplace(h) => Some(h),
            _ => None,
        };
        let query = self.input(f, "search", cx).to_lowercase();
        let matches = |h: &&Host| {
            format!(
                "{} {} {} {}",
                h.label,
                h.address,
                h.username,
                self.store.group_path(&h.group_id)
            )
            .to_lowercase()
            .contains(&query)
        };
        let all_available: Vec<_> = self
            .store
            .hosts
            .iter()
            .filter(|h| self.monitor.can_select(h, replacing))
            .collect();
        let mut available: Vec<_> = all_available.iter().copied().filter(matches).collect();
        available.sort_by_key(|h| (self.store.group_path(&h.group_id), h.label.clone()));
        let added: Vec<_> = self
            .store
            .hosts
            .iter()
            .filter(|h| !self.monitor.can_select(h, replacing))
            .filter(matches)
            .collect();
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.muted(if replacing.is_some() {
                "Replace this monitor's saved host reference."
            } else {
                "Choose from hosts not yet in Monitor."
            }))
            .child(self.field(f, "search", "Search hosts"));
        if self.store.hosts.is_empty() {
            body = body.child(
                self.empty_state(
                    IconName::SquareTerminal,
                    "No saved hosts yet",
                    "Add an SSH host in Hosts, then return here to monitor it.",
                )
                .child(
                    self.action_button("picker-create-host", "Add host", IconName::Plus)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.page = Page::Hosts;
                            s.open_form(FormKind::Host(None), w, cx);
                        })),
                ),
            );
        } else if available.is_empty() {
            body = body.child(if query.is_empty() && all_available.is_empty() {
                self.empty_state(
                    IconName::CircleCheck,
                    "All hosts are already monitored",
                    "Newly saved hosts will appear here.",
                )
            } else {
                self.empty_state(
                    IconName::Search,
                    "No available hosts match",
                    "Try a host name, address or group.",
                )
                .child(
                    self.button("picker-clear-search", "Clear search")
                        .on_click(cx.listener(|s, _, w, cx| {
                            if let Some(input) = s
                                .drawer
                                .as_ref()
                                .and_then(|f| f.fields.get("search"))
                                .cloned()
                            {
                                input.update(cx, |i, cx| i.set_value("", w, cx));
                            }
                        })),
                )
            });
        }
        let mut group: Option<String> = None;
        for (i, h) in available.into_iter().enumerate() {
            let path = self.store.group_path(&h.group_id);
            if group.as_ref() != Some(&path) {
                body = body.child(
                    self.muted(if path.is_empty() {
                        "All Hosts".to_owned()
                    } else {
                        path.clone()
                    })
                    .mt_3(),
                );
                group = Some(path);
            }
            let host = h.clone();
            let selected = f.selected.iter().any(|p| monitor::same_host(p, h));
            body = body.child(
                div()
                    .id(("monitor-host-choice", i))
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(self.c(if selected {
                        self.col().soft
                    } else {
                        self.col().bg
                    }))
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .child(
                        div()
                            .size(px(16.))
                            .border_1()
                            .rounded_sm()
                            .border_color(self.c(self.col().muted))
                            .flex_shrink_0()
                            .when(selected, |d| {
                                d.child(
                                    Icon::new(IconName::Check)
                                        .size(px(14.))
                                        .text_color(self.c(self.col().accent)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(h.label.clone())
                            .child(self.muted(format!("{}@{}", h.username, h.endpoint()))),
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        let Some(f) = s.drawer.as_ref() else { return };
                        let old = match &f.kind {
                            FormKind::MonitorReplace(h) => Some(h),
                            _ => None,
                        };
                        if !s.monitor.can_select(&host, old) {
                            return;
                        }
                        s.edit_form(
                            None,
                            |f| {
                                if matches!(f.kind, FormKind::MonitorReplace(_)) {
                                    f.selected = vec![host.clone()];
                                } else if let Some(i) =
                                    f.selected.iter().position(|h| monitor::same_host(h, &host))
                                {
                                    f.selected.remove(i);
                                } else {
                                    f.selected.push(host.clone());
                                }
                            },
                            cx,
                        );
                    })),
            );
        }
        if !added.is_empty() {
            body = body.child(
                div()
                    .mt_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(self.c(self.col().line))
                    .child(
                        self.action_button(
                            "monitor-added-disclosure",
                            format!("Already in Monitor · {}", added.len()),
                            if f.show_added {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            },
                        )
                        .bg(self.c(self.col().bg))
                        .border_0()
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.edit_form(None, |f| f.show_added = !f.show_added, cx)
                        })),
                    ),
            );
            if f.show_added {
                body = body.children(added.iter().map(|h| {
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(15.))
                                .text_color(self.c(self.col().green)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(self.muted(h.label.clone()))
                                .child(self.muted(h.endpoint())),
                        )
                        .child(self.muted("Added"))
                }));
            }
        }
        if replacing.is_some() {
            body = body.child(self.muted("Changes only this monitor's host reference. Saved hosts and terminal sessions stay unchanged.").mt_3());
        }
        body.into_any_element()
    }

    pub(super) fn make_form(&self, kind: FormKind, w: &mut Window, cx: &mut Context<Self>) -> Form {
        let mut f = Form {
            shortcut_editor: None,
            kind: kind.clone(),
            fields: BTreeMap::new(),
            mode: Authentication::Password,
            keychain: false,
            key: String::new(),
            identity: String::new(),
            group: self.group.clone(),
            group_list_open: false,
            highlighted_group: None,
            selected: vec![],
            error: String::new(),
            theme: self.theme,
            confirm: self.settings.confirm_close(),
            bell: self.settings.bell(),
            npu_monitor_enabled: self.settings.npu_monitor_enabled,
            network: false,
            retry: true,
            challenges: vec![],
            challenge_stamp: 0,
            show_added: false,
            field_errors: BTreeMap::new(),
        };
        let mut fields: Vec<(&'static str, String, &'static str, bool)> = vec![];
        match &kind {
            FormKind::Host(original) => {
                let h = original.clone().unwrap_or_else(|| Host {
                    port: 22,
                    group_id: self.group.clone(),
                    ..Host::default()
                });
                f.mode = h.authentication_mode();
                f.key = h.key_label.clone();
                f.keychain = !h.key_label.is_empty();
                f.identity = h.identity_id.clone();
                f.group = self.store.host_group_id(&h).to_owned();
                fields = vec![
                    ("label", h.label.clone(), "Host label", false),
                    (
                        "group",
                        self.store.group_path(&f.group),
                        "Search or enter a group name",
                        false,
                    ),
                    (
                        "address",
                        h.address.clone(),
                        "Hostname or IP address",
                        false,
                    ),
                    ("port", h.port.to_string(), "22", false),
                    ("user", h.username.clone(), "Username", false),
                    (
                        "password",
                        String::new(),
                        if h.credential_ref().is_empty() {
                            "Password"
                        } else {
                            "••••••••"
                        },
                        true,
                    ),
                    (
                        "key_user",
                        if h.key_username.is_empty() {
                            h.username
                        } else {
                            h.key_username
                        },
                        "Username",
                        false,
                    ),
                    ("path", h.identity_file, "Private key path", false),
                ];
            }
            FormKind::Auth(id) => {
                let h = self.panes[id].host.clone().unwrap_or_default();
                fields = vec![
                    ("user", h.username, "Username", false),
                    ("password", String::new(), "Password", true),
                    ("key_user", h.key_username, "Username", false),
                    ("path", h.identity_file, "Private key path", false),
                ];
                f.key = h.key_label;
                f.identity = h.identity_id;
                f.keychain = true;
            }
            FormKind::Key(k) => {
                fields = vec![
                    (
                        "label",
                        k.as_ref().map(|k| k.label.clone()).unwrap_or_default(),
                        "Key name",
                        false,
                    ),
                    (
                        "path",
                        k.as_ref().map(|k| k.path.clone()).unwrap_or_default(),
                        "Private key path",
                        false,
                    ),
                ];
            }
            FormKind::Identity(i) => {
                fields = vec![
                    (
                        "label",
                        i.as_ref().map(|i| i.label.clone()).unwrap_or_default(),
                        "Identity name",
                        false,
                    ),
                    (
                        "user",
                        i.as_ref().map(|i| i.username.clone()).unwrap_or_default(),
                        "Username",
                        false,
                    ),
                    (
                        "password",
                        String::new(),
                        if i.is_some() {
                            "••••••••"
                        } else {
                            "Password"
                        },
                        true,
                    ),
                ];
            }
            FormKind::Group(parent) => {
                f.group = parent.clone();
                fields.push(("label", String::new(), "Group name", false));
            }
            FormKind::RenameWorkspace(entry) => {
                fields.push(("label", entry.name.clone(), "Workspace name", false))
            }
            FormKind::RenameTabGroup(id) => fields.push((
                "label",
                self.tab_groups[*id].name.clone(),
                "Group name",
                false,
            )),
            FormKind::Settings => {
                fields = vec![
                    (
                        "size",
                        self.settings.terminal_font_size.unwrap_or(12.).to_string(),
                        "Font size (pt)",
                        false,
                    ),
                    (
                        "keepalive",
                        self.settings.keepalive().to_string(),
                        "Seconds · 0 disables",
                        false,
                    ),
                ]
            }
            FormKind::Rename(id) => fields.push((
                "label",
                self.tabs
                    .iter()
                    .find(|t| t.id == *id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                "Tab name",
                false,
            )),
            FormKind::Existing(group) => {
                f.group = group.clone();
                fields.push(("search", String::new(), "Search hosts", false));
            }
            FormKind::Monitor | FormKind::MonitorReplace(_) => {
                fields.push(("search", String::new(), "Search hosts", false))
            }
            FormKind::Forward(original) => {
                let r = original.clone();
                if let Some(r) = &r {
                    f.selected = vec![r.host.clone()];
                    f.network = r.network;
                    f.retry = r.retry;
                }
                fields = vec![
                    (
                        "label",
                        r.as_ref().map(|r| r.name.clone()).unwrap_or_default(),
                        "Service name",
                        false,
                    ),
                    (
                        "remote",
                        r.as_ref()
                            .map(|r| r.remote_port)
                            .unwrap_or(18080)
                            .to_string(),
                        "Remote port",
                        false,
                    ),
                    (
                        "local",
                        r.as_ref().map(|r| r.local_port).unwrap_or(8787).to_string(),
                        "Local port",
                        false,
                    ),
                    ("search", String::new(), "Search all saved hosts", false),
                ];
            }
        }
        for (key, value, placeholder, masked) in fields {
            let input = cx.new(|cx| {
                let mut s = InputState::new(w, cx)
                    .placeholder(placeholder)
                    .masked(masked);
                s.set_value(value, w, cx);
                s
            });
            if key == "group" {
                use gpui_component::input::InputEvent;
                cx.subscribe_in(&input, w, |s, field, event: &InputEvent, w, cx| {
                    let Some(f) = &mut s.drawer else { return };
                    if f.fields.get("group") != Some(&field) {
                        return;
                    }
                    match event {
                        InputEvent::Focus => f.group_list_open = true,
                        InputEvent::Blur => f.group_list_open = false,
                        InputEvent::Change => {
                            // Setting a picked path writes to the same editable input.
                            if !f.group.is_empty()
                                && field.read(cx).value().as_str() == s.store.group_path(&f.group)
                            {
                                return;
                            }
                            f.group.clear();
                            f.group_list_open = field.focus_handle(cx).is_focused(w);
                        }
                        _ => return,
                    }
                    f.highlighted_group = None;
                    s.scrollers["group-picker"].set_offset(point(px(0.), px(0.)));
                    cx.notify();
                })
                .detach();
            }
            cx.observe(&input, |_, _, cx| cx.notify()).detach();
            f.fields.insert(key, input);
        }
        f
    }
    pub(super) fn open_form(&mut self, kind: FormKind, w: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            &kind,
            FormKind::Host(Some(_)) | FormKind::RenameWorkspace(_) | FormKind::Existing(_)
        ) {
            self.host_selection.clear();
        } else {
            self.clear_page_search(w, cx);
        }
        self.menu = None;
        if matches!(kind, FormKind::RenameTabGroup(_)) {
            self.sidebar = true;
        }
        self.scrollers["drawer-scroll"].set_offset(point(px(0.), px(0.)));
        self.drawer = Some(self.make_form(kind, w, cx));
        if let Some(f) = &self.drawer {
            if let Some(first) = f
                .fields
                .get("label")
                .or_else(|| f.fields.get("search"))
                .or_else(|| f.fields.values().next())
            {
                first.focus_handle(cx).focus(w);
            }
        }
        cx.notify();
    }
    pub(super) fn close_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self
            .drawer
            .as_ref()
            .is_some_and(|f| matches!(f.kind, FormKind::Settings))
        {
            self.apply_theme(self.settings.theme, w, cx);
        }
        self.drawer = None;
        cx.notify();
    }
    fn edit_form(
        &mut self,
        pane: Option<usize>,
        change: impl FnOnce(&mut Form),
        cx: &mut Context<Self>,
    ) {
        let f = if let Some(id) = pane {
            self.panes.get_mut(&id).and_then(|p| p.form.as_mut())
        } else {
            self.drawer.as_mut()
        };
        if let Some(f) = f {
            change(f);
        }
        cx.notify();
    }
    fn setting_toggle(
        &self,
        label: &'static str,
        checked: bool,
        set: fn(&mut Form, bool),
        cx: &mut Context<Self>,
    ) -> Radio {
        Radio::new(label)
            .label(label)
            .checked(checked)
            .w_full()
            .min_h(px(32.))
            .flex_row_reverse()
            .items_center()
            .on_click(cx.listener(move |s, checked: &bool, _, cx| {
                s.edit_form(None, |f| set(f, *checked), cx)
            }))
    }

    fn settings_section(&self, title: &'static str) -> Div {
        div()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(self.c(self.col().line))
            .bg(self.c(self.col().card))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
    }

    fn setting_number(
        &self,
        f: &Form,
        key: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let field = &f.fields[key];
        let raw = self.input(f, key, cx);
        let (value, step, min, max) = if key == "size" {
            (
                raw.parse::<f64>().ok().filter(|v| v.is_finite()),
                0.5,
                6.,
                48.,
            )
        } else {
            (raw.parse::<u64>().ok().map(|v| v as f64), 1., 0., 86400.)
        };
        let increment = |increase: bool| {
            let next = value.map(|v| (v + if increase { step } else { -step }).clamp(min, max));
            let direction = if increase { "Increase" } else { "Decrease" };
            let key_id = SharedString::from(format!("{key}-{direction}"));
            let field = field.clone();
            Button::new(key_id)
                .ghost()
                .small()
                .icon(if increase {
                    IconName::Plus
                } else {
                    IconName::Minus
                })
                .tooltip(format!("{direction} {label}"))
                .disabled(next.is_none() || next == value)
                .on_click(move |_, w, cx| {
                    if let Some(value) = next {
                        field.update(cx, |i, cx| i.set_value(value.to_string(), w, cx));
                    }
                })
        };
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .justify_between()
            .gap_2()
            .child(label)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(increment(false))
                    .child(
                        div()
                            .w(px(72.))
                            .child(Input::new(field).bg(self.c(self.col().field))),
                    )
                    .child(increment(true)),
            )
    }

    fn field(&self, f: &Form, key: &str, label: &str) -> AnyElement {
        let Some(input) = f.fields.get(key) else {
            return div().into_any_element();
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label.to_owned()),
            )
            .child(
                Input::new(input)
                    .bg(self.c(self.col().field))
                    .when(key == "password", |i| i.mask_toggle())
                    .when(f.field_errors.contains_key(key), |i| {
                        i.border_color(self.c(self.col().red))
                    }),
            )
            .when_some(f.field_errors.get(key), |d, message| {
                d.child(
                    self.muted(message.clone())
                        .text_color(self.c(self.col().red)),
                )
            })
            .into_any_element()
    }
    fn authentication_fields(
        &self,
        f: &Form,
        pane: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Authentication"),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .p_1()
                    .rounded_md()
                    .bg(self.c(self.col().field))
                    .children(
                        [
                            (Authentication::Password, "Password"),
                            (Authentication::Key, "SSH Key"),
                            (Authentication::Identity, "Identity"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(i, (mode, label))| {
                            div()
                                .id(("auth-mode", i))
                                .flex_1()
                                .py_2()
                                .rounded_md()
                                .flex()
                                .justify_center()
                                .cursor_pointer()
                                .when(f.mode == mode, |d| {
                                    d.bg(self.c(self.col().soft))
                                        .text_color(self.c(self.col().accent))
                                })
                                .hover(|s| s.bg(self.c(self.col().hover)))
                                .child(label)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.edit_form(pane, |f| f.mode = mode, cx)
                                }))
                        }),
                    ),
            );
        match f.mode {
            Authentication::Password => {
                body = body
                    .child(self.field(f, "user", "Username"))
                    .child(self.field(f, "password", "Password"));
            }
            Authentication::Key => {
                body = body.child(self.field(f, "key_user", "Username")).child(
                    div().flex().gap_2().children(
                        [(false, "Key path"), (true, "Keychain")].into_iter().map(
                            |(keychain, label)| {
                                self.button(label, label)
                                    .when(f.keychain == keychain, |d| {
                                        d.text_color(self.c(self.col().accent))
                                    })
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.edit_form(pane, |f| f.keychain = keychain, cx)
                                    }))
                            },
                        ),
                    ),
                );
                if f.keychain {
                    body = body.child(self.key_picker(f, pane, cx));
                } else {
                    body = body.child(self.field(f, "path", "Private key path"));
                }
            }
            Authentication::Identity => {
                body = body.child(
                    div()
                        .id("identity-picker")
                        .max_h(px(220.))
                        .relative()
                        .overflow_y_scroll()
                        .track_scroll(&self.scrollers["identity-picker"])
                        .vertical_scrollbar(&self.scrollers["identity-picker"])
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.store.identities.iter().map(|i| {
                            let id = i.id.clone();
                            self.choice(&id, &i.label, &i.username, f.identity == id)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.edit_form(pane, |f| f.identity = id.clone(), cx)
                                }))
                        }))
                        .when(self.store.identities.is_empty(), |d| {
                            d.child(self.compact_state(
                                IconName::CircleUser,
                                "No saved identities",
                                "Add an identity in Keychain to use it here.",
                            ))
                        }),
                );
            }
        }
        body.into_any_element()
    }
    pub(super) fn choice(
        &self,
        id: &str,
        title: &str,
        detail: &str,
        selected: bool,
    ) -> Stateful<Div> {
        self.choice_with_colors(id, title, detail, selected, self.col())
    }

    fn choice_with_colors(
        &self,
        id: &str,
        title: &str,
        detail: &str,
        selected: bool,
        colors: &Colors,
    ) -> Stateful<Div> {
        div()
            .id(SharedString::from(id.to_owned()))
            .flex()
            .gap_3()
            .items_center()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(Colors::color(if selected {
                colors.accent
            } else {
                colors.line
            }))
            .cursor_pointer()
            .hover(|d| d.bg(Colors::color(colors.hover)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(title.to_owned())
                    .when(!detail.is_empty(), |d| {
                        d.child(self.muted(detail.to_owned()).truncate())
                    }),
            )
            .when(selected, |d| {
                d.child(
                    Icon::new(IconName::Check)
                        .size(px(16.))
                        .text_color(Colors::color(colors.accent)),
                )
            })
    }
    fn key_picker(&self, f: &Form, pane: Option<usize>, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("key-picker")
            .max_h(px(220.))
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scrollers["key-picker"])
            .vertical_scrollbar(&self.scrollers["key-picker"])
            .flex()
            .flex_col()
            .gap_2()
            .children(self.store.keys.iter().map(|k| {
                let label = k.label.clone();
                self.choice(&label, &label, &k.path, k.matches_label(&f.key))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.edit_form(pane, |f| f.key = label.clone(), cx)
                    }))
            }))
            .when(self.store.keys.is_empty(), |d| {
                d.child(self.compact_state(
                    IconName::File,
                    "No saved keys",
                    "Add a private key path in Keychain to use it here.",
                ))
            })
            .into_any_element()
    }
    pub(super) fn form_content(
        &self,
        f: &Form,
        pane: Option<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        match &f.kind {
            FormKind::Host(_) => {
                body = body
                    .child(self.field(f, "label", "Label"))
                    .child(self.field(f, "address", "Address"))
                    .child(self.field(f, "port", "Port"))
                    .child(self.authentication_fields(f, pane, cx))
                    .child(div().text_size(px(12.)).child("Group"))
                    .child(self.group_picker(f, cx));
            }
            FormKind::Auth(_) => body = body.child(self.authentication_fields(f, pane, cx)),
            FormKind::Key(_) => {
                body = body.child(self.field(f, "label", "Name")).child(self.field(
                    f,
                    "path",
                    "Private key path",
                ))
            }
            FormKind::Identity(_) => {
                body = body
                    .child(self.field(f, "label", "Name"))
                    .child(self.field(f, "user", "Username"))
                    .child(self.field(f, "password", "Password"))
            }
            FormKind::Group(_)
            | FormKind::RenameWorkspace(_)
            | FormKind::RenameTabGroup(_)
            | FormKind::Rename(_) => body = body.child(self.field(f, "label", "Name")),
            FormKind::Settings => {
                let themes = div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(Theme::ALL.into_iter().map(|theme| {
                        let preview = theme.colors();
                        let selected = f.theme == theme;
                        div()
                            .id(theme.label())
                            .flex()
                            .items_center()
                            .gap_2()
                            .flex_grow()
                            .flex_basis(px(164.))
                            .min_h(px(48.))
                            .p_2()
                            .rounded_md()
                            .border_1()
                            .border_color(self.c(if selected {
                                self.col().accent
                            } else {
                                self.col().line
                            }))
                            .when(selected, |d| d.bg(self.c(self.col().soft)))
                            .cursor_pointer()
                            .hover(|d| d.bg(self.c(self.col().hover)))
                            .child(
                                div()
                                    .size(px(30.))
                                    .flex_shrink_0()
                                    .rounded_md()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(Colors::color(preview.card))
                                    .text_color(Colors::color(preview.accent))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Aa"),
                            )
                            .child(div().min_w_0().flex_1().child(theme.label()))
                            .when(selected, |d| {
                                d.child(
                                    Icon::new(IconName::Check)
                                        .size(px(15.))
                                        .text_color(self.c(self.col().accent)),
                                )
                            })
                            .on_click(cx.listener(move |s, _, w, cx| {
                                if let Some(f) = &mut s.drawer {
                                    f.theme = theme;
                                }
                                s.apply_theme(theme, w, cx);
                            }))
                    }));
                let terminal = self
                    .settings_section("Terminal")
                    .child(self.setting_number(f, "size", "Font size (pt)", cx))
                    .child(self.setting_toggle(
                        "Confirm before closing terminals",
                        f.confirm,
                        |f, on| f.confirm = on,
                        cx,
                    ))
                    .child(self.setting_toggle(
                        "Terminal bell sound",
                        f.bell,
                        |f, on| f.bell = on,
                        cx,
                    ));
                let shortcuts = div()
                    .id("open-shortcuts")
                    .focusable()
                    .tab_stop(true)
                    .w_full()
                    .min_h(px(48.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded_lg()
                    .bg(self.c(self.col().card))
                    .border_1()
                    .border_color(self.c(self.col().line))
                    .cursor_pointer()
                    .hover(|d| d.bg(self.c(self.col().hover)))
                    .focus(|d| d.border_color(self.c(self.col().accent)))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Keyboard shortcuts"),
                    )
                    .child(Icon::new(IconName::ChevronRight).size(px(16.)))
                    .on_click(cx.listener(|s, _, w, cx| s.open_shortcuts(w, cx)));
                let connection = self
                    .settings_section("Connection")
                    .child(self.setting_number(
                        f,
                        "keepalive",
                        "SSH keepalive interval (seconds)",
                        cx,
                    ));
                let monitoring = self
                    .settings_section("Monitoring")
                    .child(self.setting_toggle(
                        "Enable NPU Monitor",
                        f.npu_monitor_enabled,
                        |f, on| f.npu_monitor_enabled = on,
                        cx,
                    ));
                // Flex bases allow both columns to grow, then stack at narrow widths.
                let sections = div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .child(terminal.flex_basis(px(406.)).flex_grow().min_w_0())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .flex_basis(px(406.))
                            .flex_grow()
                            .min_w_0()
                            .child(connection)
                            .child(monitoring),
                    );
                body = body
                    .child(self.settings_section("Appearance").child(themes))
                    .child(sections)
                    .child(shortcuts);
            }
            FormKind::Monitor | FormKind::MonitorReplace(_) => {
                body = body.child(self.monitor_host_picker(f, cx));
            }
            FormKind::Existing(_) | FormKind::Forward(_) => {
                if matches!(f.kind, FormKind::Forward(_)) {
                    body = body
                        .child(self.field(f, "label", "Service name"))
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(div().flex_1().child(self.field(f, "remote", "Remote port")))
                                .child(div().flex_1().child(self.field(f, "local", "Local port"))),
                        )
                        .child(self.muted(format!(
                                "{}:{} → This computer · 127.0.0.1:{}",
                                f.selected
                                    .first()
                                    .map(|h| h.label.as_str())
                                    .unwrap_or("Remote host"),
                                self.input(f, "remote", cx),
                                self.input(f, "local", cx)
                            )));
                    for (which, label, value) in [
                        (0, "Allow network access to remote port", f.network),
                        (1, "Automatically reconnect", f.retry),
                    ] {
                        body = body.child(
                            self.choice(label, label, if value { "On" } else { "Off" }, value)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.edit_form(
                                        None,
                                        |f| {
                                            if which == 0 {
                                                f.network = !f.network
                                            } else {
                                                f.retry = !f.retry
                                            }
                                        },
                                        cx,
                                    )
                                })),
                        );
                    }
                }
                body = body.child(self.field(f, "search", "Choose saved hosts"));
                let query = self.input(f, "search", cx).to_lowercase();
                body = body.children(
                    self.store
                        .hosts
                        .iter()
                        .filter(|h| {
                            format!("{} {}", h.label, h.address)
                                .to_lowercase()
                                .contains(&query)
                        })
                        .enumerate()
                        .map(|(i, h)| {
                            let host = h.clone();
                            let selected = f.selected.contains(h);
                            self.choice(
                                &format!("select-host-{i}"),
                                &h.label,
                                &format!(
                                    "{} · {}",
                                    h.endpoint(),
                                    self.store.group_path(&h.group_id)
                                ),
                                selected,
                            )
                            .on_click(cx.listener(
                                move |s, _, _, cx| {
                                    s.edit_form(
                                        None,
                                        |f| {
                                            if matches!(f.kind, FormKind::Forward(_)) {
                                                f.selected = vec![host.clone()];
                                            } else if let Some(i) =
                                                f.selected.iter().position(|h| h == &host)
                                            {
                                                f.selected.remove(i);
                                            } else {
                                                f.selected.push(host.clone());
                                            }
                                        },
                                        cx,
                                    )
                                },
                            ))
                        }),
                );
            }
        }
        if !f.error.is_empty() {
            body = body.child(self.notice(
                if matches!(f.kind, FormKind::Auth(_)) {
                    "Authentication needs attention"
                } else {
                    "Could not save changes"
                },
                f.error.clone(),
            ));
        }
        body.into_any_element()
    }
    fn matching_groups(&self, f: &Form, cx: &App) -> Vec<(String, String)> {
        let query = if f.group.is_empty() {
            f.fields["group"].read(cx).value().trim().to_lowercase()
        } else {
            String::new()
        };
        let mut options: Vec<_> = self
            .store
            .groups
            .iter()
            .map(|g| (g.id.clone(), self.store.group_path(&g.id)))
            .filter(|(_, path)| path.to_lowercase().contains(&query))
            .collect();
        options.sort_by_cached_key(|(_, path)| path.to_lowercase());
        options
    }

    pub(super) fn pick_host_group(&mut self, id: &str, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = &mut self.drawer {
            let path = self.store.group_path(id);
            f.group = id.to_owned();
            f.fields["group"].update(cx, |input, cx| input.set_value(path, w, cx));
            f.group_list_open = false;
            f.highlighted_group = None;
            cx.notify();
        }
    }

    pub(super) fn host_group_key(
        &mut self,
        key: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(f) = &self.drawer else { return false };
        let Some(input) = f.fields.get("group") else {
            return false;
        };
        if !input.focus_handle(cx).is_focused(w) {
            return false;
        }
        match key {
            "up" | "down" => {
                let count = self.matching_groups(f, cx).len();
                let next = match (count, f.highlighted_group, key) {
                    (0, _, _) => None,
                    (_, Some(i), "up") => Some(i.saturating_sub(1)),
                    (_, Some(i), _) => Some((i + 1).min(count - 1)),
                    (_, None, "up") => Some(count - 1),
                    _ => Some(0),
                };
                if let Some(i) = next {
                    self.scrollers["group-picker"].scroll_to_item(i);
                }
                self.edit_form(
                    None,
                    |f| {
                        f.group_list_open = true;
                        f.highlighted_group = next;
                    },
                    cx,
                );
            }
            "escape" | "enter" if f.group_list_open => {
                if key == "enter"
                    && let Some(index) = f.highlighted_group
                    && let Some((id, _)) = self.matching_groups(f, cx).get(index)
                {
                    self.pick_host_group(id, w, cx);
                } else {
                    self.edit_form(
                        None,
                        |f| {
                            f.group_list_open = false;
                            f.highlighted_group = None;
                        },
                        cx,
                    );
                }
            }
            _ => return false,
        }
        true
    }

    fn group_picker(&self, f: &Form, cx: &mut Context<Self>) -> AnyElement {
        let field = &f.fields["group"];
        let toggle = self
            .icon("host-group-toggle", IconName::ChevronDown)
            .on_mouse_down(MouseButton::Left, preserve_input_focus)
            .on_click(cx.listener(|s, _, w, cx| {
                cx.stop_propagation();
                if let Some(f) = &mut s.drawer {
                    f.group_list_open = !f.group_list_open;
                    f.highlighted_group = None;
                    f.fields["group"].focus_handle(cx).focus(w);
                    cx.notify();
                }
            }));
        let input = Input::new(field)
            .suffix(toggle)
            .bg(self.c(self.col().field));
        div()
            .id("host-group-combobox")
            .key_context("TermViAIHostGroup")
            .relative()
            .child(input)
            .on_click(cx.listener(|s, _, _, cx| {
                if let Some(f) = &mut s.drawer
                    && !f.group_list_open
                {
                    f.group_list_open = true;
                    cx.notify();
                }
            }))
            .when(f.group_list_open, |d| {
                let groups = self.matching_groups(f, cx);
                let empty = groups.is_empty();
                let list = div()
                    .id("group-picker")
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scrollers["group-picker"])
                    .flex()
                    .flex_col()
                    .children(groups.into_iter().enumerate().map(|(index, (id, path))| {
                        div()
                            .id(("group-option", index))
                            .flex_shrink_0()
                            .h(px(30.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded_md()
                            .cursor_pointer()
                            .when(f.highlighted_group == Some(index), |d| {
                                d.bg(self.c(self.col().soft))
                            })
                            .hover(|d| d.bg(self.c(self.col().hover)))
                            .child(div().flex_1().truncate().child(path))
                            .when(f.group == id, |d| {
                                d.child(Icon::new(IconName::Check).size(px(14.)))
                            })
                            .on_mouse_down(MouseButton::Left, preserve_input_focus)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                cx.stop_propagation();
                                s.pick_host_group(&id, w, cx);
                            }))
                    }))
                    .when(empty, |d| {
                        d.child(
                            self.muted(if field.read(cx).value().trim().is_empty() {
                                "Type a name to create a group."
                            } else {
                                "Save to create this group."
                            })
                            .px_2()
                            .py_1(),
                        )
                    });
                // Group is the last field. Open toward the form so Save stays accessible.
                d.child(deferred(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(relative(1.))
                        .occlude()
                        .bg(self.c(self.col().card))
                        .rounded_md()
                        .border_1()
                        .border_color(self.c(self.col().line))
                        .shadow_md()
                        .p_1()
                        .child(list)
                        .vertical_scrollbar(&self.scrollers["group-picker"]),
                ))
            })
            .into_any_element()
    }

    pub(super) fn name_editor(&self, f: &Form, cx: &mut Context<Self>) -> Div {
        let error = f
            .field_errors
            .get("label")
            .cloned()
            .unwrap_or_else(|| f.error.clone());
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(Input::new(&f.fields["label"])),
                    )
                    .child(
                        self.icon("commit-name", IconName::Check)
                            .on_click(cx.listener(|s, _, w, cx| s.save_form(w, cx))),
                    ),
            )
            .when(!error.is_empty(), |d| {
                d.child(self.muted(error).text_color(self.c(self.col().red)))
            })
    }
    pub(super) fn inline_credential(&self, cx: &mut Context<Self>) -> AnyElement {
        let f = self.drawer.as_ref().unwrap();
        let (title, delete) = match &f.kind {
            FormKind::Key(key) => ("Key", key.clone().map(Confirm::Key)),
            FormKind::Identity(identity) => ("Identity", identity.clone().map(Confirm::Identity)),
            _ => return div().into_any_element(),
        };
        div()
            .w_full()
            .min_w_0()
            .rounded_lg()
            .border_1()
            .border_color(self.c(self.col().accent))
            .bg(self.c(self.col().card))
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(title)
            .child(self.form_content(f, None, cx))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        self.button("save-credential", "Save")
                            .on_click(cx.listener(|s, _, w, cx| s.save_form(w, cx))),
                    )
                    .child(
                        self.button("cancel-credential", "Cancel")
                            .on_click(cx.listener(|s, _, w, cx| s.close_form(w, cx))),
                    )
                    .when_some(delete, |d, action| {
                        d.child(
                            self.button("delete-credential", "Delete")
                                .text_color(self.c(self.col().red))
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.confirm = Some(action.clone());
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(super) fn drawer_view(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(f) = &self.drawer else {
            return div().into_any_element();
        };
        if matches!(
            f.kind,
            FormKind::Key(_)
                | FormKind::Identity(_)
                | FormKind::Rename(_)
                | FormKind::RenameWorkspace(_)
                | FormKind::RenameTabGroup(_)
                | FormKind::Group(_)
        ) {
            return div().into_any_element();
        }
        if matches!(f.kind, FormKind::Settings) {
            if let Some(editor) = &f.shortcut_editor {
                return self.shortcut_page(editor, window, cx);
            }
            let header = self
                .tool_header(window)
                .child(self.title("Settings"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            self.button("save", "Save")
                                .min_w(px(80.))
                                .h(px(38.))
                                .bg(self.c(self.col().soft))
                                .on_click(cx.listener(|s, _, w, cx| s.save_form(w, cx))),
                        )
                        .child(
                            self.button("cancel", "Cancel")
                                .min_w(px(80.))
                                .h(px(38.))
                                .on_click(cx.listener(|s, _, w, cx| s.close_form(w, cx))),
                        ),
                );
            let body = self
                .tool_content(window)
                .child(self.form_content(f, None, cx));
            return self
                .tool_page("drawer-scroll", header, body)
                .into_any_element();
        }
        let title = match &f.kind {
            FormKind::Host(Some(_)) => "Edit Host",
            FormKind::Host(None) => "Add Host",
            FormKind::Key(Some(_)) => "Edit Key",
            FormKind::Key(None) => "Add Key",
            FormKind::Identity(Some(_)) => "Edit Identity",
            FormKind::Identity(None) => "Add Identity",
            FormKind::Group(_) => "Add Group",
            FormKind::RenameTabGroup(_) => "Rename Tab Group",
            FormKind::RenameWorkspace(_) => "Rename Workspace",
            FormKind::Settings => "Settings",
            FormKind::Rename(_) => "Rename Tab",
            FormKind::Existing(_) => "Add Existing Hosts",
            FormKind::Monitor => "Add to Monitor",
            FormKind::MonitorReplace(_) => "Choose current host",
            FormKind::Forward(Some(_)) => "Edit Service",
            FormKind::Forward(None) => "Add Service",
            _ => "Authentication",
        };
        let monitor_form = matches!(f.kind, FormKind::Monitor | FormKind::MonitorReplace(_));
        let replacement = match &f.kind {
            FormKind::MonitorReplace(h) => Some(h),
            _ => None,
        };
        let selected_count = f
            .selected
            .iter()
            .filter(|h| self.monitor.can_select(h, replacement) && self.store.hosts.contains(h))
            .count();
        let save_enabled = !monitor_form || selected_count > 0;
        let save_label = if replacement.is_some() {
            "Use host".to_owned()
        } else if monitor_form {
            format!(
                "Add {selected_count} {}",
                if selected_count == 1 { "host" } else { "hosts" }
            )
        } else {
            "Save".to_owned()
        };
        let delete = match &f.kind {
            FormKind::Key(Some(k)) => Some(Confirm::Key(k.clone())),
            FormKind::Identity(Some(i)) => Some(Confirm::Identity(i.clone())),
            FormKind::Host(Some(h)) => Some(Confirm::Host(h.clone())),
            FormKind::Forward(Some(r)) => Some(Confirm::Forward(r.id.clone())),
            _ => None,
        };
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("form-backdrop")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(rgba(0x00000066))
                    .on_click(cx.listener(|s, _, w, cx| s.close_form(w, cx))),
            )
            .child(
                div()
                    .id("drawer-panel")
                    .relative()
                    .flex_shrink_0()
                    .occlude()
                    .w(px(560.))
                    .max_w_full()
                    .h(relative(0.84))
                    .max_h(px(720.))
                    .rounded_lg()
                    .border_1()
                    .bg(self.c(self.col().bg))
                    .border_color(self.c(self.col().line))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p_5()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(
                                self.close("close-drawer")
                                    .on_click(cx.listener(|s, _, w, cx| s.close_form(w, cx))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .child(
                                div()
                                    .id("drawer-scroll")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.scrollers["drawer-scroll"])
                                    .child(
                                        div().px_5().pb_5().child(self.form_content(f, None, cx)),
                                    ),
                            )
                            // Keep the scrollbar overlay out of the measured scroll content.
                            .vertical_scrollbar(&self.scrollers["drawer-scroll"]),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_4()
                            .border_t_1()
                            .border_color(self.c(self.col().line))
                            .flex_shrink_0()
                            .when_some(delete, |d, action| {
                                d.child(
                                    self.button("delete-item", "Delete")
                                        .text_color(self.c(self.col().red))
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            s.confirm = Some(action.clone());
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(monitor_form, |d| {
                                d.child(self.muted(format!("{selected_count} selected")))
                            })
                            .child(div().flex_1())
                            .child(
                                self.button("cancel", "Cancel")
                                    .on_click(cx.listener(|s, _, w, cx| s.close_form(w, cx))),
                            )
                            .child(
                                self.button("save", save_label)
                                    .when(!save_enabled, |d| d.opacity(0.45).cursor_default())
                                    .when(save_enabled, |d| {
                                        d.hover(|d| {
                                            d.bg(self.c(self.col().soft))
                                                .text_color(self.c(self.col().accent))
                                        })
                                    })
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        if save_enabled {
                                            s.save_form(w, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn rename_saved_workspace(&mut self, id: &str, name: &str) -> anyhow::Result<()> {
        self.workspaces.rename_saved(id, name)?;
        for tab in self
            .tabs
            .iter_mut()
            .filter(|t| t.saved.as_deref() == Some(id))
        {
            tab.name = name.trim().to_owned();
            tab.custom_name = true;
        }
        Ok(())
    }

    pub(super) fn save_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = self.drawer.clone() else { return };
        let errors = self.validate_form_fields(&f, cx);
        if let Some(form) = &mut self.drawer {
            form.field_errors = errors.clone();
            form.error.clear();
        }
        if let Some(key) = errors.keys().next() {
            if let Some(input) = f.fields.get(key) {
                input.focus_handle(cx).focus(w);
            }
            cx.notify();
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            match &f.kind {
                FormKind::Host(original) => {
                    let mut h = original.clone().unwrap_or_default();
                    h.label = self.input(&f, "label", cx);
                    h.address = self.input(&f, "address", cx);
                    h.port = self
                        .input(&f, "port", cx)
                        .parse()
                        .context("Port must be 1–65535")?;
                    h.authentication = Some(f.mode);
                    // Each editor section owns only its own credential slot.
                    match f.mode {
                        Authentication::Password => h.username = self.input(&f, "user", cx),
                        Authentication::Key => {
                            h.key_username = self.input(&f, "key_user", cx);
                            if f.keychain {
                                h.key_label = f.key.clone();
                                h.identity_file.clear();
                            } else {
                                h.key_label.clear();
                                h.identity_file = self.input(&f, "path", cx);
                            }
                        }
                        Authentication::Identity => h.identity_id = f.identity.clone(),
                    }
                    h.validate()?;
                    if f.mode != Authentication::Password {
                        let mut check = h.clone();
                        check.resolve_identity(&self.store)?;
                    }
                    let password = self.input(&f, "password", cx);
                    let created = if f.mode == Authentication::Password && !password.is_empty() {
                        let id = new_credential_id();
                        wezterm_ssh::credentials::save(&id, &password, true)?;
                        h.credential_id = id.clone();
                        h.session_credential.clear();
                        Some(id)
                    } else {
                        None
                    };
                    let group_text = self.input(&f, "group", cx);
                    let selected =
                        !f.group.is_empty() && self.store.group_path(&f.group) == group_text;
                    let result = self.change_store(|s| {
                        if selected {
                            ensure!(
                                s.group(&f.group).is_some(),
                                "Group changed or deleted; choose it again"
                            );
                            h.group_id = f.group.clone();
                            h.group = s.group_path(&f.group);
                        } else {
                            s.assign_host_group(&mut h, &group_text, original.as_ref())?;
                        }
                        if let Some(original) = original {
                            let i = s
                                .hosts
                                .iter()
                                .position(|x| x == original)
                                .context("Host changed; reopen Edit Host")?;
                            s.hosts[i] = h;
                        } else {
                            s.hosts.push(h);
                        }
                        Ok(())
                    });
                    if result.is_err() {
                        if let Some(id) = created {
                            let _ = wezterm_ssh::credentials::delete(&id);
                        }
                    }
                    result?;
                }
                FormKind::Key(original) => {
                    let label = self.input(&f, "label", cx);
                    let path = self.input(&f, "path", cx);
                    self.change_store(|s| s.save_key(original.as_ref(), &label, &path))?;
                }
                FormKind::Identity(original) => {
                    let label = self.input(&f, "label", cx);
                    let username = self.input(&f, "user", cx);
                    let password = self.input(&f, "password", cx);
                    ensure!(
                        !label.trim().is_empty() && !username.trim().is_empty(),
                        "Enter a name and username"
                    );
                    ensure!(
                        original.is_some() || !password.is_empty(),
                        "Enter a password"
                    );
                    let mut i = original.clone().unwrap_or_else(|| Identity {
                        id: new_credential_id(),
                        ..Identity::default()
                    });
                    i.label = label;
                    i.username = username;
                    // Persistent identity updates share their established credential reference.
                    let old = if !password.is_empty() {
                        wezterm_ssh::credentials::load(&i.id)?
                    } else {
                        None
                    };
                    if !password.is_empty() {
                        wezterm_ssh::credentials::save(&i.id, &password, true)?;
                    }
                    let result = self.change_store(|s| {
                        if let Some(old) = original {
                            let n = s
                                .identities
                                .iter()
                                .position(|x| x == old)
                                .context("Identity changed; reopen it")?;
                            s.identities[n] = i.clone();
                        } else {
                            s.identities.push(i.clone());
                        }
                        Ok(())
                    });
                    if result.is_err() && !password.is_empty() {
                        if let Some(old) = old {
                            let _ = wezterm_ssh::credentials::save(&i.id, &old, true);
                        } else {
                            let _ = wezterm_ssh::credentials::delete(&i.id);
                        }
                    }
                    result?;
                }
                FormKind::Group(parent) => {
                    let name = self.input(&f, "label", cx);
                    self.change_store(|s| s.add_group(&name, parent).map(|_| ()))?;
                }
                FormKind::RenameWorkspace(entry) => {
                    let name = self.input(&f, "label", cx);
                    self.rename_saved_workspace(&entry.id, &name)?;
                }
                FormKind::RenameTabGroup(id) => {
                    let name = self.input(&f, "label", cx);
                    let name = name.trim();
                    ensure!(name.chars().count() <= 24, "Use at most 24 characters");
                    ensure!(
                        !self
                            .tab_groups
                            .iter()
                            .enumerate()
                            .any(|(other, group)| { other != *id && group.name == name }),
                        "A tab group with this name already exists"
                    );
                    let group = &mut self.tab_groups[*id];
                    if let Some(saved_id) = &group.saved_id {
                        if self
                            .workspaces
                            .tab_groups
                            .iter()
                            .any(|saved| &saved.id == saved_id)
                        {
                            self.workspaces.rename_tab_group(saved_id, name)?;
                        } else {
                            group.saved_id = None;
                        }
                    }
                    group.name = name.to_owned();
                }
                FormKind::Existing(group) => {
                    ensure!(
                        !group.is_empty(),
                        "Choose a group before adding existing hosts"
                    );
                    self.change_store(|s| s.move_hosts_to_group(&f.selected, group))?;
                }
                FormKind::Settings => {
                    let mut s = self.settings.clone();
                    s.terminal_font_size = Some(
                        self.input(&f, "size", cx)
                            .parse()
                            .context("Enter a font size")?,
                    );
                    s.ssh_keepalive_interval_seconds = Some(
                        self.input(&f, "keepalive", cx)
                            .parse()
                            .context("Enter a keepalive interval")?,
                    );
                    s.theme = f.theme;
                    s.confirm_close_tabs = Some(f.confirm);
                    s.audible_bell = Some(f.bell);
                    s.npu_monitor_enabled = f.npu_monitor_enabled;
                    s.save()?;
                    if self.settings.npu_monitor_enabled && !s.npu_monitor_enabled {
                        self.monitor.suspend();
                        self.monitor_detail = None;
                        self.monitor_reason = None;
                        self.monitor_technical = false;
                    }
                    self.settings = s;
                    if self.page == Page::Monitor && !self.settings.npu_monitor_enabled {
                        self.activate_tab(Page::Hosts, w, cx);
                    }
                    for p in self.panes.values() {
                        p.view.update(cx, |v, cx| {
                            v.appearance(self.settings.font_size(), self.theme, cx)
                        });
                    }
                }
                FormKind::Rename(id) => {
                    let name = self.input(&f, "label", cx);
                    ensure!(!name.trim().is_empty(), "Enter a tab name");
                    let t = self
                        .tabs
                        .iter_mut()
                        .find(|t| t.id == *id)
                        .context("Tab closed")?;
                    if let Some(saved_id) = t.saved.clone().filter(|id| {
                        self.workspaces
                            .saved
                            .iter()
                            .any(|workspace| workspace.id == *id)
                    }) {
                        self.rename_saved_workspace(&saved_id, &name)?;
                    } else {
                        t.saved = None;
                        t.name = name.trim().to_owned();
                        t.custom_name = true;
                    }
                }
                FormKind::Monitor => {
                    ensure!(!f.selected.is_empty(), "Choose at least one host");
                    ensure!(
                        f.selected.iter().all(|h| self.store.hosts.contains(h)),
                        "A host changed; reopen the host picker"
                    );
                    let old_len = self.monitor.entries.len();
                    for h in &f.selected {
                        self.monitor.add(h.clone());
                    }
                    if let Err(e) = self.monitor.save() {
                        self.monitor.entries.truncate(old_len);
                        return Err(e);
                    }
                }
                FormKind::MonitorReplace(original) => {
                    let host = f.selected.first().context("Choose a host")?;
                    ensure!(
                        self.store.hosts.contains(host),
                        "The host changed; reopen the host picker"
                    );
                    self.monitor.replace_host(original, host.clone())?;
                    self.monitor_reason = None;
                    self.monitor_technical = false;
                }
                FormKind::Forward(old) => {
                    let host = f.selected.first().cloned().context("Choose a host")?;
                    let mut r = old
                        .clone()
                        .unwrap_or_else(|| forwarding::Rule::new(host.clone()));
                    r.host = host;
                    r.name = self.input(&f, "label", cx);
                    r.remote_port = self
                        .input(&f, "remote", cx)
                        .parse()
                        .context("Enter a valid remote port")?;
                    r.local_port = self
                        .input(&f, "local", cx)
                        .parse()
                        .context("Enter a valid local port")?;
                    r.network = f.network;
                    r.retry = f.retry;
                    forwarding::manager().lock().unwrap().save(r)?;
                }
                FormKind::Auth(_) => {}
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.drawer = None,
            Err(e) => {
                if let Some(f) = &mut self.drawer {
                    f.error = format!("{e:#}");
                }
            }
        }
        let _ = w;
        cx.notify();
    }
}
