//! Let the terminal core own Tab without disabling form keyboard navigation.
use gpui::{KeyBinding, NoAction};
pub const CONTEXT: &str = "TermViAITerminal";

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
    use gpui::{KeyContext, Keymap, Keystroke, actions};
    actions!(terminal_key_test, [NextField, PreviousField]);

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
