//! Student-t degrees of freedom calibration for credit portfolio models.
//!
//! Calibrates the `df` (degrees of freedom) parameter of a Student-t copula
//! by repricing market tranche upfront quotes. Uses Brent root-finding to
//! minimize the pricing residual between the model-implied and market upfront.
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
//! 2. Price the reference tranche using the existing pricing infrastructure
//! 3. Compare model upfront to the market upfront quote
//! 4. Minimize the residual using Brent root-finding over the `df` domain
//!
//! # References
//!
//! - Demarta, S., & McNeil, A. J. (2005). "The t Copula and Related Copulas."
//! - Hull, J., Predescu, M., & White, A. (2005). "The valuation of correlation-
//!   dependent credit derivatives using a structural model."

use crate::calibration::api::schema::StudentTParams;
use crate::calibration::config::CalibrationConfig;
use crate::calibration::solver::helpers::bracket_solve_1d_with_diagnostics;
use crate::calibration::CalibrationReport;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::credit_derivatives::cds_tranche::{
    CDSTranche, CDSTranchePricer, CDSTranchePricerConfig, TrancheSide,
};
use crate::market::quotes::market_quote::MarketQuote;
use finstack_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Result;
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
    fn resolve_discount_curve_id(
        params: &StudentTParams,
        context: &MarketContext,
    ) -> Result<CurveId> {
        if let Some(curve_id) = &params.discount_curve_id {
            let _ = context.get_discount(curve_id)?;
            return Ok(curve_id.clone());
        }

        let mut discount_curves = context.curves_of_type("Discount");
        let (discount_curve_id, _) = discount_curves.next().ok_or_else(|| {
            finstack_core::Error::Input(finstack_core::InputError::NotFound {
                id: "discount curve".to_string(),
            })
        })?;
        if discount_curves.next().is_some() {
            return Err(finstack_core::Error::Validation(
                "Student-t calibration requires discount_curve_id when multiple discount curves are present"
                    .to_string(),
            ));
        }
        Ok(discount_curve_id.clone())
    }

    fn flat_base_correlation_template(
        template: &finstack_core::market_data::term_structures::BaseCorrelationCurve,
        correlation: f64,
    ) -> Result<finstack_core::market_data::term_structures::BaseCorrelationCurve> {
        let flat_corr = correlation.clamp(0.0, 0.999);
        let knots: Vec<(f64, f64)> = template
            .detachment_points()
            .iter()
            .copied()
            .map(|k| (k, flat_corr))
            .collect();
        finstack_core::market_data::term_structures::BaseCorrelationCurve::builder(
            template.id().clone(),
        )
        .knots(knots)
        .build()
    }

    /// Create a new Student-t degrees of freedom calibrator.
    pub(crate) fn new(
        params: StudentTParams,
        base_context: MarketContext,
        config: CalibrationConfig,
    ) -> Self {
        Self {
            params,
            base_context,
            config,
        }
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
        let tranche_quote = quotes
            .iter()
            .find_map(|quote| match quote {
                MarketQuote::CDSTranche(tranche_quote)
                    if tranche_quote.id().as_str() == params.tranche_instrument_id =>
                {
                    Some(tranche_quote.clone())
                }
                _ => None,
            })
            .ok_or_else(|| {
                finstack_core::Error::Input(finstack_core::InputError::NotFound {
                    id: format!("CDS tranche quote '{}'", params.tranche_instrument_id),
                })
            })?;

        let (index_id, attachment, detachment, maturity, upfront_pct, running_spread_bp) =
            match &tranche_quote {
                crate::market::quotes::cds_tranche::CDSTrancheQuote::CDSTranche {
                    index,
                    attachment,
                    detachment,
                    maturity,
                    upfront_pct,
                    running_spread_bp,
                    ..
                } => (
                    index.clone(),
                    *attachment,
                    *detachment,
                    *maturity,
                    *upfront_pct,
                    *running_spread_bp,
                ),
            };

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
        // credit-index copy. `insert_credit_index()` triggers market-context rebinding,
        // which would otherwise snap the index back to any same-id curve already stored
        // on the context and silently discard the requested flat-correlation override.
        let pricing_context = context
            .clone()
            .insert(flat_base_correlation)
            .insert_credit_index(&index_id, rebound_credit_index);

        let discount_curve_id = Self::resolve_discount_curve_id(params, &pricing_context)?;
        let discount_curve = pricing_context.get_discount(&discount_curve_id)?;
        let as_of = discount_curve.base_date();

        let tranche = CDSTranche::builder()
            .id(InstrumentId::new(params.tranche_instrument_id.clone()))
            .index_name(index_id.clone())
            .series(1)
            .attach_pct(attachment * 100.0)
            .detach_pct(detachment * 100.0)
            .notional(Money::new(1.0, finstack_core::currency::Currency::USD))
            .maturity(maturity)
            .running_coupon_bp(running_spread_bp)
            .frequency(Tenor::quarterly())
            .day_count(DayCount::Act360)
            .bdc(BusinessDayConvention::Following)
            .calendar_id_opt(None)
            .discount_curve_id(discount_curve_id)
            .credit_index_id(CurveId::from(index_id.as_str()))
            .side(TrancheSide::SellProtection)
            .effective_date_opt(None)
            .accumulated_loss(0.0)
            .standard_imm_dates(true)
            .attributes(Attributes::new())
            .build()?;

        let calibrator = Self::new(params.clone(), pricing_context, global_config.clone());
        calibrator.calibrate_df(&tranche, upfront_pct, as_of)
    }

    /// Run the Brent root-finding calibration over the df domain.
    fn calibrate_df(
        &self,
        tranche: &CDSTranche,
        market_upfront: f64,
        as_of: Date,
    ) -> Result<(MarketContext, f64, CalibrationReport)> {
        let (df_lo, df_hi) = self.params.df_bounds;
        let initial_df = self.params.initial_df;
        let tolerance = self.config.solver.tolerance();
        let max_iters = self.config.solver.max_iterations();

        let objective = |df: f64| -> f64 {
            if df <= 2.0 || !df.is_finite() {
                return f64::INFINITY;
            }
            let Ok(config) = CDSTranchePricerConfig::default().with_student_t_copula(df) else {
                return f64::INFINITY;
            };
            let pricer = CDSTranchePricer::with_params(config);
            match pricer.calculate_upfront(tranche, &self.base_context, as_of) {
                Ok(model_upfront) => model_upfront - market_upfront,
                Err(_) => f64::INFINITY,
            }
        };

        // Generate scan points for bracketing.
        let scan = self.build_scan_grid(df_lo, df_hi, initial_df);

        let (root, diagnostics) =
            bracket_solve_1d_with_diagnostics(&objective, initial_df, &scan, tolerance, max_iters)?;

        // Determine result.
        //
        // On non-convergence we return `Err` rather than `Ok` with an
        // uncalibrated fallback `df`: the previous behaviour stored the
        // fallback in the context and direct callers (those not routed through
        // the engine's `fail_on_bad_fit` check) silently received a bad
        // parameter. Returning `Err` makes the failure impossible to ignore —
        // it propagates through `StudentTTarget::solve` and the `?` in
        // `step_runtime.rs`.
        let (calibrated_df, success, reason) = match root {
            Some(df) if df.is_finite() && df > 2.0 => {
                let residual = objective(df);
                if residual.abs() <= tolerance {
                    (
                        df,
                        true,
                        format!("Student-t df calibration converged: df={:.4}", df),
                    )
                } else {
                    return Err(finstack_core::Error::Calibration {
                        message: format!(
                            "Student-t df calibration failed: best df={:.4} but residual {:.2e} exceeds tolerance {:.2e}",
                            df, residual.abs(), tolerance
                        ),
                        category: "student_t_df".to_string(),
                    });
                }
            }
            _ => {
                let fallback_df = diagnostics.best_point.unwrap_or(initial_df);
                return Err(finstack_core::Error::Calibration {
                    message: format!(
                        "Student-t df calibration failed to converge (bracket_found={}, best df={:.4})",
                        diagnostics.bracket_found, fallback_df
                    ),
                    category: "student_t_df".to_string(),
                });
            }
        };

        // Clamp to bounds.
        let calibrated_df = calibrated_df.clamp(df_lo, df_hi);

        // Build report.
        let mut residuals = BTreeMap::new();
        let final_residual = objective(calibrated_df);
        residuals.insert(
            format!("{}_df", self.params.tranche_instrument_id),
            final_residual,
        );

        let report = CalibrationReport::new(residuals, diagnostics.eval_count, success, &reason)
            .with_metadata("calibration_type", "student_t_df")
            .with_metadata("tranche_instrument_id", &self.params.tranche_instrument_id)
            .with_metadata("calibrated_df", format!("{:.6}", calibrated_df))
            .with_metadata("df_bounds", format!("[{:.2}, {:.2}]", df_lo, df_hi))
            .with_model_version(finstack_core::versions::STUDENT_T_COPULA);
        let mut report = report;
        report.update_solver_config(self.config.solver.clone());

        // Store calibrated df in the market context.
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
        const N: usize = 6;
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

        pts.retain(|x| x.is_finite() && *x > 2.0);
        pts.sort_by(|a, b| a.total_cmp(b));
        pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        pts
    }
}
