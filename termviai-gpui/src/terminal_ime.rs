//! Keep terminal preedit in sync with Windows IME lifecycle notifications.
//! GPUI 0.2.2 handles COMPOSITION, but does not forward ENDCOMPOSITION to
//! EntityInputHandler::unmark_text. The native observer only updates an epoch;
//! it never reenters GPUI or sends text to a terminal from a window callback.
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Default)]
pub struct ImeEpoch(Arc<AtomicU64>);
impl ImeEpoch {
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
    fn ended(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
    pub fn install(window: &gpui::Window) -> Self {
        let epoch = Self::default();
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = HasWindowHandle::window_handle(window) {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    if !native::attach(
                        windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut _),
                        epoch.clone(),
                    ) {
                        log::error!("Could not observe Windows IME lifecycle");
                    }
                }
            }
        }
        #[cfg(not(windows))]
        let _ = window;
        epoch
    }
}

#[derive(Default)]
pub struct Preedit {
    pub text: String,
    pub selection: Range<usize>,
    epoch: u64,
}
impl Preedit {
    pub fn mark(&mut self, text: &str, selection: Option<Range<usize>>, epoch: &ImeEpoch) {
        self.text = text.into();
        let n = text.encode_utf16().count();
        self.selection = selection
            .map(|r| r.start.min(n)..r.end.min(n))
            .unwrap_or(n..n);
        self.epoch = epoch.current();
    }
    pub fn clear(&mut self) {
        self.text.clear();
        self.selection = 0..0;
    }
    pub fn reconcile(&mut self, epoch: &ImeEpoch) -> bool {
        if !self.text.is_empty() && self.epoch != epoch.current() {
            self.clear();
            true
        } else {
            false
        }
    }
    /// Only the explicit IME commit callback supplies text to the remote shell.
    pub fn commit(&mut self, text: &str) -> Option<String> {
        self.clear();
        (!text.is_empty()).then(|| text.to_owned())
    }
}

#[cfg(windows)]
mod native {
    use super::ImeEpoch;
    use windows::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{WM_IME_ENDCOMPOSITION, WM_INPUTLANGCHANGE, WM_NCDESTROY},
        },
    };
    const ID: usize = 0x5456494d;
    pub(super) fn attach(hwnd: HWND, epoch: ImeEpoch) -> bool {
        let data = Box::into_raw(Box::new(epoch));
        // Installed once per application window, on its owning UI thread.
        if unsafe { SetWindowSubclass(hwnd, Some(observe), ID, data as usize) }.as_bool() {
            true
        } else {
            unsafe {
                drop(Box::from_raw(data));
            }
            false
        }
    }
    unsafe extern "system" fn observe(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
        id: usize,
        data: usize,
    ) -> LRESULT {
        if matches!(msg, WM_IME_ENDCOMPOSITION | WM_INPUTLANGCHANGE) {
            unsafe { &*(data as *const ImeEpoch) }.ended();
        }
        if msg == WM_NCDESTROY {
            // Stop callbacks before freeing the window-owned Arc. Views may
            // retain their own Arc until GPUI drops them after window teardown.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(observe), id);
                drop(Box::from_raw(data as *mut ImeEpoch));
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ending_without_commit_clears_phantom_text_and_unblocks_keys() {
        let epoch = ImeEpoch::default();
        let mut p = Preedit::default();
        p.mark("l's", None, &epoch);
        assert!(!p.reconcile(&epoch));
        assert!(!p.text.is_empty());
        epoch.ended();
        assert!(p.reconcile(&epoch));
        assert!(p.text.is_empty());
        assert_eq!(p.selection, 0..0);
        assert!(!p.reconcile(&epoch));
    }
    #[test]
    fn rapid_new_composition_survives_previous_end_notification() {
        let epoch = ImeEpoch::default();
        let mut p = Preedit::default();
        p.mark("old", None, &epoch);
        epoch.ended();
        p.mark("新😀", Some(1..3), &epoch);
        assert!(!p.reconcile(&epoch));
        assert_eq!(p.text, "新😀");
        assert_eq!(p.selection, 1..3);
        assert_eq!(p.commit("新😀"), Some("新😀".into()));
        epoch.ended();
        assert!(!p.reconcile(&epoch));
        assert!(p.text.is_empty());
    }
    #[test]
    fn cancellation_and_focus_loss_never_commit_preedit() {
        let epoch = ImeEpoch::default();
        let mut p = Preedit::default();
        p.mark("ls", None, &epoch);
        assert_eq!(p.commit(""), None);
        assert!(p.text.is_empty());
        p.mark("pending", None, &epoch);
        p.clear();
        assert!(p.text.is_empty());
        assert_eq!(p.selection, 0..0);
    }
    #[cfg(windows)]
    #[test]
    fn hidden_window_observes_end_language_change_and_releases_hook() {
        use windows::{
            Win32::{
                Foundation::{LPARAM, WPARAM},
                UI::WindowsAndMessaging::*,
            },
            core::w,
        };
        // No ShowWindow, foreground activation, user input or real IME change.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("TermViAI IME test"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        let epoch = ImeEpoch::default();
        assert!(native::attach(hwnd, epoch.clone()));
        assert_eq!(Arc::strong_count(&epoch.0), 2);
        unsafe {
            SendMessageW(
                hwnd,
                WM_IME_ENDCOMPOSITION,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }
        assert_eq!(epoch.current(), 1);
        unsafe {
            SendMessageW(hwnd, WM_INPUTLANGCHANGE, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        assert_eq!(epoch.current(), 2);
        unsafe {
            DestroyWindow(hwnd).unwrap();
        }
        assert_eq!(Arc::strong_count(&epoch.0), 1);
    }
}
