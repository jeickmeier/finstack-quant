//! Contractual term-index caplet schedules on the ACT/365F model clock.

mod schedule;
#[cfg(test)]
mod tests;

pub(super) use schedule::prepare_cap_floor_schedules;
pub(crate) use schedule::{CapFloorSchedule, CapletSchedule};
