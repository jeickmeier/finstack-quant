//! Forward curve calibration target.

use crate::api::schema::ForwardCurveParams;
use crate::config::CalibrationConfig;
use crate::config::CalibrationMethod;
use crate::constants::WEIGHT_MIN_FLOOR;
use crate::quotes::market_quote::ExtractQuotes;
use crate::quotes::market_quote::MarketQuote;
use crate::quotes::rates::RateQuote;
use crate::solver::global::GlobalFitOptimizer;
use crate::solver::traits::GlobalSolveTarget;
use crate::targets::rate_recipe::recipe_ois_compounding;
use crate::targets::util::{
    discount_and_forward_curve_ids, prepare_rate_calibration_quotes_with_ois_override,
    quote_annuity_proxy, scheme_factor, ContextScratch,
};
use crate::CalibrationReport;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    ForwardCurve, RateCalibrationCurveRole, RateCalibrationRecipe,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use std::cell::{Cell, RefCell};

/// Parameters for constructing a `ForwardCurveTarget`.
#[derive(Clone)]
pub(crate) struct ForwardCurveTargetParams {
    /// Base date for the curve (valuation date).
    pub(crate) base_date: Date,
    /// Currency of the forward curve (e.g. USD).
    pub(crate) currency: Currency,
    /// Unique identifier for the forward curve being calibrated.
    pub(crate) fwd_curve_id: CurveId,
    /// Tenor associated with the forward rates (e.g. 3M, 6M).
    pub(crate) tenor_years: f64,
    /// Numerical interpolation style used during the solving process.
    pub(crate) solve_interp: InterpStyle,
    /// Global calibration settings (tolerances, rate bounds).
    pub(crate) config: CalibrationConfig,
    /// Convention for converting dates to time axis (year fractions).
    pub(crate) time_day_count: DayCount,
    /// Residual normalization notional (used to scale PV residuals to per-unit notional).
    pub(crate) residual_notional: f64,
    /// Context providing supporting market data (e.g. discount curves).
    pub(crate) base_context: MarketContext,
}

/// Target for simultaneous forward-curve calibration.
///
/// This adapter bridges calibration solvers with forward-rate pricing. It
/// handles knot anchor insertion at t=0 and provides rate-bound-aware scanning
/// for numerical stability.
pub(crate) struct ForwardCurveTarget {
    /// Construction parameters (dates, ids, conventions, config).
    pub(crate) params: ForwardCurveTargetParams,
    /// Reusable scratch context (see [`ContextScratch`]).
    scratch: ContextScratch,
    /// Actual contractual reset/end times used to build projection DFs.
    projection_grid: RefCell<Option<Vec<f64>>>,
    /// Number of global parameters, populated when the solve grid is built.
    parameter_count: Cell<usize>,
}

impl ForwardCurveTarget {
    /// Create a new `ForwardCurveTarget` from parameters.
    pub(crate) fn new(params: ForwardCurveTargetParams) -> Self {
        let scratch = ContextScratch::new(params.base_context.clone());
        Self {
            params,
            scratch,
            projection_grid: RefCell::new(None),
            parameter_count: Cell::new(0),
        }
    }

    /// Execute the full calibration for a forward curve step.
    pub(crate) fn solve(
        params: &ForwardCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        // Forward-curve preflight: prepare quotes; both the discount curve (already in
        // `context`) and the forward curve being built are registered so projected legs
        // price against the right curves.
        let prepared = prepare_rate_calibration_quotes_with_ois_override(
            quotes,
            params.base_date,
            discount_and_forward_curve_ids(
                params.discount_curve_id.as_ref(),
                params.curve_id.as_ref(),
            ),
            params.conventions.curve_day_count,
            1_000_000.0,
            params.conventions.ois_compounding.clone(),
        )?;
        let prepared_quotes = prepared.quotes;
        let curve_day_count = prepared.curve_day_count;
        let rate_calibration = Self::rate_calibration(quotes, params, curve_day_count)?;

        let mut config = global_config.clone();
        config.calibration_method = params.method.clone();

        let target = ForwardCurveTarget::new(ForwardCurveTargetParams {
            base_date: params.base_date,
            currency: params.currency,
            fwd_curve_id: params.curve_id.clone(),
            tenor_years: params.tenor_years,
            solve_interp: params.interpolation,
            config: config.clone(),
            time_day_count: curve_day_count,
            residual_notional: 1_000_000.0,
            base_context: context.clone(),
        });

        let success_tolerance = config.forward_curve.validation_tolerance;

        if matches!(params.method, CalibrationMethod::Bootstrap) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Forward curve {} requires CalibrationMethod::GlobalSolve because \
                 DF-chained reset intervals are solved simultaneously; Bootstrap \
                 is sequential and cannot preserve off-grid reset-date quote meaning",
                params.curve_id
            )));
        }

        // DF-implied term rates couple adjacent reset-date forwards whenever a
        // contractual period has a calendar stub. Fit all reset-date rates
        // simultaneously against a dense grid of actual contractual intervals.
        let (curve, mut report) =
            GlobalFitOptimizer::optimize(&target, &prepared_quotes, &config, success_tolerance)?;
        let curve = curve
            .to_builder_with_id(curve.id().clone())
            .rate_calibration(rate_calibration)
            .build()?;
        report.metadata.insert(
            "forward_parameterization".to_string(),
            "contractual_reset_grid".to_string(),
        );

        report.update_solver_config(config.solver);

        let new_context = context.clone().insert(curve);
        Ok((new_context, report))
    }

    fn rate_calibration(
        quotes: &[MarketQuote],
        params: &ForwardCurveParams,
        curve_day_count: DayCount,
    ) -> Result<RateCalibrationRecipe> {
        let rate_quotes: Vec<RateQuote> = quotes.extract_quotes();
        Ok(RateCalibrationRecipe {
            currency: params.currency,
            method: (&params.method).into(),
            curve_day_count,
            ois_compounding: params
                .conventions
                .ois_compounding
                .as_ref()
                .map(recipe_ois_compounding)
                .transpose()?,
            role: RateCalibrationCurveRole::Projection {
                discount_curve_id: params.discount_curve_id.clone(),
            },
            quotes: rate_quotes.iter().map(Into::into).collect(),
        })
    }

    /// Return the reset-date parameter time for quotes whose modeled rate is
    /// derived from projection discount factors over a future accrual period.
    fn parameter_time(&self, quote: &crate::prepared::CalibrationQuote) -> Result<f64> {
        let pq = match quote {
            crate::prepared::CalibrationQuote::Rates(pq) => pq,
            other => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Forward curve calibration accepts Rates quotes only; got {:?}",
                    std::mem::discriminant(other)
                )));
            }
        };

        let intervals = self.reset_intervals(pq)?;
        let parameter_date = match pq.quote.as_ref() {
            RateQuote::Swap { .. } => intervals.last().map(|(start, _)| *start),
            RateQuote::Deposit { .. } => intervals.first().map(|(_, end)| *end),
            RateQuote::Fra { .. } | RateQuote::Futures { .. } => {
                intervals.first().map(|(start, _)| *start)
            }
        }
        .ok_or_else(|| finstack_quant_core::Error::Calibration {
            message: format!(
                "Forward quote {} produced no reset intervals",
                pq.quote.id()
            ),
            category: "global_solve".to_string(),
        })?;

        self.params.time_day_count.year_fraction(
            self.params.base_date,
            parameter_date,
            DayCountContext::default(),
        )
    }

    /// Quote-derived initial rate for the simultaneous solver.
    fn initial_guess(&self, quote: &crate::prepared::CalibrationQuote) -> Result<f64> {
        let pq = match quote {
            crate::prepared::CalibrationQuote::Rates(pq) => pq,
            other => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Forward curve calibration accepts Rates quotes only; got {:?}",
                    std::mem::discriminant(other)
                )));
            }
        };
        Ok(pq.quote.implied_rate())
    }

    /// Resolve all contractual projection intervals represented by a prepared quote.
    fn reset_intervals(
        &self,
        pq: &crate::build::prepared::PreparedQuote<RateQuote>,
    ) -> Result<Vec<(Date, Date)>> {
        match pq.quote.as_ref() {
            RateQuote::Deposit { .. } => {
                let deposit = pq
                    .instrument
                    .as_any()
                    .downcast_ref::<Deposit>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Forward deposit quote {} did not build a Deposit instrument",
                            pq.quote.id()
                        ))
                    })?;
                Ok(vec![(deposit.start_date, deposit.maturity)])
            }
            RateQuote::Fra { .. } => {
                let fra = pq
                    .instrument
                    .as_any()
                    .downcast_ref::<ForwardRateAgreement>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Forward FRA quote {} did not build a ForwardRateAgreement",
                            pq.quote.id()
                        ))
                    })?;
                Ok(vec![(fra.start_date, fra.maturity)])
            }
            RateQuote::Futures { .. } => {
                let future = pq
                    .instrument
                    .as_any()
                    .downcast_ref::<InterestRateFuture>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Forward futures quote {} did not build an InterestRateFuture",
                            pq.quote.id()
                        ))
                    })?;
                let (_, start, end) = future.resolve_dates()?;
                Ok(vec![(start, end)])
            }
            RateQuote::Swap { .. } => {
                let swap = pq
                    .instrument
                    .as_any()
                    .downcast_ref::<InterestRateSwap>()
                    .ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "Forward swap quote {} did not build an InterestRateSwap",
                            pq.quote.id()
                        ))
                    })?;
                let schedule = finstack_quant_cashflows::builder::periods::build_periods(
                    finstack_quant_cashflows::builder::periods::BuildPeriodsParams {
                        start: swap.float.start,
                        end: swap.float.end,
                        frequency: swap.float.frequency,
                        stub: swap.float.stub,
                        business_day_convention: swap.float.business_day_convention,
                        calendar_id: swap.float.calendar_id.as_deref().unwrap_or(
                            finstack_quant_cashflows::builder::calendar::WEEKENDS_ONLY_ID,
                        ),
                        end_of_month: swap.float.end_of_month,
                        day_count: finstack_quant_core::dates::DayCount::Act365F,
                        payment_lag_days: swap.float.payment_lag_days,
                        reset_lag_days: None,
                        adjust_accrual_dates: false,
                        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
                    },
                )?;
                if schedule.is_empty() {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Forward swap quote {} produced an empty floating schedule",
                            pq.quote.id()
                        ),
                        category: "global_solve".to_string(),
                    });
                }
                Ok(schedule
                    .into_iter()
                    .map(|period| (period.accrual_start, period.accrual_end))
                    .collect())
            }
        }
    }

    /// Build and validate the dense contractual reset boundary grid.
    fn projection_grid(&self, quotes: &[crate::prepared::CalibrationQuote]) -> Result<Vec<f64>> {
        let mut grid = vec![0.0];
        for quote in quotes {
            let pq = match quote {
                crate::prepared::CalibrationQuote::Rates(pq) => pq,
                other => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Forward curve calibration accepts Rates quotes only; got {:?}",
                        std::mem::discriminant(other)
                    )));
                }
            };
            for (start, end) in self.reset_intervals(pq)? {
                let start_time = self.params.time_day_count.year_fraction(
                    self.params.base_date,
                    start,
                    DayCountContext::default(),
                )?;
                let end_time = self.params.time_day_count.year_fraction(
                    self.params.base_date,
                    end,
                    DayCountContext::default(),
                )?;
                if !start_time.is_finite()
                    || !end_time.is_finite()
                    || start_time < 0.0
                    || end_time <= start_time
                {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Invalid forward reset interval for {}: [{start_time}, {end_time}]",
                            pq.quote.id()
                        ),
                        category: "global_solve".to_string(),
                    });
                }
                grid.push(start_time);
                grid.push(end_time);
            }
        }
        grid.sort_by(f64::total_cmp);
        grid.dedup_by(|a, b| (*a - *b).abs() <= 1e-12);
        if grid.len() < 2 {
            return Err(finstack_quant_core::Error::Calibration {
                message: "Forward global solve requires at least one reset interval".to_string(),
                category: "global_solve".to_string(),
            });
        }
        Ok(grid)
    }

    /// Build a curve with an explicit contractual projection grid.
    fn build_curve_with_projection_grid(
        &self,
        knots: &[(f64, f64)],
        projection_grid: Vec<f64>,
    ) -> Result<ForwardCurve> {
        let mut full_knots = knots.to_vec();
        if full_knots.is_empty() {
            return Err(finstack_quant_core::Error::Calibration {
                message: "Failed to build temp forward curve: need at least one knot".into(),
                category: "global_solve".to_string(),
            });
        }
        if full_knots[0].0 > 1e-12 {
            full_knots.insert(0, (0.0, full_knots[0].1));
        }
        ForwardCurve::builder(self.params.fwd_curve_id.clone(), self.params.tenor_years)
            .base_date(self.params.base_date)
            .knots(full_knots)
            .projection_grid(projection_grid)
            .interp(self.params.solve_interp)
            .day_count(self.params.time_day_count)
            .build()
            .map_err(|error| finstack_quant_core::Error::Calibration {
                message: format!("Failed to build reset-grid forward curve: {error}"),
                category: "global_solve".to_string(),
            })
    }
}

impl GlobalSolveTarget for ForwardCurveTarget {
    type Quote = crate::prepared::CalibrationQuote;
    type Curve = ForwardCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        let projection_grid = self.projection_grid(quotes)?;
        let mut entries = Vec::with_capacity(quotes.len());
        let mut deposits = Vec::new();
        for quote in quotes {
            let pq = match quote {
                crate::prepared::CalibrationQuote::Rates(pq) => pq,
                other => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Forward curve calibration accepts Rates quotes only; got {:?}",
                        std::mem::discriminant(other)
                    )));
                }
            };
            let initial = self.initial_guess(quote)?;
            if matches!(pq.quote.as_ref(), RateQuote::Deposit { .. }) {
                let (start, end) = self.reset_intervals(pq)?.first().copied().ok_or_else(|| {
                    finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Forward deposit quote {} produced no reset interval",
                            pq.quote.id()
                        ),
                        category: "global_solve".to_string(),
                    }
                })?;
                let start_time = self.params.time_day_count.year_fraction(
                    self.params.base_date,
                    start,
                    DayCountContext::default(),
                )?;
                let end_time = self.params.time_day_count.year_fraction(
                    self.params.base_date,
                    end,
                    DayCountContext::default(),
                )?;
                deposits.push((start_time, end_time, initial, quote.clone()));
            } else {
                let parameter_time = self.parameter_time(quote)?;
                let fallback_time = if matches!(pq.quote.as_ref(), RateQuote::Swap { .. }) {
                    let maturity = self
                        .reset_intervals(pq)?
                        .last()
                        .map(|(_, end)| *end)
                        .ok_or_else(|| finstack_quant_core::Error::Calibration {
                            message: format!(
                                "Forward swap quote {} produced no reset intervals",
                                pq.quote.id()
                            ),
                            category: "global_solve".to_string(),
                        })?;
                    Some(self.params.time_day_count.year_fraction(
                        self.params.base_date,
                        maturity,
                        DayCountContext::default(),
                    )?)
                } else {
                    None
                };
                entries.push((parameter_time, fallback_time, initial, quote.clone()));
            }
        }
        deposits.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        let mut previous_deposit: Option<(f64, f64)> = None;
        for (start, end, initial, quote) in deposits {
            let parameter_time = previous_deposit
                .filter(|(previous_start, _)| (*previous_start - start).abs() <= 1e-12)
                .map_or(start, |(_, previous_end)| previous_end);
            entries.push((parameter_time, None, initial, quote));
            previous_deposit = Some((start, end));
        }
        entries.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut resolved_entries = Vec::with_capacity(entries.len());
        for (primary_time, fallback_time, initial, quote) in entries {
            let collision = resolved_entries.iter().position(
                |(time, _, _, _): &(f64, Option<f64>, f64, Self::Quote)| {
                    (primary_time - *time).abs() <= 1e-12
                },
            );
            let time = match (collision, fallback_time) {
                (None, _) => primary_time,
                (Some(_), Some(fallback)) => fallback,
                (Some(position), None) => {
                    let existing_fallback = resolved_entries[position].1.ok_or_else(|| {
                        finstack_quant_core::Error::Calibration {
                            message: format!(
                                "Forward global solve requires distinct reset parameter times; duplicate t={primary_time:.12}"
                            ),
                            category: "global_solve".to_string(),
                        }
                    })?;
                    resolved_entries[position].0 = existing_fallback;
                    primary_time
                }
            };
            if resolved_entries.iter().any(
                |(existing, _, _, _): &(f64, Option<f64>, f64, Self::Quote)| {
                    (time - *existing).abs() <= 1e-12
                },
            ) {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Forward global solve could not disambiguate parameter time t={time:.12}"
                    ),
                    category: "global_solve".to_string(),
                });
            }
            resolved_entries.push((time, fallback_time, initial, quote));
        }
        resolved_entries.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut times = Vec::with_capacity(resolved_entries.len());
        let mut initials = Vec::with_capacity(resolved_entries.len());
        let mut active_quotes = Vec::with_capacity(resolved_entries.len());
        for (time, _, initial, quote) in resolved_entries {
            times.push(time);
            initials.push(initial);
            active_quotes.push(quote);
        }
        self.parameter_count.set(times.len());
        *self.projection_grid.borrow_mut() = Some(projection_grid);
        Ok((times, initials, active_quotes))
    }

    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        if times.len() != params.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Forward global solve dimension mismatch: {} times vs {} params",
                    times.len(),
                    params.len()
                ),
                category: "global_solve".to_string(),
            });
        }
        let control_knots = times
            .iter()
            .copied()
            .zip(params.iter().copied())
            .collect::<Vec<_>>();
        let projection_grid = self.projection_grid.borrow().clone().ok_or_else(|| {
            finstack_quant_core::Error::Calibration {
                message: "Forward global solve projection grid was not initialized".to_string(),
                category: "global_solve".to_string(),
            }
        })?;
        let control_knots = if control_knots.len() == 1 {
            let only_knot = control_knots.first().copied().ok_or_else(|| {
                finstack_quant_core::Error::Calibration {
                    message: "Forward global solve has no control knots".to_string(),
                    category: "global_solve".to_string(),
                }
            })?;
            let terminal =
                *projection_grid
                    .last()
                    .ok_or_else(|| finstack_quant_core::Error::Calibration {
                        message: "Forward projection grid is empty".to_string(),
                        category: "global_solve".to_string(),
                    })?;
            vec![only_knot, (terminal, only_knot.1)]
        } else {
            control_knots
        };
        self.build_curve_with_projection_grid(&control_knots, projection_grid)
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        if residuals.len() != quotes.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Forward global solve requires residuals.len() == quotes.len(); got {} vs {}",
                    residuals.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }
        self.scratch.with_curve(curve, |ctx| {
            for (residual, quote) in residuals.iter_mut().zip(quotes) {
                let pq = match quote {
                    crate::prepared::CalibrationQuote::Rates(pq) => pq,
                    other => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "Forward curve calibration accepts Rates quotes only; got {:?}",
                            std::mem::discriminant(other)
                        )));
                    }
                };
                if !self.params.residual_notional.is_finite() || self.params.residual_notional <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Invalid residual_notional {}: expected finite positive value",
                        self.params.residual_notional
                    )));
                }
                // Deposits price off the discount curve, not the projection curve
                // being fitted. Convert the index-basis forward error into PV per
                // unit notional (`τ·Δr`) so deposits share residual space with
                // FRA / futures / swap `value_raw / notional`.
                *residual = if let RateQuote::Deposit { rate, index, .. } = pq.quote.as_ref() {
                    let (start, end) =
                        self.reset_intervals(pq)?.first().copied().ok_or_else(|| {
                            finstack_quant_core::Error::Calibration {
                                message: format!(
                                    "Forward deposit quote {} produced no reset interval",
                                    pq.quote.id()
                                ),
                                category: "global_solve".to_string(),
                            }
                        })?;
                    let start_time = self.params.time_day_count.year_fraction(
                        self.params.base_date,
                        start,
                        DayCountContext::default(),
                    )?;
                    let end_time = self.params.time_day_count.year_fraction(
                        self.params.base_date,
                        end,
                        DayCountContext::default(),
                    )?;
                    let idx_conv =
                        finstack_quant_valuations::market::conventions::ConventionRegistry::try_global()?
                            .require_rate_index(index)?;
                    let deposit_accrual = idx_conv.day_count.year_fraction(
                        start,
                        end,
                        DayCountContext::default(),
                    )?;
                    if deposit_accrual <= 0.0 {
                        return Err(finstack_quant_core::Error::Calibration {
                            message: format!(
                                "Forward deposit quote {} has non-positive accrual {deposit_accrual}",
                                pq.quote.id()
                            ),
                            category: "global_solve".to_string(),
                        });
                    }
                    let growth =
                        curve.rate_between(start_time, end_time)? * (end_time - start_time);
                    growth - rate * deposit_accrual
                } else {
                    pq.instrument.value_raw(ctx, self.params.base_date)? / self.params.residual_notional
                };
            }
            Ok(())
        })
    }

    fn residual_key(&self, quote: &Self::Quote, idx: usize) -> String {
        match quote {
            crate::prepared::CalibrationQuote::Rates(pq) => pq.quote.id().as_str().to_string(),
            _ => format!("FORWARD-{idx:06}"),
        }
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        for (quote, weight) in quotes.iter().zip(weights_out.iter_mut()) {
            let time = match quote {
                crate::prepared::CalibrationQuote::Rates(pq) => pq.pillar_time.max(1e-6),
                other => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Forward curve calibration accepts Rates quotes only; got {:?}",
                        std::mem::discriminant(other)
                    )));
                }
            };
            let annuity = quote_annuity_proxy(quote, time);
            let pv01_inverse_sq = 1.0 / (annuity * annuity);
            let weight_value = pv01_inverse_sq
                * scheme_factor(&self.params.config.forward_curve.weighting_scheme, time);
            *weight = if weight_value.is_finite() {
                weight_value.max(WEIGHT_MIN_FLOOR)
            } else {
                WEIGHT_MIN_FLOOR
            };
        }
        Ok(())
    }

    fn lower_bounds(&self) -> Option<Vec<f64>> {
        let bounds = self
            .params
            .config
            .effective_rate_bounds(self.params.currency);
        Some(vec![bounds.min_rate; self.parameter_count.get()])
    }

    fn upper_bounds(&self) -> Option<Vec<f64>> {
        let bounds = self
            .params
            .config
            .effective_rate_bounds(self.params.currency);
        Some(vec![bounds.max_rate; self.parameter_count.get()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::prepared::PreparedQuote;
    use crate::prepared::CalibrationQuote;
    use crate::quotes::ids::{Pillar, QuoteId};
    use crate::quotes::market_quote::MarketQuote;
    use crate::quotes::rates::RateQuote;
    use crate::solver::traits::GlobalSolveTarget;
    use crate::{RateBounds, ResidualWeightingScheme};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::{
        RateCalibrationMethod, RateCalibrationPillar, RateCalibrationQuote,
    };
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations::instruments::{Attributes, Instrument};
    use finstack_quant_valuations::market::conventions::ids::IrFutureContractId;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use time::Month;

    #[derive(Clone)]
    struct DummyInstrument;

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        DummyInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for DummyInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }

        fn id(&self) -> &str {
            "dummy"
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Deposit
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::from((0_i64, Currency::USD)))
        }

        fn attributes(&self) -> &Attributes {
            static ATTRS: std::sync::OnceLock<Attributes> = std::sync::OnceLock::new();
            ATTRS.get_or_init(Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            unreachable!("dummy instrument should not mutate attributes")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn global_forward_target_uses_configured_bounds_and_weights() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut config = CalibrationConfig::default()
            .with_rate_bounds(RateBounds::new(-0.01, 0.10).expect("valid explicit rate bounds"));
        config.forward_curve.weighting_scheme = ResidualWeightingScheme::LinearTime;
        let target = ForwardCurveTarget::new(ForwardCurveTargetParams {
            base_date,
            currency: Currency::USD,
            fwd_curve_id: CurveId::new("fwd"),
            tenor_years: 0.25,
            solve_interp: InterpStyle::Linear,
            config,
            time_day_count: DayCount::Act360,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
        });
        let quotes = [(0.25, 0.05), (1.0, 0.20)]
            .into_iter()
            .map(|(pillar_time, quote_rate)| {
                let start_date = base_date + time::Duration::days((pillar_time * 360.0) as i64);
                let maturity = start_date + time::Duration::days(90);
                CalibrationQuote::Rates(PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{pillar_time}")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::ids::Pillar::Date(maturity),
                        rate: quote_rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{pillar_time}")),
                        quote_rate: Some(
                            rust_decimal::Decimal::try_from(quote_rate)
                                .expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
                        spot_lag_days: Some(0),
                        business_day_convention: BusinessDayConvention::Following,
                        calendar_id: None,
                        start_date,
                        maturity,
                        notional: Money::from((1_i64, Currency::USD)),
                        day_count: DayCount::Act360,
                    }),
                    maturity,
                    pillar_time,
                ))
            })
            .collect::<Vec<_>>();

        let _ = GlobalSolveTarget::build_time_grid_and_guesses(&target, &quotes)
            .expect("build forward global grid");
        assert_eq!(
            GlobalSolveTarget::lower_bounds(&target),
            Some(vec![-0.01, -0.01])
        );
        assert_eq!(
            GlobalSolveTarget::upper_bounds(&target),
            Some(vec![0.10, 0.10])
        );

        let mut weights = vec![0.0; quotes.len()];
        GlobalSolveTarget::residual_weights(&target, &quotes, &mut weights)
            .expect("configured residual weights");
        let expected: Vec<f64> = quotes
            .iter()
            .map(|quote| {
                let time = quote.pillar_time().max(1e-6);
                let annuity = quote_annuity_proxy(quote, time);
                let weight = (1.0 / (annuity * annuity)) * time;
                if weight.is_finite() {
                    weight.max(WEIGHT_MIN_FLOOR)
                } else {
                    WEIGHT_MIN_FLOOR
                }
            })
            .collect();
        assert_eq!(weights.len(), expected.len());
        for (got, want) in weights.iter().zip(expected.iter()) {
            assert!(
                (got - want).abs() < 1e-12,
                "PV01-inverse LinearTime weight {got} != {want}"
            );
        }

        let (curve, report) = GlobalFitOptimizer::optimize(
            &target,
            &quotes,
            &target.params.config,
            target.params.config.forward_curve.validation_tolerance,
        )
        .expect("bounded global solve should return its best curve");
        assert!(
            curve.forwards().iter().all(|rate| *rate <= 0.10),
            "all fitted forwards must respect configured upper bound: {:?}",
            curve.forwards()
        );
        assert!(
            !report.success,
            "an unreachable 20% quote under a 10% bound must not report success"
        );
    }

    #[test]
    fn multiple_forward_deposits_use_distinct_end_parameters_and_df_residuals() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let config = CalibrationConfig {
            calibration_method: CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            ..CalibrationConfig::default()
        };
        let target = ForwardCurveTarget::new(ForwardCurveTargetParams {
            base_date,
            currency: Currency::USD,
            fwd_curve_id: CurveId::new("USD-FWD"),
            tenor_years: 0.25,
            solve_interp: InterpStyle::Linear,
            config: config.clone(),
            time_day_count: DayCount::Act360,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
        });
        let quotes = [(90_i64, 0.04), (180_i64, 0.05)]
            .into_iter()
            .map(|(days, rate)| {
                let maturity = base_date + time::Duration::days(days);
                CalibrationQuote::Rates(PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{days}D")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::ids::Pillar::Date(maturity),
                        rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{days}D")),
                        quote_rate: Some(
                            rust_decimal::Decimal::try_from(rate).expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
                        spot_lag_days: Some(0),
                        business_day_convention: BusinessDayConvention::Following,
                        calendar_id: None,
                        start_date: base_date,
                        maturity,
                        notional: Money::from((1_i64, Currency::USD)),
                        day_count: DayCount::Act360,
                    }),
                    maturity,
                    days as f64 / 360.0,
                ))
            })
            .collect::<Vec<_>>();

        let (times, _, _) = GlobalSolveTarget::build_time_grid_and_guesses(&target, &quotes)
            .expect("deposit parameter grid");
        assert_eq!(times, vec![0.0, 0.25]);

        let (curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, 1e-8)
            .expect("multi-deposit global solve");
        assert!(report.success, "{}", report.convergence_reason);
        assert!((curve.rate_between(0.0, 0.25).expect("3M rate") - 0.04).abs() < 1e-8);
        assert!((curve.rate_between(0.0, 0.5).expect("6M rate") - 0.05).abs() < 1e-8);
    }

    /// Regression: with a curve time axis (Act/365F) that differs from the
    /// deposit index day count (Act/360 for USD-SOFR), the deposit residual
    /// must reprice the quote on the *deposit's* accrual basis. The old
    /// shortcut set the Act/365F simple rate equal to the Act/360 quote,
    /// baking a ~365/360 (≈1.4% relative) day-count basis into the front of
    /// the projection curve.
    #[test]
    fn deposit_residual_uses_index_day_count_for_accrual() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let config = CalibrationConfig {
            calibration_method: CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            ..CalibrationConfig::default()
        };
        let target = ForwardCurveTarget::new(ForwardCurveTargetParams {
            base_date,
            currency: Currency::USD,
            fwd_curve_id: CurveId::new("USD-FWD"),
            tenor_years: 0.25,
            solve_interp: InterpStyle::Linear,
            config: config.clone(),
            // Curve time axis deliberately different from the Act/360 index day_count.
            time_day_count: DayCount::Act365F,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
        });
        let quotes = [(90_i64, 0.04), (180_i64, 0.05)]
            .into_iter()
            .map(|(days, rate)| {
                let maturity = base_date + time::Duration::days(days);
                CalibrationQuote::Rates(PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{days}D")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::ids::Pillar::Date(maturity),
                        rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{days}D")),
                        quote_rate: Some(
                            rust_decimal::Decimal::try_from(rate).expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
                        spot_lag_days: Some(0),
                        business_day_convention: BusinessDayConvention::Following,
                        calendar_id: None,
                        start_date: base_date,
                        maturity,
                        notional: Money::from((1_i64, Currency::USD)),
                        day_count: DayCount::Act360,
                    }),
                    maturity,
                    // Pillar time on the curve's Act/365F axis.
                    days as f64 / 365.0,
                ))
            })
            .collect::<Vec<_>>();

        let (curve, report) = GlobalFitOptimizer::optimize(&target, &quotes, &config, 1e-8)
            .expect("mixed day-count deposit global solve");
        assert!(report.success, "{}", report.convergence_reason);

        for (days, quote_rate) in [(90_f64, 0.04), (180_f64, 0.05)] {
            let t_end = days / 365.0;
            let growth = curve.rate_between(0.0, t_end).expect("implied forward") * t_end;
            // Repricing on the deposit's own Act/360 accrual must recover the quote.
            let act360_rate = growth / (days / 360.0);
            assert!(
                (act360_rate - quote_rate).abs() < 1e-8,
                "{days}d deposit reprices to {act360_rate}, expected {quote_rate}"
            );
            // The curve-axis (Act/365F) simple rate is therefore the quote scaled
            // by 365/360 — NOT the raw quote (the old, wrong behaviour).
            let curve_axis_rate = curve.rate_between(0.0, t_end).expect("curve-axis rate");
            assert!(
                (curve_axis_rate - quote_rate * 365.0 / 360.0).abs() < 1e-6,
                "{days}d curve-axis rate {curve_axis_rate} should carry the 365/360 rescale"
            );
        }
    }

    #[test]
    fn par_deposit_and_par_swap_share_pv_per_notional_residuals() {
        let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("valid date");
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 1.0),
                (0.25, (-0.04_f64 * 0.25).exp()),
                (2.0, (-0.04_f64 * 2.0).exp()),
            ])
            .build()
            .expect("discount curve");
        let quotes = [
            MarketQuote::Rates(RateQuote::Deposit {
                id: QuoteId::new("DEP-3M"),
                index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                pillar: Pillar::Tenor("3M".parse().expect("valid tenor")),
                rate: 0.04,
            }),
            MarketQuote::Rates(RateQuote::Swap {
                id: QuoteId::new("SWAP-2Y"),
                index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                pillar: Pillar::Tenor("2Y".parse().expect("valid tenor")),
                rate: 0.04,
                spread_decimal: None,
            }),
        ];
        let params = ForwardCurveParams {
            curve_id: CurveId::new("USD-FWD"),
            currency: Currency::USD,
            base_date,
            tenor_years: 0.25,
            discount_curve_id: CurveId::new("USD-OIS"),
            method: CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            interpolation: InterpStyle::Linear,
            conventions: crate::RatesStepConventions {
                curve_day_count: Some(DayCount::Act365F),
                ois_compounding: None,
            },
        };

        let (_market, report) = ForwardCurveTarget::solve(
            &params,
            &quotes,
            &MarketContext::new().insert(discount),
            &CalibrationConfig::default(),
        )
        .expect("forward calibration");
        assert!(report.success, "{}", report.convergence_reason);
        assert_eq!(
            report.residuals.len(),
            2,
            "deposit and swap must both contribute residuals"
        );
        for (key, residual) in &report.residuals {
            assert!(
                residual.abs() <= 1e-8,
                "{key} residual {residual} is not PV-per-notional (~1e-8); \
                 mixed rate vs annuity units would leave one quote at Δr and the other at A·Δr"
            );
        }
    }

    #[test]
    fn futures_initial_guess_subtracts_convexity_adjustment() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let target = ForwardCurveTarget::new(ForwardCurveTargetParams {
            base_date,
            currency: Currency::USD,
            fwd_curve_id: CurveId::new("fwd"),
            tenor_years: 1.0,
            solve_interp: InterpStyle::Linear,
            config: CalibrationConfig::default(),
            time_day_count: DayCount::Act365F,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
        });

        let quote = CalibrationQuote::Rates(PreparedQuote::new(
            Arc::new(RateQuote::Futures {
                id: QuoteId::new("SR3"),
                contract: IrFutureContractId::new("CME:SR3"),
                expiry: base_date,
                price: 98.50,
                convexity_adjustment: 0.0010,
            }),
            Arc::new(DummyInstrument),
            base_date,
            1.0,
        ));

        let guess = target.initial_guess(&quote).expect("initial guess");
        assert!((guess - 0.014).abs() < 1e-12, "expected 1.40%, got {guess}");
    }

    #[test]
    fn explicit_curve_day_count_controls_forward_curve_and_quote_times() {
        fn calibrate(day_count: DayCount) -> ForwardCurve {
            let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
            let discount = DiscountCurve::builder("USD-OIS")
                .base_date(base_date)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 0.96)])
                .build()
                .expect("discount curve");
            let quote = MarketQuote::Rates(RateQuote::Fra {
                id: QuoteId::new("FRA-3X6"),
                index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                start: crate::quotes::ids::Pillar::Date(base_date + time::Duration::days(90)),
                end: crate::quotes::ids::Pillar::Date(base_date + time::Duration::days(180)),
                rate: 0.04,
            });
            let params = ForwardCurveParams {
                curve_id: CurveId::new("USD-FWD"),
                currency: Currency::USD,
                base_date,
                tenor_years: 0.25,
                discount_curve_id: CurveId::new("USD-OIS"),
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: InterpStyle::Linear,
                conventions: crate::RatesStepConventions {
                    curve_day_count: Some(day_count),
                    ois_compounding: None,
                },
            };

            let (market, report) = ForwardCurveTarget::solve(
                &params,
                &[quote],
                &MarketContext::new().insert(discount),
                &CalibrationConfig::default(),
            )
            .expect("forward calibration");
            assert!(report.success, "{}", report.convergence_reason);
            market
                .get_forward("USD-FWD")
                .expect("calibrated forward")
                .as_ref()
                .clone()
        }

        let act365f = calibrate(DayCount::Act365F);
        let act360 = calibrate(DayCount::Act360);

        assert_eq!(act365f.day_count(), DayCount::Act365F);
        assert_eq!(act360.day_count(), DayCount::Act360);
        let recipe = act365f
            .rate_calibration()
            .expect("forward target must stamp replay recipe");
        assert_eq!(recipe.curve_day_count, DayCount::Act365F);
        assert!(matches!(
            recipe.method,
            RateCalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false
            }
        ));
        assert!(matches!(
            &recipe.role,
            RateCalibrationCurveRole::Projection { discount_curve_id }
                if discount_curve_id.as_str() == "USD-OIS"
        ));

        let grid_365 = act365f.projection_grid().expect("Act365F grid");
        let grid_360 = act360.projection_grid().expect("Act360 grid");
        assert!((grid_365[1] - 90.0 / 365.0).abs() < 1e-12);
        assert!((grid_365[2] - 180.0 / 365.0).abs() < 1e-12);
        assert!((grid_360[1] - 90.0 / 360.0).abs() < 1e-12);
        assert!((grid_360[2] - 180.0 / 360.0).abs() < 1e-12);

        assert!(act365f
            .knots()
            .iter()
            .any(|time| (*time - 90.0 / 365.0).abs() < 1e-12));
        assert!(act360
            .knots()
            .iter()
            .any(|time| (*time - 90.0 / 360.0).abs() < 1e-12));
    }

    #[test]
    fn forward_replay_quote_retains_absolute_date_pillar() {
        let date = Date::from_calendar_date(2025, Month::September, 17).expect("valid pillar date");
        let quote = RateQuote::Swap {
            id: QuoteId::new("SWAP-DATE"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
            pillar: crate::quotes::ids::Pillar::Date(date),
            rate: 0.039,
            spread_decimal: Some(0.00025),
        };

        assert!(matches!(
            RateCalibrationQuote::from(&quote),
            RateCalibrationQuote::Swap {
                pillar: RateCalibrationPillar::Date(value),
                spread_decimal: Some(spread),
                ..
            } if value == date && (spread - 0.00025).abs() < f64::EPSILON
        ));
    }
}
