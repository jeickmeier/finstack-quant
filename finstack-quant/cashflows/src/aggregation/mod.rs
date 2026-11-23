//! Currency-preserving aggregation of cashflows into `Period`s.
//!
//! # Period Contract
//!
//! All aggregation functions bucket flows into half-open intervals
//! `[period.start, period.end)`: a flow dated exactly on `period.end` belongs
//! to the *next* period. Periods must be sorted by start date and
//! non-overlapping; the public entry points validate this and return
//! [`finstack_quant_core::Error::Validation`] otherwise.
//!
//! # Summation Policy
//!
//! Nominal per-currency rollups
//! ([`aggregate_by_period`](crate::aggregation::aggregate_by_period) and
//! [`aggregate_cashflows_checked`](crate::aggregation::aggregate_cashflows_checked))
//! sum exact `Decimal` [`Money`](finstack_quant_core::money::Money) amounts via
//! [`Money::checked_add`](finstack_quant_core::money::Money::checked_add),
//! preserving cent-exact totals and failing loudly on
//! currency mismatch or `Decimal` overflow. Present-value rollups
//! ([`crate::builder::CashFlowSchedule::pv_by_period`]) accumulate `f64` row
//! PVs via a Neumaier-compensated accumulator before materializing `Money`. No
//! per-flow ISO-4217 rounding is applied during accumulation. Results are
//! deterministic given sorted inputs (the public wrappers sort unsorted inputs
//! by date before accumulating).
//!
//! # Historical-Flow PV Convention
//!
//! PV aggregation functions assign **zero PV** to flows dated on or before the
//! valuation date (`as_of`). Historical flows still appear in
//! plain amount aggregation (`aggregate_by_period`).

mod credit_pv;
mod periods;
#[cfg(test)]
mod tests;

pub use credit_pv::{credit_adjusted_cashflow_pvs, weighted_average_life_from_principal};
pub(crate) use credit_pv::{missing_principal_delta, pv_by_period};
pub(crate) use periods::calendar_year_ladder_rows;
pub use periods::{
    CalendarYearLadderRow, PeriodAggregation, aggregate_by_period, aggregate_cashflows_checked,
    calendar_year_ladder,
};
