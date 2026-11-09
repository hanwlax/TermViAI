//! Dedicated reverse TCP transport. No PTY, shell command, REST or config discovery.
use crate::channelwrap::ChannelWrap;
use crate::sessionwrap::SessionWrap;
use anyhow::{ensure, Context};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RemoteForward {
    pub bind: String,
    pub remote_port: u16,
    pub local_port: u16,
    pub cancel: Arc<AtomicBool>,
    /// Sent only after the server acknowledges tcpip-forward.
    pub ready: mpsc::Sender<()>,
    /// Optional session registration, completed durably before requesting a listener.
    pub setup: Option<RemoteForwardSetup>,
}
pub struct RemoteForwardSetup {
    pub command: String,
    /// Return false to abandon this transport without opening a listener.
    pub commit: Box<dyn FnOnce(anyhow::Result<String>) -> anyhow::Result<bool> + Send + Sync>,
}
#[derive(Debug, thiserror::Error)]
#[error("Remote SSH session registration is unavailable")]
pub struct RemoteForwardSetupUnavailable;
impl std::fmt::Debug for RemoteForwardSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteForwardSetup").finish_non_exhaustive()
    }
}
fn register(sess: &mut SessionWrap, command: &str, cancel: &AtomicBool) -> anyhow::Result<String> {
    let mut channel = sess.open_session()?;
    channel.request_exec(command)?;
    sess.set_blocking(false);
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut out = vec![];
        let mut err = vec![];
        loop {
            ensure!(!cancel.load(Ordering::Relaxed), "Cancelled");
            ensure!(Instant::now() < deadline, "Session registration timed out");
            for (idx, data) in [(0, &mut out), (1, &mut err)] {
                pump_read(&mut *channel.reader(idx), data)?;
                ensure!(data.len() < LIMIT, "Registration output exceeds limit");
            }
            if let Some(line) = out
                .split_inclusive(|b| *b == b'\n')
                .filter(|line| line.ends_with(b"\n"))
                .find_map(|line| line.strip_prefix(b"TERMVIAI_FORWARD_V1="))
            {
                return Ok(String::from_utf8(line.to_vec())?);
            }
            if channel.exit_status().is_some() {
                anyhow::bail!(
                    "Session registration unavailable: {}",
                    String::from_utf8_lossy(&err)
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    channel.close();
    sess.set_blocking(true);
    result
}
const LIMIT: usize = 64 * 1024;
struct Pipe {
    channel: ChannelWrap,
    socket: TcpStream,
    up: Vec<u8>,
    down: Vec<u8>,
    local_eof: bool,
    remote_eof: bool,
    sent_eof: bool,
    shut_write: bool,
}
fn pump_read(r: &mut dyn Read, buf: &mut Vec<u8>) -> io::Result<bool> {
    if buf.len() >= LIMIT {
        return Ok(false);
    }
    let mut bytes = [0; 16384];
    let max = bytes.len().min(LIMIT - buf.len());
    match r.read(&mut bytes[..max]) {
        Ok(0) => Ok(true),
        Ok(n) => {
            buf.extend_from_slice(&bytes[..n]);
            Ok(false)
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}
fn pump_write(w: &mut dyn Write, buf: &mut Vec<u8>) -> io::Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    match w.write(buf) {
        Ok(0) => Err(io::ErrorKind::WriteZero.into()),
        Ok(n) => {
            buf.drain(..n);
            Ok(())
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(e),
    }
}
impl Pipe {
    fn tick(&mut self) -> anyhow::Result<bool> {
        if !self.local_eof {
            self.local_eof = pump_read(&mut self.socket, &mut self.up)?;
        }
        if !self.remote_eof {
            match &mut self.channel {
                #[cfg(feature = "libssh-rs")]
                ChannelWrap::LibSsh(c) if self.down.len() < LIMIT => {
                    // stdout().read() maps zero to WouldBlock in a nonblocking
                    // libssh session, including EOF. Use the explicit API and
                    // check EOF only after draining all buffered channel data.
                    let mut bytes = [0; 16384];
                    let count = bytes.len().min(LIMIT - self.down.len());
                    match c.read_nonblocking(&mut bytes[..count], false) {
                        Ok(0) => self.remote_eof = c.is_eof(),
                        Ok(n) => self.down.extend_from_slice(&bytes[..n]),
                        Err(libssh_rs::Error::TryAgain) => (),
                        Err(e) => return Err(e.into()),
                    }
                }
                #[cfg(feature = "ssh2")]
                ChannelWrap::Ssh2(c) => {
                    self.remote_eof = pump_read(&mut c.stream(0), &mut self.down)? && c.eof();
                }
                #[cfg(feature = "libssh-rs")]
                ChannelWrap::LibSsh(_) => (),
            }
        }
        pump_write(&mut *self.channel.writer(), &mut self.up)?;
        pump_write(&mut self.socket, &mut self.down)?;
        if self.local_eof && self.up.is_empty() && !self.sent_eof {
            self.sent_eof = match &mut self.channel {
                #[cfg(feature = "libssh-rs")]
                ChannelWrap::LibSsh(c) => match c.send_eof() {
                    Ok(()) => true,
                    Err(libssh_rs::Error::TryAgain) => false,
                    Err(e) => return Err(e.into()),
                },
                #[cfg(feature = "ssh2")]
                ChannelWrap::Ssh2(c) => match c.send_eof() {
                    Ok(()) => true,
                    Err(e) if e.code() == ssh2::ErrorCode::Session(-37) => false,
                    Err(e) => return Err(e.into()),
                },
            };
        }
        if self.remote_eof && self.down.is_empty() && !self.shut_write {
            self.socket.shutdown(Shutdown::Write)?;
            self.shut_write = true;
        }
        Ok(!(self.sent_eof && self.shut_write))
    }
}
impl Drop for Pipe {
    fn drop(&mut self) {
        self.channel.close();
    }
}

pub(crate) fn run(sess: &mut SessionWrap, mut request: RemoteForward) -> anyhow::Result<()> {
    ensure!(
        request.remote_port > 0 && request.local_port > 0,
        "Ports must be 1–65535"
    );
    ensure!(
        matches!(request.bind.as_str(), "127.0.0.1" | "0.0.0.0"),
        "Invalid remote bind address"
    );
    if request.cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    if let Some(setup) = request.setup.take() {
        let response = register(sess, &setup.command, &request.cancel);
        ensure!(!request.cancel.load(Ordering::Relaxed), "Cancelled");
        if !(setup.commit)(response)? {
            return Err(RemoteForwardSetupUnavailable.into());
        }
    }
    #[cfg(feature = "ssh2")]
    let mut listener = None;
    match sess {
        #[cfg(feature = "libssh-rs")]
        SessionWrap::LibSsh(s) => {
            s.sess
                .listen_forward(Some(&request.bind), request.remote_port)
                .context("Remote listener refused (port occupied or forwarding prohibited)")?;
        }
        #[cfg(feature = "ssh2")]
        SessionWrap::Ssh2(s) => {
            listener = Some(
                s.sess
                    .channel_forward_listen(request.remote_port, Some(&request.bind), Some(64))
                    .context("Remote listener refused (port occupied or forwarding prohibited)")?
                    .0,
            );
            s.sess.set_keepalive(true, 15);
        }
    }
    sess.set_blocking(false);
    if request.cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    request.ready.send(()).ok();
    let mut pipes: Vec<Pipe> = vec![];
    let mut pending: Vec<(ChannelWrap, mpsc::Receiver<io::Result<TcpStream>>)> = vec![];
    let mut last_alive = Instant::now();
    while !request.cancel.load(Ordering::Relaxed) {
        let accepted = match sess {
            #[cfg(feature = "libssh-rs")]
            SessionWrap::LibSsh(s) => {
                ensure!(s.sess.is_connected(), "SSH transport disconnected");
                match s.sess.accept_forward(Duration::ZERO) {
                    Ok((_, c)) => Some(ChannelWrap::LibSsh(c)),
                    Err(libssh_rs::Error::TryAgain) => None,
                    Err(e) => return Err(e.into()),
                }
            }
            #[cfg(feature = "ssh2")]
            SessionWrap::Ssh2(_) => match listener.as_mut().unwrap().accept() {
                Ok(c) => Some(ChannelWrap::Ssh2(c)),
                Err(e) if e.code() == ssh2::ErrorCode::Session(-37) => None,
                Err(e) => return Err(e.into()),
            },
        };
        if let Some(mut channel) = accepted {
            if pipes.len() + pending.len() >= 64 {
                channel.close();
            } else {
                let (tx, rx) = mpsc::channel();
                let port = request.local_port;
                std::thread::spawn(move || {
                    let result = TcpStream::connect_timeout(
                        &SocketAddr::from(([127, 0, 0, 1], port)),
                        Duration::from_secs(2),
                    )
                    .and_then(|s| {
                        s.set_nonblocking(true)?;
                        s.set_nodelay(true)?;
                        Ok(s)
                    });
                    tx.send(result).ok();
                });
                pending.push((channel, rx));
            }
        }
        let mut i = 0;
        while i < pending.len() {
            match pending[i].1.try_recv() {
                Err(mpsc::TryRecvError::Empty) => i += 1,
                result => {
                    let (mut channel, _) = pending.swap_remove(i);
                    if let Ok(Ok(socket)) = result {
                        pipes.push(Pipe {
                            channel,
                            socket,
                            up: vec![],
                            down: vec![],
                            local_eof: false,
                            remote_eof: false,
                            sent_eof: false,
                            shut_write: false,
                        });
                    } else {
                        channel.close();
                    }
                }
            }
        }
        pipes.retain_mut(|p| match p.tick() {
            Ok(open) => open,
            Err(error) => {
                log::debug!("Forwarded TCP stream closed: {error:#}");
                false
            }
        });
        if last_alive.elapsed() >= Duration::from_secs(15) {
            match sess {
                #[cfg(feature = "libssh-rs")]
                SessionWrap::LibSsh(s) => s.sess.send_ignore(b"termviai-forward")?,
                #[cfg(feature = "ssh2")]
                SessionWrap::Ssh2(s) => {
                    s.sess.keepalive_send()?;
                }
            }
            last_alive = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(if pipes.is_empty() { 20 } else { 2 }));
    }
    for (mut c, _) in pending {
        c.close();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_backpressure_and_partial_writes() {
        let mut input = io::Cursor::new(vec![42; LIMIT * 2]);
        let mut buf = vec![];
        for _ in 0..10 {
            pump_read(&mut input, &mut buf).unwrap();
        }
        assert_eq!(buf.len(), LIMIT);
        assert_eq!(input.position(), LIMIT as u64);
        let mut small = [0; 7];
        pump_write(&mut &mut small[..], &mut buf).unwrap();
        assert_eq!(buf.len(), LIMIT - 7);
        assert_eq!(small, [42; 7]);
    }
    #[test]
    fn socket_half_close_keeps_reply_readable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut a = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut b, _) = listener.accept().unwrap();
        a.shutdown(Shutdown::Write).unwrap();
        assert!(pump_read(&mut b, &mut vec![]).unwrap());
        b.write_all(b"reply").unwrap();
        b.shutdown(Shutdown::Write).unwrap();
        let mut data = vec![];
        a.read_to_end(&mut data).unwrap();
        assert_eq!(data, b"reply");
    }
}
