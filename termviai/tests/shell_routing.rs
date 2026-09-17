//! Real subprocess pipes exercise the routing callback; not a PTY/GUI test.
#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Stdio};
use termviai_broadcast::{route_input, BroadcastState};

#[test]
fn three_shells_broadcast_and_fourth_shell_remains_independent() {
    let mut shells: Vec<_> = (0..4)
        .map(|_| {
            Command::new("/bin/sh")
                .arg("-s")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut state = BroadcastState::new(0..4);
    state.set_member(3, false).unwrap();
    state.set_enabled(true);
    let mut dispatch = |source, bytes: &[u8]| {
        let report = route_input(
            &state,
            source,
            bytes,
            |_| true,
            |id, input| shells[id].stdin.as_mut().unwrap().write_all(input),
        )
        .unwrap();
        assert!(report.failures.is_empty());
    };
    dispatch(1, b"printf 'broadcast\\n'\n");
    dispatch(3, b"printf 'independent\\n'\n");
    for (id, mut shell) in shells.into_iter().enumerate() {
        drop(shell.stdin.take());
        let output = shell.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        let expected: &[u8] = if id == 3 {
            b"independent\n"
        } else {
            b"broadcast\n"
        };
        assert_eq!(output.stdout, expected);
    }
}
