//! Read-only aggregation for the native Monitor hierarchy; never changes samples.
use crate::monitor::{Die, Snapshot};
#[derive(Debug)]
pub struct Metrics {
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub core: Option<f64>,
}
pub fn metrics(dies: &[&Die]) -> Metrics {
    let sum = |value: fn(&Die) -> Option<f64>| -> Option<f64> {
        if dies.is_empty() {
            return None;
        }
        dies.iter()
            .map(|d| value(d))
            .collect::<Option<Vec<_>>>()
            .map(|v| v.iter().sum())
    };
    Metrics {
        used: sum(|d| d.used_mb),
        total: sum(|d| d.total_mb),
        core: sum(|d| d.core).map(|v| v / dies.len() as f64),
    }
}
pub fn process_memory(s: &Snapshot, pid: u32, npu: u32, die: Option<u32>) -> Option<f64> {
    let rows: Vec<_> = s
        .processes
        .iter()
        .filter(|p| p.pid == pid && p.npu == npu && die.is_none_or(|d| d == p.die))
        .collect();
    if rows.is_empty() {
        return None;
    }
    rows.iter()
        .map(|p| p.memory_mb)
        .collect::<Option<Vec<_>>>()
        .map(|v| v.iter().sum())
}
pub fn locations(s: &Snapshot, pid: u32) -> String {
    s.processes
        .iter()
        .filter(|p| p.pid == pid)
        .map(|p| (p.npu, p.die))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|(n, d)| format!("NPU {n} · Die {d}"))
        .collect::<Vec<_>>()
        .join(", ")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_metrics_are_not_reported_as_zero_or_partial_totals() {
        let a = Die {
            npu: 4,
            die: 2,
            used_mb: Some(1024.),
            total_mb: Some(4096.),
            core: Some(10.),
        };
        let mut b = a.clone();
        b.die = 3;
        b.used_mb = None;
        b.core = Some(30.);
        let m = metrics(&[&a, &b]);
        assert_eq!(m.used, None);
        assert_eq!(m.total, Some(8192.));
        assert_eq!(m.core, Some(20.));
        assert_eq!(metrics(&[]).core, None);
    }
    #[test]
    fn process_memory_is_scoped_but_locations_include_all_occupied_dies() {
        let s:Snapshot=serde_json::from_value(serde_json::json!({"dies":[],"processes_available":true,"stop_reason":"","processes":[
   {"npu":4,"die":2,"pid":42,"name":"job","memory_mb":100,"token":"t","command":"job","owner":"root","controllable":true},
   {"npu":4,"die":3,"pid":42,"name":"job","memory_mb":200,"token":"t","command":"job","owner":"root","controllable":true},
   {"npu":9,"die":0,"pid":42,"name":"job","memory_mb":400,"token":"t","command":"job","owner":"root","controllable":true}]})).unwrap();
        assert_eq!(s.processes(Some(4), None).len(), 1);
        assert_eq!(process_memory(&s, 42, 4, None), Some(300.));
        assert_eq!(process_memory(&s, 42, 4, Some(3)), Some(200.));
        assert_eq!(
            locations(&s, 42),
            "NPU 4 · Die 2, NPU 4 · Die 3, NPU 9 · Die 0"
        );
    }
}
