//! Principal intervals used by coupon and fee accrual.

use finstack_quant_core::dates::Date;
use rust_decimal::Decimal;

/// Partition an accrual window only where the economic balance changes.
pub(super) fn balance_segments(
    history: &[(Date, Decimal)],
    start: Date,
    end: Date,
    fallback: Decimal,
) -> Vec<(Date, Date, Decimal)> {
    let first = history.partition_point(|(date, _)| *date <= start);
    let mut balance = first.checked_sub(1).map_or(fallback, |i| history[i].1);
    let mut segment_start = start;
    let mut result = Vec::new();
    for &(date, next_balance) in history[first..].iter().take_while(|(date, _)| *date < end) {
        if next_balance != balance {
            if segment_start < date {
                result.push((segment_start, date, balance));
            }
            segment_start = date;
            balance = next_balance;
        }
    }
    if segment_start < end {
        result.push((segment_start, end, balance));
    }
    result
}

pub(super) fn accrual_context_state(
    schedule: &super::super::specs::ScheduleParams,
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
