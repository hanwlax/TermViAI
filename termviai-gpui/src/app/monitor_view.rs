use super::*;
use gpui_component::tooltip::Tooltip;
use termviai_gpui::monitor_presenter::{locations, metrics, process_memory};

// Leaves about 200 px for the HBM bar after labels, values and padding.
const NPU_CARD_MIN_WIDTH: f32 = 408.;

fn gb(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v / 1024.)).unwrap_or("—".into())
}
fn percent(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0}%")).unwrap_or("—".into())
}
fn can_stop(e: &monitor::Entry, ps: &[monitor::Process]) -> bool {
    e.live()
        && e.snapshot.as_ref().is_some_and(|s| s.processes_available)
        && !ps.is_empty()
        && ps.iter().all(|p| p.controllable && !p.token.is_empty())
}
pub(super) fn sampling_icon(e: &monitor::Entry) -> &'static str {
    if !e.error.is_empty() {
        "termviai/reconnect.svg"
    } else if e.paused {
        "termviai/play.svg"
    } else {
        "termviai/pause.svg"
    }
}
impl AppView {
    fn monitor_problem(&self, id: usize, cx: &mut Context<Self>) -> AnyElement {
        use termviai_gpui::monitor_presenter::{Issue, issue};
        let e = &self.monitor.entries[id];
        let problem = issue(e, &self.store);
        let host = e.host.clone();
        let mut actions = div().flex().flex_wrap().gap_2();
        if problem == Issue::MissingHost {
            actions = actions
                .child(
                    self.action_button("monitor-choose-host", "Choose host", IconName::Replace)
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.open_form(FormKind::MonitorReplace(host.clone()), w, cx)
                        })),
                )
                .child(
                    self.button("monitor-remove-missing", "Remove monitor")
                        .bg(self.c(self.col().card))
                        .border_0()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.confirm = Some(Confirm::RemoveMonitor(id));
                            cx.notify();
                        })),
                );
        } else {
            if problem == Issue::Attention {
                actions = actions.child(
                    self.action_button(
                        "monitor-open-host",
                        "Open terminal",
                        IconName::SquareTerminal,
                    )
                    .on_click(cx.listener(move |s, _, w, cx| s.connect(Some(host.clone()), w, cx))),
                );
            }
            actions = actions.child(
                self.action_button(
                    "monitor-retry-state",
                    if problem == Issue::Collector {
                        "Retry collection"
                    } else {
                        "Retry now"
                    },
                    IconName::Redo,
                )
                .when(e.busy(), |d| d.opacity(0.45).cursor_default())
                .on_click(cx.listener(move |s, _, _, cx| {
                    if s.monitor.entries.get(id).is_some_and(|e| !e.busy()) {
                        let result = s.monitor.set_paused(id, false);
                        s.notify_error(result, cx);
                    }
                })),
            );
            if !e.paused && !e.needs_attention() {
                actions = actions.child(
                    self.button("monitor-pause-retries", "Pause retries")
                        .border_0()
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let result = s.monitor.set_paused(id, true);
                            s.notify_error(result, cx);
                        })),
                );
            }
        }
        let mut copy = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(problem.title()),
            )
            .child(self.muted(problem.description()).text_size(px(13.)))
            .when(problem != Issue::MissingHost, |d| {
                d.child(self.muted(e.recovery_hint()))
            })
            .child(actions)
            .child(
                self.action_button(
                    "monitor-technical-toggle",
                    "Technical details",
                    if self.monitor_technical {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                )
                .border_0()
                .text_size(px(12.))
                .text_color(self.c(self.col().muted))
                .on_click(cx.listener(|s, _, _, cx| {
                    s.monitor_technical = !s.monitor_technical;
                    cx.notify();
                })),
            );
        if self.monitor_technical {
            copy = copy.child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(self.c(self.col().field))
                    .text_size(px(12.))
                    .text_color(self.c(self.col().muted))
                    .child(e.error.clone()),
            );
        }
        div()
            .w_full()
            .flex_shrink_0()
            .max_w(px(580.))
            .mx_auto()
            .mt(px(42.))
            .p_6()
            .rounded_lg()
            .border_1()
            .border_color(self.c(self.col().line))
            .bg(self.c(self.col().card))
            .flex()
            .items_start()
            .gap_4()
            .child(self.state_icon(
                match problem {
                    Issue::MissingHost => IconName::Replace,
                    Issue::Connection => IconName::ExternalLink,
                    _ => IconName::TriangleAlert,
                },
                true,
            ))
            .child(copy)
            .into_any_element()
    }

    fn monitor_bar(&self, value: Option<f64>) -> Div {
        div()
            .h(px(4.))
            .rounded_full()
            .bg(self.c(self.col().line))
            .child(
                div()
                    .h_full()
                    .w(relative((value.unwrap_or(0.) / 100.).clamp(0., 1.) as f32))
                    .rounded_full()
                    .bg(self.c(self.col().accent)),
            )
    }
    fn monitor_status(&self, e: &monitor::Entry) -> Div {
        div()
            .text_size(px(12.))
            .text_color(self.c(if !e.error.is_empty() {
                self.col().red
            } else if e.live() {
                self.col().green
            } else {
                self.col().muted
            }))
            .child(e.status().to_owned())
    }
    pub(super) fn toggle_monitor_sampling(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.monitor.entries.get(id) {
            let pause = !entry.paused && entry.error.is_empty();
            let result = self.monitor.set_paused(id, pause);
            self.notify_error(result, cx);
        }
    }
    fn monitor_action(&self, id: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let e = &self.monitor.entries[id];
        let label = if !e.error.is_empty() {
            "Retry"
        } else if e.paused {
            "Resume"
        } else {
            "Pause"
        };
        self.button(("monitor-pause", id), "")
            .child(
                Icon::new(IconName::Redo)
                    .path(sampling_icon(e))
                    .size(px(15.)),
            )
            .child(label)
            .on_click(cx.listener(move |s, _, _, cx| {
                cx.stop_propagation();
                s.toggle_monitor_sampling(id, cx);
            }))
    }
    fn monitor_stop_button(
        &self,
        key: impl Into<ElementId>,
        label: impl Into<SharedString>,
        id: usize,
        ps: Vec<monitor::Process>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = can_stop(&self.monitor.entries[id], &ps);
        self.button(key, label)
            .when(!enabled, |d| d.opacity(0.45))
            .when(enabled, |d| {
                d.hover(|d| {
                    d.bg(self.c(self.col().soft))
                        .text_color(self.c(self.col().red))
                })
            })
            .on_click(cx.listener(move |s, _, _, cx| {
                cx.stop_propagation();
                if enabled && s.monitor.entries.get(id).is_some_and(|e| can_stop(e, &ps)) {
                    s.confirm = Some(Confirm::Stop(id, ps.clone()));
                    cx.notify();
                }
            }))
    }
    pub(super) fn monitor_page(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self
            .monitor_detail
            .filter(|(id, _, _)| *id < self.monitor.entries.len());
        let scroll = if selected.is_some() {
            "monitor-detail"
        } else {
            "monitor-page"
        };
        let width = self.tool_content_width(w);
        let inspect = selected.is_some_and(|(_, card, _)| card.is_some());
        let beside = inspect && width >= 850.;
        let inspector_width = (width * 0.45).clamp(330., 490.);
        let gutter = Self::tool_gutter(w);
        let resource_width = if beside {
            width - inspector_width
        } else {
            width
        };
        let mut header = self.tool_header(w);
        let mut body = self.tool_content(w);
        if !self.monitor.load_error.is_empty() {
            body = body.child(self.notice(
                "Monitor preferences could not be loaded",
                self.monitor.load_error.clone(),
            ));
        }
        if let Some((id, _, _)) = selected {
            let e = &self.monitor.entries[id];
            let ps = e
                .snapshot
                .as_ref()
                .map(|s| s.processes(None, None))
                .unwrap_or_default();
            header = header
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("monitor-back")
                                .text_size(px(12.))
                                .text_color(self.c(self.col().accent))
                                .cursor_pointer()
                                .child("Monitor")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.monitor_detail = None;
                                    s.monitor_reason = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(13.))
                                .text_color(self.c(self.col().muted)),
                        )
                        .child(self.muted(e.host.label.clone()).truncate()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .w_full()
                        .flex_wrap()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(self.title(e.host.label.clone()))
                                .child(self.muted(format!(
                                    "{}@{} · {}",
                                    e.host.username,
                                    e.host.endpoint(),
                                    self.store.group_path(&e.host.group_id)
                                ))),
                        )
                        .when(e.error.is_empty(), |d| d.child(self.monitor_action(id, cx)))
                        .when(!e.error.is_empty(), |d| {
                            d.child(
                                self.muted(
                                    termviai_gpui::monitor_presenter::issue(e, &self.store)
                                        .status(),
                                )
                                .text_color(self.c(self.col().red)),
                            )
                        })
                        .when(e.error.is_empty() && !ps.is_empty(), |d| {
                            d.child(self.monitor_stop_button(
                                "end-host",
                                "End all NPU processes",
                                id,
                                ps,
                                cx,
                            ))
                        }),
                );
            if let Some(snapshot) = e.snapshot.as_ref().filter(|_| e.error.is_empty()) {
                let total = metrics(&snapshot.dies.iter().collect::<Vec<_>>());
                let per_die = if snapshot
                    .dies
                    .iter()
                    .all(|d| d.total_mb == snapshot.dies.first().and_then(|x| x.total_mb))
                {
                    format!(
                        "{} GB / Die",
                        gb(snapshot.dies.first().and_then(|d| d.total_mb))
                    )
                } else {
                    format!("{} GB total", gb(total.total))
                };
                body = body.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_5()
                        .child(self.monitor_status(e))
                        .child(self.muted(format!(
                            "{} NPU / {} Die",
                            snapshot.cards().len(),
                            snapshot.dies.len()
                        )))
                        .child(self.muted(per_die))
                        .child(self.muted(if e.paused {
                            "Snapshot · paused".into()
                        } else {
                            format!("Every {}s", self.monitor.interval)
                        })),
                );
            }
            if !e.error.is_empty() {
                body = body.child(self.monitor_problem(id, cx));
            }
            if e.error.is_empty() {
                if let Some(snapshot) = &e.snapshot {
                    body = body
                        .when(snapshot.dies.is_empty(), |d| {
                            d.child(self.empty_state(
                                IconName::GalleryVerticalEnd,
                                "No NPU resources detected",
                                "This host's latest collection did not report any NPU devices.",
                            ))
                        })
                        .child(
                            Self::card_grid(Self::card_columns(resource_width, NPU_CARD_MIN_WIDTH))
                                .children(
                                    snapshot
                                        .cards()
                                        .into_iter()
                                        .map(|npu| self.monitor_npu_card(id, npu, cx)),
                                ),
                        )
                        .child(self.muted("Hover a card to preview processes · Click to inspect"));
                } else {
                    body = body.child(self.empty_state(
                        IconName::LoaderCircle,
                        "Waiting for the first sample",
                        "Connecting to the host and collecting NPU resources.",
                    ));
                }
            }
            if !e.action_error.is_empty() {
                body = body
                    .child(self.notice("Process action needs attention", e.action_error.clone()));
            }
        } else {
            header =
                header.child(self.title("Monitor")).child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(self.add_button("monitor-add", "Add hosts").on_click(
                            cx.listener(|s, _, w, cx| s.open_form(FormKind::Monitor, w, cx)),
                        ))
                        .child(self.interval_button().on_click(cx.listener(
                            |s, e: &ClickEvent, _, cx| {
                                s.menu = Some((MenuKind::MonitorInterval, e.position()));
                                cx.notify();
                            },
                        ))),
                );
            body = body
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_5()
                        .child(self.muted(format!("{} hosts", self.monitor.entries.len())))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(self.c(self.col().green))
                                .child(format!(
                                    "{} collecting",
                                    self.monitor
                                        .entries
                                        .iter()
                                        .filter(|e| !e.paused
                                            && e.error.is_empty()
                                            && e.snapshot.is_some())
                                        .count()
                                )),
                        )
                        .child(self.muted(format!(
                            "{} paused",
                            self.monitor.entries.iter().filter(|e| e.paused).count()
                        )))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(self.c(self.col().red))
                                .child(format!(
                                    "{} unavailable",
                                    self.monitor
                                        .entries
                                        .iter()
                                        .filter(|e| !e.paused && !e.error.is_empty())
                                        .count()
                                )),
                        ),
                )
                .child(
                    Self::card_grid(Self::card_columns(width, 396.)).children(
                        self.monitor
                            .entries
                            .iter()
                            .enumerate()
                            .map(|(id, _)| self.monitor_host_card(id, cx)),
                    ),
                );
            if self.monitor.entries.is_empty() && self.monitor.load_error.is_empty() {
                body = body.child(
                    self.empty_state(
                        IconName::ChartPie,
                        "Keep an eye on your hosts",
                        "Add saved SSH hosts to view NPU memory, activity and processes.",
                    )
                    .child(
                        self.action_button("monitor-empty-add", "Add hosts", IconName::Plus)
                            .on_click(
                                cx.listener(|s, _, w, cx| s.open_form(FormKind::Monitor, w, cx)),
                            ),
                    ),
                );
            }
        }
        div()
            .relative()
            .w_full()
            .flex_1()
            .min_h_0()
            .flex()
            .when(!beside, |d| d.flex_col())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.tool_page(scroll, header, body)),
            )
            .when(inspect, |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .when(beside, |d| d.w(px(inspector_width)).h_full())
                        .when(!beside, |d| d.h(relative(0.52)).w_full().px(px(gutter)))
                        .child(self.monitor_inspector(cx)),
                )
            })
            .into_any_element()
    }
    fn monitor_host_card(&self, id: usize, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.monitor.entries[id];
        let mut card = div()
            .id(("monitor-host", id))
            .group("monitor-host")
            .w_full()
            .min_w_0()
            .h(px(148.))
            .p_4()
            .rounded_lg()
            .bg(self.c(self.col().card))
            .border_1()
            .border_color(self.c(self.col().line))
            .flex()
            .flex_col()
            .gap_2()
            .cursor_pointer()
            .hover(|d| d.bg(self.c(self.col().hover)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::SquareTerminal)
                            .size(px(17.))
                            .text_color(self.c(self.col().muted)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(e.host.label.clone()),
                            )
                            .child(self.muted(e.host.address.clone()).truncate()),
                    )
                    .child(
                        self.icon(("monitor-menu", id), IconName::Ellipsis)
                            .on_click(cx.listener(move |s, ev: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                s.menu = Some((MenuKind::Monitor(id), ev.position()));
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(self.monitor_status(e))
                    .child(self.muted(if !e.error.is_empty() {
                        "Connection unavailable".into()
                    } else if let Some(s) = &e.snapshot {
                        format!("{} NPU · {} Die", s.cards().len(), s.dies.len())
                    } else {
                        "Waiting for SSH".into()
                    })),
            );
        if let Some(s) = e.snapshot.as_ref().filter(|_| e.error.is_empty()) {
            let m = metrics(&s.dies.iter().collect::<Vec<_>>());
            let mem = m
                .used
                .zip(m.total)
                .and_then(|(u, t)| (t > 0.).then_some(u / t * 100.));
            card = card.child(
                div()
                    .flex()
                    .gap_5()
                    .children([("HBM", mem), ("AICore", m.core)].into_iter().map(
                        |(label, val)| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .items_center()
                                        .child(self.muted(label))
                                        .child(
                                            div()
                                                .text_size(px(18.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(percent(val)),
                                        ),
                                )
                                .child(self.monitor_bar(val))
                        },
                    )),
            );
        } else {
            card = card.child(
                div().flex().items_center().gap_2().child(
                    self.muted(if e.error.is_empty() {
                        "Waiting for resource data"
                    } else {
                        "HBM and AICore data unavailable"
                    })
                    .flex_1(),
                ),
            );
        }
        let card = card.on_click(cx.listener(move |s, _, _, cx| {
            s.monitor_detail = Some((id, None, None));
            s.monitor_technical = false;
            s.monitor_reason = None;
            s.scrollers["monitor-detail"].set_offset(point(px(0.), px(0.)));
            cx.notify();
        }));
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(card)
            .when(self.monitor_reason == Some(id), |d| {
                d.child(
                    div()
                        .rounded_lg()
                        .p_3()
                        .bg(self.c(self.col().card))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("SSH connection")
                        .child(self.muted(e.host.endpoint()))
                        .child(self.muted(e.status().to_owned()))
                        .when(!e.error.is_empty(), |d| {
                            d.child(self.monitor_problem(id, cx))
                        })
                        .child(
                            self.button(("close-connection", id), "Close details")
                                .on_click(cx.listener(|s, _, _, cx| {
                                    s.monitor_reason = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .into_any_element()
    }
    fn monitor_npu_card(&self, id: usize, npu: u32, cx: &mut Context<Self>) -> AnyElement {
        let e = &self.monitor.entries[id];
        let s = e.snapshot.as_ref().unwrap();
        let mut dies: Vec<_> = s.dies.iter().filter(|d| d.npu == npu).collect();
        dies.sort_by_key(|d| d.die);
        let ps = s.processes(Some(npu), None);
        let peek_rows: Vec<_> = ps
            .iter()
            .take(5)
            .map(|p| {
                format!(
                    "{} · PID {} · {}\n{} GB · {}",
                    p.name,
                    p.pid,
                    p.owner,
                    gb(process_memory(s, p.pid, npu, None)),
                    if p.cwd.is_empty() {
                        "Working directory unavailable"
                    } else {
                        &p.cwd
                    }
                )
            })
            .collect();
        let available = s.processes_available;
        let count = ps.len();
        let paused = e.paused;
        let colors = self.theme.colors();
        div()
            .id(("monitor-npu", npu as usize))
            .w_full()
            .min_w_0()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(self.c(self.col().line))
            .bg(self.c(self.col().card))
            .when(
                self.monitor_detail
                    .is_some_and(|(host, card, _)| host == id && card == Some(npu)),
                |d| d.border_color(self.c(self.col().accent)),
            )
            .flex()
            .flex_col()
            .gap_3()
            .cursor_pointer()
            .hover(|d| d.bg(self.c(self.col().hover)))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .pb_2()
                    .border_b_1()
                    .border_color(self.c(self.col().line))
                    .child(
                        div()
                            .w(px(54.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("NPU {npu}")),
                    )
                    .child(self.muted("HBM · GB").flex_1())
                    .child(self.muted("AICore").w(px(44.))),
            )
            .children(dies.into_iter().map(|d| {
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.muted(format!("Die {}", d.die)).w(px(54.)))
                    .child(
                        div().flex_1().min_w_0().child(
                            self.monitor_bar(
                                d.used_mb
                                    .zip(d.total_mb)
                                    .and_then(|(u, t)| (t > 0.).then_some(u / t * 100.)),
                            ),
                        ),
                    )
                    .child(div().text_size(px(12.)).w(px(36.)).child(gb(d.used_mb)))
                    .child(self.muted(percent(d.core)).w(px(44.)))
            }))
            .tooltip(move |w, cx| {
                Tooltip::element({
                    let rows = peek_rows.clone();
                    move |_, _| {
                        div()
                            .w(px(310.))
                            .p_3()
                            .bg(Colors::color(colors.card))
                            .text_color(Colors::color(colors.text))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(format!(
                                "NPU {npu} · {}{}",
                                if available {
                                    format!("{count} processes")
                                } else {
                                    "Processes unavailable".into()
                                },
                                if paused { " · Paused" } else { "" }
                            ))
                            .children(
                                rows.iter()
                                    .map(|r| div().text_size(px(12.)).child(r.clone())),
                            )
                            .when(available && count == 0, |d| {
                                d.child("No running NPU processes")
                            })
                            .when(count > 5, |d| {
                                d.child(format!("+ {} more processes", count - 5))
                            })
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(Colors::color(colors.muted))
                                    .child("Click to inspect command and process actions"),
                            )
                    }
                })
                .build(w, cx)
            })
            .on_click(cx.listener(move |s, _, _, cx| {
                s.expanded_process = None;
                s.monitor_detail = Some((id, Some(npu), None));
                s.monitor_reason = None;
                s.scrollers["monitor-process"].set_offset(point(px(0.), px(0.)));
                cx.notify();
            }))
            .into_any_element()
    }
    fn monitor_inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let scope = self
            .monitor_detail
            .and_then(|(id, n, d)| n.map(|n| (id, n, d)));
        let id = if let Some(id) = self.monitor_reason {
            id
        } else if let Some((id, _, _)) = scope {
            id
        } else {
            return div().into_any_element();
        };
        let Some(e) = self.monitor.entries.get(id) else {
            return div().into_any_element();
        };
        let reason = self.monitor_reason.is_some();
        let mut content = div().flex().flex_col().gap_4();
        let mut controls = div().flex().flex_col().gap_4();
        let title = if reason {
            "Connection details".into()
        } else {
            format!("NPU {}", scope.unwrap().1)
        };
        if reason {
            content = content.child(self.monitor_problem(id, cx));
        } else if let Some(snapshot) = &e.snapshot {
            let (_, npu, die) = scope.unwrap();
            let mut dies: Vec<_> = snapshot.dies.iter().filter(|d| d.npu == npu).collect();
            dies.sort_by_key(|d| d.die);
            let filters = std::iter::once(None).chain(dies.iter().map(|d| Some(d.die)));
            controls = controls.child(div().flex().flex_wrap().gap_2().children(
                filters.enumerate().map(|(i, filter)| {
                    self.button(
                        ("monitor-die-filter", i),
                        filter.map(|d| format!("Die {d}")).unwrap_or("All".into()),
                    )
                    .when(filter == die, |d| {
                        d.bg(self.c(self.col().soft))
                            .text_color(self.c(self.col().accent))
                    })
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.monitor_detail = Some((id, Some(npu), filter));
                        s.scrollers["monitor-process"].set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    }))
                }),
            ));
            let shown: Vec<_> = dies
                .into_iter()
                .filter(|d| die.is_none_or(|n| n == d.die))
                .collect();
            let m = metrics(&shown);
            controls = controls.child(self.muted(format!(
                "HBM {} / {} GB    AICore {}",
                gb(m.used),
                gb(m.total),
                percent(m.core)
            )));
            let ps = snapshot.processes(Some(npu), die);
            controls = controls.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(self.muted(if snapshot.processes_available {
                        format!("Processes · {}", ps.len())
                    } else {
                        "Processes · Unavailable".into()
                    }))
                    .child(
                        self.monitor_stop_button(
                            "end-card",
                            die.map(|d| format!("End Die {d} tasks"))
                                .unwrap_or("End all card tasks".into()),
                            id,
                            ps.clone(),
                            cx,
                        ),
                    ),
            );
            if !e.error.is_empty() {
                content = content.child(
                    self.notice("Collection is unavailable", e.recovery_hint())
                        .child(self.button("process-error-details", "Details").on_click(
                            cx.listener(move |s, _, _, cx| {
                                s.monitor_reason = Some(id);
                                s.monitor_technical = true;
                                cx.notify();
                            }),
                        )),
                );
            }
            if !can_stop(e, &ps) && !ps.is_empty() {
                content = content.child(self.notice(
                    "Process actions unavailable",
                    if e.paused {
                        "Resume monitoring to enable process actions.".into()
                    } else if !e.live() {
                        "Waiting for a fresh sample before enabling process control.".into()
                    } else if !snapshot.stop_reason.is_empty() {
                        snapshot.stop_reason.clone()
                    } else {
                        "Process control is unavailable for this scope.".into()
                    },
                ));
            }
            if ps.is_empty() {
                content = content.child(if snapshot.processes_available && e.error.is_empty() {
                    self.empty_state(
                        IconName::GalleryVerticalEnd,
                        "No running NPU processes",
                        "This scope is idle. Processes will appear here when detected.",
                    )
                } else {
                    self.empty_state(
                        IconName::TriangleAlert,
                        "Process information unavailable",
                        "Wait for a fresh collection before checking running processes.",
                    )
                });
            }
            content = content
                .children(ps.into_iter().enumerate().map(|(i, p)| {
                    div()
                        .p_4()
                        .rounded_lg()
                        .bg(self.c(self.col().card))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(p.name.clone()),
                                        )
                                        .child(self.muted(format!("PID {} · {}", p.pid, p.owner))),
                                )
                                .child(self.monitor_stop_button(
                                    ("end-process", i),
                                    "End process",
                                    id,
                                    vec![p.clone()],
                                    cx,
                                )),
                        )
                        .child(self.muted(format!(
                            "HBM {} GB · selected scope",
                            gb(process_memory(snapshot, p.pid, npu, die))
                        )))
                        .child(
                            self.action_button(
                                ("inspect-command", i),
                                "Command and working directory",
                                if self.expanded_process == Some((id, p.pid)) {
                                    IconName::ChevronUp
                                } else {
                                    IconName::ChevronDown
                                },
                            )
                            .on_click(cx.listener({
                                let pid = p.pid;
                                move |s, _, _, cx| {
                                    s.expanded_process = if s.expanded_process == Some((id, pid)) {
                                        None
                                    } else {
                                        Some((id, pid))
                                    };
                                    cx.notify();
                                }
                            })),
                        )
                        .when(self.expanded_process == Some((id, p.pid)), |d| {
                            d.child(div().text_size(px(13.)).child(format!(
                                "Working directory: {}",
                                if p.cwd.is_empty() {
                                    "Unavailable"
                                } else {
                                    &p.cwd
                                }
                            )))
                            .child(div().text_size(px(13.)).child(if p.command.is_empty() {
                                "Command unavailable".into()
                            } else {
                                p.command.clone()
                            }))
                        })
                        .child(self.muted(locations(snapshot, p.pid)))
                }))
                .child(self.muted("Ending a shared process affects every Die it occupies."));
        } else {
            content = content.child(self.empty_state(
                IconName::LoaderCircle,
                "No sample available",
                "Waiting for a fresh collection from this host.",
            ));
        }
        if !e.action_error.is_empty() {
            content = content
                .child(self.notice("Process action needs attention", e.action_error.clone()));
        }
        div()
            .id("monitor-inspector")
            .relative()
            .w_full()
            .max_w_full()
            .h_full()
            .flex_shrink_0()
            .occlude()
            .bg(self.c(self.col().bg))
            .border_l_1()
            .border_color(self.c(self.col().line))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_5()
                    .py_3()
                    .bg(self.c(self.col().side))
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(self.muted(format!("{} · {}", e.host.label, e.status()))),
                    )
                    .child(
                        self.close("close-monitor-overlay")
                            .on_click(cx.listener(|s, _, _, cx| s.close_monitor_overlay(cx))),
                    ),
            )
            .when(!reason && e.snapshot.is_some(), |d| {
                d.child(
                    controls
                        .px_5()
                        .pb_3()
                        .gap_2()
                        .flex_shrink_0()
                        .bg(self.c(self.col().side))
                        .border_b_1()
                        .border_color(self.c(self.col().line)),
                )
            })
            .child(
                div()
                    .id("monitor-process-scroll")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scrollers["monitor-process"])
                    .vertical_scrollbar(&self.scrollers["monitor-process"])
                    .px_5()
                    .py_4()
                    .child(content),
            )
            .into_any_element()
    }
    pub(super) fn close_monitor_overlay(&mut self, cx: &mut Context<Self>) {
        self.expanded_process = None;
        self.monitor_reason = None;
        if let Some((id, _, _)) = self.monitor_detail {
            self.monitor_detail = Some((id, None, None));
        }
        cx.notify();
    }
}
