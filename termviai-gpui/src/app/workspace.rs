use super::*;
use anyhow::{Context as _, ensure};
impl AppView {
    pub(super) fn prepare_auth(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<_> = self.panes.keys().copied().collect();
        for id in ids {
            let v = self.panes[&id].session.auth.view();
            if matches!(v.phase, Phase::Waiting | Phase::Challenge)
                && self.panes[&id].form.is_none()
            {
                let f = self.make_form(FormKind::Auth(id), w, cx);
                self.panes.get_mut(&id).unwrap().form = Some(f);
            }
            if v.phase == Phase::Challenge {
                let replace = self.panes[&id]
                    .form
                    .as_ref()
                    .is_some_and(|f| f.challenge_stamp != v.revision);
                if replace {
                    let fields = v
                        .prompts
                        .iter()
                        .map(|(label, echo)| {
                            cx.new(|cx| {
                                InputState::new(w, cx)
                                    .placeholder(label.clone())
                                    .masked(!echo)
                            })
                        })
                        .collect();
                    let f = self.panes.get_mut(&id).unwrap().form.as_mut().unwrap();
                    f.challenges = fields;
                    f.challenge_stamp = v.revision;
                }
            }
        }
    }
    fn submit_auth(&mut self, id: usize, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<()> {
            let p = self.panes.get(&id).context("Terminal closed")?;
            let f = p.form.clone().context("Authentication page closed")?;
            let control = p.session.auth.clone();
            let phase = control.view().phase;
            if phase == Phase::Challenge {
                let answers = f
                    .challenges
                    .iter()
                    .map(|f| f.read(cx).value().to_string())
                    .collect();
                control.send(authentication::Command::Answers(answers))?;
                return Ok(());
            }
            let original = p.host.clone().context("Not an SSH terminal")?;
            let mut chosen = original.clone();
            chosen.authentication = Some(f.mode);
            let mut secret = String::new();
            match f.mode {
                Authentication::Password => {
                    chosen.username = self.input(&f, "user", cx);
                    let password = self.input(&f, "password", cx);
                    ensure!(!password.is_empty(), "Enter a password");
                    secret = format!("session-auth-{}", new_credential_id());
                    wezterm_ssh::credentials::save(&secret, &password, false)?;
                    chosen.session_credential = secret.clone();
                }
                Authentication::Key => {
                    chosen.key_username = self.input(&f, "key_user", cx);
                    if f.keychain {
                        chosen.key_label = f.key.clone();
                        chosen.identity_file.clear();
                    } else {
                        chosen.key_label.clear();
                        chosen.identity_file = self.input(&f, "path", cx);
                    }
                }
                Authentication::Identity => chosen.identity_id = f.identity.clone(),
            }
            let pending = Pending {
                original: original.clone(),
                selected: chosen.clone(),
                mode: f.mode,
                secret,
                saved: self.store.hosts.contains(&original),
            };
            chosen.resolve_identity(&self.store)?;
            chosen.validate()?;
            let candidate = session::candidate(
                format!("{:?}", f.mode),
                Ok(chosen),
                self.settings.keepalive(),
            );
            ensure!(
                candidate.error.is_none(),
                "{}",
                candidate
                    .error
                    .as_deref()
                    .unwrap_or("Invalid authentication")
            );
            control.send(authentication::Command::Retry(candidate))?;
            let p = self.panes.get_mut(&id).unwrap();
            p.pending = Some(pending);
            p.save_attempted = false;
            p.error.clear();
            Ok(())
        })();
        if let Err(e) = result {
            if let Some(p) = self.panes.get_mut(&id) {
                if let Some(f) = &mut p.form {
                    f.error = format!("{e:#}");
                } else {
                    p.error = format!("{e:#}");
                }
            }
        }
        cx.notify();
    }
    fn auth_view(&self, id: usize, tab: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = &self.panes[&id];
        let view = p.session.auth.view();
        let title = match view.phase {
            Phase::Verify => "Verify SSH host",
            Phase::Challenge => "Additional authentication",
            Phase::Waiting => "Choose authentication",
            Phase::Failed | Phase::Closed => "Connection unavailable",
            _ => "Connecting…",
        };
        let mut content = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(self.muted(p.host.as_ref().map(|h| h.endpoint()).unwrap_or_default()));
        if !view.error.is_empty() {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(self.c(self.col().red))
                    .child(view.error.clone()),
            );
        }
        if view.phase == Phase::Waiting {
            if let Some(f) = &p.form {
                content = content.child(self.form_content(f, Some(id), cx));
            }
        }
        if view.phase == Phase::Challenge {
            if let Some(f) = &p.form {
                for (i, field) in f.challenges.iter().enumerate() {
                    content = content.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.muted(
                                view.prompts.get(i).map(|v| v.0.clone()).unwrap_or_default(),
                            ))
                            .child(Input::new(field)),
                    );
                }
            }
        }
        if view.phase == Phase::Trying {
            content = content.child(self.muted(format!("Trying {}", view.label)));
        }
        if matches!(view.phase, Phase::Waiting | Phase::Failed) {
            content = content.children(
                view.logs
                    .iter()
                    .rev()
                    .take(3)
                    .rev()
                    .map(|l| self.muted(l.clone())),
            );
        }
        let mut footer = div().flex().justify_end().gap_2().p_3().flex_shrink_0();
        match view.phase {
            Phase::Verify => {
                let control = p.session.auth.clone();
                footer = footer.child(self.button(("trust-host", id), "Trust & Connect").on_click(
                    cx.listener(move |s, _, _, cx| {
                        let r = control.send(authentication::Command::Trust(true));
                        s.notify_error(r, cx);
                    }),
                ));
            }
            Phase::Waiting | Phase::Challenge => {
                footer = footer.child(
                    self.button(
                        ("retry-auth", id),
                        if view.phase == Phase::Challenge {
                            "Continue"
                        } else {
                            "Connect & Save"
                        },
                    )
                    .hover(|d| d.bg(self.c(self.col().accent)).text_color(rgb(0xffffff)))
                    .on_click(cx.listener(move |s, _, _, cx| s.submit_auth(id, cx))),
                )
            }
            Phase::Failed | Phase::Closed => {
                footer = footer.child(
                    self.button(("retry-session", id), "Reconnect")
                        .on_click(cx.listener(move |s, _, w, cx| s.reconnect(tab, w, cx))),
                )
            }
            _ => {}
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(self.c(self.col().bg))
            .child(
                div()
                    .id(("auth-scroll", id))
                    .relative()
                    .track_scroll(&self.panes[&id].auth_scroll)
                    .vertical_scrollbar(&self.panes[&id].auth_scroll)
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
            .child(footer)
            .into_any_element()
    }
    pub(super) fn terminal_page(
        &self,
        tab_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(t) = self.tabs.iter().find(|t| t.id == tab_id) else {
            return div().into_any_element();
        };
        let mut layout = t.layout.clone();
        if let Some(drag) = &self.drag_source {
            if let Some(source) = drag.pane {
                if drag.tab == tab_id {
                    if let Some(removed) = layout.clone().remove(source) {
                        layout = removed;
                    }
                }
            }
        }
        if let Some(zoom) = t.zoom {
            layout = Layout::Pane(zoom);
        }
        let ids = t.layout.visual_panes();
        let count = ids.len();
        let online = ids
            .iter()
            .filter(|id| self.panes.get(id).is_some_and(|p| p.session.online()))
            .count();
        let mut page = div().flex_1().min_h_0().min_w_0().flex().flex_col().child(
            div()
                .flex_1()
                .min_h_0()
                .p_2()
                .child(self.layout_view(&layout, tab_id, vec![], cx)),
        );
        if count > 1 {
            let selected = if t.broadcast.enabled() {
                t.broadcast.member_count()
            } else {
                0
            };
            let all = selected == count;
            let fraction = t.tray_fraction(Instant::now());
            if t.tray_motion
                .is_some_and(|(start, _)| start.elapsed() < Duration::from_millis(180))
            {
                window.request_animation_frame();
            }
            if fraction > 0. {
                let members = div()
                    .id(("broadcast-members", tab_id))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .overflow_x_scroll()
                    .children(ids.iter().map(|id| {
                        let id = *id;
                        let p = &self.panes[&id];
                        let on = t.broadcast.enabled() && t.broadcast.is_member(id);
                        let offline = !p.session.online();
                        let label = p
                            .host
                            .as_ref()
                            .map(|h| {
                                if h.label.is_empty() {
                                    h.address.clone()
                                } else {
                                    h.label.clone()
                                }
                            })
                            .unwrap_or("Local".into());
                        div()
                            .id(("broadcast-member", id))
                            .h(px(30.))
                            .min_w(px(68.))
                            .max_w(px(190.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .rounded_md()
                            .border_1()
                            .border_color(if on {
                                gpui::transparent_black()
                            } else {
                                self.c(self.col().line)
                            })
                            .bg(if on {
                                self.c(self.col().soft)
                            } else {
                                gpui::transparent_black()
                            })
                            .text_size(px(14.))
                            .text_color(self.c(if on {
                                self.col().text
                            } else {
                                self.col().muted
                            }))
                            .cursor_pointer()
                            .hover(|d| {
                                d.bg(self.c(self.col().icon_hover))
                                    .border_color(self.c(self.col().muted))
                            })
                            .child(
                                div()
                                    .size(px(14.))
                                    .flex_shrink_0()
                                    .border_1()
                                    .rounded(px(3.))
                                    .border_color(self.c(if on {
                                        self.col().accent
                                    } else {
                                        self.col().muted
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(on, |d| {
                                        d.child(
                                            svg()
                                                .path("icons/check.svg")
                                                .size(px(12.))
                                                .text_color(self.c(self.col().accent)),
                                        )
                                    }),
                            )
                            .child(div().min_w_0().truncate().child(label))
                            .when(offline, |d| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(11.))
                                        .text_color(self.c(self.col().red))
                                        .child("Offline"),
                                )
                            })
                            .on_click(cx.listener(move |s, _, _, cx| s.member(tab_id, id, cx)))
                    }));
                page = page.child(
                    div()
                        .h(px(49. * fraction))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(49.))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .bg(self.c(self.col().top))
                                .border_t_1()
                                .border_color(self.c(self.col().line))
                                .child(self.muted("Send to").flex_shrink_0())
                                .child(members)
                                .child(
                                    div()
                                        .id(("broadcast-select-all", tab_id))
                                        .flex_shrink_0()
                                        .h(px(30.))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .text_size(px(12.))
                                        .text_color(self.c(self.col().muted))
                                        .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                        .child(if all { "Clear selection" } else { "Select all" })
                                        .on_click(cx.listener(move |s, _, _, cx| {
                                            s.broadcast_all(tab_id, cx)
                                        })),
                                ),
                        ),
                );
            }
            return page
                .child(
                    div()
                        .h(px(34.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .px_2()
                        .bg(self.c(self.col().top))
                        .border_t_1()
                        .border_color(self.c(self.col().line))
                        .child(
                            div()
                                .id(("broadcast-power", tab_id))
                                .size(px(27.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                .child(self.radio(all))
                                .on_click(
                                    cx.listener(move |s, _, _, cx| s.broadcast_all(tab_id, cx)),
                                ),
                        )
                        .child(
                            div()
                                .id(("broadcast-expand", tab_id))
                                .min_w_0()
                                .h(px(28.))
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .gap_2()
                                .rounded_md()
                                .text_size(px(12.))
                                .cursor_pointer()
                                .hover(|d| d.bg(self.c(self.col().icon_hover)))
                                .child(div().min_w_0().truncate().child(if selected == 0 {
                                    "Broadcast off".into()
                                } else {
                                    format!("Broadcast to {selected} terminals")
                                }))
                                .child(
                                    Icon::new(if t.tray {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronUp
                                    })
                                    .size(px(14.))
                                    .text_color(self.c(self.col().muted)),
                                )
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab_id) {
                                        t.toggle_tray();
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .flex_shrink_0()
                                .pr(px(6.))
                                .text_size(px(12.))
                                .text_color(self.c(if online < count {
                                    self.col().red
                                } else {
                                    self.col().muted
                                }))
                                .child(if online == count {
                                    "All online".into()
                                } else {
                                    format!("{online} online · {} offline", count - online)
                                }),
                        ),
                )
                .into_any_element();
        }
        page.child(
            div()
                .h(px(34.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .child(self.muted(if online == 1 {
                    "Connected · SSH / PTY"
                } else {
                    "Offline"
                }))
                .child(div().flex_1())
                .when(online < count, |d| {
                    d.child(
                        self.button("reconnect-offline", "Reconnect offline")
                            .on_click(cx.listener(move |s, _, w, cx| s.reconnect(tab_id, w, cx))),
                    )
                }),
        )
        .into_any_element()
    }

    fn layout_view(
        &self,
        layout: &Layout,
        tab: usize,
        path: Vec<bool>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match layout {
            Layout::Pane(id) => self.pane_view(*id, tab, cx),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let horizontal = *axis == SplitAxis::Horizontal;
                let mut a = path.clone();
                a.push(false);
                let mut b = path.clone();
                b.push(true);
                let info = ResizeDrag {
                    tab,
                    path: path.clone(),
                };
                let p = path.clone();
                div()
                    .id(SharedString::from(format!("split-{tab}-{path:?}")))
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .when(!horizontal, |d| d.flex_col())
                    .on_drag_move::<ResizeDrag>(cx.listener(
                        move |s, e: &DragMoveEvent<ResizeDrag>, _, cx| {
                            let drag = e.drag(cx).clone();
                            if drag.tab != tab || drag.path != p {
                                return;
                            }
                            let ratio = if horizontal {
                                (e.event.position.x - e.bounds.left()) / e.bounds.size.width
                            } else {
                                (e.event.position.y - e.bounds.top()) / e.bounds.size.height
                            };
                            if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
                                t.layout.set_ratio(&p, ratio);
                            }
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .min_h_0()
                            .flex_shrink_0()
                            .when(horizontal, |d| d.w(relative(*ratio)).h_full())
                            .when(!horizontal, |d| d.h(relative(*ratio)).w_full())
                            .child(self.layout_view(first, tab, a, cx)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("divider-{tab}-{path:?}")))
                            .flex_shrink_0()
                            .when(horizontal, |d| {
                                d.w(px(4.)).h_full().cursor(CursorStyle::ResizeLeftRight)
                            })
                            .when(!horizontal, |d| {
                                d.h(px(4.)).w_full().cursor(CursorStyle::ResizeUpDown)
                            })
                            .bg(self.c(self.col().line))
                            .hover(|d| d.bg(self.c(self.col().accent)))
                            .on_drag(info, |drag, _, _, cx| cx.new(|_| drag.clone())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(self.layout_view(second, tab, b, cx)),
                    )
                    .into_any_element()
            }
        }
    }
    fn pane_view(&self, id: usize, tab: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(p) = self.panes.get(&id) else {
            return div().into_any_element();
        };
        let t = self.tabs.iter().find(|t| t.id == tab).unwrap();
        let group = t.layout.panes().len() > 1;
        let on = t.broadcast.enabled() && t.broadcast.is_member(id);
        let label = p
            .host
            .as_ref()
            .map(|h| h.label.clone())
            .unwrap_or("Local terminal".into());
        let drag = Drag {
            pane: Some(id),
            tab,
            label: label.clone(),
        };
        let state = p.session.auth.view();
        let has_terminal = p.session.online() || p.session.state.lock().unwrap().bytes > 0;
        let owner = cx.entity();
        let header = div()
            .id(("pane-header", id))
            .h(px(32.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(self.c(self.col().side))
            .border_t_1()
            .border_color(if self.focused == Some(id) {
                self.c(self.col().accent)
            } else {
                gpui::transparent_black()
            })
            .cursor(CursorStyle::PointingHand)
            .child(Icon::new(IconName::SquareTerminal).size(px(14.)))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(p.notices.clone())
            .when(group, |d| {
                d.child(
                    div()
                        .id(("pane-bc", id))
                        .size(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .hover(|d| d.bg(self.c(self.col().icon_hover)))
                        .child(self.radio(on))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            cx.stop_propagation();
                            s.member(tab, id, cx);
                        })),
                )
            })
            .when(group, |d| {
                d.child(
                    self.icon(
                        ("zoom", id),
                        if t.zoom.is_some() {
                            IconName::Minimize
                        } else {
                            IconName::Maximize
                        },
                    )
                    .on_click(cx.listener(move |s, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
                            t.zoom = if t.zoom.is_some() { None } else { Some(id) };
                        }
                        cx.notify();
                    })),
                )
            })
            .child(
                self.close(("close-pane", id))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        cx.stop_propagation();
                        if s.settings.confirm_close() {
                            s.confirm = Some(Confirm::Pane(id));
                            cx.notify();
                        } else {
                            s.close_pane(id, cx);
                        }
                    })),
            )
            .on_click(cx.listener(move |s, _, w, cx| {
                s.focused = Some(id);
                s.panes[&id].view.focus_handle(cx).focus(w);
                cx.notify();
            }))
            .on_drag(drag, move |d: &Drag, _, _, cx| {
                owner.update(cx, |s, cx| {
                    s.clear_drag();
                    s.drag_source = Some(d.clone());
                    cx.notify();
                });
                cx.new(|_| d.clone())
            });
        let content = if !has_terminal && !matches!(state.phase, Phase::Connected) {
            self.auth_view(id, tab, cx)
        } else {
            p.view.clone().into_any_element()
        };
        div()
            .id(("pane", id))
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .border_1()
            .border_color(self.c(self.col().line))
            .child(header)
            .child(div().flex_1().min_h_0().min_w_0().p_1().child(content))
            .on_drag_move::<Drag>(cx.listener(move |s, e: &DragMoveEvent<Drag>, _, cx| {
                let d = e.drag(cx);
                let source = s.dock_source(d);
                if source.is_none() || source == Some(id) || !e.bounds.contains(&e.event.position) {
                    if s.drop_target.is_some_and(|(t, p, _)| t == tab && p == id) {
                        s.drop_target = None;
                        cx.notify();
                    }
                    return;
                }
                s.tab_drop_target = None;
                let u = (e.event.position.x - e.bounds.left()) / e.bounds.size.width;
                let v = (e.event.position.y - e.bounds.top()) / e.bounds.size.height;
                let side = if u.min(1. - u) <= v.min(1. - v) {
                    if u < 0.5 { Side::Left } else { Side::Right }
                } else if v < 0.5 {
                    Side::Top
                } else {
                    Side::Bottom
                };
                if s.drop_target != Some((tab, id, side)) {
                    s.drop_target = Some((tab, id, side));
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(move |s, d: &Drag, _, cx| {
                if let Some((target_tab, target, side)) = s.drop_target {
                    if target_tab == tab && target == id {
                        cx.stop_propagation();
                        s.dock(d, target_tab, target, side, cx);
                    }
                }
            }))
            .when_some(
                self.drop_target.filter(|(t, p, _)| *t == tab && *p == id),
                |d, (_, _, side)| {
                    d.child(
                        div()
                            .absolute()
                            .bg(self.c(self.col().accent).opacity(0.22))
                            .border_2()
                            .border_color(self.c(self.col().accent))
                            .when(matches!(side, Side::Left | Side::Right), |d| {
                                d.top_0().bottom_0().w(relative(0.5))
                            })
                            .when(matches!(side, Side::Top | Side::Bottom), |d| {
                                d.left_0().right_0().h(relative(0.5))
                            })
                            .when(side == Side::Left, |d| d.left_0())
                            .when(side == Side::Right, |d| d.right_0())
                            .when(side == Side::Top, |d| d.top_0())
                            .when(side == Side::Bottom, |d| d.bottom_0()),
                    )
                },
            )
            .into_any_element()
    }
}
