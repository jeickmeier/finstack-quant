//! Student-t degrees of freedom calibration for credit portfolio models.
//!
//! Calibrates the `df` (degrees of freedom) parameter of a Student-t copula
//! by repricing market tranche upfront quotes. Uses Brent root-finding to
//! solve for zero NPV of the tranche including its settlement-dated upfront.
//!
//! # Mathematical Background
//!
//! The Student-t copula introduces tail dependence -- the tendency for joint
//! defaults to cluster during market stress more than Gaussian correlation
//! predicts. The degrees of freedom parameter `df` controls the severity of
//! this clustering: lower `df` means heavier tails and more tail dependence.
//!
//! This calibration target finds the `df` value that, when combined with a
//! pre-calibrated base correlation curve, best reproduces the observed tranche
//! upfront quote.
//!
//! # Calibration Algorithm
//!
//! 1. For each candidate `df`, construct a `StudentTCopula(df)`
//! 2. Price the reference tranche, including the quoted upfront at cash settlement
//! 3. Normalize its NPV by tranche notional (a discounted upfront fraction)
//! 4. Solve inside the requested `df` bounds and require the returned parameter
//!    to reprice within 10 bp of tranche notional
//!
//! # References
//!
//! - Demarta, S., & McNeil, A. J. (2005). "The t Copula and Related Copulas." `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//! - Hull, J., Predescu, M., & White, A. (2005). "The valuation of correlation-
//!   dependent credit derivatives using a structural model."

use crate::api::schema::StudentTParams;
use crate::build::cds_tranche::{build_cds_tranche_instrument, CdsTrancheBuildOverrides};
use crate::build::BuildCtx;
use crate::config::CalibrationConfig;
use crate::quotes::market_quote::MarketQuote;
use crate::solver::helpers::bracket_solve_1d_nearest_first_with_diagnostics;
use crate::CalibrationReport;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CdsTranche, CdsTranchePricer, CdsTranchePricerConfig,
};
use std::collections::BTreeMap;

/// Calibrator for Student-t copula degrees of freedom from tranche quotes.
///
/// Searches over the `df` parameter space to match observed tranche upfront
/// quotes using Brent root-finding. The calibrated `df` is stored in the
/// market context as a `MarketScalar::Unitless` under a key derived from
/// the step configuration.
pub(crate) struct StudentTTarget {
    /// Parameters defining the calibration structure.
    pub(crate) params: StudentTParams,
    /// Baseline market context used when pricing trial copula configurations.
    pub(crate) base_context: MarketContext,
    /// Global calibration settings (solver controls).
    pub(crate) config: CalibrationConfig,
}

impl StudentTTarget {
    /// Validate the scalar search domain and fixed asset correlation.
    ///
    /// # Arguments
    ///
    /// * `params` - Degrees-of-freedom bounds and initial guess, with a finite
    ///   asset correlation in `[0, 1)`; the initial guess must lie inside the bounds.
    pub(crate) fn validate_params(params: &StudentTParams) -> Result<()> {
        let (lo, hi) = params.df_bounds;
        if !lo.is_finite() || !hi.is_finite() || lo <= 2.0 || lo >= hi {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t df_bounds must satisfy 2.0 < lo < hi; got ({lo}, {hi})"
            )));
        }
        if !params.initial_df.is_finite() || !(lo..=hi).contains(&params.initial_df) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t initial_df must lie inside df_bounds [{lo}, {hi}]; got {}",
                params.initial_df
            )));
        }
        if !params.correlation.is_finite() || !(0.0..1.0).contains(&params.correlation) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t correlation must be finite and in [0, 1); got {}",
                params.correlation
            )));
        }
        Ok(())
    }

    fn resolve_discount_curve_id(
        params: &StudentTParams,
        context: &MarketContext,
    ) -> Result<CurveId> {
        if let Some(curve_id) = &params.discount_curve_id {
            return Ok(curve_id.clone());
        }
        // Preflight has already verified exactly one discount curve is present.
        let (discount_curve_id, _) =
            context.curves_of_type("Discount").next().ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: "discount curve".to_string(),
                })
            })?;
        Ok(discount_curve_id.clone())
    }

    fn flat_base_correlation_template(
        template: &finstack_quant_core::market_data::term_structures::BaseCorrelationCurve,
        correlation: f64,
    ) -> Result<finstack_quant_core::market_data::term_structures::BaseCorrelationCurve> {
        let knots: Vec<(f64, f64)> = template
            .detachment_points()
            .iter()
            .copied()
            .map(|k| (k, correlation))
            .collect();
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
            template.id().clone(),
        )
        .knots(knots)
        .build()
    }

    /// Create a calibrator after checking its scalar domain and correlation.
    ///
    /// # Arguments
    ///
    /// * `params` - Search bounds, an in-bounds initial guess and fixed correlation.
    /// * `base_context` - Discount, hazard and index curves used for trial pricing.
    /// * `config` - Solver iteration controls; fit acceptance remains 10 bp of
    ///   tranche notional in discounted upfront units.
    pub(crate) fn new(
        params: StudentTParams,
        base_context: MarketContext,
        config: CalibrationConfig,
    ) -> Result<Self> {
        Self::validate_params(&params)?;
        Ok(Self {
            params,
            base_context,
            config,
        })
    }

    /// Execute the full calibration for a Student-t df step.
    ///
    /// This is a scalar calibration: it finds a single `df` value that
    /// minimizes the pricing residual for a reference tranche, then stores
    /// the result as a `MarketScalar::Unitless` in the market context.
    ///
    /// # Returns
    ///
    /// A tuple of `(MarketContext, f64, CalibrationReport)` where:
    /// - The context contains the calibrated `df` stored under the scalar key
    ///   `"{tranche_instrument_id}_STUDENT_T_DF"`.
    /// - The extra `f64` is the calibrated degrees-of-freedom value, returned
    ///   separately because `step_runtime` needs it to build a `StepOutput::Scalar`
    ///   without parsing it back from report metadata.
    pub(crate) fn solve(
        params: &StudentTParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, f64, CalibrationReport)> {
        Self::validate_params(params)?;
        let tranche_quote = quotes
            .iter()
            .find_map(|quote| match quote {
                MarketQuote::CdsTranche(tranche_quote)
                    if tranche_quote.id().as_str() == params.tranche_instrument_id =>
                {
                    Some(tranche_quote.clone())
                }
                _ => None,
            })
            .ok_or_else(|| {
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: format!("CDS tranche quote '{}'", params.tranche_instrument_id),
                })
            })?;

        let index_id = tranche_quote.index.clone();
        let attachment = tranche_quote.attachment;
        let detachment = tranche_quote.detachment;

        let base_correlation_curve =
            context.get_base_correlation(&params.base_correlation_curve_id)?;
        let flat_base_correlation = Self::flat_base_correlation_template(
            base_correlation_curve.as_ref(),
            params.correlation,
        )?;
        let credit_index = context.get_credit_index(&index_id)?.as_ref().clone();
        let rebound_credit_index = CreditIndexData {
            base_correlation_curve: std::sync::Arc::new(flat_base_correlation.clone()),
            ..credit_index
        };
        // Keep the context-level base correlation curve in sync with the overridden
        // credit-index reference. Insertion resolves the index dependencies from
        // the context, so install the requested flat-correlation override first.
        let pricing_context = context
            .clone()
            .insert(flat_base_correlation)
            .insert_credit_index(&index_id, rebound_credit_index)?;

        let discount_curve_id = Self::resolve_discount_curve_id(params, &pricing_context)?;
        let discount_curve = pricing_context.get_discount(&discount_curve_id)?;
        let as_of = discount_curve.base_date();

        let tranche_width = detachment - attachment;
        if !tranche_width.is_finite() || tranche_width <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t calibration tranche width must be positive; attachment={attachment}, detachment={detachment}"
            )));
        }
        let mut curve_ids = finstack_quant_core::HashMap::default();
        curve_ids.insert("discount".to_string(), discount_curve_id.to_string());
        curve_ids.insert("credit".to_string(), index_id);
        let build_context = BuildCtx::new(as_of, 1.0 / tranche_width, curve_ids);
        let instrument = build_cds_tranche_instrument(
            &tranche_quote,
            &build_context,
            &CdsTrancheBuildOverrides::default(),
        )?;
        let tranche = instrument
            .as_any()
            .downcast_ref::<CdsTranche>()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "shared tranche quote builder did not return CdsTranche".to_string(),
                )
            })?
            .clone();

        let calibrator = Self::new(params.clone(), pricing_context, global_config.clone())?;
        calibrator.calibrate_df(&tranche, as_of)
    }

    /// Run the Brent root-finding calibration over the df domain.
    fn calibrate_df(
        &self,
        tranche: &CdsTranche,
        as_of: Date,
    ) -> Result<(MarketContext, f64, CalibrationReport)> {
        let (df_lo, df_hi) = self.params.df_bounds;
        let initial_df = self.params.initial_df;
        let max_iters = self.config.solver.max_iterations();

        // The residual is the complete quote-built contract's NPV divided by
        // tranche notional. The upfront cashflow is discounted at its contractual
        // settlement date by the pricer, just like the premium/protection legs.
        const STUDENT_T_UPFRONT_TOLERANCE: f64 = 1e-3;
        let acceptance_tolerance = STUDENT_T_UPFRONT_TOLERANCE;

        let template = CdsTranchePricerConfig::default();
        let price_residual = |df: f64| -> f64 {
            if !df.is_finite() || !(df_lo..=df_hi).contains(&df) {
                return f64::INFINITY;
            }
            let Ok(config) = template.clone().with_student_t_copula(df) else {
                return f64::INFINITY;
            };
            let Ok(pricer) = CdsTranchePricer::with_config(config) else {
                return f64::INFINITY;
            };
            match pricer.price_tranche(tranche, &self.base_context, as_of) {
                Ok(npv) => npv.amount() / tranche.notional.amount(),
                Err(_) => f64::INFINITY,
            }
        };

        let residual0 = price_residual(initial_df);
        let (calibrated_df, eval_count, residual) = if residual0.is_finite()
            && residual0.abs() <= acceptance_tolerance
        {
            (initial_df, 1usize, residual0)
        } else {
            let scan = self.build_scan_grid(df_lo, df_hi, initial_df);
            let (root, diagnostics) = bracket_solve_1d_nearest_first_with_diagnostics(
                &price_residual,
                initial_df,
                &scan,
                acceptance_tolerance,
                max_iters,
            )?;
            match root {
                Some(df) if df.is_finite() && (df_lo..=df_hi).contains(&df) => {
                    let residual = diagnostics
                        .best_value
                        .filter(|_| {
                            diagnostics
                                .best_point
                                .is_some_and(|point| (point - df).abs() < 1e-12)
                        })
                        .unwrap_or_else(|| price_residual(df));
                    if residual.abs() <= acceptance_tolerance {
                        (df, diagnostics.eval_count, residual)
                    } else {
                        return Err(finstack_quant_core::Error::Calibration {
                            message: format!(
                                "Student-t df calibration failed: best df={:.4} but residual {:.2e} exceeds tolerance {:.2e}",
                                df, residual.abs(), acceptance_tolerance
                            ),
                            category: "student_t_df".to_string(),
                        });
                    }
                }
                _ => {
                    let fallback_df = diagnostics.best_point.unwrap_or(initial_df);
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Student-t df calibration failed to converge (bracket_found={}, best df={:.4})",
                            diagnostics.bracket_found, fallback_df
                        ),
                        category: "student_t_df".to_string(),
                    });
                }
            }
        };

        if !residual.is_finite() || residual.abs() > acceptance_tolerance {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Student-t df calibration failed: df={calibrated_df:.4} has residual {residual:.2e}, exceeding tolerance {acceptance_tolerance:.2e}"
                ),
                category: "student_t_df".to_string(),
            });
        }
        let reason = format!(
            "Student-t df calibration converged: df={:.4}",
            calibrated_df
        );

        let mut residuals = BTreeMap::new();
        residuals.insert(
            format!("{}_df", self.params.tranche_instrument_id),
            residual,
        );

        let report = CalibrationReport::new(residuals, eval_count, true, &reason)
            .with_success_tolerance(acceptance_tolerance)
            .with_metadata("calibration_type", "student_t_df")
            .with_metadata("residual_units", "discounted_upfront_fraction")
            .with_metadata("tranche_instrument_id", &self.params.tranche_instrument_id)
            .with_metadata("calibrated_df", format!("{:.6}", calibrated_df))
            .with_metadata("df_bounds", format!("[{:.2}, {:.2}]", df_lo, df_hi))
            .with_model_version(crate::versions::STUDENT_T_COPULA);
        let mut report = report;
        report.update_solver_config(self.config.solver.clone());

        let scalar_key = format!("{}_STUDENT_T_DF", self.params.tranche_instrument_id);
        let new_context = self
            .base_context
            .clone()
            .insert_price(&scalar_key, MarketScalar::Unitless(calibrated_df));

        Ok((new_context, calibrated_df, report))
    }

    /// Build a scan grid for the Brent solver over the df domain.
    fn build_scan_grid(&self, lo: f64, hi: f64, initial: f64) -> Vec<f64> {
        let mut pts = Vec::with_capacity(64);
        pts.push(lo);
        pts.push(hi);
        pts.push(initial);

        // Linear grid.
        const N: usize = 8;
        for i in 0..=N {
            let t = i as f64 / N as f64;
            let df = lo + t * (hi - lo);
            pts.push(df);
        }

        // Extra refinement around the initial guess.
        for delta in [0.1, 0.25, 0.5, 1.0, 2.0, 5.0] {
            for sign in [-1.0, 1.0] {
                let df = initial + sign * delta;
                if df > lo && df < hi {
                    pts.push(df);
                }
            }
        }

        pts.retain(|x| x.is_finite() && (lo..=hi).contains(x));
        pts.sort_by(|a, b| a.total_cmp(b));
        pts.dedup();
        pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::{CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams};
    use crate::quotes::cds_tranche::CdsTrancheQuote;
    use crate::quotes::ids::QuoteId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::{
        BaseCorrelationCurve, DiscountCurve, HazardCurve,
    };
    use finstack_quant_core::HashMap;
    use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use std::sync::Arc;
    use time::Month;

    fn params() -> StudentTParams {
        StudentTParams {
            tranche_instrument_id: "TRANCHE-1".to_string(),
            base_correlation_curve_id: "CDX_CORR".to_string(),
            discount_curve_id: Some("USD-OIS".into()),
            initial_df: 6.0,
            df_bounds: (2.5, 12.0),
            correlation: 0.3,
        }
    }

    fn fixture_market(base_date: Date, rate: f64) -> MarketContext {
        let discount = DiscountCurve::flat("USD-OIS", base_date, rate).expect("discount curve");
        let hazard = HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(1.0, 0.001), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve");
        let correlation = BaseCorrelationCurve::builder("CDX_CORR")
            .knots([(3.0, 0.3), (7.0, 0.3)])
            .build()
            .expect("correlation curve");
        let index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(correlation.clone()))
            .build()
            .expect("credit index");
        MarketContext::new()
            .insert(discount)
            .insert(hazard)
            .insert(correlation)
            .insert_credit_index("CDX.NA.IG", index)
            .expect("credit-index dependencies")
    }

    fn fixture_quote(upfront_pct: f64) -> CdsTrancheQuote {
        CdsTrancheQuote {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX.NA.IG".to_string(),
            series: 42,
            attachment: 0.03,
            detachment: 0.07,
            maturity: Date::from_calendar_date(2030, Month::June, 20).expect("maturity"),
            upfront_pct,
            coupon_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }
    }

    fn quoted_tranche(quote: &CdsTrancheQuote, base_date: Date) -> CdsTranche {
        let curve_ids = HashMap::from_iter([
            ("discount".to_string(), "USD-OIS".to_string()),
            ("credit".to_string(), "CDX.NA.IG".to_string()),
        ]);
        let instrument = build_cds_tranche_instrument(
            quote,
            &BuildCtx::new(
                base_date,
                1.0 / (quote.detachment - quote.attachment),
                curve_ids,
            ),
            &CdsTrancheBuildOverrides::default(),
        )
        .expect("quote-built unit-notional tranche");
        instrument
            .as_any()
            .downcast_ref::<CdsTranche>()
            .expect("CDS tranche")
            .clone()
    }

    fn pricer(df: f64) -> CdsTranchePricer {
        CdsTranchePricer::with_config(
            CdsTranchePricerConfig::default()
                .with_student_t_copula(df)
                .expect("Student-t configuration"),
        )
        .expect("tranche pricer")
    }

    fn settled_quote(market: &MarketContext, base_date: Date, df: f64) -> CdsTrancheQuote {
        // The quote is cash due at settlement. Convert today's no-upfront NPV
        // to that dated cash amount independently of the calibration residual.
        let mut quote = fixture_quote(1.0);
        let mut tranche = quoted_tranche(&quote, base_date);
        let (settlement, _) = tranche.upfront.take().expect("upfront settlement");
        let settlement_df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(base_date, settlement)
            .expect("settlement discount factor");
        let model_pv = pricer(df)
            .price_tranche(&tranche, market, base_date)
            .expect("no-upfront price")
            .amount();
        quote.upfront_pct = model_pv / (tranche.notional.amount() * settlement_df);
        quote
    }

    fn student_t_envelope(
        params: StudentTParams,
        market: &MarketContext,
        quote: CdsTrancheQuote,
    ) -> CalibrationEnvelope {
        let (prior_market, mut market_data) = crate::test_support::split_market_context(market);
        market_data.push(crate::api::market_datum::MarketDatum::CdsTrancheQuote(
            quote,
        ));
        CalibrationEnvelope {
            schema_url: None,
            schema: crate::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "student-t-bounds".to_string(),
                description: None,
                quote_sets: [("quotes".to_string(), vec![QuoteId::new("TRANCHE-1")])]
                    .into_iter()
                    .collect(),
                settings: CalibrationConfig::default(),
                steps: vec![CalibrationStep {
                    id: "student".to_string(),
                    quote_set: "quotes".to_string(),
                    params: StepParams::StudentT(params),
                }],
            },
            market_data,
            prior_market,
        }
    }

    #[test]
    fn student_t_out_of_bounds_initial_guess_fails_public_preflight() {
        let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
        let market = fixture_market(base_date, 0.03);
        let quote = settled_quote(&market, base_date, 6.0);
        let mut params = params();
        params.df_bounds = (20.0, 30.0);
        let envelope = student_t_envelope(params, &market, quote);
        let error = crate::api::engine::execute(&envelope).expect_err(
            "an exact fit outside the requested domain must not be clamped and accepted",
        );
        assert!(error
            .to_string()
            .contains("initial_df must lie inside df_bounds"));
    }

    #[test]
    fn student_t_parameter_validation_rejects_invalid_domains_and_correlation() {
        for bounds in [(2.0, 12.0), (12.0, 12.0), (12.0, 2.5), (f64::NAN, 12.0)] {
            let mut invalid = params();
            invalid.df_bounds = bounds;
            assert!(StudentTTarget::validate_params(&invalid).is_err());
        }
        for initial in [2.1, 20.0, f64::NAN, f64::INFINITY] {
            let mut invalid = params();
            invalid.initial_df = initial;
            assert!(StudentTTarget::validate_params(&invalid).is_err());
        }
        for correlation in [-0.01, 1.0, f64::NAN, f64::INFINITY] {
            let mut invalid = params();
            invalid.correlation = correlation;
            assert!(StudentTTarget::validate_params(&invalid).is_err());
        }
    }

    #[test]
    fn student_t_scan_grid_stays_inside_requested_bounds() {
        let target =
            StudentTTarget::new(params(), MarketContext::new(), CalibrationConfig::default())
                .expect("valid target");
        for initial in [6.0, 20.0, f64::NAN] {
            let grid = target.build_scan_grid(2.5, 12.0, initial);
            assert!(grid
                .iter()
                .all(|df| df.is_finite() && (2.5..=12.0).contains(df)));
            assert_eq!(grid.first(), Some(&2.5));
            assert_eq!(grid.last(), Some(&12.0));
        }
    }

    #[test]
    fn student_t_narrow_domains_preserve_distinct_scan_endpoints() {
        let target =
            StudentTTarget::new(params(), MarketContext::new(), CalibrationConfig::default())
                .expect("valid target");
        for initial in [5.0, 5.01] {
            let grid = target.build_scan_grid(5.0, 5.01, initial);
            assert!(grid.len() >= 8);
            assert_eq!(grid.first(), Some(&5.0));
            assert_eq!(grid.last(), Some(&5.01));
        }
        let adjacent = f64::from_bits(5.0_f64.to_bits() + 1);
        let grid = target.build_scan_grid(5.0, adjacent, 5.0);
        assert_eq!(grid, vec![5.0, adjacent]);
    }

    #[test]
    fn student_t_narrow_domains_fail_public_calibration_without_panicking() {
        let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
        let market = fixture_market(base_date, 0.03);
        let quote = fixture_quote(0.25);
        let adjacent = f64::from_bits(5.0_f64.to_bits() + 1);
        for (upper, expected_error) in [
            (5.01, "Student-t df calibration failed"),
            (adjacent, "scan grid has"),
        ] {
            for initial in [5.0, upper] {
                let mut params = params();
                params.df_bounds = (5.0, upper);
                params.initial_df = initial;
                StudentTTarget::validate_params(&params).expect("valid narrow parameter domain");
                let initial_residual = pricer(initial)
                    .price_tranche(&quoted_tranche(&quote, base_date), &market, base_date)
                    .expect("initial full-trade price")
                    .amount();
                assert!(initial_residual.abs() > 1e-3, "the scan must run");
                let envelope = student_t_envelope(params, &market, quote.clone());
                let error = crate::api::engine::execute(&envelope)
                    .expect_err("an unreachable quote must return a checked calibration error");
                assert!(
                    error.to_string().contains(expected_error),
                    "unexpected error for [{}, {}]: {error}",
                    5.0,
                    upper
                );
            }
        }
    }

    #[test]
    fn student_t_reprices_upfront_at_cash_settlement() {
        let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
        let market = fixture_market(base_date, 0.15);
        let quote = settled_quote(&market, base_date, 6.0);
        let mut no_upfront = quoted_tranche(&quote, base_date);
        no_upfront.upfront = None;
        let no_upfront_pv = pricer(6.0)
            .price_tranche(&no_upfront, &market, base_date)
            .expect("no-upfront price")
            .amount();
        assert!(
            (no_upfront_pv - quote.upfront_pct).abs() > 1e-5,
            "the fixture must distinguish today's PV from cash due at settlement"
        );
        let (_, df, report) = StudentTTarget::solve(
            &params(),
            &[MarketQuote::CdsTranche(quote.clone())],
            &market,
            &CalibrationConfig::default(),
        )
        .expect("the exact settled quote should calibrate");
        assert_eq!(df, 6.0);
        assert!(report.success);
        assert!(report.max_residual < 1e-12);
        assert_eq!(report.success_tolerance, Some(1e-3));
        let full_trade_pv = pricer(df)
            .price_tranche(&quoted_tranche(&quote, base_date), &market, base_date)
            .expect("full quote-built trade price")
            .amount();
        assert!(full_trade_pv.abs() < 1e-12);
    }

    #[test]
    fn student_t_solve_from_different_guess_checks_the_returned_fit() {
        let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
        let market = fixture_market(base_date, 0.03);
        let quote = settled_quote(&market, base_date, 6.0);
        let mut params = params();
        params.initial_df = 3.0;
        let (_, df, report) = StudentTTarget::solve(
            &params,
            &[MarketQuote::CdsTranche(quote.clone())],
            &market,
            &CalibrationConfig::default(),
        )
        .expect("bounded solver should find the market fit");
        assert!((params.df_bounds.0..=params.df_bounds.1).contains(&df));
        assert!(
            (df - params.initial_df).abs() > 0.1,
            "the root search must run"
        );
        let residual = pricer(df)
            .price_tranche(&quoted_tranche(&quote, base_date), &market, base_date)
            .expect("returned-parameter repricing")
            .amount();
        assert!(residual.abs() <= 1e-3);
        assert!((report.max_residual - residual.abs()).abs() < 1e-12);
    }

    #[test]
    fn student_t_loose_solver_tolerance_does_not_relax_fit_acceptance() {
        let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
        let market = fixture_market(base_date, 0.03);
        let config = CalibrationConfig {
            solver: crate::SolverConfig::default().with_tolerance(1.0),
            ..CalibrationConfig::default()
        };
        StudentTTarget::solve(
            &params(),
            &[MarketQuote::CdsTranche(fixture_quote(0.25))],
            &market,
            &config,
        )
        .expect_err("solver precision cannot turn an unreachable quote into a successful fit");
    }
}
