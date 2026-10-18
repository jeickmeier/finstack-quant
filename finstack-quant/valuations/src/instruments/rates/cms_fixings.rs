//! Historical CMS (par swap rate) fixing lookups for seasoned CMS trades.
//!
//! Mirrors the cap/floor seasoned-fixing pattern: a coupon whose fixing date
//! lies before the valuation date must be valued off the recorded fixing, not
//! re-projected from the live curve (which produces phantom P&L every time
//! the curve moves). CMS fixings are par swap rates, so the series is keyed
//! by reference tenor as well as the projection curve — see
//! [`finstack_quant_core::market_data::fixings::cms_fixing_series_id`].

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::cms_fixing_series_id;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Look up the recorded CMS (par swap) rate fixed on `fixing_date`.
///
/// # Errors
///
/// Returns a validation error when the fixing series
/// `FIXING:CMS-{tenor}:{forward_curve_id}` is absent from the market context
/// or has no observation on the exact fixing date. Seasoned CMS coupons must
/// never silently fall back to live-curve projection.
pub(crate) fn historical_cms_fixing(
    curves: &MarketContext,
    forward_curve_id: &CurveId,
    cms_tenor_years: f64,
    fixing_date: Date,
) -> Result<f64> {
    let fixings_id = cms_fixing_series_id(forward_curve_id.as_str(), cms_tenor_years);
    let series = curves.get_series(&fixings_id).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "Seasoned CMS coupon requires historical fixing series '{}' for fixing date {}. \
             Fixed-but-unpaid CMS coupons must be valued off observed swap-rate fixings, \
             not the live forward curve.",
            fixings_id, fixing_date
        ))
    })?;
    series.value_on_exact(fixing_date).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "Missing CMS fixing in series '{}' on {}: {e}",
            fixings_id, fixing_date
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use time::macros::date;

    #[test]
    fn missing_series_is_a_hard_error() {
        let ctx = MarketContext::new();
        let err =
            historical_cms_fixing(&ctx, &CurveId::new("USD-SOFR"), 10.0, date!(2025 - 01 - 02))
                .expect_err("missing series must error");
        let msg = err.to_string();
        assert!(msg.contains("FIXING:CMS-10Y:USD-SOFR"), "{msg}");
    }

    #[test]
    fn exact_date_lookup_returns_recorded_rate() {
        let series = ScalarTimeSeries::new(
            "FIXING:CMS-10Y:USD-SOFR",
            vec![(date!(2025 - 01 - 02), 0.0412)],
            None,
        )
        .expect("series");
        let ctx = MarketContext::new().insert_series(series);
        let rate =
            historical_cms_fixing(&ctx, &CurveId::new("USD-SOFR"), 10.0, date!(2025 - 01 - 02))
                .expect("fixing");
        assert!((rate - 0.0412).abs() < 1e-12);

        let err =
            historical_cms_fixing(&ctx, &CurveId::new("USD-SOFR"), 10.0, date!(2025 - 01 - 03))
                .expect_err("unobserved date must error");
        assert!(err.to_string().contains("2025-01-03"));
    }
}
