// Compile the production sources, rather than copying their schemas or replacing
// the terminal/SSH engines used by TermViAI.
#[path = "../../mux/src/ssh/authentication.rs"]
pub mod authentication;
pub mod broadcast;
#[path = "../../wezterm-gui/src/forwarding.rs"]
pub mod forwarding;
#[path = "../../wezterm-gui/src/hosts.rs"]
pub mod hosts;
pub mod layout;
#[path = "../../wezterm-gui/src/monitor.rs"]
pub mod monitor;
pub mod monitor_presenter;
pub mod session;
pub mod settings;
pub mod terminal_keys;
pub mod theme;
#[path = "../../wezterm-gui/src/workspaces.rs"]
pub mod workspaces;
