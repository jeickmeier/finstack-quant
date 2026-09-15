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
//! - Demarta, S., & McNeil, A. J. (2005). "The t Copula and Related Copulas." `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//! - Hull, J., Predescu, M., & White, A. (2005). "The valuation of correlation-
//!   dependent credit derivatives using a structural model."

use crate::api::schema::StudentTParams;
use crate::build::cds_tranche::{build_cds_tranche_instrument, CDSTrancheBuildOverrides};
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
    CDSTranche, CDSTranchePricer, CDSTranchePricerConfig,
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
        let flat_corr = correlation.clamp(0.0, 0.999);
        let knots: Vec<(f64, f64)> = template
            .detachment_points()
            .iter()
            .copied()
            .map(|k| (k, flat_corr))
            .collect();
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
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
        let upfront_pct = tranche_quote.upfront_pct;

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

        let tranche_width = detachment - attachment;
        if !tranche_width.is_finite() || tranche_width <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Student-t calibration tranche width must be positive; attachment={attachment}, detachment={detachment}"
            )));
        }
        let mut pricing_quote = tranche_quote;
        pricing_quote.upfront_pct = 0.0;
        let mut curve_ids = finstack_quant_core::HashMap::default();
        curve_ids.insert("discount".to_string(), discount_curve_id.to_string());
        curve_ids.insert("credit".to_string(), index_id);
        let build_context = BuildCtx::new(as_of, 1.0 / tranche_width, curve_ids);
        let instrument = build_cds_tranche_instrument(
            &pricing_quote,
            &build_context,
            &CDSTrancheBuildOverrides::default(),
        )?;
        let tranche = instrument
            .as_any()
            .downcast_ref::<CDSTranche>()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "shared tranche quote builder did not return CDSTranche".to_string(),
                )
            })?
            .clone();

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
        let max_iters = self.config.solver.max_iterations();

        // Separate the root x-tolerance from quadrature-limited residual accuracy.
        const STUDENT_T_UPFRONT_TOLERANCE: f64 = 1e-3;
        let acceptance_tolerance = self
            .config
            .solver
            .tolerance()
            .max(STUDENT_T_UPFRONT_TOLERANCE);

        let template = CDSTranchePricerConfig::default();
        let price_residual = |df: f64| -> f64 {
            if df <= 2.0 || !df.is_finite() {
                return f64::INFINITY;
            }
            let Ok(config) = template.clone().with_student_t_copula(df) else {
                return f64::INFINITY;
            };
            let Ok(pricer) = CDSTranchePricer::with_params(config) else {
                return f64::INFINITY;
            };
            match pricer.calculate_upfront(tranche, &self.base_context, as_of) {
                Ok(model_upfront) => model_upfront - market_upfront,
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
                Some(df) if df.is_finite() && df > 2.0 => {
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

        let unclamped_df = calibrated_df;
        let calibrated_df = unclamped_df.clamp(df_lo, df_hi);
        let final_residual = if (calibrated_df - unclamped_df).abs() < 1e-12 {
            residual
        } else {
            price_residual(calibrated_df)
        };

        let success = true;
        let reason = format!(
            "Student-t df calibration converged: df={:.4}",
            calibrated_df
        );

        let mut residuals = BTreeMap::new();
        residuals.insert(
            format!("{}_df", self.params.tranche_instrument_id),
            final_residual,
        );

        let report = CalibrationReport::new(residuals, eval_count, success, &reason)
            .with_metadata("calibration_type", "student_t_df")
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
