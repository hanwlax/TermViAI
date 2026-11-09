//! Opt-in loopback fixture: never contacts a user's saved SSH hosts.
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use wezterm_ssh::{RemoteForward, Session, SessionEvent};
fn connect() -> Session {
    let port = std::env::var("TERMVIAI_TEST_SSH_PORT")
        .expect("Start the disposable loopback sshd fixture first");
    let mut cfg = wezterm_ssh::Config::new().for_host("127.0.0.1");
    cfg.insert("hostname".into(), "127.0.0.1".into());
    cfg.insert("port".into(), port);
    cfg.insert(
        "user".into(),
        std::env::var("TERMVIAI_TEST_SSH_USER").unwrap(),
    );
    cfg.insert(
        "termviai_identity_file".into(),
        std::env::var("TERMVIAI_TEST_SSH_KEY").unwrap(),
    );
    cfg.insert("termviai_auth_mode".into(), "key".into());
    cfg.insert("termviai_no_ssh_config".into(), "yes".into());
    cfg.insert(
        "userknownhostsfile".into(),
        std::env::var("TERMVIAI_TEST_SSH_KNOWN").unwrap(),
    );
    cfg.insert("wezterm_ssh_backend".into(), "libssh".into());
    cfg.insert("connecttimeout".into(), "5".into());
    let (s, events) = Session::connect(cfg).unwrap();
    smol::block_on(async {
        loop {
            let e = smol::future::or(async { events.recv().await.unwrap() }, async {
                smol::Timer::after(Duration::from_secs(15)).await;
                panic!("test authentication timeout")
            })
            .await;
            match e {
                SessionEvent::Authenticated => break,
                SessionEvent::Banner(_) => (),
                SessionEvent::HostVerify(v) => v.answer(true).await.unwrap(),
                other => panic!("fixture SSH: {:?}", other),
            }
        }
    });
    s
}
fn command(s: &Session, cmd: &str) -> String {
    let mut e = smol::block_on(s.exec(cmd, None)).unwrap();
    e.stdout.set_non_blocking(true).unwrap();
    e.stderr.set_non_blocking(true).unwrap();
    let mut out = vec![];
    let start = std::time::Instant::now();
    loop {
        let mut buf = [0; 8192];
        match e.stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(err) => panic!("{}", err),
        };
        assert!(
            start.elapsed() < Duration::from_secs(25),
            "remote command timed out: {}",
            String::from_utf8_lossy(&out)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    String::from_utf8(out).unwrap()
}
#[test]
#[ignore = "requires disposable loopback sshd and TERMVIAI_TEST_SSH_* variables"]
fn reverse_forward_concurrent_half_close_and_stop() {
    let local = TcpListener::bind("127.0.0.1:0").unwrap();
    local.set_nonblocking(true).unwrap();
    let port = local.local_addr().unwrap().port();
    let service = std::thread::spawn(move || {
        let until = std::time::Instant::now() + Duration::from_secs(25);
        let mut workers = vec![];
        while workers.len() < 3 {
            match local.accept() {
                Ok((mut s, _)) => {
                    s.set_nonblocking(false).unwrap();
                    s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
                    workers.push(std::thread::spawn(move || {
                        let mut data = vec![];
                        s.read_to_end(&mut data).unwrap();
                        assert_eq!(data, vec![b'x'; 180000]);
                        s.write_all(&vec![b'y'; 210000]).unwrap();
                        s.shutdown(Shutdown::Write).unwrap();
                    }));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("{}", e),
            };
            assert!(std::time::Instant::now() < until, "local accepts timed out");
        }
        for w in workers {
            w.join().unwrap();
        }
    });
    let terminal = connect();
    // A server that cannot register a session must not receive a listener
    // before a fresh, ordinary-forwarding fallback is created.
    let unavailable = connect();
    let (unavailable_ready, unavailable_rx) = mpsc::channel();
    let remote: u16 = std::env::var("TERMVIAI_TEST_REMOTE_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let error = smol::block_on(unavailable.run_remote_forward(RemoteForward {
        bind: "127.0.0.1".into(),
        remote_port: remote,
        local_port: port,
        cancel: Arc::new(AtomicBool::new(false)),
        ready: unavailable_ready,
        setup: Some(wezterm_ssh::RemoteForwardSetup {
            command: "printf 'TERMVIAI_FORWARD_V1=fixture\\n'".into(),
            commit: Box::new(|result| {
                assert_eq!(result?, "fixture\n");
                Ok(false)
            }),
        }),
    }))
    .unwrap_err();
    assert!(error
        .downcast_ref::<wezterm_ssh::RemoteForwardSetupUnavailable>()
        .is_some());
    assert!(unavailable_rx.try_recv().is_err());
    let forward = connect();
    let cancel = Arc::new(AtomicBool::new(false));
    let (ready, rx) = mpsc::channel();
    let stop = cancel.clone();
    let worker = std::thread::spawn(move || {
        smol::block_on(forward.run_remote_forward(RemoteForward {
            bind: "127.0.0.1".into(),
            remote_port: remote,
            local_port: port,
            cancel: stop,
            ready,
            setup: None,
        }))
    });
    rx.recv_timeout(Duration::from_secs(15)).unwrap();
    // A second attempt must not announce success for an occupied remote port.
    let duplicate = connect();
    let (ready, duplicate_ready) = mpsc::channel();
    assert!(smol::block_on(duplicate.run_remote_forward(RemoteForward {
        bind: "127.0.0.1".into(),
        remote_port: remote,
        local_port: port,
        cancel: Arc::new(AtomicBool::new(false)),
        ready,
        setup: None,
    }))
    .is_err());
    assert!(duplicate_ready.try_recv().is_err());
    let script=format!("import socket,concurrent.futures\ndef run(i):\n s=socket.create_connection(('127.0.0.1',{}),10);s.settimeout(15);s.sendall(b'x'*180000);s.shutdown(socket.SHUT_WR);data=b''\n while True:\n  chunk=s.recv(65536)\n  if not chunk:break\n  data+=chunk\n assert data==b'y'*210000\n s.close()\n return len(data)\nwith concurrent.futures.ThreadPoolExecutor(max_workers=3) as e: print(list(e.map(run,range(3))))",remote);
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(script);
    let out = command(
        &terminal,
        &format!(
            "python3 -c 'import base64;exec(base64.b64decode(\"{}\"))'",
            b64
        ),
    );
    assert!(out.contains("[210000, 210000, 210000]"), "{}", out);
    service.join().unwrap();
    cancel.store(true, Ordering::Relaxed);
    worker.join().unwrap().unwrap();
    assert_eq!(
        command(&terminal, "printf terminal-still-alive"),
        "terminal-still-alive"
    );
    let out=command(&terminal,&format!("python3 -c 'import socket;s=socket.socket();s.settimeout(2);print(s.connect_ex((\"127.0.0.1\",{})))'",remote));
    assert_ne!(out.trim(), "0", "remote listener survived stop");
}
