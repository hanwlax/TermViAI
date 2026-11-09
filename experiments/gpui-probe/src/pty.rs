//! Disposable local terminals. No TermViAI hosts, credentials or SSH settings are read.
use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const HISTORY: usize = 2000;
// ConPTY's INHERIT_CURSOR handshake must be answered, otherwise it waits before
// starting the console application. CSI parsing also handles fragmented reads.
pub struct Responses(SyncSender<Vec<u8>>);
impl vt100::Callbacks for Responses {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let value = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        let reply = match (i1, c, value) {
            (None, 'n', 6) => {
                let (r, c) = screen.cursor_position();
                Some(format!("\x1b[{};{}R", r + 1, c + 1))
            }
            (None, 'n', 5) => Some("\x1b[0n".into()),
            (None, 'c', 0) => Some("\x1b[?1;2c".into()),
            _ => None,
        };
        if let Some(reply) = reply {
            let _ = self.0.try_send(reply.into_bytes());
        }
    }
}

pub struct Output {
    pub parser: vt100::Parser<Responses>,
    pub bytes: u64,
    pub error: Option<String>,
    pub ended: bool,
}
pub struct Terminal {
    pub output: Arc<Mutex<Output>>,
    pub revision: Arc<AtomicU64>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    input: Option<SyncSender<Vec<u8>>>,
    pub dimensions: (u16, u16),
}
impl Terminal {
    pub fn new(mode: &str) -> Result<Self> {
        let pair = native_pty_system().openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let cmd = if mode == "shell" {
            #[cfg(windows)]
            let mut c = CommandBuilder::new("cmd.exe");
            #[cfg(windows)]
            c.arg("/Q");
            #[cfg(not(windows))]
            let c = CommandBuilder::new("/bin/sh");
            c
        } else {
            let mut fixture = std::env::current_exe()?;
            fixture.set_file_name(if cfg!(windows) {
                "termviai-gpui-fixture.exe"
            } else {
                "termviai-gpui-fixture"
            });
            let mut c = CommandBuilder::new(fixture);
            c.arg(if mode == "stream" {
                "--stream"
            } else {
                "--terminal-fixture"
            });
            c
        };
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Start local terminal")?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let (tx, rx) = sync_channel::<Vec<u8>>(32);
        let output = Arc::new(Mutex::new(Output {
            parser: vt100::Parser::new_with_callbacks(24, 80, HISTORY, Responses(tx.clone())),
            bytes: 0,
            error: None,
            ended: false,
        }));
        let revision = Arc::new(AtomicU64::new(0));
        let out = output.clone();
        let rev = revision.clone();
        thread::spawn(move || {
            let mut buf = [0; 16384];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut o = out.lock().unwrap();
                        o.parser.process(&buf[..n]);
                        o.bytes += n as u64;
                        rev.fetch_add(1, Ordering::Release);
                    }
                    Err(e) => {
                        out.lock().unwrap().error = Some(e.to_string());
                        break;
                    }
                }
            }
            out.lock().unwrap().ended = true;
            rev.fetch_add(1, Ordering::Release);
        });
        // A strong reference here would create a cycle through Responses' sender.
        let out = Arc::downgrade(&output);
        let rev = revision.clone();
        thread::spawn(move || {
            while let Ok(bytes) = rx.recv() {
                if let Err(e) = writer.write_all(&bytes).and_then(|_| writer.flush()) {
                    if let Some(out) = out.upgrade() {
                        out.lock().unwrap().error = Some(e.to_string());
                    }
                    rev.fetch_add(1, Ordering::Release);
                    break;
                }
            }
        });
        Ok(Self {
            output,
            revision,
            master: pair.master,
            child,
            input: Some(tx),
            dimensions: (24, 80),
        })
    }
    pub fn poll_exit(&mut self) -> bool {
        if self.output.lock().unwrap().ended {
            return false;
        }
        if let Ok(Some(_)) = self.child.try_wait() {
            self.output.lock().unwrap().ended = true;
            return true;
        }
        false
    }
    pub fn send(&self, bytes: &[u8]) -> Result<()> {
        // Bounded input queue: surface congestion rather than silently dropping input.
        self.input
            .as_ref()
            .context("Terminal closed")?
            .try_send(bytes.to_vec())
            .context("Terminal input is busy")?;
        self.output
            .lock()
            .unwrap()
            .parser
            .screen_mut()
            .set_scrollback(0);
        Ok(())
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.clamp(2, 300);
        let cols = cols.clamp(2, 500);
        if self.dimensions != (rows, cols) {
            self.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            self.output
                .lock()
                .unwrap()
                .parser
                .screen_mut()
                .set_size(rows, cols);
            self.dimensions = (rows, cols);
        }
        Ok(())
    }
    pub fn scroll(&self, delta: i32) {
        let mut out = self.output.lock().unwrap();
        let n = out.parser.screen().scrollback() as i32;
        out.parser
            .screen_mut()
            .set_scrollback((n + delta).max(0) as usize);
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn fixture() {
    println!("\x1b[32mGPUI-PTY-READY\x1b[0m 中文 Unicode >= <= ]#");
    println!("(c) Microsoft Corporation.保留所有权利。");
    println!("Grid: |中文|ABCD|日本|한국| END");
    println!("Mixed: A中B文C / e\u{301} / \x1b[31m红色中文\x1b[0m tail");
    let mut line = String::new();
    while std::io::stdin().read_line(&mut line).unwrap_or(0) > 0 {
        println!("ACK:{}", line.trim_end());
        if line.trim() == "exit" {
            break;
        }
        line.clear();
    }
}
pub fn stream() {
    let mut out = std::io::stdout().lock();
    for batch in 0..3000 {
        for line in 0..10 {
            let _ = writeln!(
                out,
                "\x1b[38;5;111m{:06}\x1b[0m  GPU output probe  \x1b[32mOK\x1b[0m  UTF-8 中文 /workspace/termviai",
                batch * 10 + line
            );
        }
        if out.flush().is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = writeln!(out, "STREAM-COMPLETE: 30000 lines");
}
pub fn check() -> Result<()> {
    let mut terminal = Terminal::new("fixture")?;
    let wait = |needle: &str| -> Result<()> {
        let start = Instant::now();
        loop {
            if terminal
                .output
                .lock()
                .unwrap()
                .parser
                .screen()
                .contents()
                .contains(needle)
            {
                return Ok(());
            }
            anyhow::ensure!(
                start.elapsed() < Duration::from_secs(10),
                "PTY did not produce {needle}: {}",
                terminal.output.lock().unwrap().parser.screen().contents()
            );
            thread::sleep(Duration::from_millis(20));
        }
    };
    wait("GPUI-PTY-READY")?;
    terminal.send(b"ascii-check\r")?;
    wait("ACK:ascii-check")?;
    terminal.send("中文-check\r".as_bytes())?;
    wait("ACK:中文-check")?;
    terminal.resize(12, 40)?;
    terminal.resize(32, 120)?;
    terminal.send(b"exit\r")?;
    println!(
        "{}",
        serde_json::json!({"pty":"passed", "unicode":"passed", "resize":"passed", "history_limit":HISTORY})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn shrinking_through_wide_character_can_accept_more_output() {
        let mut p = vt100::Parser::new(2, 5, 10);
        p.process("abc中".as_bytes());
        p.screen_mut().set_size(2, 4);
        p.process(b"\x1b[1;4Hx");
        assert_eq!(p.screen().cell(0, 3).unwrap().contents(), "x");
        assert!(!p.screen().cell(0, 3).unwrap().is_wide());
    }

    #[test]
    fn fragmented_console_query_responds_with_current_cursor() {
        let (tx, rx) = std::sync::mpsc::sync_channel(2);
        let mut parser = vt100::Parser::new_with_callbacks(24, 80, 0, super::Responses(tx));
        parser.process(b"abc\x1b[");
        parser.process(b"6n");
        assert_eq!(rx.try_recv().unwrap(), b"\x1b[1;4R");
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn parser_preserves_ansi_unicode_and_caps_history() {
        let mut p = vt100::Parser::new(4, 40, super::HISTORY);
        p.process("\x1b[32mhello 中文\x1b[0m\r\n".as_bytes());
        assert!(p.screen().contents().contains("hello 中文"));
        assert_eq!(
            p.screen().cell(0, 0).unwrap().fgcolor(),
            vt100::Color::Idx(2)
        );
        for _ in 0..10000 {
            p.process(b"line\r\n");
        }
        p.screen_mut().set_scrollback(usize::MAX);
        assert!(p.screen().scrollback() <= super::HISTORY);
    }
}
