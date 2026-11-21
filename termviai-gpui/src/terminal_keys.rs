//! Default application shortcuts and terminal focus bindings.
use crate::layout::Side;
use gpui::{KeyBinding, Keystroke, NoAction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const CONTEXT: &str = "TermViAITerminal";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Tab(usize),
    Group(usize),
    CycleTab { reverse: bool },
    CycleGroup { reverse: bool },
    FocusPane(Side),
    ToggleMember,
    ToggleBroadcast,
    Copy,
    Paste,
}

/// Stable slot IDs allow the stored overrides to survive changes to labels and defaults.
pub struct Slot {
    pub id: &'static str,
    pub default: &'static str,
    pub action: Shortcut,
}
impl Slot {
    const fn new(id: &'static str, default: &'static str, action: Shortcut) -> Self {
        Self {
            id,
            default,
            action,
        }
    }
    pub fn label(&self) -> String {
        match self.action {
            Shortcut::Tab(i) | Shortcut::Group(i) => format!("Position {}", i + 1),
            Shortcut::CycleTab { reverse } | Shortcut::CycleGroup { reverse } => {
                if reverse { "Previous" } else { "Next" }.into()
            }
            _ => self.default.rsplit('-').next().unwrap().to_uppercase(),
        }
    }
}
pub struct ShortcutRow {
    pub section: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub slots: &'static [Slot],
}

use Shortcut::*;
pub const ROWS: &[ShortcutRow] = &[
    ShortcutRow {
        section: "Navigation",
        title: "Switch to numbered tab",
        summary: "Ctrl + 1–9, 0",
        slots: &[
            Slot::new("tab.1", "ctrl-1", Tab(0)),
            Slot::new("tab.2", "ctrl-2", Tab(1)),
            Slot::new("tab.3", "ctrl-3", Tab(2)),
            Slot::new("tab.4", "ctrl-4", Tab(3)),
            Slot::new("tab.5", "ctrl-5", Tab(4)),
            Slot::new("tab.6", "ctrl-6", Tab(5)),
            Slot::new("tab.7", "ctrl-7", Tab(6)),
            Slot::new("tab.8", "ctrl-8", Tab(7)),
            Slot::new("tab.9", "ctrl-9", Tab(8)),
            Slot::new("tab.10", "ctrl-0", Tab(9)),
        ],
    },
    ShortcutRow {
        section: "Navigation",
        title: "Switch to numbered tab group",
        summary: "Ctrl + Shift + 1–9, 0",
        slots: &[
            Slot::new("group.1", "ctrl-shift-1", Group(0)),
            Slot::new("group.2", "ctrl-shift-2", Group(1)),
            Slot::new("group.3", "ctrl-shift-3", Group(2)),
            Slot::new("group.4", "ctrl-shift-4", Group(3)),
            Slot::new("group.5", "ctrl-shift-5", Group(4)),
            Slot::new("group.6", "ctrl-shift-6", Group(5)),
            Slot::new("group.7", "ctrl-shift-7", Group(6)),
            Slot::new("group.8", "ctrl-shift-8", Group(7)),
            Slot::new("group.9", "ctrl-shift-9", Group(8)),
            Slot::new("group.10", "ctrl-shift-0", Group(9)),
        ],
    },
    ShortcutRow {
        section: "Navigation",
        title: "Previous / next tab",
        summary: "Ctrl + Alt + ← / →",
        slots: &[
            Slot::new("tab.previous", "ctrl-alt-left", CycleTab { reverse: true }),
            Slot::new("tab.next", "ctrl-alt-right", CycleTab { reverse: false }),
        ],
    },
    ShortcutRow {
        section: "Navigation",
        title: "Previous / next tab group",
        summary: "Ctrl + Alt + ↑ / ↓",
        slots: &[
            Slot::new(
                "group.previous",
                "ctrl-alt-up",
                CycleGroup { reverse: true },
            ),
            Slot::new("group.next", "ctrl-alt-down", CycleGroup { reverse: false }),
        ],
    },
    ShortcutRow {
        section: "Terminal panes",
        title: "Focus adjacent terminal",
        summary: "Alt + ← / → / ↑ / ↓",
        slots: &[
            Slot::new("focus.left", "alt-left", FocusPane(Side::Left)),
            Slot::new("focus.right", "alt-right", FocusPane(Side::Right)),
            Slot::new("focus.up", "alt-up", FocusPane(Side::Top)),
            Slot::new("focus.down", "alt-down", FocusPane(Side::Bottom)),
        ],
    },
    ShortcutRow {
        section: "Terminal panes",
        title: "Toggle focused terminal broadcast",
        summary: "Alt + B",
        slots: &[Slot::new("broadcast.member", "alt-b", ToggleMember)],
    },
    ShortcutRow {
        section: "Terminal panes",
        title: "Toggle terminal group broadcast",
        summary: "Ctrl + Alt + B",
        slots: &[Slot::new("broadcast.group", "ctrl-alt-b", ToggleBroadcast)],
    },
    ShortcutRow {
        section: "Terminal clipboard",
        title: "Copy terminal selection",
        summary: "Ctrl + Shift + C",
        slots: &[Slot::new("clipboard.copy", "ctrl-shift-c", Copy)],
    },
    ShortcutRow {
        section: "Terminal clipboard",
        title: "Paste into terminal",
        summary: "Ctrl + Shift + V",
        slots: &[Slot::new("clipboard.paste", "ctrl-shift-v", Paste)],
    },
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShortcutMap(BTreeMap<String, String>);

impl ShortcutMap {
    pub fn binding<'a>(&'a self, slot: &'a Slot) -> &'a str {
        self.0
            .get(slot.id)
            .map(String::as_str)
            .unwrap_or(slot.default)
    }

    pub fn resolve(&self, chord: &str, terminal_focused: bool) -> Option<Shortcut> {
        ROWS.iter()
            .flat_map(|row| row.slots)
            .find(|slot| self.binding(slot) == chord && (chord != "ctrl-c" || slot.action == Copy))
            .map(|s| s.action)
            .filter(|action| terminal_focused || !matches!(action, Copy | Paste))
    }

    pub fn check(&self, slot: &Slot, chord: &str) -> anyhow::Result<()> {
        let key = Keystroke::parse(chord)?;
        anyhow::ensure!(
            key.modifiers.modified()
                && !(key.modifiers.shift
                    && key.modifiers.number_of_modifiers() == 1
                    && (key.key.chars().count() == 1 || key.key == "space")),
            "Include a modifier that keeps normal typing available."
        );
        anyhow::ensure!(
            !matches!(
                chord,
                "ctrl-t"
                    | "ctrl-s"
                    | "ctrl-tab"
                    | "ctrl-shift-tab"
                    | "shift-tab"
                    | "ctrl-="
                    | "ctrl-+"
                    | "ctrl--"
                    | "ctrl-shift-="
                    | "alt-f4"
            ),
            "This combination already has an application or window action."
        );
        anyhow::ensure!(
            chord != "ctrl-c" || slot.action == Copy,
            "Ctrl+C is reserved for copying text."
        );
        // Navigation also runs outside terminal panes. Keep Windows text commands available there.
        if matches!(
            slot.action,
            Tab(_) | Group(_) | CycleTab { .. } | CycleGroup { .. }
        ) {
            anyhow::ensure!(
                !matches!(
                    chord,
                    "ctrl-c" | "ctrl-v" | "ctrl-x" | "ctrl-a" | "ctrl-z" | "ctrl-y"
                ),
                "This combination is used for text editing outside terminals."
            );
        }
        for row in ROWS {
            if let Some(other) = row
                .slots
                .iter()
                .find(|other| other.id != slot.id && self.binding(other) == chord)
            {
                anyhow::bail!("Already used by {} ({}).", row.title, other.label());
            }
        }
        Ok(())
    }
    pub fn assign(&mut self, slot: &Slot, chord: &str) -> anyhow::Result<()> {
        self.check(slot, chord)?;
        if chord == slot.default {
            self.0.remove(slot.id);
        } else {
            self.0.insert(slot.id.into(), chord.into());
        }
        Ok(())
    }
}
pub fn display_keys(chord: &str) -> Vec<String> {
    let Ok(k) = Keystroke::parse(chord) else {
        return vec![chord.into()];
    };
    let m = k.modifiers;
    let mut parts: Vec<_> = [
        (m.control, "Ctrl"),
        (m.alt, "Alt"),
        (m.platform, "Win"),
        (m.shift, "Shift"),
        (m.function, "Fn"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, s)| s.to_owned())
    .collect();
    parts.push(match k.key.as_str() {
        "left" => "←".into(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        _ => k.key.to_uppercase(),
    });
    parts
}

/// GPUI resolves actions before raw key listeners. NoAction removes the Root
/// focus-navigation binding in this context, allowing the existing terminal key
/// listener to encode the actual keystroke (including Shift and terminal modes).
pub fn bindings() -> [KeyBinding; 2] {
    [
        KeyBinding::new("tab", NoAction, Some(CONTEXT)),
        KeyBinding::new("shift-tab", NoAction, Some(CONTEXT)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{KeyContext, Keymap, actions};
    actions!(terminal_key_test, [NextField, PreviousField]);

    #[test]
    fn clipboard_bindings_only_match_a_focused_terminal_even_after_rebinding() {
        let mut map = ShortcutMap::default();
        for key in ["ctrl-shift-c", "ctrl-shift-v", "ctrl-c"] {
            assert_eq!(map.resolve(key, false), None);
        }
        let slot = ROWS
            .iter()
            .flat_map(|row| row.slots)
            .find(|slot| slot.action == Copy)
            .unwrap();
        map.assign(slot, "ctrl-c").unwrap();
        assert_eq!(map.resolve("ctrl-c", true), Some(Copy));
        assert_eq!(map.resolve("ctrl-c", false), None);
        assert_eq!(map.resolve("ctrl-shift-c", true), None);
        assert_eq!(map.resolve("ctrl-shift-v", true), Some(Paste));
    }

    #[test]
    fn edit_checks_conflicts_and_preserves_other_position_bindings() {
        let mut map = ShortcutMap::default();
        let first = &ROWS[0].slots[0];
        for key in [
            "ctrl-2", "alt-b", "ctrl-c", "ctrl-t", "ctrl-tab", "shift-a", "q",
        ] {
            assert!(map.assign(first, key).is_err(), "{key}");
        }
        assert_eq!(map, ShortcutMap::default());
        map.assign(first, "ctrl-alt-k").unwrap();
        assert_eq!(map.resolve("ctrl-alt-k", true), Some(Tab(0)));
        assert_eq!(map.resolve("ctrl-1", true), None);
        assert_eq!(map.resolve("ctrl-2", true), Some(Tab(1)));
        map.assign(first, first.default).unwrap();
        assert_eq!(map, ShortcutMap::default());
    }

    #[test]
    fn default_shortcuts_keep_tabs_groups_and_panes_distinct() {
        use Shortcut::*;
        let map = ShortcutMap::default();
        let decode = |binding: &str| map.resolve(binding, true);
        for (index, digit) in "1234567890".chars().enumerate() {
            assert_eq!(decode(&format!("ctrl-{digit}")), Some(Tab(index)));
            assert_eq!(decode(&format!("ctrl-shift-{digit}")), Some(Group(index)));
        }
        for (binding, shortcut) in [
            ("ctrl-alt-left", CycleTab { reverse: true }),
            ("ctrl-alt-right", CycleTab { reverse: false }),
            ("ctrl-alt-up", CycleGroup { reverse: true }),
            ("ctrl-alt-down", CycleGroup { reverse: false }),
            ("alt-left", FocusPane(Side::Left)),
            ("alt-right", FocusPane(Side::Right)),
            ("alt-up", FocusPane(Side::Top)),
            ("alt-down", FocusPane(Side::Bottom)),
            ("alt-b", ToggleMember),
            ("ctrl-alt-b", ToggleBroadcast),
            ("ctrl-shift-c", Copy),
            ("ctrl-shift-v", Paste),
        ] {
            assert_eq!(decode(binding), Some(shortcut));
        }
        for binding in [
            "1",
            "shift-1",
            "ctrl-c",
            "ctrl-v",
            "ctrl-alt-shift-b",
            "cmd-alt-right",
            "alt-shift-down",
        ] {
            assert_eq!(decode(binding), None);
        }
    }

    #[test]
    fn terminal_tabs_bypass_root_actions_but_forms_keep_focus_navigation() {
        let mut map = Keymap::default();
        map.add_bindings([
            KeyBinding::new("tab", NextField, Some("Root")),
            KeyBinding::new("shift-tab", PreviousField, Some("Root")),
        ]);
        map.add_bindings(bindings());
        for key in ["tab", "shift-tab"] {
            let stroke = Keystroke::parse(key).unwrap();
            let (matches, pending) = map.bindings_for_input(
                &[stroke.clone()],
                &[
                    KeyContext::parse("Root").unwrap(),
                    KeyContext::parse(CONTEXT).unwrap(),
                ],
            );
            assert!(
                matches.is_empty(),
                "{key} must reach the terminal key listener"
            );
            assert!(!pending, "{key} must not wait for a key chord");
            let (matches, pending) = map.bindings_for_input(
                &[stroke],
                &[
                    KeyContext::parse("Root").unwrap(),
                    KeyContext::parse("Input").unwrap(),
                ],
            );
            assert_eq!(matches.len(), 1, "{key} still navigates form fields");
            assert!(!pending);
        }
    }
}
