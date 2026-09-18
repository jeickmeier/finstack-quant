//! Level-payment amortization arithmetic shared by the pool-flow engine and
//! the loan-level metrics.

/// Level payment per unit of balance over `periods` payment periods at the
/// nominal per-period rate `period_rate`: `r / (1 − (1 + r)^−n)`, `1 / n`
/// at a zero rate, and the whole balance when the term is empty or the
/// arithmetic degenerates.
///
/// # Arguments
///
/// * `period_rate` - Nominal rate per payment period as a decimal (annual
///   coupon × months per period / 12).
/// * `periods` - Payment periods left on the schedule.
#[must_use]
pub fn level_payment_per_unit(period_rate: f64, periods: f64) -> f64 {
    if !periods.is_finite() || periods <= 0.0 {
        return 1.0;
    }
    if period_rate.abs() < 1e-12 {
        return 1.0 / periods;
    }
    let denom = 1.0 - (1.0 + period_rate).powf(-periods);
    if denom.is_finite() && denom.abs() > 1e-12 {
        period_rate / denom
    } else {
        1.0
    }
}

/// Payment periods left on a loan's amortization schedule as of a period
/// opening, in periods of `months_per_period` months.
///
/// # Arguments
///
/// * `age_months` - Months from the loan's origination to the period opening.
/// * `amortization_term_months` - Schedule length from origination, when the
///   loan carries one.
/// * `months_to_maturity` - Months from the period's payment date to the
///   loan's maturity, used when no term is given (the schedule then ends at
///   maturity, counting the current period).
/// * `months_per_period` - Payment period length in months (at least 1).
#[must_use]
pub fn remaining_schedule_periods(
    age_months: u32,
    amortization_term_months: Option<u32>,
    months_to_maturity: u32,
    months_per_period: u32,
) -> u32 {
    let months_per_period = months_per_period.max(1);
    match amortization_term_months {
        Some(term) => term
            .saturating_sub(age_months)
            .max(1)
            .div_ceil(months_per_period),
        None => months_to_maturity.div_ceil(months_per_period) + 1,
    }
}
