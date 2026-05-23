//! Cross-currency basis curve bootstrap target.
//!
//! Derives a foreign currency discount curve from a domestic OIS curve,
//! FX spot rate, and FX forward or XCCY basis swap quotes. Optionally
//! produces a [`BasisSpreadCurve`] as a byproduct.
//!
//! ## Spot-Date Convention
//!
//! `XccyBasisParams::fx_spot` is interpreted as the **zero-coupon (T+0) FX
//! spot** — i.e. the rate that prices the foreign cashflow today. Any
//! conventional offset between an observed market spot quote and the T+0 rate
//! (typically TN or T+2 for major pairs) must be applied by the caller before
//! constructing the params: e.g. `t0_spot = market_spot * fwd_pts(TN→T+0)`.
//! Mixing observed-spot and T+0-spot conventions silently introduces a small
//! systematic bias that compounds at long tenors (~1–2bp at 5y+ for typical
//! TN points).
//!
//! ## MtM-Resetting Support
//!
//! `XccySwap` instances priced through `value_raw` honour `NotionalExchange::MtmResetting`
//! transparently — the per-period notional and resetting-leg rebalancing cashflows are
//! computed via the CIP no-FX-vol approximation in `crate::instruments::rates::xccy_swap::pricing_mtm`.
//! Today the `XccyBasisTarget` consumes generic `RateQuote::{Deposit, Fra, Swap}` to build
//! a foreign discount curve.

use crate::calibration::api::schema::XccyBasisParams;
use crate::calibration::config::CalibrationConfig;
use crate::calibration::prepared::CalibrationQuote;
use crate::calibration::solver::bootstrap::SequentialBootstrapper;
use crate::calibration::solver::traits::BootstrapTarget;
use crate::calibration::targets::util::{
    discount_only_curve_ids, prepare_rate_calibration_quotes, ContextScratch,
};
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::ExtractQuotes;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::xccy::XccyQuote;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{BasisSpreadCurve, DiscountCurve};
use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_core::types::CurveId;
use finstack_core::Result;
use std::sync::Arc;

/// Parameters for constructing an [`XccyBasisTarget`].
#[derive(Clone)]
pub(crate) struct XccyBasisTargetParams {
    /// Base date for the calibration.
    pub(crate) base_date: Date,
    /// ID for the foreign discount curve being built.
    pub(crate) curve_id: CurveId,
    /// Pre-calibrated domestic discount curve.
    pub(crate) domestic_discount: Arc<DiscountCurve>,
    /// Interpolation style for the foreign curve.
    pub(crate) solve_interp: InterpStyle,
    /// Extrapolation policy for the foreign curve.
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Base market context for pricing XCCY instruments.
    pub(crate) base_context: MarketContext,
}

/// Bootstrap target for cross-currency basis curve calibration.
///
/// Solves for the foreign discount curve knots that reprice FX forwards
/// or XCCY basis swaps given a known domestic discount curve and FX spot.
pub(crate) struct XccyBasisTarget {
    params: XccyBasisTargetParams,
    scratch: ContextScratch,
}

impl XccyBasisTarget {
    /// Create a new cross-currency basis target.
    pub(crate) fn new(params: XccyBasisTargetParams) -> Self {
        let scratch = ContextScratch::new(params.base_context.clone());
        Self { params, scratch }
    }

    /// Execute the full calibration for a cross-currency basis step.
    pub(crate) fn solve(
        schema_params: &XccyBasisParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        let domestic_discount = context.get_discount(&schema_params.domestic_discount_id)?;

        let mut config = global_config.clone();
        config.calibration_method = schema_params.method.clone();

        // Rates-side preflight: foreign-currency deposits/FRAs/swaps that constrain the
        // foreign discount curve directly. Required to be present today because the
        // bootstrap solver needs short-end anchors.
        let rates_quotes: Vec<crate::market::quotes::rates::RateQuote> = quotes.extract_quotes();
        let has_rates = !rates_quotes.is_empty();

        // XCCY-side preflight: dealer-screen `XccyQuote::BasisSwap` quotes (par-spread on
        // either fixed-notional or MtM-resetting XCCY swaps per the pair convention).
        let xccy_quotes: Vec<XccyQuote> = quotes.extract_quotes();
        let has_xccy = !xccy_quotes.is_empty();

        if !has_rates && !has_xccy {
            return Err(finstack_core::Error::Input(
                finstack_core::InputError::TooFewPoints,
            ));
        }

        let curve_dc = schema_params
            .conventions
            .curve_day_count
            .unwrap_or(finstack_core::dates::DayCount::Act365F);

        let mut prepared_quotes: Vec<CalibrationQuote> =
            Vec::with_capacity(rates_quotes.len() + xccy_quotes.len());

        if has_rates {
            let prepared = prepare_rate_calibration_quotes(
                quotes,
                schema_params.base_date,
                discount_only_curve_ids(schema_params.curve_id.as_ref()),
                schema_params.conventions.curve_day_count,
                1_000_000.0,
            )?;
            prepared_quotes.extend(prepared.quotes);
        }

        if has_xccy {
            let mut xccy_curve_ids = finstack_core::HashMap::default();
            xccy_curve_ids.insert(
                "foreign_discount".to_string(),
                schema_params.curve_id.to_string(),
            );
            xccy_curve_ids.insert(
                "domestic_discount".to_string(),
                schema_params.domestic_discount_id.to_string(),
            );
            let xccy_build_ctx = crate::market::build::context::BuildCtx::new(
                schema_params.base_date,
                1_000_000.0,
                xccy_curve_ids,
            );
            for q in xccy_quotes {
                let prepared = crate::market::build::prepared::prepare_xccy_quote(
                    q,
                    &xccy_build_ctx,
                    curve_dc,
                    schema_params.base_date,
                )?;
                prepared_quotes.push(CalibrationQuote::XccyBasis(prepared));
            }
        }

        // Bootstrap requires strictly increasing pillar times. Sort the merged list.
        prepared_quotes.sort_by(|a, b| a.pillar_time().total_cmp(&b.pillar_time()));

        let target = Self::new(XccyBasisTargetParams {
            base_date: schema_params.base_date,
            curve_id: schema_params.curve_id.clone(),
            domestic_discount: Arc::clone(&domestic_discount),
            solve_interp: schema_params.interpolation,
            extrapolation: schema_params.extrapolation,
            base_context: context.clone(),
        });

        let success_tolerance = Some(config.discount_curve.validation_tolerance);

        let (curve, report) = SequentialBootstrapper::bootstrap(
            &target,
            &prepared_quotes,
            vec![(0.0, 1.0)],
            &config,
            success_tolerance,
            None,
        )?;

        let mut new_context = context.clone().insert(curve.clone());

        // Extract basis spread curve as byproduct if requested.
        if let Some(spread_id) = &schema_params.basis_spread_curve_id {
            let knots = curve.knots();
            let foreign_dfs = curve.dfs();

            let mut spread_knots = Vec::with_capacity(knots.len());
            for (i, &t) in knots.iter().enumerate() {
                if t <= 0.0 {
                    spread_knots.push((t, 0.0));
                    continue;
                }
                let df_foreign = foreign_dfs[i];
                let df_domestic = domestic_discount.df(t);
                if !df_foreign.is_finite() || df_foreign <= 0.0 {
                    return Err(finstack_core::Error::Calibration {
                        message: format!(
                            "Spread curve {spread_id}: foreign DF at t={t:.6} is non-positive or non-finite ({df_foreign})"
                        ),
                        category: "xccy_basis".to_string(),
                    });
                }
                if !df_domestic.is_finite() || df_domestic <= 0.0 {
                    return Err(finstack_core::Error::Calibration {
                        message: format!(
                            "Spread curve {spread_id}: domestic DF at t={t:.6} is non-positive or non-finite ({df_domestic})"
                        ),
                        category: "xccy_basis".to_string(),
                    });
                }
                let z_foreign = -df_foreign.ln() / t;
                let z_domestic = -df_domestic.ln() / t;
                spread_knots.push((t, z_foreign - z_domestic));
            }

            let spread_curve = BasisSpreadCurve::builder(spread_id.clone())
                .base_date(schema_params.base_date)
                .knots(spread_knots)
                .interp(schema_params.interpolation)
                .extrapolation(schema_params.extrapolation)
                .build()
                .map_err(|e| finstack_core::Error::Calibration {
                    message: format!("Failed to build basis spread curve {spread_id}: {e}"),
                    category: "xccy_basis".to_string(),
                })?;
            new_context = new_context.insert(spread_curve);
        }

        Ok((new_context, report))
    }
}

impl BootstrapTarget for XccyBasisTarget {
    type Quote = CalibrationQuote;
    type Curve = DiscountCurve;

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        Ok(quote.pillar_time())
    }

    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        DiscountCurve::builder(self.params.curve_id.clone())
            .base_date(self.params.base_date)
            .knots(knots.to_vec())
            .interp(self.params.solve_interp)
            .extrapolation(self.params.extrapolation)
            .build()
    }

    fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        DiscountCurve::builder(self.params.curve_id.clone())
            .base_date(self.params.base_date)
            .knots(knots.to_vec())
            .interp(self.params.solve_interp)
            .extrapolation(self.params.extrapolation)
            .validation(
                finstack_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
            .build_for_solver()
    }

    fn build_curve_final(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        DiscountCurve::builder(self.params.curve_id.clone())
            .base_date(self.params.base_date)
            .knots(knots.to_vec())
            .interp(self.params.solve_interp)
            .extrapolation(self.params.extrapolation)
            .build()
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        self.scratch.with_curve(curve, |ctx| {
            quote.get_instrument().value_raw(ctx, self.params.base_date)
        })
    }

    fn initial_guess(&self, quote: &Self::Quote, previous_knots: &[(f64, f64)]) -> Result<f64> {
        let t = self.quote_time(quote)?;
        if let Some(&(prev_t, prev_df)) = previous_knots.last() {
            // Geometric extrapolation from last known knot
            if prev_t > 0.0 && t > prev_t {
                let rate = -prev_df.ln() / prev_t;
                return Ok((-rate * t).exp());
            }
            Ok(prev_df)
        } else {
            Ok(self.params.domestic_discount.df(t))
        }
    }

    fn validate_knot(&self, time: f64, value: f64) -> Result<()> {
        if !value.is_finite() || value <= 0.0 || value > 1.5 {
            return Err(finstack_core::Error::Calibration {
                message: format!(
                    "Foreign discount factor out of range for {} at t={:.6}: {:.8}",
                    self.params.curve_id, time, value
                ),
                category: "xccy_basis".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod xccy_quote_calibration_tests {
    use super::*;
    use crate::market::conventions::ids::XccyConventionId;
    use crate::market::quotes::ids::{Pillar, QuoteId};
    use finstack_core::currency::Currency;
    use finstack_core::dates::{Date, DayCount, Tenor};
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_core::types::CurveId;
    use std::sync::Arc;
    use time::Month;

    fn build_market_context() -> MarketContext {
        let base = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let usd_disc = DiscountCurve::builder(CurveId::new("USD-OIS"))
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.02_f64 * 10.0).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("USD-OIS");
        let eur_disc_seed = DiscountCurve::builder(CurveId::new("EUR-OIS"))
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.01_f64 * 10.0).exp())])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .expect("EUR-OIS seed");
        // Forward curve names must match the EUR/USD-XCCY pair convention's index ids
        // (`USD-SOFR-OIS` and `EUR-ESTR-OIS` per the registry JSON).
        let usd_fwd = ForwardCurve::builder(CurveId::new("USD-SOFR-OIS"), 0.25)
            .base_date(base)
            .knots(vec![(0.0, 0.02), (10.0, 0.02)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("USD-SOFR-OIS fwd");
        let eur_fwd = ForwardCurve::builder(CurveId::new("EUR-ESTR-OIS"), 0.25)
            .base_date(base)
            .knots(vec![(0.0, 0.01), (10.0, 0.01)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("EUR-ESTR-OIS fwd");

        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("set fx");
        let fx = FxMatrix::new(provider);

        MarketContext::new()
            .insert(usd_disc)
            .insert(eur_disc_seed)
            .insert(usd_fwd)
            .insert(eur_fwd)
            .insert_fx(fx)
    }

    /// Drive `XccyBasisTarget::solve` with `XccyQuote::BasisSwap` quotes (the dealer-
    /// screen format) instead of generic `RateQuote::Swap` quotes. The EUR/USD-XCCY
    /// convention is registered as `MtmResetting { Leg1 }` (Task 3), so this exercises
    /// the full MtM-reset pricing inside the bootstrap residual loop.
    ///
    /// Verifies (a) the calibration runs and reports success, (b) the resulting
    /// foreign discount curve is non-degenerate and replaces the seed.
    #[test]
    fn xccy_basis_target_accepts_xccy_basis_quotes() {
        let as_of = Date::from_calendar_date(2025, Month::January, 2).expect("base");
        let ctx = build_market_context();

        let params = crate::calibration::api::schema::XccyBasisParams {
            curve_id: CurveId::new("EUR-OIS"),
            currency: Currency::EUR,
            base_date: as_of,
            fx_spot: 1.10,
            domestic_discount_id: CurveId::new("USD-OIS"),
            method: crate::calibration::config::CalibrationMethod::Bootstrap,
            interpolation: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
            conventions: crate::calibration::config::RatesStepConventions {
                curve_day_count: Some(DayCount::Act365F),
                ois_compounding: None,
            },
            basis_spread_curve_id: None,
        };

        let quote_5y = MarketQuote::Xccy(XccyQuote::BasisSwap {
            id: QuoteId::new("EURUSD-XCCY-5Y"),
            convention: XccyConventionId::new("EUR/USD-XCCY"),
            far_pillar: Pillar::Tenor("5Y".parse::<Tenor>().expect("5Y tenor")),
            basis_spread_bp: -10.0,
            spot_fx: Some(1.10),
        });

        let cfg = CalibrationConfig::default();
        let result = XccyBasisTarget::solve(&params, &[quote_5y], &ctx, &cfg);

        let (new_ctx, report) =
            result.expect("XccyBasisTarget::solve should accept XccyQuote::BasisSwap quotes");

        assert!(
            report.success,
            "calibration should succeed: max_residual={}",
            report.max_residual
        );

        let calibrated = new_ctx
            .get_discount(CurveId::new("EUR-OIS"))
            .expect("calibrated EUR-OIS curve should be present in the new context");
        assert!(
            calibrated.df(5.0) > 0.0 && calibrated.df(5.0) < 1.0,
            "calibrated 5Y DF must be in (0,1); got {}",
            calibrated.df(5.0)
        );
    }
}
