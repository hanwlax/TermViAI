//! Windows shell smoke test. This exercises real cmd.exe pipes, not ConPTY/SSH.
#![cfg(windows)]
use std::io::Write;
use std::process::{Command, Stdio};
use termviai_broadcast::{route_input, BroadcastState};

#[test]
fn three_windows_shells_broadcast_and_fourth_is_independent() {
    let mut children: Vec<_> = (0..4)
        .map(|_| {
            Command::new("cmd.exe")
                .args(["/d", "/q"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("start Windows command shell")
        })
        .collect();
    for child in &mut children {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"@echo off\r\nprompt $S\r\n")
            .unwrap();
    }
    let mut state = BroadcastState::new(0..4);
    state.set_member(3, false).unwrap();
    state.set_enabled(true);
    for (source, bytes) in [
        (0, b"echo TERmX_BROADCAST_ONLY\r\n".as_slice()),
        (3, b"echo TERmX_INDEPENDENT_ONLY\r\n".as_slice()),
    ] {
        let report = route_input(
            &state,
            source,
            bytes,
            |_| true,
            |id, data| children[id].stdin.as_mut().unwrap().write_all(data),
        )
        .unwrap();
        assert!(report.failures.is_empty());
    }
    state.set_enabled(false);
    let report = route_input(
        &state,
        1,
        b"echo TERmX_OFF_ONLY\r\n",
        |_| true,
        |id, data| children[id].stdin.as_mut().unwrap().write_all(data),
    )
    .unwrap();
    assert_eq!(report.delivered, [1]);
    for child in &mut children {
        child.stdin.take().unwrap().write_all(b"exit\r\n").unwrap();
    }
    for (id, child) in children.into_iter().enumerate() {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            text.contains("TERmX_BROADCAST_ONLY"),
            id != 3,
            "shell {id}: {text}"
        );
        assert_eq!(
            text.contains("TERmX_INDEPENDENT_ONLY"),
            id == 3,
            "shell {id}: {text}"
        );
        assert_eq!(
            text.contains("TERmX_OFF_ONLY"),
            id == 1,
            "shell {id}: {text}"
        );
    }
}
