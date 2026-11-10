//! Conservative util-linux wall framing. This is a presentation heuristic, not
//! sender authentication: programs can print the same bytes. Unknown formats,
//! incomplete frames and control sequences are passed through byte-for-byte.
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const WIDTH: usize = 79;
const LIMIT: usize = 64 * 1024;
const WAIT: Duration = Duration::from_millis(250);
const MAX_NOTICES: usize = 100;
const MAX_STORED_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Notice {
    pub id: u64,
    pub sender: String,
    pub tty: String,
    pub date: String,
    pub body: String,
    /// Original printable header/body. Framing CR/BEL/padding are not UI text.
    pub original: String,
    /// Exact bytes retained while the notification is in the session inbox.
    pub wire: Vec<u8>,
}
#[derive(Default)]
pub struct Inbox {
    pub items: VecDeque<Notice>,
    pub revision: u64,
    next_id: u64,
    read_through: u64,
    bytes: usize,
}
impl Inbox {
    pub fn push(&mut self, mut notice: Notice) {
        self.next_id += 1;
        notice.id = self.next_id;
        self.bytes += notice.wire.len();
        self.items.push_front(notice);
        while self.items.len() > MAX_NOTICES || self.bytes > MAX_STORED_BYTES {
            if let Some(old) = self.items.pop_back() {
                self.bytes -= old.wire.len();
            }
        }
        self.revision += 1;
    }
    pub fn unread(&self) -> usize {
        self.items
            .iter()
            .filter(|n| n.id > self.read_through)
            .count()
    }
    pub fn mark_read(&mut self) {
        self.read_through = self.next_id;
        self.revision += 1;
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.bytes = 0;
        self.read_through = self.next_id;
        self.revision += 1;
    }
}

#[derive(Default)]
pub struct Output {
    pub terminal: Vec<u8>,
    pub notices: Vec<Notice>,
}
#[derive(Default)]
pub struct Filter {
    pending: Vec<u8>,
    since: Option<Instant>,
    context: StreamContext,
}
// Only inspect ordinary terminal text, never payload inside OSC/DCS/APC strings.
#[derive(Default)]
enum StreamContext {
    #[default]
    Ground,
    Escape,
    Csi,
    String {
        osc: bool,
        escape: bool,
    },
}
impl StreamContext {
    fn advance(&mut self, bytes: &[u8]) {
        for &b in bytes {
            *self = match (&*self, b) {
                (Self::String { escape, .. }, b'\\') if *escape => Self::Ground,
                (Self::String { osc: true, .. }, 7) => Self::Ground,
                (Self::String { osc, .. }, _) => Self::String {
                    osc: *osc,
                    escape: b == 27,
                },
                (_, 27) => Self::Escape,
                (Self::Escape, b']') => Self::String {
                    osc: true,
                    escape: false,
                },
                (Self::Escape, b'P' | b'_' | b'^' | b'X') => Self::String {
                    osc: false,
                    escape: false,
                },
                (Self::Escape, b'[') => Self::Csi,
                (Self::Csi, 0x40..=0x7e) => Self::Ground,
                (Self::Csi, _) => Self::Csi,
                _ => Self::Ground,
            }
        }
    }
}
impl Filter {
    pub fn feed(&mut self, data: &[u8], now: Instant) -> Output {
        let mut out = Output::default();
        if self.since.is_some_and(|t| now.duration_since(t) >= WAIT) {
            // A blank padded row also occurs inside wall messages. Wait a bounded
            // collection window, then accept only an entire sequence of frames.
            // Mixed/interleaved output or ambiguous continuation is all passed through.
            if let Some(notices) = complete_sequence(&self.pending) {
                out.notices = notices;
                self.pending.clear();
            } else {
                self.context.advance(&self.pending);
                out.terminal.append(&mut self.pending);
            }
            self.since = None;
        }
        self.pending.extend_from_slice(data);
        let prefixes = prefixes();
        loop {
            let Some(start) = prefixes
                .iter()
                .filter_map(|prefix| self.pending.windows(prefix.len()).position(|s| s == prefix))
                .min()
            else {
                // Retain only a possible fragmented prefix. Ordinary output has no delay.
                let keep = prefixes
                    .iter()
                    .map(|prefix| {
                        (1..prefix.len())
                            .rev()
                            .find(|&n| self.pending.ends_with(&prefix[..n]))
                            .unwrap_or(0)
                    })
                    .max()
                    .unwrap_or(0);
                let released: Vec<_> = self.pending.drain(..self.pending.len() - keep).collect();
                self.context.advance(&released);
                out.terminal.extend(released);
                break;
            };
            let released: Vec<_> = self.pending.drain(..start).collect();
            self.context.advance(&released);
            out.terminal.extend(released);
            if !matches!(self.context, StreamContext::Ground) {
                let b = self.pending.remove(0);
                self.context.advance(&[b]);
                out.terminal.push(b);
                continue;
            }
            if self.pending.len() > LIMIT {
                self.context.advance(&self.pending);
                out.terminal.append(&mut self.pending);
                self.since = None;
                break;
            }
            match parse(&self.pending) {
                Parsed::Complete(_, _) => break,
                Parsed::Invalid => {
                    // Release the candidate's first byte and resume scanning, so a
                    // later, independent valid frame can still be recognized.
                    let b = self.pending.remove(0);
                    self.context.advance(&[b]);
                    out.terminal.push(b);
                    self.since = None;
                }
                Parsed::More => break,
            }
        }
        if self.pending.is_empty() {
            self.since = None;
        } else if self.since.is_none() {
            self.since = Some(now);
        }
        out
    }
    pub fn finish(&mut self) -> Vec<u8> {
        self.since = None;
        self.context.advance(&self.pending);
        std::mem::take(&mut self.pending)
    }
}
fn prefixes() -> [Vec<u8>; 2] {
    ["\r\n", "\r\r\n"]
        .map(|eol| format!("\r{}{eol}Broadcast message from ", " ".repeat(WIDTH)).into_bytes())
}
fn complete_sequence(bytes: &[u8]) -> Option<Vec<Notice>> {
    let prefixes = prefixes();
    let mut rest = bytes;
    let mut notices = vec![];
    while !rest.is_empty() {
        if !prefixes.iter().any(|prefix| rest.starts_with(prefix)) {
            return None;
        }
        match parse(rest) {
            Parsed::Complete(len, n) => {
                notices.push(n);
                rest = &rest[len..];
            }
            _ => return None,
        }
    }
    (!notices.is_empty()).then_some(notices)
}

enum Parsed {
    More,
    Invalid,
    Complete(usize, Notice),
}
fn end_line(bytes: &[u8], eol: &[u8]) -> Option<usize> {
    bytes.windows(eol.len()).position(|s| s == eol)
}
fn safe_text(bytes: &[u8]) -> Option<&str> {
    let s = std::str::from_utf8(bytes).ok()?;
    (!s.chars().any(|c| c.is_control())).then_some(s)
}
fn parse(bytes: &[u8]) -> Parsed {
    let eol = if bytes[WIDTH + 2] == b'\r' {
        "\r\r\n"
    } else {
        "\r\n"
    };
    let header_start = WIDTH + 1 + eol.len();
    let Some(header_len) = end_line(&bytes[header_start..], eol.as_bytes()) else {
        return if bytes.len() > 1024 {
            Parsed::Invalid
        } else {
            Parsed::More
        };
    };
    let header_bytes = &bytes[header_start..header_start + header_len];
    let Some(header_bytes) = header_bytes.strip_suffix(b"\x07\x07") else {
        return Parsed::Invalid;
    };
    let Some(header) = safe_text(header_bytes) else {
        return Parsed::Invalid;
    };
    let Some(rest) = header.trim_end().strip_prefix("Broadcast message from ") else {
        return Parsed::Invalid;
    };
    let Some((sender, tail)) = rest.split_once(" (") else {
        return Parsed::Invalid;
    };
    let Some((tty, date)) = tail.split_once(") (") else {
        return Parsed::Invalid;
    };
    let Some((user, host)) = sender.split_once('@') else {
        return Parsed::Invalid;
    };
    if user.is_empty()
        || host.is_empty()
        || sender.chars().any(char::is_whitespace)
        || !(tty.starts_with("pts/") || tty.starts_with("tty"))
        || tty
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '/'))
    {
        return Parsed::Invalid;
    }
    // Older util-linux truncates the banner at byte 79, including the date.
    // Require a recognizable ctime date prefix even for that exact-width form.
    let date_parts: Vec<_> = date.split_whitespace().collect();
    if date_parts.len() < 4
        || !["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&date_parts[0])
        || ![
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .contains(&date_parts[1])
        || !date_parts[2]
            .parse::<u8>()
            .is_ok_and(|d| (1..=31).contains(&d))
        || !date_parts[3]
            .chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == ')' || c == ' ')
        || (!date.ends_with("):") && header_bytes.len() != WIDTH)
    {
        return Parsed::Invalid;
    }
    let separator = format!("{}{eol}", " ".repeat(WIDTH));
    let mut cursor = header_start + header_len + eol.len();
    let available = &bytes[cursor..];
    if available.len() < separator.len() {
        return if separator.as_bytes().starts_with(available) {
            Parsed::More
        } else {
            Parsed::Invalid
        };
    }
    if !available.starts_with(separator.as_bytes()) {
        return Parsed::Invalid;
    }
    cursor += separator.len();
    let mut body = Vec::new();
    loop {
        let Some(n) = end_line(&bytes[cursor..], eol.as_bytes()) else {
            return if bytes.len() > LIMIT
                || bytes[cursor..].iter().any(|b| *b < 0x20 && *b != b'\r')
            {
                Parsed::Invalid
            } else {
                Parsed::More
            };
        };
        let line = &bytes[cursor..cursor + n];
        cursor += n + eol.len();
        if line == &separator.as_bytes()[..WIDTH] {
            if body.is_empty() {
                return Parsed::Invalid;
            }
            let body = body.join("\n");
            return Parsed::Complete(
                cursor,
                Notice {
                    id: 0,
                    sender: sender.to_string(),
                    tty: tty.to_string(),
                    date: date.trim_end_matches("):").to_string(),
                    original: format!("{}\n\n{}", header.trim_end(), body),
                    body,
                    wire: bytes[..cursor].to_vec(),
                },
            );
        }
        let Some(line) = safe_text(line) else {
            return Parsed::Invalid;
        };
        // Empty body lines are ambiguous with the wall footer: fall back.
        if line.trim().is_empty() || line.starts_with("Broadcast message from ") {
            return Parsed::Invalid;
        }
        body.push(line.trim_end().to_string());
        if cursor > LIMIT {
            return Parsed::Invalid;
        }
    }
}

/// Native text renderer input. Never treat remote text as markup or link targets.
pub fn literal_text_html(text: &str) -> String {
    format!(
        "<pre>{}</pre>",
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(body: &str) -> Vec<u8> {
        let header = "Broadcast message from root@host (pts/12) (Tue Nov 10 16:35:48 2026):";
        format!(
            "\r{}\r\n{header:<79}\x07\x07\r\n{}\r\n{body}\r\n{}\r\n",
            " ".repeat(79),
            " ".repeat(79),
            " ".repeat(79)
        )
        .into_bytes()
    }
    #[test]
    fn every_split_and_single_byte_chunks_preserve_surrounding_output() {
        let data = frame("00993641 测试占用");
        for split in 0..=data.len() {
            let mut f = Filter::default();
            let now = Instant::now();
            let mut a = f.feed(b"prompt> ", now);
            let b = f.feed(&data[..split], now);
            let c = f.feed(&data[split..], now);
            let settled = f.feed(&[], now + WAIT);
            let d = f.feed(b"normal\x1b[31m output", now + WAIT);
            a.terminal.extend(b.terminal);
            a.terminal.extend(c.terminal);
            a.terminal.extend(settled.terminal);
            a.terminal.extend(d.terminal);
            a.terminal.extend(f.finish());
            a.notices.extend(b.notices);
            a.notices.extend(c.notices);
            a.notices.extend(settled.notices);
            a.notices.extend(d.notices);
            assert_eq!(a.terminal, b"prompt> normal\x1b[31m output");
            assert_eq!(a.notices.len(), 1);
            assert_eq!(a.notices[0].wire, data);
            assert_eq!(a.notices[0].body, "00993641 测试占用");
        }
        let mut f = Filter::default();
        let mut notices = vec![];
        let now = Instant::now();
        for b in data {
            let out = f.feed(&[b], now);
            assert!(out.terminal.is_empty());
            notices.extend(out.notices);
        }
        notices.extend(f.feed(&[], now + WAIT).notices);
        assert_eq!(notices.len(), 1);
    }
    #[test]
    fn unknown_truncated_controls_and_invalid_utf8_are_lossless() {
        let valid = frame("hello");
        let mut cases = vec![
            b"Broadcast message from root@host\r\nnot a wall frame\r\n".to_vec(),
            frame("bad\x1b[31m text"),
            frame("a\nwrong ending"),
            frame(""),
        ];
        let mut bad = frame("hello");
        let at = bad.iter().position(|b| *b == 7).unwrap();
        bad[at] = b' ';
        cases.push(bad);
        let mut bad = valid.clone();
        bad[200] = 0xff;
        cases.push(bad);
        cases.extend((1..valid.len()).map(|n| valid[..n].to_vec()));
        for data in cases {
            let mut f = Filter::default();
            let mut out = f.feed(&data, Instant::now());
            out.terminal.extend(f.finish());
            assert!(out.notices.is_empty());
            assert_eq!(out.terminal, data);
        }
    }
    #[test]
    fn timeout_does_not_invent_a_notification_or_reorder_bytes() {
        let mut f = Filter::default();
        let now = Instant::now();
        let data = frame("hello");
        let prefix = &data[..data.len() - 3];
        assert!(f.feed(prefix, now).terminal.is_empty());
        let out = f.feed(b"prompt", now + WAIT);
        assert!(out.notices.is_empty());
        assert_eq!(out.terminal, [prefix, b"prompt"].concat());
    }
    #[test]
    fn repeated_frames_and_multiple_nonempty_lines() {
        let a = frame("line one\r\nline two");
        let data = [a.clone(), a].concat();
        let mut filter = Filter::default();
        let now = Instant::now();
        filter.feed(&data, now);
        let out = filter.feed(&[], now + WAIT);
        assert!(out.terminal.is_empty());
        assert_eq!(out.notices.len(), 2);
        assert_eq!(out.notices[0].body, "line one\nline two");
    }
    #[test]
    fn old_truncated_banner_is_accepted_only_with_full_framing() {
        let h = "Broadcast message from root@localhost.localdomain (pts/12) (Tue Nov 10 16:35:48 2026):";
        let data = format!(
            "\r{}\r\n{}\x07\x07\r\n{}\r\n00993641 测试占用\r\n{}\r\n",
            " ".repeat(79),
            &h[..79],
            " ".repeat(79),
            " ".repeat(79)
        );
        let mut filter = Filter::default();
        let now = Instant::now();
        filter.feed(data.as_bytes(), now);
        let out = filter.feed(&[], now + WAIT);
        assert_eq!(out.notices.len(), 1);
        assert_eq!(out.notices[0].sender, "root@localhost.localdomain");
    }
    #[test]
    fn oversized_candidate_is_bounded_and_released() {
        let data = frame(&"x".repeat(LIMIT + 1));
        let mut f = Filter::default();
        let mut out = f.feed(&data, Instant::now());
        out.terminal.extend(f.finish());
        assert!(out.notices.is_empty());
        assert_eq!(out.terminal, data);
    }
    #[test]
    fn inbox_read_clear_and_capacity_do_not_hide_new_arrivals() {
        let mut filter = Filter::default();
        let now = Instant::now();
        filter.feed(&frame("hello"), now);
        let notice = filter.feed(&[], now + WAIT).notices.remove(0);
        let mut inbox = Inbox::default();
        for _ in 0..101 {
            inbox.push(notice.clone());
        }
        assert_eq!(inbox.items.len(), 100);
        assert_eq!(inbox.unread(), 100);
        inbox.mark_read();
        assert_eq!(inbox.unread(), 0);
        inbox.push(notice.clone());
        assert_eq!(inbox.unread(), 1);
        inbox.clear();
        assert!(inbox.items.is_empty());
        inbox.push(notice);
        assert_eq!(inbox.unread(), 1);
    }
    #[test]
    fn mixed_output_and_padded_blank_body_fall_back_as_a_whole() {
        let padded = " ".repeat(79);
        for data in [
            [frame("hello"), b"ordinary command output\r\n".to_vec()].concat(),
            frame(&format!("first\r\n{padded}\r\nlast")),
        ] {
            for chunk in [1, 17, data.len()] {
                let mut filter = Filter::default();
                let now = Instant::now();
                let mut terminal = vec![];
                let mut notices = vec![];
                for bytes in data.chunks(chunk) {
                    let out = filter.feed(bytes, now);
                    terminal.extend(out.terminal);
                    notices.extend(out.notices);
                }
                let out = filter.feed(&[], now + WAIT);
                terminal.extend(out.terminal);
                notices.extend(out.notices);
                assert!(notices.is_empty());
                assert_eq!(terminal, data);
            }
        }
    }
    #[test]
    fn control_string_payload_is_never_extracted_and_markup_is_literal() {
        for start in [b"\x1b]2;".as_slice(), b"\x1bP", b"\x1b_"] {
            let data = [start, &frame("hello"), b"\x1b\\"].concat();
            let mut filter = Filter::default();
            let now = Instant::now();
            let mut terminal = vec![];
            let mut notices = vec![];
            for b in &data {
                let out = filter.feed(&[*b], now);
                terminal.extend(out.terminal);
                notices.extend(out.notices);
            }
            let out = filter.feed(&[], now + WAIT);
            terminal.extend(out.terminal);
            notices.extend(out.notices);
            assert!(notices.is_empty());
            assert_eq!(terminal, data);
        }
        assert_eq!(
            literal_text_html("<a href='https://example.org'>x & y</a>"),
            "<pre>&lt;a href='https://example.org'&gt;x &amp; y&lt;/a&gt;</pre>"
        );
    }
    #[test]
    fn pty_onlcr_expansion_and_mixed_line_endings_are_distinguished() {
        // Captured locally through os.openpty(): ONLCR changes each CRLF to CRCRLF.
        let data = String::from_utf8(frame("00993641 测试占用"))
            .unwrap()
            .replace("\r\n", "\r\r\n")
            .into_bytes();
        for split in 0..=data.len() {
            let mut f = Filter::default();
            let now = Instant::now();
            assert!(f.feed(&data[..split], now).terminal.is_empty());
            assert!(f.feed(&data[split..], now).terminal.is_empty());
            let out = f.feed(&[], now + WAIT);
            assert!(out.terminal.is_empty());
            assert_eq!(out.notices.len(), 1);
            assert_eq!(out.notices[0].wire, data);
        }
        let mixed = String::from_utf8(data)
            .unwrap()
            .replacen("\r\r\n", "\r\n", 1)
            .into_bytes();
        let mut f = Filter::default();
        let now = Instant::now();
        let mut out = f.feed(&mixed, now);
        let later = f.feed(&[], now + WAIT);
        out.terminal.extend(later.terminal);
        assert!(out.notices.is_empty() && later.notices.is_empty());
        assert_eq!(out.terminal, mixed);
    }
}
