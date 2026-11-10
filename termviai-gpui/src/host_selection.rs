//! Transient Hosts-page selection; never persists credentials or list indices.
use crate::hosts::Host;
#[derive(Default)]
pub struct HostSelection(Vec<Host>);
impl HostSelection {
    pub fn contains(&self, host: &Host) -> bool {
        self.0.contains(host)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
    pub fn toggle(&mut self, host: Host) {
        if let Some(i) = self.0.iter().position(|h| h == &host) {
            self.0.remove(i);
        } else {
            self.0.push(host);
        }
    }
    pub fn retain(&mut self, current: &[Host]) {
        self.0.retain(|h| current.contains(h));
    }
    /// Consume first, then connect in visual/store order. Changed or removed
    /// records cannot silently reuse the selection with a different credential.
    pub fn take(&mut self, current: &[Host]) -> Vec<Host> {
        let selected = std::mem::take(&mut self.0);
        let mut result = vec![];
        for host in current {
            if selected.contains(host) && !result.contains(host) {
                result.push(host.clone());
            }
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn host(name: &str) -> Host {
        Host {
            label: name.into(),
            address: "host.invalid".into(),
            port: 22,
            username: name.into(),
            ..Default::default()
        }
    }
    #[test]
    fn toggling_and_consuming_opens_once_in_list_order() {
        let (a, b, c) = (host("a"), host("b"), host("c"));
        let mut s = HostSelection::default();
        s.toggle(c.clone());
        s.toggle(a.clone());
        s.toggle(b.clone());
        s.toggle(b.clone());
        assert_eq!(s.len(), 2);
        assert_eq!(s.take(&[a.clone(), b, c.clone(), a.clone()]), vec![a, c]);
        assert!(s.is_empty());
        assert!(s.take(&[]).is_empty());
    }
    #[test]
    fn edited_and_removed_hosts_are_not_opened() {
        let (a, b) = (host("a"), host("b"));
        let mut s = HostSelection::default();
        s.toggle(a.clone());
        s.toggle(b.clone());
        let mut updated = a.clone();
        updated.identity_id = "another-identity".into();
        s.retain(&[updated.clone(), b.clone()]);
        assert!(!s.contains(&a));
        assert!(s.take(&[updated]).is_empty());
    }
    #[test]
    fn distinct_accounts_remain_separate_and_clear_cancels_the_batch() {
        let (a, b) = (host("alice"), host("bob"));
        let mut s = HostSelection::default();
        s.toggle(a.clone());
        s.toggle(b.clone());
        assert_eq!(s.take(&[a.clone(), b.clone()]), vec![a.clone(), b.clone()]);
        s.toggle(a.clone());
        s.clear();
        assert!(s.take(&[a, b]).is_empty());
    }
}
