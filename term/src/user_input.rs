//! Capture only explicitly scoped, synchronous user input after terminal encoding.
//! Parser replies on other threads and writes for other panes bypass the capture.
type PaneId = usize;
use std::cell::RefCell;
use std::io;

const MAX_INPUT: usize = 8 * 1024 * 1024;
struct Capture {
    pane: PaneId,
    bytes: Vec<u8>,
}
thread_local! {
    static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        CAPTURE.with(|slot| *slot.borrow_mut() = None);
    }
}

/// This scope must never cross an await or dispatch unrelated events. The guard
/// clears capture on errors and unwinding, before delivery to raw transports.
pub fn capture(
    pane: PaneId,
    encode: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    CAPTURE.with(|slot| {
        anyhow::ensure!(slot.borrow().is_none(), "nested user input capture");
        *slot.borrow_mut() = Some(Capture {
            pane,
            bytes: Vec::new(),
        });
        Ok::<_, anyhow::Error>(())
    })?;
    let _reset = Reset;
    encode()?;
    Ok(CAPTURE.with(|slot| slot.borrow_mut().take().unwrap().bytes))
}

pub fn write(pane: PaneId, bytes: &[u8]) -> Option<io::Result<usize>> {
    CAPTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let capture = slot.as_mut().filter(|capture| capture.pane == pane)?;
        if bytes.len() > MAX_INPUT.saturating_sub(capture.bytes.len()) {
            return Some(Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Broadcast input exceeds 8 MiB; paste smaller chunks",
            )));
        }
        capture.bytes.extend_from_slice(bytes);
        Some(Ok(bytes.len()))
    })
}
pub fn active(pane: PaneId) -> bool {
    CAPTURE.with(|slot| slot.borrow().as_ref().map(|capture| capture.pane) == Some(pane))
}
