//! Shared inflation curve bumping logic.

use super::currency::infer_currency_from_id;
use super::BumpRequest;
use crate::calibration::api::schema::{InflationCurveParams, StepParams};
use crate::calibration::config::CalibrationMethod;
use crate::calibration::step_runtime;
use crate::calibration::CalibrationConfig;
use crate::market::conventions::ids::InflationSwapConventionId;
use crate::market::quotes::ids::QuoteId;
use crate::market::quotes::inflation::InflationQuote;
use crate::market::quotes::market_quote::MarketQuote;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::InflationCurve;

use finstack_core::dates::Date;

/// Infer currency from an inflation curve ID using token-by-token heuristics.
///
/// Best-effort fallback for callers that don't have explicit currency metadata.
/// Returns USD if no known currency or benchmark-rate token appears in the ID.
pub fn infer_currency_from_curve_id(curve: &InflationCurve) -> Currency {
    infer_currency_from_id(curve.id().as_str())
}

/// Derive the observation lag string from the curve's `indexation_lag_months`.
///
/// Returns `"NONE"` when the lag is 0, otherwise formats as `"{n}M"`.
pub fn observation_lag_from_curve(curve: &InflationCurve) -> String {
    let months = curve.indexation_lag_months();
    if months == 0 {
        "NONE".to_string()
    } else {
        format!("{months}M")
    }
}

/// Bump inflation curve by shocking implied zero-coupon swap rates and re-calibrating.
///
/// Converts the current inflation curve back to implied ZCIS rates,
/// applies shocks to those rates, and re-executes the inflation bootstrapper.
///
/// # Arguments
/// * `currency` - Currency of the inflation index (required; InflationCurve does not carry currency metadata).
/// * `observation_lag` - Observation lag string (e.g. "3M", "NONE") matching the original calibration.
pub fn bump_inflation_rates(
    curve: &InflationCurve,
    context: &MarketContext,
    bump: &BumpRequest,
    discount_id: &finstack_core::types::CurveId,
    as_of: Date,
    currency: Currency,
    observation_lag: &str,
) -> finstack_core::Result<InflationCurve> {
    let base_date = as_of;
    let curve_id = curve.id();

    // Map currency to standard inflation convention ID
    let convention_id = match currency {
        Currency::USD => InflationSwapConventionId::new("USD-CPI"),
        Currency::EUR => InflationSwapConventionId::new("EUR-HICP"),
        Currency::GBP => InflationSwapConventionId::new("UK-RPI"),
        _ => InflationSwapConventionId::new(format!("{}-CPI", currency)), // Best guess fallback
    };

    let base_cpi = curve.base_cpi();
    let knots = curve.knots(); // time in years

    let mut quotes = Vec::new();

    for &t in knots {
        if t <= 0.0 {
            continue;
        } // Skip base point

        let cpi = curve.cpi(t);
        // Implied zero-coupon rate: (CPI(T) / Base)^(1/T) - 1
        let implied_rate = (cpi / base_cpi).powf(1.0 / t) - 1.0;

        let mut bumped_rate = implied_rate;

        // Apply bump
        match bump {
            BumpRequest::Parallel(bp) => {
                bumped_rate += bp * 1e-4;
            }
            BumpRequest::Tenors(targets) => {
                for (target_t, bp) in targets {
                    // 0.1 year tolerance
                    if (t - *target_t).abs() < 0.1 {
                        bumped_rate += bp * 1e-4;
                    }
                }
            }
        }

        let maturity_days = (t * 365.25).round() as i64;
        let maturity = base_date + time::Duration::days(maturity_days);

        let id = QuoteId::new(format!("{}-ZCIS-{}", curve_id.as_str(), maturity));
        quotes.push(InflationQuote::InflationSwap {
            id,
            maturity,
            rate: bumped_rate,
            index: curve_id.as_str().to_string(), // Use curve ID as index name? Or generic?
            convention: convention_id.clone(),
        });
    }

    if quotes.is_empty() {
        // No knots to bump? return clone
        return Ok(curve.clone());
    }

    let market_quotes: Vec<MarketQuote> = quotes.into_iter().map(MarketQuote::Inflation).collect();
    let params = InflationCurveParams {
        curve_id: curve_id.clone(),
        currency,
        base_date,
        discount_curve_id: discount_id.clone(),
        index: curve_id.as_str().to_string(),
        observation_lag: observation_lag.to_string(),
        base_cpi,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        seasonal_factors: None,
    };

    // Re-calibration uses the default CalibrationConfig — see the "Calibration
    // config — known limitation" note in this module's docs (`bumps/mod.rs`).
    let cfg = CalibrationConfig::default();
    let step = StepParams::Inflation(params.clone());
    let (ctx, _report) =
        step_runtime::execute_params_and_apply(&step, &market_quotes, context, &cfg)?;
    Ok(ctx
        .get_inflation_curve(params.curve_id.as_str())?
        .as_ref()
        .clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::Date;
    use finstack_core::types::CurveId;
    use time::macros::date;

    fn sample_date() -> Date {
        date!(2025 - 01 - 01)
    }

    fn sample_curve(id: &str, lag_months: u32) -> finstack_core::Result<InflationCurve> {
        InflationCurve::builder(id)
            .base_date(sample_date())
            .base_cpi(300.0)
            .indexation_lag_months(lag_months)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .build()
    }

    fn non_positive_knot_curve(id: &str, lag_months: u32) -> finstack_core::Result<InflationCurve> {
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
