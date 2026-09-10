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
