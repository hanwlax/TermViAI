use crate::{BroadcastState, UnknownPane};
use std::hash::Hash;

#[derive(Debug)]
pub struct DeliveryReport<P, E> {
    pub delivered: Vec<P>,
    pub disconnected: Vec<P>,
    pub failures: Vec<(P, E)>,
}

/// Fan out the same byte slice; encoding belongs to the source terminal.
///
/// `write` must write the entire slice (e.g. Write::write_all), without
/// recursively invoking this router. Failures never stop other deliveries.
/// A partial write failure is not retried: replaying input could duplicate it.
/// The adapter must use bounded/nonblocking transport submission if needed;
/// this policy function itself does not create queues or background threads.
pub fn route_input<P, E>(
    state: &BroadcastState<P>,
    source: P,
    data: &[u8],
    mut is_connected: impl FnMut(P) -> bool,
    mut write: impl FnMut(P, &[u8]) -> Result<(), E>,
) -> Result<DeliveryReport<P, E>, UnknownPane<P>>
where
    P: Copy + Eq + Hash + Ord,
{
    let targets = state.targets_for(source)?;
    let mut report = DeliveryReport {
        delivered: Vec::new(),
        disconnected: Vec::new(),
        failures: Vec::new(),
    };
    if data.is_empty() {
        return Ok(report);
    }
    for target in targets {
        if !is_connected(target) {
            report.disconnected.push(target);
            continue;
        }
        match write(target, data) {
            Ok(()) => report.delivered.push(target),
            Err(error) => report.failures.push((target, error)),
        }
    }
    Ok(report)
}
