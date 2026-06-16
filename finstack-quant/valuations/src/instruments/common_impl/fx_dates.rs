//! FX date utilities re-exported from finstack_quant_core, plus pair-aware helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::fx::fx_spot_date;
use finstack_quant_core::dates::Date;
use finstack_quant_core::Result;

pub use finstack_quant_core::dates::fx::{
    add_joint_business_days, adjust_joint_calendar, roll_spot_date, ResolvedCalendarPair,
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
pub fn fx_spot_date_for_pair(
    trade_date: Date,
    spot_lag_days: u32,
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
    fx_spot_date(
        trade_date,
        spot_lag_days,
        base_cal_id,
        quote_cal_id,
        usd_cal_id,
    )
}
