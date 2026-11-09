//! Pane-local authentication handoff. Contains opaque credential references only.
//! Every alternative uses a fresh transport: SSH usernames cannot be changed on
//! an authenticated/partially authenticated connection, and MaxAuthTries is finite.
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wezterm_ssh::{ConfigMap, Session, SessionEvent};

#[derive(Clone, Debug)]
pub struct Candidate {
    pub label: String,
    pub config: ConfigMap,
    pub error: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Trying,
    Waiting,
    Challenge,
    Verify,
    Opening,
    Connected,
    Failed,
    Closed,
}
#[derive(Clone, Debug)]
pub struct View {
    pub phase: Phase,
    pub label: String,
    pub error: String,
    pub logs: Vec<String>,
    pub prompts: Vec<(String, bool)>,
    pub authenticated: bool,
    pub revision: u64,
}
#[derive(Debug)]
pub enum Command {
    Retry(Candidate),
    Answers(Vec<String>),
    Trust(bool),
    Cancel,
}
#[derive(Debug)]
pub struct Control {
    view: Mutex<View>,
    tx: smol::channel::Sender<Command>,
    rx: smol::channel::Receiver<Command>,
    cancelled: std::sync::atomic::AtomicBool,
}
impl Control {
    pub fn new() -> Arc<Self> {
        let (tx, rx) = smol::channel::bounded(1);
        Arc::new(Self {
            view: Mutex::new(View {
                phase: Phase::Trying,
                label: String::new(),
                error: String::new(),
                logs: vec![],
                prompts: vec![],
                authenticated: false,
                revision: 0,
            }),
            tx,
            rx,
            cancelled: std::sync::atomic::AtomicBool::new(false),
        })
    }
    pub fn view(&self) -> View {
        self.view.lock().unwrap().clone()
    }
    pub fn send(&self, command: Command) -> anyhow::Result<()> {
        if matches!(command, Command::Cancel) {
            self.cancel();
            return Ok(());
        }
        self.tx
            .try_send(command)
            .map_err(|_| anyhow::anyhow!("An authentication request is already in progress"))
    }
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = self.tx.try_send(Command::Cancel);
    }
    fn check_cancelled(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.cancelled.load(std::sync::atomic::Ordering::Acquire),
            "Connection closed"
        );
        Ok(())
    }
    pub fn update(&self, phase: Phase, error: String) {
        let mut v = self.view.lock().unwrap();
        v.phase = phase;
        v.error = error;
        v.prompts.clear();
        v.revision += 1;
    }
    fn log(&self, text: String) {
        let mut v = self.view.lock().unwrap();
        if v.logs.len() >= 24 {
            v.logs.remove(0);
        }
        v.logs.push(text);
        v.revision += 1;
    }
    fn wait(&self) -> anyhow::Result<Command> {
        loop {
            self.check_cancelled()?;
            if let Ok(c) = self.rx.try_recv() {
                return Ok(c);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn event(&self, rx: &smol::channel::Receiver<SessionEvent>) -> anyhow::Result<SessionEvent> {
        let deadline = std::time::Instant::now() + Duration::from_secs(40);
        loop {
            self.check_cancelled()?;
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "SSH authentication response timed out"
            );
            match rx.try_recv() {
                Ok(e) => return Ok(e),
                Err(smol::channel::TryRecvError::Closed) => {
                    anyhow::bail!("SSH connection ended before authentication completed")
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}

pub fn connect(control: &Control, candidates: Vec<Candidate>) -> anyhow::Result<Session> {
    connect_with(control, candidates, Session::connect)
}
fn connect_with<S>(
    control: &Control,
    candidates: Vec<Candidate>,
    mut start: impl FnMut(ConfigMap) -> anyhow::Result<(S, smol::channel::Receiver<SessionEvent>)>,
) -> anyhow::Result<S> {
    let mut queue: VecDeque<_> = candidates.into();
    let mut interactive = false;
    loop {
        control.check_cancelled()?;
        let candidate = if let Some(c) = queue.pop_front() {
            c
        } else {
            control.update(Phase::Waiting, String::new());
            loop {
                match control.wait()? {
                    Command::Retry(c) => {
                        interactive = true;
                        break c;
                    }
                    Command::Cancel => anyhow::bail!("Connection closed"),
                    _ => {}
                }
            }
        };
        {
            let mut v = control.view.lock().unwrap();
            v.label = candidate.label.clone();
        }
        control.update(Phase::Trying, String::new());
        if let Some(error) = candidate.error {
            control.log(format!("{}: {error}", candidate.label));
            continue;
        }
        let (session, events) = start(candidate.config)?;
        let failure = loop {
            match control.event(&events)? {
                SessionEvent::Banner(_) => {}
                SessionEvent::HostVerify(verify) => {
                    control.update(Phase::Verify, verify.message.clone());
                    let trust = matches!(control.wait()?, Command::Trust(true));
                    smol::block_on(verify.answer(trust))?;
                    anyhow::ensure!(trust, "SSH host key was not trusted");
                    control.update(Phase::Trying, String::new());
                }
                SessionEvent::HostVerificationFailed(failed) => {
                    anyhow::bail!("SSH host verification failed: {failed:?}")
                }
                SessionEvent::Authenticate(auth) => {
                    if !interactive {
                        // Dropping the event closes its reply channel. Do not submit
                        // empty passwords or replay a stored secret into an OTP prompt.
                        break "Not accepted or additional authentication required".to_owned();
                    }
                    {
                        let mut v = control.view.lock().unwrap();
                        v.phase = Phase::Challenge;
                        v.error = auth.instructions.clone();
                        v.prompts = auth
                            .prompts
                            .iter()
                            .map(|p| (p.prompt.clone(), p.echo))
                            .collect();
                        v.revision += 1;
                    }
                    match control.wait()? {
                        Command::Answers(answers) => smol::block_on(auth.answer(answers))?,
                        _ => anyhow::bail!("Authentication cancelled"),
                    }
                    control.update(Phase::Trying, String::new());
                }
                SessionEvent::AuthenticationFailed(error) => {
                    break error;
                }
                SessionEvent::Error(error) => return Err(anyhow::anyhow!(error)),
                SessionEvent::Authenticated => {
                    control.log(format!("{}: accepted", candidate.label));
                    let mut v = control.view.lock().unwrap();
                    v.authenticated = true;
                    v.phase = Phase::Opening;
                    v.revision += 1;
                    return Ok(session);
                }
            }
        };
        drop(events);
        drop(session);
        {
            let error = failure;
            control.log(format!("{}: {error}", candidate.label));
            if interactive {
                control.update(Phase::Waiting, error);
                // Keep the error visible while awaiting another explicit attempt.
                match control.wait()? {
                    Command::Retry(c) => queue.push_back(c),
                    _ => anyhow::bail!("Authentication cancelled"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_wakes_wait_without_sending_credentials() {
        let c = Control::new();
        c.cancel();
        assert!(c.wait().is_err());
    }
    #[test]
    fn view_does_not_contain_commands_or_passwords() {
        let c = Control::new();
        c.send(Command::Answers(vec!["secret".into()])).unwrap();
        assert!(!format!("{:?}", c.view()).contains("secret"));
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    fn candidate(label: &str) -> Candidate {
        Candidate {
            label: label.into(),
            config: ConfigMap::from([("user".into(), label.into())]),
            error: None,
        }
    }
    fn events(event: SessionEvent) -> smol::channel::Receiver<SessionEvent> {
        let (tx, rx) = smol::channel::bounded(1);
        tx.try_send(event).unwrap();
        rx
    }
    #[test]
    fn alternatives_use_fresh_connections_in_order_and_stop_at_success() {
        for success in ["key", "identity", "password"] {
            let c = Control::new();
            let mut attempted = vec![];
            let result = connect_with(
                &c,
                vec![
                    candidate("key"),
                    candidate("identity"),
                    candidate("password"),
                ],
                |config| {
                    let user = config["user"].clone();
                    attempted.push(user.clone());
                    let accepted = user == success;
                    Ok((
                        user,
                        events(if accepted {
                            SessionEvent::Authenticated
                        } else {
                            SessionEvent::AuthenticationFailed("Rejected".into())
                        }),
                    ))
                },
            )
            .unwrap();
            assert_eq!(result, success);
            assert_eq!(attempted.last().unwrap(), success);
            assert!(c.view().authenticated);
            assert_eq!(
                attempted.len(),
                match success {
                    "key" => 1,
                    "identity" => 2,
                    _ => 3,
                }
            );
        }
    }
    #[test]
    fn network_and_host_verification_errors_do_not_try_another_password() {
        let c = Control::new();
        let mut attempted = 0;
        let result = connect_with(&c, vec![candidate("key"), candidate("password")], |_| {
            attempted += 1;
            Ok((
                (),
                events(SessionEvent::Error("socket disconnected".into())),
            ))
        });
        assert!(result.is_err());
        assert_eq!(attempted, 1);
        assert!(!c.view().authenticated);
        assert_ne!(c.view().phase, Phase::Waiting);
    }
    #[test]
    fn invalid_local_credentials_do_not_block_later_alternatives() {
        let c = Control::new();
        let mut first = candidate("key");
        first.error = Some("File missing".into());
        let mut attempted = vec![];
        connect_with(&c, vec![first, candidate("password")], |config| {
            attempted.push(config["user"].clone());
            Ok(((), events(SessionEvent::Authenticated)))
        })
        .unwrap();
        assert_eq!(attempted, vec!["password"]);
        assert!(c.view().logs[0].contains("File missing"));
    }
    #[test]
    fn all_rejected_wait_for_explicit_retry_and_success_is_not_reported_early() {
        let c = Control::new();
        let worker = c.clone();
        let thread = std::thread::spawn(move || {
            connect_with(
                &worker,
                vec![
                    candidate("key"),
                    candidate("identity"),
                    candidate("password"),
                ],
                |config| {
                    let user = config["user"].clone();
                    Ok((
                        user.clone(),
                        events(if user == "replacement" {
                            SessionEvent::Authenticated
                        } else {
                            SessionEvent::AuthenticationFailed("Rejected".into())
                        }),
                    ))
                },
            )
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while c.view().phase != Phase::Waiting {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(c.view().logs.len(), 3);
        assert!(!c.view().authenticated);
        c.send(Command::Retry(candidate("replacement"))).unwrap();
        assert_eq!(thread.join().unwrap().unwrap(), "replacement");
        assert!(c.view().authenticated);
    }
}

#[cfg(test)]
mod loopback_tests {
    use super::*;
    #[test]
    #[ignore = "Requires the synthetic loopback SSH fixture and TERMVIAI_AUTH_FIXTURE"]
    fn loopback_authentication_recovery() {
        for backend in ["libssh", "ssh2"] {
            verify_backend(backend);
        }
    }
    fn verify_backend(backend: &str) {
        let dir = std::env::var("TERMVIAI_AUTH_FIXTURE").expect("fixture directory");
        let password_id = "session-termviai-loopback-password";
        let identity_id = "session-termviai-loopback-identity";
        struct Cleanup(&'static str, &'static str);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = wezterm_ssh::credentials::delete(self.0);
                let _ = wezterm_ssh::credentials::delete(self.1);
            }
        }
        let _cleanup = Cleanup(password_id, identity_id);
        let candidate = |label: &str, user: &str, mode: &str, key: &str, credential: &str| {
            let mut config = wezterm_ssh::Config::new().for_host("127.0.0.1");
            for (k, v) in [
                ("hostname", "127.0.0.1".to_string()),
                ("port", "22371".into()),
                ("user", user.into()),
                ("wezterm_ssh_backend", backend.into()),
                ("termviai_auth_attempt", "yes".into()),
                ("termviai_auth_mode", mode.into()),
                ("connecttimeout", "5".into()),
                ("userknownhostsfile", format!("{dir}/known_hosts")),
            ]
            .iter()
            {
                config.insert(k.to_string(), v.clone());
            }
            if !key.is_empty() {
                config.insert("termviai_identity_file".into(), format!("{dir}/{key}"));
            }
            if !credential.is_empty() {
                config.insert("termviai_credential".into(), credential.into());
            }
            Candidate {
                label: label.into(),
                config,
                error: None,
            }
        };
        let run = |plan: Vec<Candidate>, mut retry: Option<Candidate>, passphrase: bool| {
            let control = Control::new();
            let c = control.clone();
            let worker = std::thread::spawn(move || connect(&c, plan));
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            let mut revision = u64::MAX;
            while !worker.is_finished() {
                if std::time::Instant::now() > deadline {
                    control.cancel();
                    panic!("Fixture timed out: {:?}", control.view());
                }
                let view = control.view();
                if revision != view.revision {
                    revision = view.revision;
                    match view.phase {
                        Phase::Verify => control.send(Command::Trust(true)).unwrap(),
                        Phase::Waiting => {
                            assert!(!view.authenticated);
                            control
                                .send(Command::Retry(
                                    retry.take().expect("Unexpected authentication rejection"),
                                ))
                                .unwrap();
                        }
                        Phase::Challenge => {
                            assert!(passphrase);
                            control
                                .send(Command::Answers(vec!["fixture-passphrase".into()]))
                                .unwrap();
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let session = worker.join().unwrap().unwrap();
            assert!(control.view().authenticated);
            let mut exec = smol::block_on(session.exec("fixture", None)).unwrap();
            let mut output = String::new();
            std::io::Read::read_to_string(&mut exec.stdout, &mut output).unwrap();
            assert!(output.contains("fixture-ready"));
            control.view().logs
        };
        wezterm_ssh::credentials::save(identity_id, "identity-good", false).unwrap();
        wezterm_ssh::credentials::save(password_id, "manual-good", false).unwrap();
        let key = || candidate("Key", "key-user", "key", "good_key", "");
        let bad_key = || candidate("Key", "key-user", "key", "bad_key", "");
        let identity = || candidate("Identity", "identity-user", "identity", "", identity_id);
        let password = || candidate("Password", "manual-user", "password", "", password_id);
        assert_eq!(
            run(vec![key(), identity(), password()], None, false).len(),
            1
        );
        assert_eq!(
            run(vec![bad_key(), identity(), password()], None, false).len(),
            2
        );
        wezterm_ssh::credentials::save(identity_id, "invalid", false).unwrap();
        assert_eq!(
            run(vec![bad_key(), identity(), password()], None, false).len(),
            3
        );
        wezterm_ssh::credentials::save(password_id, "invalid", false).unwrap();
        let logs = run(
            vec![bad_key(), identity(), password()],
            Some(candidate(
                "Replacement key",
                "key-user",
                "key",
                "encrypted_key",
                "",
            )),
            true,
        );
        assert_eq!(logs.len(), 4);
        assert!(logs.last().unwrap().contains("accepted"));
    }
}
