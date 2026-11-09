//! Console-subsystem test producer, separate from the GUI-subsystem application.
#[path = "pty.rs"]
mod pty;
fn main() {
    if std::env::args().any(|s| s == "--stream") {
        pty::stream();
    } else {
        pty::fixture();
    }
}
