//! Pool flows driven by instrument schedules.
//!
//! When a pool holds real instruments (`InstrumentCollateral`) the period
//! flows come from each instrument's own contractual schedule, bucketed by
//! legal payment period, instead of the balance-and-rate model. Defaults and
//! prepayments follow the deal's behavioural assumptions (or the
//! instrument's hazard curve when it carries one), applied as survival
//! scaling of the contractual amounts — the representative-line convention
//! the rate engine uses. Collateral draws (revolver utilization increases,
//! delayed-draw term-loan draws, loan-equivalent draws at default) are funded
//! through the reserve account by `fund_collateral_draws`; revolver
//! repayments replenish it through `replenish_reserve_from_repayments`.
//!
//! The bucketed schedules ([`PreparedInstrumentSchedules`]) are built once
//! and shared: the deterministic [`InstrumentScheduleFlowSource`] and the
//! stochastic path source both drive [`run_period`] with per-name inputs
//! ([`NamePeriod`]) — a period default probability, a recovery rate and, for
//! revolvers with simulated utilization, the end-of-period utilization.

mod period;
mod schedule;
mod source;
mod types;

pub(super) use period::{NamePeriod, PeriodModel, hazard_period_default, run_period};
pub(crate) use schedule::PreparedInstrumentSchedules;
pub(crate) use source::InstrumentScheduleFlowSource;
pub(super) use types::{FROZEN_UTILIZATION_VOL, NameState, PeriodDefaultSource, SpreadTerms};
