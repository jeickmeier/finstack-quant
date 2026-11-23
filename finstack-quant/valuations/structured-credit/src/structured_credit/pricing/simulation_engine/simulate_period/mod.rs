//! One payment period of the deal simulation, in phases: pool collections
//! and the recovery queue ([`cash`]), loss allocation and loss-driven tests
//! ([`losses`]), the side accounts that set the waterfall cash ([`cash`]),
//! the period's resolved waterfall ([`period_waterfall`]), execution, and
//! recording the payments per tranche ([`record`]).

mod cash;
mod losses;
mod period_waterfall;
mod record;
mod step;
#[cfg(test)]
mod tests;

pub(super) use step::simulate_period;
