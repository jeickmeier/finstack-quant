//! FX date utilities re-exported from finstack_quant_core, plus pair-aware helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::fx::fx_spot_date;
use finstack_quant_core::dates::{
    adjust, resolve_joint_calendar, BusinessDayConvention, Date, DateExt, Tenor,
};
use finstack_quant_core::Result;

/// Roll a trade date to spot under the pair's contractual spot lag.
///
/// USD final-date checks use the supplied USD leg calendar or the registered
/// `usny` calendar. Non-USD crosses always use `usny` for their final date.
///
/// # Errors
///
/// Returns an error if calendar resolution fails or no business day is found
/// within the search window (see [`fx_spot_date`]).
///
/// # Arguments
///
/// * `trade_date` - FX trade date from which spot settlement is rolled.
/// * `settlement_days` - Contractual spot lag in business days, from 0 through 31; pass
///   [`finstack_quant_core::dates::fx::fx_standard_settlement_days`] for market spot.
/// * `base_currency` - Base leg currency used to select its market settlement lag.
/// * `quote_currency` - Quote leg currency used to select its market settlement lag.
/// * `base_cal_id` - Optional base-currency business-calendar identifier.
/// * `quote_cal_id` - Optional quote-currency business-calendar identifier.
pub fn fx_spot_date_for_pair(
    trade_date: Date,
    settlement_days: u32,
    base_currency: Currency,
    quote_currency: Currency,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
) -> Result<Date> {
    let usd_cal_id = if base_currency == Currency::USD {
        Some(base_cal_id.unwrap_or("usny"))
    } else if quote_currency == Currency::USD {
        Some(quote_cal_id.unwrap_or("usny"))
    } else {
        Some("usny")
    };
    fx_spot_date(
        trade_date,
        base_currency,
        quote_currency,
        settlement_days,
        base_cal_id,
        quote_cal_id,
        usd_cal_id,
    )
}

/// Add a standard calendar tenor to an FX spot date and apply the joint roll.
///
/// # Arguments
///
/// * `spot_date` - Pair spot date from which the standard tenor runs.
/// * `tenor` - Calendar tenor such as 1M, 3M, or 1Y.
/// * `business_day_convention` - Joint-calendar roll applied to the unadjusted maturity.
/// * `end_of_month` - When `true` and `spot_date` is month-end, preserve month-end.
/// * `base_cal_id` - Optional base-currency calendar; `None` uses weekends only.
/// * `quote_cal_id` - Optional quote-currency calendar; `None` uses weekends only.
///
/// # Errors
///
/// Returns an error when tenor arithmetic, calendar resolution, or date
/// adjustment fails.
pub fn add_fx_standard_tenor(
    spot_date: Date,
    tenor: Tenor,
    business_day_convention: BusinessDayConvention,
    end_of_month: bool,
    base_cal_id: Option<&str>,
    quote_cal_id: Option<&str>,
) -> Result<Date> {
    let preserve_eom = end_of_month && spot_date == spot_date.end_of_month();
    let mut maturity = tenor.add_to_date(
        spot_date,
        None,
        finstack_quant_core::dates::BusinessDayConvention::Unadjusted,
    )?;
    if preserve_eom {
        maturity = maturity.end_of_month();
    }
    let calendar = resolve_joint_calendar(base_cal_id, quote_cal_id)?;
    adjust(maturity, business_day_convention, calendar)
}
