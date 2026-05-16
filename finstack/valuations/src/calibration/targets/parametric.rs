//! Nelson-Siegel / Nelson-Siegel-Svensson parametric curve calibration target.
//!
//! Implements [`GlobalSolveTarget`] to fit parametric yield curves from
//! market instruments using the Levenberg-Marquardt optimizer.

use crate::calibration::api::schema::ParametricCurveParams;
use crate::calibration::config::CalibrationConfig;
use crate::calibration::prepared::CalibrationQuote;
use crate::calibration::solver::global::GlobalFitOptimizer;
use crate::calibration::solver::traits::GlobalSolveTarget;
use crate::calibration::targets::util::{
    discount_only_curve_ids, prepare_rate_calibration_quotes, ContextScratch,
};
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::MarketQuote;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{NelsonSiegelModel, NsVariant, ParametricCurve};
use finstack_core::market_data::traits::Discounting;
use finstack_core::types::CurveId;
use finstack_core::Result;

/// Parameters for constructing a [`ParametricCurveTarget`].
#[derive(Clone)]
pub(crate) struct ParametricCurveTargetParams {
    /// Base date for the calibration.
    pub(crate) base_date: Date,
    /// Curve identifier.
    pub(crate) curve_id: CurveId,
    /// NS or NSS variant.
    pub(crate) variant: NsVariant,
    /// Optional initial parameter guesses.
    pub(crate) initial_params: Option<NelsonSiegelModel>,
    /// Base market context.
    pub(crate) base_context: MarketContext,
    /// Residual normalization notional (used to scale PV residuals to per-unit notional).
    ///
    /// Calibration tolerances are interpreted in **per-notional** residual units, so
    /// a realistic notional can be used for instrument construction without making
    /// solver tolerances unrealistically tight in absolute currency terms.
    pub(crate) residual_notional: f64,
}

/// Calibration target for parametric (NS/NSS) curves.
///
/// Uses global optimization to fit 4 (NS) or 6 (NSS) parameters
/// from rate instrument quotes.
pub(crate) struct ParametricCurveTarget {
    params: ParametricCurveTargetParams,
    /// Pre-computed sample times for building the discount curve proxy.
    /// Computed once from quote pillars in [`Self::solve`] to avoid
    /// re-sorting/deduplicating on every LM iteration.
    sample_times: Vec<f64>,
    /// Reusable scratch context (see [`ContextScratch`]).
    scratch: ContextScratch,
    /// Residual normalization notional, mirroring the notional used to build
    /// the calibration instruments. Residuals are divided by this value so that
    /// the solver works in per-notional units and `validation_tolerance` (default
    /// `1e-8`) is comparable to the sibling targets.
    residual_notional: f64,
}

impl ParametricCurveTarget {
    /// Create a new parametric curve target with pre-computed sample times.
    pub(crate) fn new(
        params: ParametricCurveTargetParams,
        sample_times: Vec<f64>,
        config: &CalibrationConfig,
    ) -> Self {
        let scratch = ContextScratch::from_config(params.base_context.clone(), config);
        let residual_notional = params.residual_notional;
        Self {
            params,
            sample_times,
            scratch,
            residual_notional,
        }
    }

    /// Build the sample time grid from a set of prepared quotes.
    fn build_sample_times(quotes: &[CalibrationQuote]) -> Vec<f64> {
        let mut times = vec![0.0];
        for q in quotes {
            let t = q.pillar_time();
            if t > 0.0 {
                times.push(t);
            }
        }
        times.sort_by(|a, b| a.total_cmp(b));
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let max_t = times.last().copied().unwrap_or(30.0);
        let mut t = 0.5;
        while t < max_t {
            times.push(t);
            t += 0.5;
        }
        times.sort_by(|a, b| a.total_cmp(b));
        times.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        times
    }

    /// Clamp NS/NSS parameters to feasible region. Used by both solver-curve and
    /// final-curve builders so the reported curve matches what was priced.
    fn clamp_params(&self, params: &[f64]) -> Vec<f64> {
        const TAU_LO: f64 = 0.01;
        const TAU_HI: f64 = 30.0;
        const TAU_MIN_SEPARATION: f64 = 0.01;
        let mut p = params.to_vec();
        match self.params.variant {
            NsVariant::Ns => {
                if p.len() == 4 {
                    p[3] = p[3].clamp(TAU_LO, TAU_HI);
                }
            }
            NsVariant::Nss => {
                if p.len() == 6 {
                    p[4] = p[4].clamp(TAU_LO, TAU_HI);
                    p[5] = p[5].clamp(TAU_LO, TAU_HI);
                    if (p[4] - p[5]).abs() < TAU_MIN_SEPARATION {
                        // Push tau2 above tau1, but stay inside [TAU_LO, TAU_HI].
                        p[5] = (p[4] + 0.5).min(TAU_HI);
                        if (p[4] - p[5]).abs() < TAU_MIN_SEPARATION {
                            p[4] = (p[5] - 0.5).max(TAU_LO);
                        }
                    }
                }
            }
        }
        p
    }

    /// Execute the full calibration for a parametric curve step.
    pub(crate) fn solve(
        schema_params: &ParametricCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        let discount_id = schema_params
            .discount_curve_id
            .as_ref()
            .unwrap_or(&schema_params.curve_id);
        let residual_notional: f64 = 1_000_000.0;
        let prepared = prepare_rate_calibration_quotes(
            quotes,
            schema_params.base_date,
            discount_only_curve_ids(discount_id.as_ref()),
            None,
            residual_notional,
        )?;
        let prepared_quotes = prepared.quotes;

        let initial_params = schema_params.initial_params.clone().or_else(|| {
            Some(match schema_params.model {
                NsVariant::Ns => NelsonSiegelModel::Ns {
                    beta0: 0.03,
                    beta1: -0.02,
                    beta2: 0.01,
                    tau: 1.5,
                },
                NsVariant::Nss => NelsonSiegelModel::Nss {
                    beta0: 0.03,
                    beta1: -0.02,
                    beta2: 0.01,
                    beta3: 0.01,
                    tau1: 1.5,
                    tau2: 5.0,
                },
            })
        });

        let config = global_config.clone();
        let target = Self::new(
            ParametricCurveTargetParams {
                base_date: schema_params.base_date,
                curve_id: schema_params.curve_id.clone(),
                variant: schema_params.model,
                initial_params: initial_params.clone(),
                base_context: context.clone(),
                residual_notional,
            },
            Self::build_sample_times(&prepared_quotes),
            &config,
        );
        let success_tolerance = Some(config.discount_curve.validation_tolerance);
        let (curve, report) =
            GlobalFitOptimizer::optimize(&target, &prepared_quotes, &config, success_tolerance)?;

        let new_context = context.clone().insert(curve);
        Ok((new_context, report))
    }

    fn default_guesses(&self) -> Vec<f64> {
        if let Some(ref model) = self.params.initial_params {
            return model.to_params_vec();
        }
        match self.params.variant {
            NsVariant::Ns => vec![0.03, -0.02, 0.01, 1.5],
            NsVariant::Nss => vec![0.03, -0.02, 0.01, 0.01, 1.5, 5.0],
        }
    }
}

impl GlobalSolveTarget for ParametricCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = ParametricCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        let guesses = self.default_guesses();
        // This target ignores `times`, but the shared input validation
        // requires a positive, increasing grid.
        let times: Vec<f64> = (1..=guesses.len()).map(|i| i as f64).collect();
        Ok((times, guesses, quotes.to_vec()))
    }

    fn build_curve_from_params(&self, _times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        // Clamp the same way as the solver curve so the final reported curve
        // matches what the LM iterations actually priced against.
        let p = self.clamp_params(params);
        let model = NelsonSiegelModel::from_params_vec(self.params.variant, &p)?;
        ParametricCurve::builder(self.params.curve_id.clone())
            .base_date(self.params.base_date)
            .model(model)
            .build()
    }

    fn build_curve_for_solver_from_params(
        &self,
        _times: &[f64],
        params: &[f64],
    ) -> Result<Self::Curve> {
        let p = self.clamp_params(params);
        let model = NelsonSiegelModel::from_params_vec(self.params.variant, &p)?;
        ParametricCurve::builder(self.params.curve_id.clone())
            .base_date(self.params.base_date)
            .model(model)
            .build()
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        let knots: Vec<(f64, f64)> = self
            .sample_times
            .iter()
            .map(|&t| (t, curve.df(t)))
            .collect();
        let disc_curve = finstack_core::market_data::term_structures::DiscountCurve::builder(
            self.params.curve_id.clone(),
        )
        .base_date(self.params.base_date)
        .knots(knots)
        .validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build_for_solver()?;

        self.scratch.with_curve(&disc_curve, |ctx| {
            for (i, q) in quotes.iter().enumerate() {
                let pv = q.get_instrument().value_raw(ctx, self.params.base_date)?;
                residuals[i] = pv / self.residual_notional;
            }
            Ok(())
        })
    }

    fn lower_bounds(&self) -> Option<Vec<f64>> {
        Some(match self.params.variant {
            NsVariant::Ns => vec![
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                0.01,
            ],
            NsVariant::Nss => vec![
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                0.01,
                0.01,
            ],
        })
    }

    fn upper_bounds(&self) -> Option<Vec<f64>> {
        Some(match self.params.variant {
            NsVariant::Ns => vec![f64::INFINITY, f64::INFINITY, f64::INFINITY, 30.0],
            NsVariant::Nss => {
                vec![
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::INFINITY,
                    30.0,
                    30.0,
                ]
            }
        })
    }
}
