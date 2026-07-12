//! Helper functions for cashflow emission.

use crate::primitives::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{adjust, Date, DateExt, HolidayCalendar};
use finstack_quant_core::money::Money;

/// Add a PIK cashflow if the amount is strictly positive.
///
/// Returns the PIK amount for outstanding balance tracking (`0.0` when no
/// flow was added).
///
/// # Errors
///
/// Returns `Error::Validation` when `pik_amt` is negative. A negative PIK
/// coupon (negative all-in rate on a PIK/split leg) would have to *reduce*
/// the outstanding balance; that de-capitalization policy is not modeled, so
/// it fails loudly instead of being silently dropped. Configure an
/// `all_in_floor_bp` of zero (or pay cash) for negative-rate PIK structures.
#[inline]
pub(in crate::builder) fn add_pik_flow_if_nonzero(
    flows: &mut Vec<CashFlow>,
    date: Date,
    pik_amt: f64,
    ccy: Currency,
    rate: Option<f64>,
    accrual_factor: f64,
) -> finstack_quant_core::Result<f64> {
    if pik_amt < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "negative PIK coupon amount {pik_amt} on {date}: de-capitalizing PIK is not \
             supported; floor the all-in rate at zero or use a cash coupon"
        )));
    }
    if pik_amt > 0.0 {
        flows.push(CashFlow {
            date,
            reset_date: None,
            amount: Money::new(pik_amt, ccy),
            kind: CFKind::PIK,
            accrual_factor,
            rate,
        });
        Ok(pik_amt)
    } else {
        Ok(0.0)
    }
}

/// Compute reset date with calendar adjustment.
///
/// Market standard: reset dates are computed as `accrual_start - reset_lag_days`
/// **business days** using the fixing calendar (or accrual calendar), then adjusted
/// to a business day using the specified business-day convention.
///
/// With a zero lag, the reset date is exactly the accrual start. Callers that
/// require a business-day fixing must provide an adjusted accrual start or a
/// positive reset lag; silently moving an explicit zero-lag reset earlier
/// changes the contract and can incorrectly turn a new trade into a seasoned
/// one that requires historical fixings.
#[inline]
pub(in crate::builder) fn compute_reset_date(
    accrual_start: Date,
    reset_lag_days: i32,
    bdc: finstack_quant_core::dates::BusinessDayConvention,
    cal: &dyn HolidayCalendar,
) -> finstack_quant_core::Result<Date> {
    if reset_lag_days == 0 {
        return Ok(accrual_start);
    }

    // Business-day subtraction avoids weekend/holiday traps where calendar-day subtraction
    // plus ModifiedFollowing could accidentally roll past the accrual start/end.
    let mut reset_date = accrual_start.add_business_days(-reset_lag_days, cal)?;
    reset_date = adjust(reset_date, bdc, cal)?;
    Ok(reset_date)
}
