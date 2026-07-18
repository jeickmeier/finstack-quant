//! Shared inflation curve bumping logic.

use super::currency::infer_currency_from_id;
use super::BumpRequest;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::InflationCurve;

use finstack_quant_core::dates::Date;

/// Infer currency from an inflation curve ID using token-by-token heuristics.
///
/// Best-effort fallback for callers that don't have explicit currency metadata.
/// Returns USD if no known currency or benchmark-rate token appears in the ID.
///
/// # Arguments
///
/// * `curve` - Inflation curve whose identifier is tokenized for an inferred
///   currency when explicit metadata is unavailable.
pub fn infer_currency_from_curve_id(curve: &InflationCurve) -> Currency {
    infer_currency_from_id(curve.id().as_str())
}

/// Derive the observation lag string from the curve's `indexation_lag_months`.
///
/// Returns `"NONE"` when the lag is 0, otherwise formats as `"{n}M"`.
///
/// # Arguments
///
/// * `curve` - Inflation curve whose integer indexation lag is represented in
///   the calibration schema's canonical string form.
pub fn observation_lag_from_curve(curve: &InflationCurve) -> String {
    let months = curve.indexation_lag_months();
    if months == 0 {
        "NONE".to_string()
    } else {
        format!("{months}M")
    }
}

/// Bump an inflation curve by shifting its implied zero-coupon inflation rates.
///
/// The shock is applied directly on the curve's own knots. For each knot time
/// `t` the implied zero-coupon inflation rate is recovered and shifted, then
/// the CPI level is rebuilt from the shifted rate:
///
/// ```text
/// r(t)     = (CPI(t) / CPI_base)^(1/t) − 1
/// CPI'(t)  = CPI_base · (1 + r(t) + δ)^t,        δ = bp / 10_000
/// ```
///
/// # Guarantees
///
/// * **Zero-shock identity** — with `δ = 0` the reconstruction collapses to
///   `CPI_base · ((CPI(t)/CPI_base)^(1/t))^t = CPI(t)`, reproducing the curve to
///   floating-point round-off at every knot.
/// * **Faithfulness** — an `X bp` parallel bump moves the implied zero-coupon
///   inflation rate by exactly `X bp` at every knot.
/// * **Additivity** — `+X bp` followed by `−X bp` returns to the base curve.
///
/// # Why not imply-and-re-bootstrap
///
/// This function previously implied ZCIS quotes, placed them at
/// `base + round(t · 365.25)` days, and re-ran the inflation bootstrapper. That
/// round trip was not the identity — the synthetic maturity grid did not map
/// back to the original knot times, and the bootstrapper applied its own
/// observation-lag, day-count, and schedule conventions that the implied rate
/// never accounted for. A `0 bp` shock moved `CPI(1y)` from 307.50 to 309.57
/// (+0.67%), and a 25 bp shock realized 101 bp. This mirrors the defect that
/// affected [`bump_discount_curve_synthetic`](super::bump_discount_curve_synthetic);
/// the fix is the same — remove the round trip rather than reconcile three
/// separate convention sets.
///
/// # Arguments
///
/// * `curve` - Existing inflation curve whose knots are shifted in implied
///   zero-coupon inflation-rate space.
/// * `_context` - Unused. Retained for signature compatibility with the other
///   bump entry points; no calibration step runs any more.
/// * `bump` - Parallel or tenor-specific inflation-rate shock in
///   [`BumpRequest`] basis point units.
/// * `_discount_id` - Unused. No zero-coupon inflation swaps are present-valued
///   any more.
/// * `_as_of` - Unused. The shift is applied on the curve's own time grid.
/// * `_currency` - Unused. No inflation swap conventions are selected any more.
/// * `_observation_lag` - Unused. The curve's own indexation lag is preserved
///   by the rebuild.
///
/// # Errors
///
/// Returns an error when the shifted knots fail inflation-curve validation.
pub fn bump_inflation_rates(
    curve: &InflationCurve,
    _context: &MarketContext,
    bump: &BumpRequest,
    _discount_id: &finstack_quant_core::types::CurveId,
    _as_of: Date,
    _currency: Currency,
    _observation_lag: &str,
) -> finstack_quant_core::Result<InflationCurve> {
    let base_cpi = curve.base_cpi();
    let knots = curve.knots();
    let cpi_levels = curve.cpi_levels();

    let bumped: Vec<(f64, f64)> = knots
        .iter()
        .zip(cpi_levels.iter())
        .map(|(&t, &cpi)| {
            // The t <= 0 anchor carries the base CPI level and no rate
            // information, so it passes through untouched.
            if t <= 0.0 {
                return (t, cpi);
            }

            let shift = match bump {
                BumpRequest::Parallel(bp) => bp * 1e-4,
                BumpRequest::Tenors(targets) => targets
                    .iter()
                    // 0.1 year tolerance, matching the previous selection rule.
                    .filter(|(target_t, _)| (t - *target_t).abs() < 0.1)
                    .map(|(_, bp)| bp * 1e-4)
                    .sum(),
            };

            let implied_rate = (cpi / base_cpi).powf(1.0 / t) - 1.0;
            (t, base_cpi * (1.0 + implied_rate + shift).powf(t))
        })
        .collect();

    InflationCurve::builder(curve.id().clone())
        .base_cpi(base_cpi)
        .base_date(curve.base_date())
        .day_count(curve.day_count())
        .indexation_lag_months(curve.indexation_lag_months())
        .interp(curve.interp_style())
        .extrapolation(curve.extrapolation())
        .knots(bumped)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::types::CurveId;
    use time::macros::date;

    fn sample_date() -> Date {
        date!(2025 - 01 - 01)
    }

    fn sample_curve(id: &str, lag_months: u32) -> finstack_quant_core::Result<InflationCurve> {
        InflationCurve::builder(id)
            .base_date(sample_date())
            .base_cpi(300.0)
            .indexation_lag_months(lag_months)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .build()
    }

    fn non_positive_knot_curve(
        id: &str,
        lag_months: u32,
    ) -> finstack_quant_core::Result<InflationCurve> {
        InflationCurve::builder(id)
            .base_date(sample_date())
            .base_cpi(300.0)
            .indexation_lag_months(lag_months)
            .knots([(-1.0, 294.0), (0.0, 300.0)])
            .build()
    }

    #[test]
    fn infer_currency_prefers_known_curve_id_markers() {
        let usd_curve = sample_curve("USD-CPI", 3);
        let eur_curve = sample_curve("EUR-HICP", 3);
        let gbp_curve = sample_curve("GBP-RPI", 3);

        assert!(usd_curve.is_ok(), "USD sample curve should build");
        assert!(eur_curve.is_ok(), "EUR sample curve should build");
        assert!(gbp_curve.is_ok(), "GBP sample curve should build");

        if let Ok(curve) = usd_curve {
            assert_eq!(infer_currency_from_curve_id(&curve), Currency::USD);
        }
        if let Ok(curve) = eur_curve {
            assert_eq!(infer_currency_from_curve_id(&curve), Currency::EUR);
        }
        if let Ok(curve) = gbp_curve {
            assert_eq!(infer_currency_from_curve_id(&curve), Currency::GBP);
        }
    }

    #[test]
    fn infer_currency_defaults_to_usd_for_unknown_ids() {
        let curve = sample_curve("CA-CPI", 3);
        assert!(curve.is_ok(), "fallback sample curve should build");
        if let Ok(curve) = curve {
            assert_eq!(infer_currency_from_curve_id(&curve), Currency::USD);
        }
    }

    #[test]
    fn observation_lag_formats_zero_and_non_zero_months() {
        let no_lag_curve = sample_curve("USD-CPI", 0);
        let three_month_curve = sample_curve("USD-CPI", 3);
        let one_year_curve = sample_curve("USD-CPI", 12);

        assert!(no_lag_curve.is_ok(), "zero-lag sample curve should build");
        assert!(
            three_month_curve.is_ok(),
            "three-month lag sample curve should build"
        );
        assert!(
            one_year_curve.is_ok(),
            "twelve-month lag sample curve should build"
        );

        if let Ok(curve) = no_lag_curve {
            assert_eq!(observation_lag_from_curve(&curve), "NONE");
        }
        if let Ok(curve) = three_month_curve {
            assert_eq!(observation_lag_from_curve(&curve), "3M");
        }
        if let Ok(curve) = one_year_curve {
            assert_eq!(observation_lag_from_curve(&curve), "12M");
        }
    }

    #[test]
    fn bump_inflation_rates_returns_clone_when_curve_has_only_base_knot() {
        let curve = non_positive_knot_curve("USD-CPI", 3);
        assert!(curve.is_ok(), "base-knot-only sample curve should build");

        if let Ok(curve) = curve {
            let bumped = bump_inflation_rates(
                &curve,
                &MarketContext::new(),
                &BumpRequest::Parallel(10.0),
                &CurveId::new("USD-OIS"),
                sample_date(),
                Currency::USD,
                "3M",
            );
            assert!(
                bumped.is_ok(),
                "base-knot-only curve should bypass recalibration"
            );

            if let Ok(bumped) = bumped {
                assert_eq!(bumped.id(), curve.id());
                assert_eq!(bumped.base_cpi(), curve.base_cpi());
                assert_eq!(bumped.knots(), curve.knots());
                assert_eq!(
                    bumped.indexation_lag_months(),
                    curve.indexation_lag_months()
                );
            }
        }
    }
}
