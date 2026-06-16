//! Helpers to compute all-in rates using core market_data curves.
//!
//! These helpers properly compute floating rate projections using
//! calendar-aware tenor addition for accurate period end dates.
//!
//! For seasoned instruments (dates before the valuation date), the helpers
//! first attempt to look up historical fixings from `MarketContext`. When
//! fixings are not available, they gracefully fall back to forward curve
//! projection.

#![allow(dead_code)]

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings;
use rust_decimal::prelude::ToPrimitive;

use crate::instruments::fixed_income::structured_credit::types::TrancheCoupon;

/// Calculate period end date from a tenor value in years.
///
/// Converts decimal tenor to proper month-based rolling:
/// - 0.25 years → 3 months
/// - 0.5 years → 6 months
/// - 1.0 years → 12 months
///
/// For tenors that don't map cleanly to months, uses day approximation
/// as a fallback.
///
/// # Market Standard
///
/// Most floating rate indices use standard tenors (1M, 3M, 6M, 12M) where
/// proper month arithmetic is essential:
/// - End-of-month dates should roll to end-of-month
/// - Holiday adjustments (modified following) would be applied downstream
#[inline]
pub(crate) fn tenor_to_period_end(start: Date, tenor_years: f64, day_count: DayCount) -> Date {
    // Infallible helper that silently falls back to `start` on failure.
    //
    // For precision-first code paths, use `try_tenor_to_period_end` and propagate errors.
    use finstack_quant_core::dates::{BusinessDayConvention, Tenor};
    let tenor = Tenor::from_years(tenor_years, day_count);
    tenor
        .add_to_date(start, None, BusinessDayConvention::Unadjusted)
        .unwrap_or(start)
}

/// Fallible variant of [`tenor_to_period_end`].
///
/// Prefer this in pricing/valuation code so date arithmetic failures are surfaced as structured
/// errors instead of panics or silent fallbacks.
#[inline]
pub(crate) fn try_tenor_to_period_end(
    start: Date,
    tenor_years: f64,
    day_count: DayCount,
) -> finstack_quant_core::Result<Date> {
    use finstack_quant_core::dates::{BusinessDayConvention, Tenor};
    let tenor = Tenor::from_years(tenor_years, day_count);
    tenor.add_to_date(start, None, BusinessDayConvention::Unadjusted)
}

/// Try to look up a historical fixing and apply floating rate adjustments (gearing, spread,
/// floor, cap) via [`calculate_floating_rate`].
///
/// Returns `Some(all_in_rate)` when the fixing is found, `None` otherwise.
/// Callers fall back to forward projection on `None`.
fn try_fixing_with_adjustments(
    index_id: &str,
    date: Date,
    as_of: Date,
    params: &crate::cashflow::builder::FloatingRateParams,
    market: &MarketContext,
) -> Option<f64> {
    let series = fixings::get_fixing_series(market, index_id).ok()?;
    let raw_fixing =
        fixings::require_fixing_value_exact(Some(series), index_id, date, as_of).ok()?;
    Some(crate::cashflow::builder::rate_helpers::calculate_floating_rate(raw_fixing, params))
}

/// Try to look up a historical fixing for an asset index rate.
///
/// Returns `Some(index_rate)` when the fixing is found, `None` otherwise.
/// The caller is responsible for adding the spread. Callers fall back to
/// forward projection on `None`.
fn try_asset_fixing(
    index_id: &str,
    date: Date,
    as_of: Date,
    market: &MarketContext,
) -> Option<f64> {
    let series = fixings::get_fixing_series(market, index_id).ok()?;
    fixings::require_fixing_value_exact(Some(series), index_id, date, as_of).ok()
}

/// Compute tranche all-in rate (fixed => fixed; floating => index forward + spread with caps/floors).
///
/// For floating rate tranches, this properly calculates the period end date
/// using calendar-aware month addition based on the index tenor.
///
/// When `date < as_of` and a fixing series exists in `MarketContext`, the
/// historical fixing rate is used instead of the forward projection.
/// Missing fixings for past dates gracefully fall back to forward projection.
pub(crate) fn tranche_all_in_rate(
    coupon: &TrancheCoupon,
    date: Date,
    as_of: Date,
    market: &MarketContext,
) -> f64 {
    // Infallible wrapper that never panics. For correctness-first valuation, prefer
    // `try_tranche_all_in_rate` and propagate errors.
    match coupon {
        TrancheCoupon::Fixed { rate } => *rate,
        TrancheCoupon::Floating(spec) => {
            let spread_bp_f64 = spec.spread_bp.to_f64().unwrap_or_default();
            let gearing_f64 = spec.gearing.to_f64().unwrap_or(1.0);
            let floor_bp_f64 = spec.index_floor_bp.and_then(|d| d.to_f64());
            let cap_bp_f64 = spec.all_in_cap_bp.and_then(|d| d.to_f64());
            let fallback_rate = spread_bp_f64 / 10_000.0;

            let params = crate::cashflow::builder::FloatingRateParams {
                spread_bp: spread_bp_f64,
                gearing: gearing_f64,
                index_floor_bp: floor_bp_f64,
                all_in_cap_bp: cap_bp_f64,
                ..Default::default()
            };

            // Seasoned path: try historical fixings for dates before valuation
            if date < as_of {
                if let Some(fixing_rate) = try_fixing_with_adjustments(
                    spec.index_id.as_str(),
                    date,
                    as_of,
                    &params,
                    market,
                ) {
                    return fixing_rate;
                }
                // Fall through to forward projection if fixings unavailable
            }

            let Ok(fwd) = market.get_forward(spec.index_id.as_str()) else {
                return fallback_rate;
            };

            let tenor = fwd.tenor();
            let Ok(period_end) = try_tenor_to_period_end(date, tenor, fwd.day_count()) else {
                return fallback_rate;
            };

            crate::cashflow::builder::project_floating_rate_from_market(
                date,
                period_end,
                spec.index_id.as_str(),
                &params,
                market,
            )
            .unwrap_or(fallback_rate)
        }
    }
}

/// Fallible variant of [`tranche_all_in_rate`].
///
/// This returns an error if required market data is missing or the rate projection fails.
///
/// When `date < as_of` and a fixing series exists in `MarketContext`, the
/// historical fixing rate is used instead of the forward projection.
/// Missing fixings for past dates gracefully fall back to forward projection.
pub(crate) fn try_tranche_all_in_rate(
    coupon: &TrancheCoupon,
    date: Date,
    as_of: Date,
    market: &MarketContext,
) -> finstack_quant_core::Result<f64> {
    match coupon {
        TrancheCoupon::Fixed { rate } => Ok(*rate),
        TrancheCoupon::Floating(spec) => {
            let spread_bp_f64 = spec
                .spread_bp
                .to_f64()
                .ok_or(finstack_quant_core::InputError::Invalid)?;
            let gearing_f64 = spec
                .gearing
                .to_f64()
                .ok_or(finstack_quant_core::InputError::Invalid)?;
            let floor_bp_f64 = spec
                .index_floor_bp
                .map(|d| d.to_f64().ok_or(finstack_quant_core::InputError::Invalid))
                .transpose()?;
            let cap_bp_f64 = spec
                .all_in_cap_bp
                .map(|d| d.to_f64().ok_or(finstack_quant_core::InputError::Invalid))
                .transpose()?;

            let params = crate::cashflow::builder::FloatingRateParams {
                spread_bp: spread_bp_f64,
                gearing: gearing_f64,
                index_floor_bp: floor_bp_f64,
                all_in_cap_bp: cap_bp_f64,
                ..Default::default()
            };

            // Seasoned path: try historical fixings for dates before valuation
            if date < as_of {
                if let Some(fixing_rate) = try_fixing_with_adjustments(
                    spec.index_id.as_str(),
                    date,
                    as_of,
                    &params,
                    market,
                ) {
                    return Ok(fixing_rate);
                }
                // Fall through to forward projection if fixings unavailable
            }

            let fwd = market.get_forward(spec.index_id.as_str())?;
            let tenor = fwd.tenor();
            let period_end = try_tenor_to_period_end(date, tenor, fwd.day_count())?;

            crate::cashflow::builder::project_floating_rate_from_market(
                date,
                period_end,
                spec.index_id.as_str(),
                &params,
                market,
            )
        }
    }
}

/// Compute asset all-in rate given optional index id and spread; falls back to provided rate.
///
/// Uses the forward curve's own day count convention for year fraction calculations
/// to ensure consistency with how the curve was calibrated.
///
/// When `date < as_of` and a fixing series exists in `MarketContext`, the
/// historical fixing rate is used instead of the forward projection.
/// Missing fixings for past dates gracefully fall back to forward projection.
pub(crate) fn asset_all_in_rate(
    index_id: Option<&str>,
    spread_bps: Option<f64>,
    fallback_rate: f64,
    date: Date,
    as_of: Date,
    market: &MarketContext,
) -> f64 {
    if let Some(idx) = index_id {
        let spread = spread_bps.unwrap_or(0.0) / 10_000.0;

        // Seasoned path: try historical fixings for dates before valuation
        if date < as_of {
            if let Some(fixing) = try_asset_fixing(idx, date, as_of, market) {
                return fixing + spread;
            }
            // Fall through to forward projection if fixings unavailable
        }

        if let Ok(fwd) = market.get_forward(idx) {
            let base = fwd.base_date();
            let dc = fwd.day_count();
            let t2 = dc
                .year_fraction(base, date, DayCountContext::default())
                .unwrap_or(0.0);
            let tenor = fwd.tenor();
            let t1 = (t2 - tenor).max(0.0);
            let idx_rate = fwd.rate_period(t1, t2);
            return idx_rate + spread;
        }
    }
    fallback_rate
}

/// Fallible variant of [`asset_all_in_rate`].
///
/// This returns an error if the forward curve is missing or if date/year-fraction computation
/// fails. Use this in valuation code paths where silent fallbacks are unacceptable.
///
/// When `date < as_of` and a fixing series exists in `MarketContext`, the
/// historical fixing rate is used instead of the forward projection.
/// Missing fixings for past dates gracefully fall back to forward projection.
pub(crate) fn try_asset_all_in_rate(
    index_id: Option<&str>,
    spread_bps: Option<f64>,
    date: Date,
    as_of: Date,
    market: &MarketContext,
) -> finstack_quant_core::Result<f64> {
    let Some(idx) = index_id else {
        return Err(finstack_quant_core::InputError::NotFound {
            id: "asset.index_id".to_string(),
        }
        .into());
    };
    let spread = spread_bps.unwrap_or(0.0) / 10_000.0;

    // Seasoned path: try historical fixings for dates before valuation
    if date < as_of {
        if let Some(fixing) = try_asset_fixing(idx, date, as_of, market) {
            return Ok(fixing + spread);
        }
        // Fall through to forward projection if fixings unavailable
    }

    let fwd = market.get_forward(idx)?;
    let base = fwd.base_date();
    let dc = fwd.day_count();
    let t2 = dc.year_fraction(base, date, DayCountContext::default())?;
    let tenor = fwd.tenor();
    let t1 = (t2 - tenor).max(0.0);
    let idx_rate = fwd.rate_period(t1, t2);
    Ok(idx_rate + spread)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::FloatingRateSpec;
    use finstack_quant_core::dates::{BusinessDayConvention, Tenor};
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::macros::date;

    fn sample_market() -> MarketContext {
        let curve_result = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(date!(2025 - 01 - 01))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (0.25, 0.032), (1.0, 0.035)])
            .build();
        assert!(curve_result.is_ok(), "forward curve should build");
        match curve_result {
            Ok(curve) => MarketContext::new().insert(curve),
            Err(_) => unreachable!(),
        }
    }

    fn floating_coupon() -> TrancheCoupon {
        TrancheCoupon::Floating(FloatingRateSpec {
            index_id: CurveId::new("USD-SOFR-3M"),
            spread_bp: Decimal::new(150, 0),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        })
    }

    #[test]
    fn tenor_helpers_roll_standard_periods() {
        let start = date!(2025 - 01 - 31);
        assert_eq!(
            tenor_to_period_end(start, 0.25, DayCount::Act360),
            date!(2025 - 04 - 30)
        );
        assert_eq!(
            try_tenor_to_period_end(start, 1.0, DayCount::Act365F),
            Ok(date!(2026 - 01 - 31))
        );
    }

    /// Far-future as_of ensures no dates are "in the past", preserving original forward-only behavior.
    const FAR_FUTURE: Date = date!(2099 - 01 - 01);

    #[test]
    fn tranche_all_in_rate_handles_fixed_and_missing_forward_cases() {
        let fixed = tranche_all_in_rate(
            &TrancheCoupon::Fixed { rate: 0.045 },
            date!(2025 - 02 - 01),
            FAR_FUTURE,
            &MarketContext::new(),
        );
        assert_eq!(fixed, 0.045);

        let missing_curve = tranche_all_in_rate(
            &floating_coupon(),
            date!(2025 - 02 - 01),
            FAR_FUTURE,
            &MarketContext::new(),
        );
        assert!((missing_curve - 0.015).abs() < 1e-12);

        let try_missing = try_tranche_all_in_rate(
            &floating_coupon(),
            date!(2025 - 02 - 01),
            FAR_FUTURE,
            &MarketContext::new(),
        );
        assert!(try_missing.is_err(), "missing forward curve should error");
    }

    #[test]
    fn tranche_all_in_rate_uses_forward_curve_when_available() {
        let market = sample_market();
        let rate = tranche_all_in_rate(
            &floating_coupon(),
            date!(2025 - 02 - 01),
            FAR_FUTURE,
            &market,
        );
        let try_rate = try_tranche_all_in_rate(
            &floating_coupon(),
            date!(2025 - 02 - 01),
            FAR_FUTURE,
            &market,
        );

        assert!(
            rate > 0.015,
            "forward projection should exceed pure spread fallback"
        );
        assert!(
            try_rate.is_ok(),
            "fallible helper should succeed with market data"
        );
        if let Ok(value) = try_rate {
            assert!((value - rate).abs() < 1e-12);
        }
    }

    #[test]
    fn asset_all_in_rate_falls_back_or_errors_as_documented() {
        let market = sample_market();
        let date = date!(2025 - 04 - 01);

        let projected = asset_all_in_rate(
            Some("USD-SOFR-3M"),
            Some(50.0),
            0.08,
            date,
            FAR_FUTURE,
            &market,
        );
        let fallback_no_index =
            asset_all_in_rate(None, Some(50.0), 0.08, date, FAR_FUTURE, &market);
        let fallback_missing_curve =
            asset_all_in_rate(Some("MISSING"), Some(50.0), 0.08, date, FAR_FUTURE, &market);
        let try_projected =
            try_asset_all_in_rate(Some("USD-SOFR-3M"), Some(50.0), date, FAR_FUTURE, &market);
        let try_missing_id = try_asset_all_in_rate(None, Some(50.0), date, FAR_FUTURE, &market);

        assert!(projected > 0.0);
        assert_eq!(fallback_no_index, 0.08);
        assert_eq!(fallback_missing_curve, 0.08);
        assert!(try_projected.is_ok(), "valid forward lookup should succeed");
        if let Ok(value) = try_projected {
            assert!((value - projected).abs() < 1e-12);
        }
        assert!(try_missing_id.is_err(), "missing index id should error");
    }

    // -----------------------------------------------------------------------
    // Seasoned instrument tests (fixing lookup when date < as_of)
    // -----------------------------------------------------------------------

    fn sample_market_with_fixings() -> MarketContext {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

        let curve = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(date!(2025 - 01 - 01))
            .day_count(DayCount::Act360)
            .knots([(0.0, 0.03), (0.25, 0.032), (1.0, 0.035)])
            .build()
            .expect("forward curve should build");

        // Historical fixings: 2% on Jan 15, 2.5% on Feb 1
        let fixing_series = ScalarTimeSeries::new(
            "FIXING:USD-SOFR-3M",
            vec![
                (date!(2025 - 01 - 15), 0.02),
                (date!(2025 - 02 - 01), 0.025),
            ],
            None,
        )
        .expect("fixing series should build");

        MarketContext::new()
            .insert(curve)
            .insert_series(fixing_series)
    }

    #[test]
    fn tranche_rate_uses_fixing_when_date_before_as_of_and_fixings_exist() {
        let market = sample_market_with_fixings();
        let date = date!(2025 - 02 - 01); // historical date
        let as_of = date!(2025 - 06 - 01); // valuation date well after

        // Fixing rate for 2025-02-01 is 0.025. With spread 150bp (gearing=1, floor=0):
        // calculate_floating_rate applies: (0.025 + 0.015) * 1.0 = 0.04 (gearing_includes_spread)
        let rate = tranche_all_in_rate(&floating_coupon(), date, as_of, &market);
        let expected = 0.025 + 0.015; // fixing + spread in decimal
        assert!(
            (rate - expected).abs() < 1e-10,
            "tranche should use fixing rate: got {rate}, expected {expected}"
        );

        // Fallible variant should agree
        let try_rate = try_tranche_all_in_rate(&floating_coupon(), date, as_of, &market);
        assert!(try_rate.is_ok());
        assert!((try_rate.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn tranche_rate_falls_back_to_forward_when_date_before_as_of_but_no_fixings() {
        // Market with forward curve but NO fixing series
        let market = sample_market();
        let date = date!(2025 - 02 - 01);
        let as_of = date!(2025 - 06 - 01);

        let rate = tranche_all_in_rate(&floating_coupon(), date, as_of, &market);
        // Should still get a valid forward-projected rate (not panic, not zero)
        assert!(
            rate > 0.015,
            "should fall back to forward projection: got {rate}"
        );

        // Should match the FAR_FUTURE (forward-only) rate since no fixings exist
        let forward_rate = tranche_all_in_rate(&floating_coupon(), date, FAR_FUTURE, &market);
        assert!(
            (rate - forward_rate).abs() < 1e-12,
            "fallback should match forward-only rate: got {rate}, expected {forward_rate}"
        );
    }

    #[test]
    fn asset_rate_uses_fixing_when_date_before_as_of_and_fixings_exist() {
        let market = sample_market_with_fixings();
        let date = date!(2025 - 02 - 01);
        let as_of = date!(2025 - 06 - 01);

        // Fixing is 0.025 for 2025-02-01, spread 50bp = 0.005
        let rate = asset_all_in_rate(Some("USD-SOFR-3M"), Some(50.0), 0.08, date, as_of, &market);
        let expected = 0.025 + 0.005;
        assert!(
            (rate - expected).abs() < 1e-10,
            "asset should use fixing rate: got {rate}, expected {expected}"
        );

        // Fallible variant should agree
        let try_rate = try_asset_all_in_rate(Some("USD-SOFR-3M"), Some(50.0), date, as_of, &market);
        assert!(try_rate.is_ok());
        assert!((try_rate.unwrap() - expected).abs() < 1e-10);
    }

    #[test]
    fn asset_rate_falls_back_to_forward_when_date_before_as_of_but_no_fixings() {
        // Market with forward curve but NO fixing series
        let market = sample_market();
        let date = date!(2025 - 04 - 01);
        let as_of = date!(2025 - 06 - 01);

        let rate = asset_all_in_rate(Some("USD-SOFR-3M"), Some(50.0), 0.08, date, as_of, &market);
        assert!(
            rate > 0.0,
            "should fall back to forward projection: got {rate}"
        );

        // Should match the FAR_FUTURE (forward-only) rate since no fixings exist
        let forward_rate = asset_all_in_rate(
            Some("USD-SOFR-3M"),
            Some(50.0),
            0.08,
            date,
            FAR_FUTURE,
            &market,
        );
        assert!(
            (rate - forward_rate).abs() < 1e-12,
            "fallback should match forward-only rate: got {rate}, expected {forward_rate}"
        );
    }
}
