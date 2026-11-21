use super::*;
use gpui_component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use termviai_gpui::terminal_keys::{ROWS, ShortcutMap, ShortcutRow, Slot, display_keys};

#[derive(Clone)]
pub(super) struct Editor {
    draft: ShortcutMap,
    search: Entity<InputState>,
    recorder: FocusHandle,
    edit: Option<Edit>,
    status: String,
}

#[derive(Clone)]
struct Edit {
    slot: &'static Slot,
    recording: bool,
    pending: Option<String>,
    error: String,
}

impl Edit {
    fn new(slot: &'static Slot) -> Self {
        Self {
            slot,
            recording: false,
            pending: None,
            error: String::new(),
        }
    }
}

fn search_text(s: &str) -> String {
    s.to_lowercase().replace([' ', '+', '-'], "")
}

impl AppView {
    fn editor_mut(&mut self) -> Option<&mut Editor> {
        self.drawer.as_mut()?.shortcut_editor.as_mut()
    }

    pub(super) fn open_shortcuts(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        match Settings::load() {
            Ok(current) => self.settings.keyboard_shortcuts = current.keyboard_shortcuts,
            Err(error) => {
                self.error = error.to_string();
                cx.notify();
                return;
            }
        }
        let search =
            cx.new(|cx| InputState::new(w, cx).placeholder("Find an action or key combination"));
        cx.subscribe(
            &search,
            |s, _, event: &gpui_component::input::InputEvent, cx| {
                if matches!(event, gpui_component::input::InputEvent::Change)
                    && let Some(editor) = s.editor_mut()
                {
                    editor.edit = None;
                    cx.notify();
                }
            },
        )
        .detach();
        let recorder = cx.focus_handle();
        cx.on_blur(&recorder, w, |s, _, cx| {
            if let Some(edit) = s.editor_mut().and_then(|e| e.edit.as_mut()) {
                edit.recording = false;
                cx.notify();
            }
        })
        .detach();
        search.focus_handle(cx).focus(w);
        if let Some(f) = self.drawer.as_mut() {
            f.shortcut_editor = Some(Editor {
                draft: self.settings.keyboard_shortcuts.clone(),
                search,
                recorder,
                edit: None,
                status: String::new(),
            });
        }
        self.scrollers["shortcut-settings"].set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn leave_shortcuts(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = self.drawer.as_mut() {
            form.shortcut_editor = None;
        }
        self.focus.focus(w);
        cx.notify();
    }

    pub(super) fn shortcut_editor_key(
        &mut self,
        event: &KeyDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.confirm.is_some() || self.menu.is_some() {
            return false;
        }
        if self
            .drawer
            .as_ref()
            .and_then(|f| f.shortcut_editor.as_ref())
            .is_none()
        {
            return false;
        }
        let key = &event.keystroke;
        let mapped =
            KeybindingKeystroke::new_with_mapper(key.clone(), false, cx.keyboard_mapper().as_ref());
        // Windows repeats key-down until release. A rejected Ctrl+T must never open
        // a new tab while the user is still holding the recorded combination.
        if event.is_held && self.shortcut_keys_down.contains(mapped.key()) {
            return true;
        }
        let editor = self.editor_mut().unwrap();
        if key.key == "escape" && !key.modifiers.modified() {
            if event.is_held {
                return true;
            }
            if editor.edit.take().is_some() {
                editor.search.focus_handle(cx).focus(w);
                cx.notify();
            } else {
                self.leave_shortcuts(w, cx);
            }
            return true;
        }
        let Some(edit) = &mut editor.edit else {
            return false;
        };
        if !edit.recording || !editor.recorder.is_focused(w) {
            return false;
        }
        if event.is_held {
            return true;
        }
        let chord = mapped.unparse();
        edit.error = editor
            .draft
            .check(edit.slot, &chord)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        edit.pending = Some(chord);
        edit.recording = false;
        self.shortcut_keys_down.insert(mapped.key().to_owned());
        cx.notify();
        true
    }

    fn save_shortcuts(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self
            .drawer
            .as_mut()
            .and_then(|f| f.shortcut_editor.as_mut())
        else {
            return;
        };
        if editor.edit.is_none() {
            editor.status = match self.settings.save_shortcuts(&editor.draft) {
                Ok(()) => "Changes saved".into(),
                Err(e) => format!("Could not save changes. {e}"),
            };
            cx.notify();
        }
    }

    pub(super) fn shortcut_page(
        &self,
        editor: &Editor,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let query = search_text(&editor.search.read(cx).value());
        let changed = self.settings.keyboard_shortcuts != editor.draft;
        let mut body = self
            .tool_content(window)
            .pb_0()
            .max_w(px(1010. + 2. * Self::tool_gutter(window)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("shortcut-back")
                            .ghost()
                            .small()
                            .label("Settings")
                            .on_click(cx.listener(|s, _, w, cx| s.leave_shortcuts(w, cx))),
                    )
                    .child(Icon::new(IconName::ChevronRight).size(px(14.)))
                    .child(self.muted("Keyboard shortcuts")),
            )
            .child(self.title("Keyboard shortcuts"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&editor.search)
                                .prefix(Icon::new(IconName::Search).size(px(15.))),
                        ),
                    )
                    .child(
                        Button::new("shortcuts-reset")
                            .icon(IconName::Undo2)
                            .label("Restore defaults")
                            .on_click(cx.listener(|s, _, _, cx| {
                                if let Some(e) = s.editor_mut() {
                                    e.draft = ShortcutMap::default();
                                    e.edit = None;
                                    e.status.clear();
                                }
                                cx.notify();
                            })),
                    ),
            );
        let mut found = false;
        for (section, icon) in [
            ("Navigation", "termviai/layers.svg"),
            ("Terminal panes", "icons/square-terminal.svg"),
            ("Terminal clipboard", "termviai/clipboard.svg"),
        ] {
            let mut rows = div()
                .w_full()
                .rounded_lg()
                .border_1()
                .border_color(self.c(self.col().line))
                .bg(self.c(self.col().card))
                .overflow_hidden();
            let mut count = 0;
            for row in ROWS.iter().filter(|r| r.section == section) {
                if !search_text(row.title).contains(&query)
                    && !row.slots.iter().any(|s| {
                        let chord = editor.draft.binding(s);
                        search_text(chord).contains(&query)
                            || search_text(&display_keys(chord).join("")).contains(&query)
                    })
                {
                    continue;
                }
                let mut item = div()
                    .when(count > 0, |d| {
                        d.border_t_1().border_color(self.c(self.col().line))
                    })
                    .child(self.shortcut_row(row, editor, cx));
                if let Some(edit) = &editor.edit
                    && row.slots.iter().any(|s| s.id == edit.slot.id)
                {
                    item = item.child(self.shortcut_edit(row, editor, edit, cx));
                }
                rows = rows.child(item);
                count += 1;
            }
            if count > 0 {
                found = true;
                let group = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(
                                svg()
                                    .path(icon)
                                    .size(px(16.))
                                    .text_color(self.c(self.col().muted)),
                            )
                            .child(section),
                    )
                    .child(rows);
                body=body.child(group.when(section=="Terminal clipboard",|d|d.child(self.muted("Selecting terminal text copies it automatically. Right-click a terminal to paste. In other text fields, use Ctrl+C to copy."))));
            }
        }
        let status = if !editor.status.is_empty() {
            editor.status.as_str()
        } else if changed {
            "Unsaved changes"
        } else {
            ""
        };
        body = body
            .when(!found, |d| d.child(self.muted("No matching shortcuts.")))
            .child(
                div()
                    .border_t_1()
                    .border_color(self.c(self.col().line))
                    .pt_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(self.muted(status.to_owned())))
                    .child(
                        Button::new("shortcuts-cancel")
                            .label("Cancel")
                            .on_click(cx.listener(|s, _, w, cx| s.leave_shortcuts(w, cx))),
                    )
                    .child(
                        Button::new("shortcuts-save")
                            .primary()
                            .label("Save changes")
                            .disabled(!changed || editor.edit.is_some())
                            .on_click(cx.listener(|s, _, _, cx| s.save_shortcuts(cx))),
                    ),
            );
        self.tool_scroll("shortcut-settings", body)
            .into_any_element()
    }

    fn shortcut_row(
        &self,
        row: &'static ShortcutRow,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let first = &row.slots[0];
        let custom = row
            .slots
            .iter()
            .any(|s| editor.draft.binding(s) != s.default);
        let tokens = if row.slots.len() == 1 {
            display_keys(editor.draft.binding(first))
        } else if custom {
            vec!["Custom bindings".to_owned()]
        } else {
            row.summary.split(" + ").map(str::to_owned).collect()
        };
        let selected = editor
            .edit
            .as_ref()
            .is_some_and(|e| row.slots.iter().any(|s| s.id == e.slot.id));
        div()
            .id(first.id)
            .group(first.id)
            .focusable()
            .tab_stop(true)
            .min_h(px(44.))
            .px_4()
            .py_2()
            .cursor_pointer()
            .border_1()
            .border_color(gpui::transparent_black())
            .hover(|d| d.bg(self.c(self.col().hover)))
            .focus(|d| d.border_color(self.c(self.col().accent)))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(row.title))
            .children(tokens.into_iter().map(|key| {
                div()
                    .text_size(px(12.))
                    .px_2()
                    .rounded_md()
                    .border_1()
                    .border_color(self.c(self.col().line))
                    .bg(self.c(self.col().soft))
                    .child(key)
            }))
            .child(
                svg()
                    .path("termviai/pencil.svg")
                    .size(px(15.))
                    .text_color(self.c(self.col().muted))
                    .when(!selected, |d| {
                        d.opacity(0.).group_hover(first.id, |d| d.opacity(1.))
                    }),
            )
            .when(selected, |b| b.bg(self.c(self.col().hover)))
            .on_click(cx.listener(move |s, _, _, cx| {
                if let Some(e) = s.editor_mut() {
                    e.edit = if selected {
                        None
                    } else {
                        Some(Edit::new(first))
                    };
                    e.status.clear();
                }
                cx.notify();
            }))
    }

    fn shortcut_edit(
        &self,
        row: &'static ShortcutRow,
        editor: &Editor,
        edit: &Edit,
        cx: &mut Context<Self>,
    ) -> Div {
        let slot = edit.slot;
        let owner = cx.entity().downgrade();
        let target = Button::new("shortcut-position")
            .label(slot.label())
            .icon(IconName::ChevronDown)
            .dropdown_menu(move |mut menu, _, _| {
                for choice in row.slots {
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(choice.label())
                            .checked(choice.id == slot.id)
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    if let Some(e) = s.editor_mut() {
                                        e.edit = Some(Edit::new(choice));
                                    }
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });
        let key = edit
            .pending
            .as_deref()
            .unwrap_or_else(|| editor.draft.binding(slot));
        let label = if edit.recording {
            "Press a combination…".to_owned()
        } else {
            display_keys(key).join(" + ")
        };
        let note = if !edit.error.is_empty() {
            edit.error.as_str()
        } else if edit.recording {
            "Press Esc to cancel recording."
        } else {
            "Select the key combination to record a replacement."
        };
        let can_apply = edit.pending.is_some() && edit.error.is_empty() && !edit.recording;
        div()
            .bg(self.c(self.col().side))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(row.slots.len() > 1, |d| d.child(target))
                    .child(
                        div()
                            .track_focus(&editor.recorder)
                            .key_context("TermViAIShortcutRecorder")
                            .child(
                                Button::new("record-shortcut")
                                    .label(label)
                                    .when(edit.recording, |b| {
                                        b.border_color(self.c(self.col().accent))
                                    })
                                    .on_click(cx.listener(|s, _, w, cx| {
                                        if let Some(e) = s.editor_mut()
                                            && let Some(edit) = &mut e.edit
                                        {
                                            edit.recording = true;
                                            edit.pending = None;
                                            edit.error.clear();
                                            e.recorder.focus(w);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        Button::new("apply-shortcut")
                            .label("Apply")
                            .disabled(!can_apply)
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(e) = s.editor_mut()
                                    && let Some(edit) = &e.edit
                                    && let Some(key) = &edit.pending
                                {
                                    match e.draft.assign(edit.slot, key) {
                                        Ok(()) => {
                                            e.edit = None;
                                            e.status.clear();
                                            e.search.focus_handle(cx).focus(w);
                                        }
                                        Err(err) => e.status = err.to_string(),
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("cancel-shortcut")
                            .label("Cancel")
                            .on_click(cx.listener(|s, _, w, cx| {
                                if let Some(e) = s.editor_mut() {
                                    e.edit = None;
                                    e.search.focus_handle(cx).focus(w);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                self.muted(note.to_owned())
                    .when(!edit.error.is_empty(), |d| {
                        d.text_color(self.c(self.col().red))
                    }),
            )
    }
}
