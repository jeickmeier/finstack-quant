//! FX date utilities re-exported from finstack_quant_core, plus pair-aware helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::fx::fx_spot_date;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, Tenor};
use finstack_quant_core::Result;

pub use finstack_quant_core::dates::fx::{
    add_joint_business_days, adjust_joint_calendar, ResolvedCalendarPair,
};

/// Roll a trade date to spot using the market (CLS-consistent) FX convention,
/// deriving the USD calendar from the currency pair.
///
/// When either leg of the pair is USD, that leg's calendar ID is passed to
/// [`fx_spot_date`] as the USD settlement calendar, so a US holiday on an
/// intermediate day does not delay spot while the final value date must still
/// be a good USD business day (e.g. EUR/USD traded Thu 2025-07-03 settles
/// Mon 2025-07-07, not Tue 2025-07-08; see
/// FX spot convention).
///
/// For non-USD crosses the instruments do not carry a USD calendar ID, so we
/// pass `usd_cal_id = None` and behavior remains the symmetric two-calendar
/// rule (a known limitation: the CLS rule would additionally require the final
/// value date to be a good USD day).
///
/// # Errors
///
/// Returns an error if calendar resolution fails or the iteration limit is
/// exceeded (see [`fx_spot_date`]).
///
/// # Arguments
///
/// * `trade_date` - FX trade date from which spot settlement is rolled.
/// * `settlement_days` - Contractual T+N spot lag in joint business days.
/// * `base_currency` - Base leg currency used to detect the pair's USD leg.
/// * `quote_currency` - Quote leg currency used to detect the pair's USD leg.
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
        base_cal_id
    } else if quote_currency == Currency::USD {
        quote_cal_id
    } else {
        None
    };
    let lag_days = i32::try_from(settlement_days).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "FX settlement_days {settlement_days} exceeds the supported range"
        ))
    })?;
    fx_spot_date(trade_date, lag_days, base_cal_id, quote_cal_id, usd_cal_id)
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
    adjust_joint_calendar(maturity, business_day_convention, base_cal_id, quote_cal_id)
}
