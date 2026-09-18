//! Shared scope for direct Pane writes and terminal user-input encoding.
pub use wezterm_term::user_input::{active, capture, write};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::WriterWrapper;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            // Exercise short writes, including the terminal's background queue.
            let len = bytes.len().min(2);
            self.0.lock().unwrap().extend_from_slice(&bytes[..len]);
            Ok(len)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn source_only_capture_excludes_protocol_and_other_panes() {
        let source = Sink::default();
        let other = Sink::default();
        let mut writer = WriterWrapper::new(7, Box::new(source.clone()));
        let mut second = WriterWrapper::new(8, Box::new(other.clone()));
        let mut parser = writer.clone();
        let bytes = capture(7, || {
            writer.write_all(b"\x1b[200~hello\x1b[201~")?;
            writer.flush()?;
            second.write_all(b"other")?;
            std::thread::spawn(move || parser.write_all(b"reply"))
                .join()
                .unwrap()?;
            Ok(())
        })
        .unwrap();
        assert_eq!(bytes, b"\x1b[200~hello\x1b[201~");
        assert_eq!(*source.0.lock().unwrap(), b"reply");
        assert_eq!(*other.0.lock().unwrap(), b"other");
        writer.write_all(&bytes).unwrap();
        assert_eq!(*source.0.lock().unwrap(), b"reply\x1b[200~hello\x1b[201~");
    }
    #[test]
    fn actual_terminal_encodes_keys_paste_and_protocol_replies() {
        use wezterm_term::{KeyCode, KeyModifiers, Terminal, TerminalSize};
        let sink = Sink::default();
        let writer = WriterWrapper::new(7, Box::new(sink.clone()));
        let mut terminal = Terminal::new(
            TerminalSize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 96,
            },
            Arc::new(config::TermConfig::new()),
            "TermViAI",
            "test",
            Box::new(writer),
        );
        terminal.set_input_capture_id(7);
        let keys = capture(7, || {
            terminal.key_down(KeyCode::Char('c'), KeyModifiers::CTRL)?;
            terminal.key_down(KeyCode::UpArrow, KeyModifiers::NONE)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(keys, b"\x03\x1b[A");
        terminal.advance_bytes(b"\x1b[?2004h");
        let paste = capture(7, || terminal.send_paste("hello 服务器")).unwrap();
        assert_eq!(paste, "\x1b[200~hello 服务器\x1b[201~".as_bytes());
        assert!(sink.0.lock().unwrap().is_empty());
        terminal.advance_bytes(b"\x1b[6n");
        terminal.send_raw_input(b"raw-through-queue").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !sink.0.lock().unwrap().ends_with(b"raw-through-queue")
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            *sink.0.lock().unwrap(),
            b"\x1b[1;1Rraw-through-queue",
            "protocol reply and raw input must reach source once, with short writes handled"
        );
    }

    fn test_terminal(writer: WriterWrapper) -> wezterm_term::Terminal {
        let mut terminal = wezterm_term::Terminal::new(
            wezterm_term::TerminalSize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 96,
            },
            Arc::new(config::TermConfig::new()),
            "TermViAI",
            "test",
            Box::new(writer),
        );
        terminal.set_input_capture_id(7);
        terminal
    }

    fn wait_for_bytes(sink: &Sink, expected: &[u8]) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while sink.0.lock().unwrap().len() < expected.len() && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(&*sink.0.lock().unwrap(), expected);
    }

    #[test]
    fn reconnect_writer_recovers_broadcast_source_after_write_or_flush_failure() {
        struct BrokenWriter {
            fail_on_flush: bool,
            ended: std::sync::mpsc::Sender<()>,
        }
        impl Write for BrokenWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.fail_on_flush {
                    Ok(bytes.len())
                } else {
                    Err(io::ErrorKind::BrokenPipe.into())
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        impl Drop for BrokenWriter {
            fn drop(&mut self) {
                let _ = self.ended.send(());
            }
        }
        for fail_on_flush in [false, true] {
            let (ended, done) = std::sync::mpsc::channel();
            let mut terminal = test_terminal(WriterWrapper::new(
                7,
                Box::new(BrokenWriter {
                    fail_on_flush,
                    ended,
                }),
            ));
            terminal.advance_bytes(b"preserved history");
            let _ = terminal.send_raw_input(b"failed input");
            done.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
            assert!(terminal.send_raw_input(b"buffered after failure").is_err());

            let sink = Sink::default();
            let mut direct = WriterWrapper::new(7, Box::new(sink.clone()));
            terminal.replace_writer(Box::new(direct.clone()));
            // Direct Pane writes and the terminal encoder share this generation.
            direct.write_all(b"direct:").unwrap();
            let bytes = capture(7, || {
                terminal.key_down(
                    wezterm_term::KeyCode::Char('x'),
                    wezterm_term::KeyModifiers::NONE,
                )?;
                terminal.send_paste("服务器")
            })
            .unwrap();
            assert_eq!(bytes, "x服务器".as_bytes());
            assert_eq!(&*sink.0.lock().unwrap(), b"direct:");
            // A broadcast dispatch includes the source through send_raw_input.
            terminal.send_raw_input(&bytes).unwrap();
            terminal.advance_bytes(b"\x1b[6n");
            terminal.send_raw_input(b":independent").unwrap();
            wait_for_bytes(&sink, "direct:x服务器\x1b[1;18R:independent".as_bytes());
            assert!(terminal.screen().lines_in_phys_range(0..1)[0]
                .as_str()
                .starts_with("preserved history"));
        }
    }

    #[test]
    fn reconnect_writer_discards_queued_input_without_waiting_for_old_transport() {
        struct BlockedWriter {
            started: std::sync::mpsc::Sender<()>,
            release: std::sync::mpsc::Receiver<()>,
            ended: std::sync::mpsc::Sender<()>,
            sink: Sink,
        }
        impl Write for BlockedWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.started.send(()).unwrap();
                self.release
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                self.sink.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        impl Drop for BlockedWriter {
            fn drop(&mut self) {
                let _ = self.ended.send(());
            }
        }
        let (started, entered) = std::sync::mpsc::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let (ended, done) = std::sync::mpsc::channel();
        let old = Sink::default();
        let mut terminal = test_terminal(WriterWrapper::new(
            7,
            Box::new(BlockedWriter {
                started,
                release: resume,
                ended,
                sink: old.clone(),
            }),
        ));
        terminal.send_raw_input(b"in flight").unwrap();
        entered
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        terminal.send_raw_input(b"queued old input").unwrap();
        let fresh = Sink::default();
        terminal.replace_writer(Box::new(WriterWrapper::new(7, Box::new(fresh.clone()))));
        terminal.send_raw_input(b"new input").unwrap();
        wait_for_bytes(&fresh, b"new input");
        release.send(()).unwrap();
        done.recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(&*old.0.lock().unwrap(), b"in flight");
        assert_eq!(&*fresh.0.lock().unwrap(), b"new input");
    }

    #[test]
    fn capture_cleans_up_errors_panics_and_rejects_nested_scopes() {
        assert!(capture(7, || anyhow::bail!("encode failed")).is_err());
        assert!(!active(7));
        let _ = std::panic::catch_unwind(|| capture(7, || panic!("encoder panic")));
        assert!(!active(7));
        capture(7, || {
            assert!(capture(8, || Ok(())).is_err());
            assert!(active(7));
            assert!(write(7, &vec![0; 8 * 1024 * 1024 + 1]).unwrap().is_err());
            Ok(())
        })
        .unwrap();
        assert!(!active(7));
    }
}
