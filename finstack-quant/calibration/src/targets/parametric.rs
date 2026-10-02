//! Nelson-Siegel / Nelson-Siegel-Svensson parametric curve calibration target.
//!
//! Implements `GlobalSolveTarget` to fit parametric yield curves from
//! market instruments using the Levenberg-Marquardt optimizer.

use crate::api::schema::ParametricCurveParams;
use crate::config::CalibrationConfig;
use crate::prepared::CalibrationQuote;
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::rates::RateQuote;
use crate::solver::global::GlobalFitOptimizer;
use crate::solver::traits::GlobalSolveTarget;
use crate::targets::util::{
    discount_and_forward_curve_ids, prepare_rate_calibration_quotes, ContextScratch,
};
use crate::CalibrationReport;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    NelsonSiegelModel, NsVariant, ParametricCurve,
};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::collections::BTreeSet;

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
    /// Dates at which the rate instruments can query discount factors, mapped
    /// once onto the analytical curve's ACT/365F clock.
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
    pub(crate) fn new(params: ParametricCurveTargetParams, sample_times: Vec<f64>) -> Self {
        let scratch = ContextScratch::new(params.base_context.clone());
        let residual_notional = params.residual_notional;
        Self {
            params,
            sample_times,
            scratch,
            residual_notional,
        }
    }

    /// Build an exact date grid for the prepared instruments' discount queries.
    ///
    /// Instrument pricing resolves a concrete `DiscountCurve`. Deposits only
    /// query their two cashflow dates, so those dates suffice. Swaps and futures
    /// can query every overnight observation boundary as well as coupon/payment
    /// dates; include all calendar dates through their final prepared pillar.
    /// The prepared swap pillar already includes the payment delay. Lookbacks
    /// before the valuation date remain historical-fixing requirements.
    ///
    /// Every future date used by the canonical rate pricers is consequently an
    /// exact knot. Residuals depend on analytical discount factors, without an
    /// interpolation approximation or a tolerance-dependent sampling heuristic.
    fn build_sample_times(base_date: Date, quotes: &[CalibrationQuote]) -> Result<Vec<f64>> {
        let mut dates = BTreeSet::from([base_date]);
        let mut daily_horizon = base_date;
        for quote in quotes {
            let CalibrationQuote::Rates(prepared) = quote else {
                return Err(finstack_quant_core::Error::Validation(
                    "Parametric calibration requires rate quotes".to_string(),
                ));
            };
            if matches!(prepared.quote.as_ref(), RateQuote::Deposit { .. }) {
                let deposit = prepared
                    .instrument
                    .as_any()
                    .downcast_ref::<Deposit>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Prepared deposit quote '{}' does not contain a Deposit instrument",
                            prepared.quote.id()
                        ))
                    })?;
                dates.insert(deposit.start_date);
                dates.insert(deposit.maturity);
            } else {
                daily_horizon = daily_horizon.max(prepared.pillar_date);
            }
        }
        let mut date = base_date;
        while date < daily_horizon {
            date = date.next_day().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Parametric calibration date grid exceeds the supported date range".to_string(),
                )
            })?;
            dates.insert(date);
        }
        dates
            .into_iter()
            .filter(|date| *date >= base_date)
            .map(|date| {
                DayCount::Act365F.year_fraction(base_date, date, DayCountContext::default())
            })
            .collect()
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
        let residual_notional: f64 = 1_000_000.0;
        let prepared = prepare_rate_calibration_quotes(
            quotes,
            schema_params.base_date,
            discount_and_forward_curve_ids(
                schema_params.curve_id.as_ref(),
                schema_params.curve_id.as_ref(),
            ),
            Some(DayCount::Act365F),
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
                initial_params,
                base_context: context.clone(),
                residual_notional,
            },
            Self::build_sample_times(schema_params.base_date, &prepared_quotes)?,
        );
        let success_tolerance = config.discount_curve.validation_tolerance;
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

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        quote.quote_id().to_string()
    }

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
        let disc_curve = finstack_quant_core::market_data::term_structures::DiscountCurve::builder(
            self.params.curve_id.clone(),
        )
        .base_date(self.params.base_date)
        .day_count(DayCount::Act365F)
        .knots(knots)
        // Interpolation is never used by these date-based calibration quotes:
        // every economically queried date is an exact analytical knot.
        .interp(InterpStyle::LogLinear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build_for_solver()?;

        self.scratch.with_curve(&disc_curve, |ctx| {
            for (i, q) in quotes.iter().enumerate() {
                let pv = q.calibration_value_raw(ctx, self.params.base_date)?;
                residuals[i] = pv / self.residual_notional;
            }
            Ok(())
        })
    }

    fn lower_bounds(&self) -> Option<Vec<f64>> {
        Some(match self.params.variant {
            NsVariant::Ns => vec![-2.0, -2.0, -2.0, 0.01],
            NsVariant::Nss => vec![-2.0, -2.0, -2.0, -2.0, 0.01, 0.01],
        })
    }

    fn upper_bounds(&self) -> Option<Vec<f64>> {
        Some(match self.params.variant {
            NsVariant::Ns => vec![2.0, 2.0, 2.0, 30.0],
            NsVariant::Nss => vec![2.0, 2.0, 2.0, 2.0, 30.0, 30.0],
        })
    }
}
