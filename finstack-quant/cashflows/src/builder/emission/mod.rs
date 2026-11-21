//! Date-level cashflow emission for the build pipeline.
//!
//! Submodules: `coupons`, `amortization`, `fees`, and `balances`. Each
//! `emit_*_on` function pushes the flows for one date into the build state;
//! the coupon emitters also return any PIK amount to capitalize into the
//! outstanding balance.

mod amortization;
mod balances;
mod coupons;
mod fees;

pub(crate) use coupons::{ResolvedFloatMarket, emit_fixed_coupons_on, emit_float_coupons_on};

pub(super) use amortization::emit_amortization_on;

pub(super) use fees::emit_fees_on;

pub(super) fn accrual_context_state(
    schedule: &super::specs::ScheduleParams,
    coupon_period: Option<(
        finstack_quant_core::dates::Date,
        finstack_quant_core::dates::Date,
    )>,
    is_termination: bool,
) -> finstack_quant_core::Result<finstack_quant_core::dates::DayCountContextState> {
    finstack_quant_core::dates::DayCountContextState::try_new(
        Some(schedule.calendar_id.to_string()),
        (schedule.day_count == finstack_quant_core::dates::DayCount::ActActIsma)
            .then_some(schedule.frequency),
        None,
        coupon_period,
        Some(is_termination),
    )
}
