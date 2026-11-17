//! Forward curve calibration target.

use crate::CalibrationReport;
use crate::api::schema::ForwardCurveParams;
use crate::build::prepared::PreparedQuote;
use crate::config::CalibrationConfig;
use crate::config::CalibrationMethod;
use crate::quotes::ExtractQuotes;
use crate::quotes::MarketQuote;
use crate::quotes::RateQuote;
use crate::solver::global;
use crate::solver::traits::GlobalSolveTarget;
use crate::solver::traits::TimeGridAndGuesses;
use crate::targets::rate_recipe::rate_calibration_recipe;
use crate::targets::util::{
    ContextScratch, discount_and_forward_curve_ids, prepare_rate_calibration_quotes,
    pv01_residual_weight,
};
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{ForwardCurve, RateCalibrationCurveRole};
use finstack_quant_core::math::Compounding;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_rates::deposit::Deposit;
use finstack_quant_valuations_rates::fra::ForwardRateAgreement;
use finstack_quant_valuations_rates::ir_future::InterestRateFuture;
use finstack_quant_valuations_rates::irs::InterestRateSwap;
use std::cell::Cell;

/// Parameters for constructing a `ForwardCurveTarget`.
#[derive(Clone)]
pub(crate) struct ForwardCurveTargetParams {
    /// Base date for the curve (valuation date).
    pub(crate) base_date: Date,
    /// Currency of the forward curve (e.g. USD).
    pub(crate) currency: Currency,
    /// Unique identifier for the forward curve being calibrated.
    pub(crate) fwd_curve_id: CurveId,
    /// Contractual index tenor metadata attached to the calibrated curve.
    pub(crate) index_tenor: Option<Tenor>,
    /// Policy controlling queries beyond the final fitted pillar time.
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Numerical interpolation style used during the solving process.
    pub(crate) solve_interp: InterpStyle,
    /// Global calibration settings (tolerances, rate bounds).
    pub(crate) config: CalibrationConfig,
    /// Convention for converting dates to time axis (year fractions).
    pub(crate) time_day_count: DayCount,
    /// Residual normalization notional (used to scale PV residuals to per-unit notional).
    pub(crate) residual_notional: f64,
}

/// Target for simultaneous forward-curve calibration.
///
/// Maps each quote's projection horizon to an ordered positive pillar grid and
/// solves for piecewise segment continuous forwards parameterizing positive
/// normalized projection factors `P(0) = 1`, `P_i = P_{i-1} * exp(-f_i * dt_i)`.
pub(crate) struct ForwardCurveTarget {
    /// Construction parameters (dates, ids, conventions, config).
    pub(crate) params: ForwardCurveTargetParams,
    /// Reusable scratch context (see [`ContextScratch`]).
    scratch: ContextScratch,
    /// Number of global parameters, populated when the solve grid is built.
    parameter_count: Cell<usize>,
}

impl ForwardCurveTarget {
    /// Create a new `ForwardCurveTarget` from parameters.
    pub(crate) fn new(params: ForwardCurveTargetParams, base_context: MarketContext) -> Self {
        let scratch = ContextScratch::new(base_context);
        Self {
            params,
            scratch,
            parameter_count: Cell::new(0),
        }
    }

    /// Execute the full calibration for a forward curve step.
    pub(crate) fn solve(
        params: &ForwardCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(ForwardCurve, CalibrationReport)> {
        // Forward-curve preflight: prepare quotes; both the discount curve (already in
        // `context`) and the forward curve being built are registered so projected legs
        // price against the right curves.
        let prepared = prepare_rate_calibration_quotes(
            quotes,
            params.base_date,
            discount_and_forward_curve_ids(
                params.discount_curve_id.as_ref(),
                params.curve_id.as_ref(),
            ),
            params.conventions.curve_day_count,
            1_000_000.0,
            params.conventions.ois_compounding,
        )?;
        let prepared_quotes = prepared.quotes;
        let curve_day_count = prepared.curve_day_count;
        let rate_quotes: Vec<RateQuote> = quotes.extract_quotes();
        let rate_calibration = rate_calibration_recipe(
            params.currency,
            &params.method,
            curve_day_count,
            params.conventions.ois_compounding.as_ref(),
            RateCalibrationCurveRole::Projection {
                discount_curve_id: params.discount_curve_id.clone(),
            },
            &rate_quotes,
        )?;

        let config = global_config.clone();

        let target = ForwardCurveTarget::new(
            ForwardCurveTargetParams {
                base_date: params.base_date,
                currency: params.currency,
                fwd_curve_id: params.curve_id.clone(),
                index_tenor: params.index_tenor,
                extrapolation: params.extrapolation,
                solve_interp: params.interpolation,
                config: config.clone(),
                time_day_count: curve_day_count,
                residual_notional: 1_000_000.0,
            },
            context.clone(),
        );

        let success_tolerance = config.forward_curve.validation_tolerance;

        if matches!(params.method, CalibrationMethod::Bootstrap) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Forward curve {} requires CalibrationMethod::GlobalSolve because \
                 DF-chained reset intervals are solved simultaneously; Bootstrap \
                 is sequential and cannot preserve off-grid reset-date quote meaning",
                params.curve_id
            )));
        }

        // Fit piecewise segment continuous forwards parameterizing positive
        // normalized projection factors simultaneously over all quote pillars.
        let (curve, mut report) = global::optimize(
            &target,
            &prepared_quotes,
            &config,
            success_tolerance,
            matches!(
                params.method,
                CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: true
                }
            ),
        )?;
        let curve = curve
            .to_builder_with_id(curve.id().clone())
            .rate_calibration(rate_calibration)
            .build()?;
        report.metadata.insert(
            "forward_parameterization".to_string(),
            "positive_projection_factors".to_string(),
        );

        report.update_solver_config(config.solver);

        Ok((curve, report))
    }

    /// Return the deposit accrual interval `(start_date, maturity)`.
    fn deposit_interval(&self, pq: &PreparedQuote<RateQuote>) -> Result<(Date, Date)> {
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
        Ok((deposit.start_date, deposit.maturity))
    }

    /// Return the contractual terminal projection date for a quote.
    fn pillar_date(&self, pq: &PreparedQuote<RateQuote>) -> Result<Date> {
        match pq.quote.as_ref() {
            RateQuote::Deposit { .. } => Ok(self.deposit_interval(pq)?.1),
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
                Ok(fra.maturity)
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
                Ok(future.resolve_dates()?.2)
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
                let schedule = finstack_quant_cashflows::builder::build_periods(
                    finstack_quant_cashflows::builder::BuildPeriodsParams {
                        start: swap.float_leg.start,
                        end: swap.float_leg.end,
                        frequency: swap.float_leg.frequency,
                        stub: swap.float_leg.stub,
                        business_day_convention: swap.float_leg.business_day_convention,
                        calendar_id: swap
                            .float_leg
                            .calendar_id
                            .as_deref()
                            .unwrap_or(finstack_quant_core::dates::WEEKENDS_ONLY_ID),
                        end_of_month: swap.float_leg.end_of_month,
                        day_count: finstack_quant_core::dates::DayCount::Act365F,
                        payment_lag_days: swap.float_leg.payment_lag_days,
                        reset_lag_days: None,
                        adjust_accrual_dates: false,
                        roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                    },
                )?;
                schedule
                    .last()
                    .map(|period| period.accrual_end.max(period.payment_date))
                    .ok_or_else(|| finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Forward swap quote {} produced an empty floating schedule",
                            pq.quote.id()
                        ),
                        category: "global_solve".to_string(),
                    })
            }
        }
    }
}

impl GlobalSolveTarget for ForwardCurveTarget {
    type Quote = PreparedQuote<RateQuote>;
    type Curve = ForwardCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<TimeGridAndGuesses<Self::Quote>> {
        if quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let mut entries = Vec::with_capacity(quotes.len());
        for pq in quotes {
            let pillar_date = self.pillar_date(pq)?;
            let pillar_time = self.params.time_day_count.year_fraction(
                self.params.base_date,
                pillar_date,
                DayCountContext::default(),
            )?;
            if !pillar_time.is_finite() || pillar_time <= 0.0 {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Forward quote {} has non-positive pillar time {pillar_time:.12} ({pillar_date})",
                        pq.quote.id()
                    ),
                    category: "global_solve".to_string(),
                });
            }
            entries.push((pillar_time, pillar_date, pq.quote.implied_rate()));
        }
        entries.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let mut times: Vec<f64> = Vec::with_capacity(entries.len());
        let mut initials: Vec<f64> = Vec::with_capacity(entries.len());
        let mut prev_date: Option<Date> = None;
        for (time, date, initial) in entries {
            if let (Some(&last_time), Some(last_date)) = (times.last(), prev_date)
                && (time - last_time).abs() <= 1e-12
            {
                if date != last_date {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Distinct forward pillar dates {last_date} and {date} map to identical time t={time:.12} under {:?}",
                            self.params.time_day_count
                        ),
                        category: "global_solve".to_string(),
                    });
                }
                continue;
            }
            times.push(time);
            initials.push(initial);
            prev_date = Some(date);
        }

        self.parameter_count.set(times.len());
        Ok(TimeGridAndGuesses {
            times,
            initials,
            quotes: quotes.to_vec(),
        })
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
        if times.is_empty() {
            return Err(finstack_quant_core::Error::Calibration {
                message: "Forward global solve requires at least one positive pillar".to_string(),
                category: "global_solve".to_string(),
            });
        }

        let mut factors = Vec::with_capacity(times.len() + 1);
        factors.push((0.0, 1.0));
        let mut factor = 1.0;
        let mut prev_t = 0.0;
        for (&t, &seg_rate) in times.iter().zip(params.iter()) {
            let dt = t - prev_t;
            if !t.is_finite() || !seg_rate.is_finite() || dt <= 0.0 {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Invalid forward curve segment: prev_t={prev_t}, t={t}, rate={seg_rate}"
                    ),
                    category: "global_solve".to_string(),
                });
            }
            factor *= Compounding::Continuous.df_from_rate(seg_rate, dt)?;
            factors.push((t, factor));
            prev_t = t;
        }

        ForwardCurve::builder(self.params.fwd_curve_id.clone())
            .base_date(self.params.base_date)
            .index_tenor_opt(self.params.index_tenor)
            .knots(factors)
            .interp(self.params.solve_interp)
            .extrapolation(self.params.extrapolation)
            .day_count(self.params.time_day_count)
            .build()
            .map_err(|error| finstack_quant_core::Error::Calibration {
                message: format!("Failed to build forward projection factor curve: {error}"),
                category: "global_solve".to_string(),
            })
    }

    fn residual_units(&self) -> crate::report::ResidualUnits {
        crate::report::ResidualUnits::PvPerUnitNotional
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
            for (residual, pq) in residuals.iter_mut().zip(quotes) {
                if !self.params.residual_notional.is_finite() || self.params.residual_notional <= 0.0
                {
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
                    let (start, end) = self.deposit_interval(pq)?;
                    let idx_conv =
                        finstack_quant_valuations_core::market::conventions::ConventionRegistry::try_global()?
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
                        curve.rate_between_dates(start, end, deposit_accrual)? * deposit_accrual;
                    growth - rate * deposit_accrual
                } else {
                    pq.instrument.value_raw(ctx, self.params.base_date)?
                        / self.params.residual_notional
                };
            }
            Ok(())
        })
    }

    fn quote_value(&self, quote: &Self::Quote) -> Option<f64> {
        Some(quote.quote.value())
    }

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        quote.quote.id().as_str().to_string()
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        for (quote, weight) in quotes.iter().zip(weights_out.iter_mut()) {
            let time = quote.pillar_time.max(1e-6);
            *weight = pv01_residual_weight(
                quote,
                time,
                &self.params.config.forward_curve.weighting_scheme,
            );
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
    use crate::ResidualWeightingScheme;
    use crate::build::prepared::PreparedQuote;
    use crate::constants::WEIGHT_MIN_FLOOR;
    use crate::quotes::MarketQuote;
    use crate::quotes::RateQuote;
    use crate::quotes::{Pillar, QuoteId};
    use crate::solver::traits::GlobalSolveTarget;
    use crate::targets::util::quote_annuity_proxy;
    use crate::validation::RateBounds;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::{
        RateCalibrationMethod, RateCalibrationPillar, RateCalibrationQuote,
    };
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::market::conventions::ids::IrFutureContractId;
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use time::Month;

    #[derive(Clone)]
    struct DummyInstrument;

    finstack_quant_valuations_core::impl_empty_cashflow_provider!(
        DummyInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for DummyInstrument {
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<
            finstack_quant_valuations_core::instruments::MarketDependencies,
        > {
            Ok(finstack_quant_valuations_core::instruments::MarketDependencies::new())
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

    /// A forward step that omits `method` must default to the only method the
    /// forward target accepts, not to a bootstrap it rejects at solve time.
    #[test]
    fn forward_params_default_to_global_solve() {
        let params: ForwardCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-SOFR-3M",
            "currency": "USD",
            "base_date": "2025-01-15",
            "index_tenor": {"count": 3, "unit": "months"},
            "discount_curve_id": "USD-OIS",
        }))
        .expect("forward params without method parse");
        assert_eq!(
            params.method,
            CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false
            }
        );
    }

    #[test]
    fn global_forward_target_uses_configured_bounds_and_weights() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut config = CalibrationConfig::default()
            .with_rate_bounds(RateBounds::new(-0.01, 0.10).expect("valid explicit rate bounds"));
        config.forward_curve.weighting_scheme = ResidualWeightingScheme::LinearTime;
        let target = ForwardCurveTarget::new(
            ForwardCurveTargetParams {
                base_date,
                currency: Currency::USD,
                fwd_curve_id: CurveId::new("fwd"),
                index_tenor: Some(Tenor::quarterly()),
                extrapolation: ExtrapolationPolicy::Extend,
                solve_interp: InterpStyle::Linear,
                config,
                time_day_count: DayCount::Act360,
                residual_notional: 1.0,
            },
            MarketContext::new(),
        );
        let quotes = [(0.25, 0.05), (1.0, 0.20)]
            .into_iter()
            .map(|(pillar_time, quote_rate)| {
                let start_date = base_date + time::Duration::days((pillar_time * 360.0) as i64);
                let maturity = start_date + time::Duration::days(90);
                PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{pillar_time}")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::Pillar::Date(maturity),
                        rate: quote_rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{pillar_time}")),
                        fixed_rate: Some(
                            rust_decimal::Decimal::try_from(quote_rate)
                                .expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
                        business_day_convention: BusinessDayConvention::Following,
                        calendar_id: None,
                        start_date,
                        maturity,
                        notional: Money::from((1_i64, Currency::USD)),
                        day_count: DayCount::Act360,
                    }),
                    maturity,
                    pillar_time,
                )
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
                let time = quote.pillar_time.max(1e-6);
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

        let (curve, report) = global::optimize(
            &target,
            &quotes,
            &target.params.config,
            target.params.config.forward_curve.validation_tolerance,
            true,
        )
        .expect("bounded global solve should return its best curve");
        for period in curve.knots().windows(2) {
            let rate = (curve.df(period[0]).expect("start df").ln()
                - curve.df(period[1]).expect("end df").ln())
                / (period[1] - period[0]);
            assert!(
                rate <= 0.10 + 1e-12,
                "fitted segment continuous forward exceeds upper bound: {rate}"
            );
        }
        assert!(
            !report.success,
            "an unreachable 20% quote under a 10% bound must not report success"
        );
    }

    #[test]
    fn multiple_forward_deposits_use_distinct_end_parameters_and_df_residuals() {
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let config = CalibrationConfig {
            ..CalibrationConfig::default()
        };
        let target = ForwardCurveTarget::new(
            ForwardCurveTargetParams {
                base_date,
                currency: Currency::USD,
                fwd_curve_id: CurveId::new("USD-FWD"),
                index_tenor: Some(Tenor::quarterly()),
                extrapolation: ExtrapolationPolicy::Extend,
                solve_interp: InterpStyle::Linear,
                config: config.clone(),
                time_day_count: DayCount::Act360,
                residual_notional: 1.0,
            },
            MarketContext::new(),
        );
        let quotes = [(90_i64, 0.04), (180_i64, 0.05)]
            .into_iter()
            .map(|(days, rate)| {
                let maturity = base_date + time::Duration::days(days);
                PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{days}D")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::Pillar::Date(maturity),
                        rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{days}D")),
                        fixed_rate: Some(
                            rust_decimal::Decimal::try_from(rate).expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
                        business_day_convention: BusinessDayConvention::Following,
                        calendar_id: None,
                        start_date: base_date,
                        maturity,
                        notional: Money::from((1_i64, Currency::USD)),
                        day_count: DayCount::Act360,
                    }),
                    maturity,
                    days as f64 / 360.0,
                )
            })
            .collect::<Vec<_>>();

        let TimeGridAndGuesses { times, .. } =
            GlobalSolveTarget::build_time_grid_and_guesses(&target, &quotes)
                .expect("deposit parameter grid");
        assert_eq!(times, vec![0.25, 0.5]);

        let (curve, report) = global::optimize(&target, &quotes, &config, 1e-8, true)
            .expect("multi-deposit global solve");
        assert!(report.success, "{}", report.convergence_reason);
        assert!((curve.rate_between(0.0, 0.25, 0.25).expect("3M rate") - 0.04).abs() < 1e-8);
        assert!((curve.rate_between(0.0, 0.5, 0.5).expect("6M rate") - 0.05).abs() < 1e-8);
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
            ..CalibrationConfig::default()
        };
        let target = ForwardCurveTarget::new(
            ForwardCurveTargetParams {
                base_date,
                currency: Currency::USD,
                fwd_curve_id: CurveId::new("USD-FWD"),
                index_tenor: Some(Tenor::quarterly()),
                extrapolation: ExtrapolationPolicy::Extend,
                solve_interp: InterpStyle::Linear,
                config: config.clone(),
                // Curve time axis deliberately different from the Act/360 index day_count.
                time_day_count: DayCount::Act365F,
                residual_notional: 1.0,
            },
            MarketContext::new(),
        );
        let quotes = [(90_i64, 0.04), (180_i64, 0.05)]
            .into_iter()
            .map(|(days, rate)| {
                let maturity = base_date + time::Duration::days(days);
                PreparedQuote::new(
                    Arc::new(RateQuote::Deposit {
                        id: QuoteId::new(format!("DEP-{days}D")),
                        index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                        pillar: crate::quotes::Pillar::Date(maturity),
                        rate,
                    }),
                    Arc::new(Deposit {
                        id: InstrumentId::new(format!("DEP-{days}D")),
                        fixed_rate: Some(
                            rust_decimal::Decimal::try_from(rate).expect("valid deposit rate"),
                        ),
                        discount_curve_id: CurveId::new("USD-OIS"),
                        instrument_pricing_overrides: Default::default(),
                        metric_pricing_overrides: Default::default(),
                        scenario_pricing_overrides: Default::default(),
                        attributes: Default::default(),
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
                )
            })
            .collect::<Vec<_>>();

        let (curve, report) = global::optimize(&target, &quotes, &config, 1e-8, true)
            .expect("mixed day-count deposit global solve");
        assert!(report.success, "{}", report.convergence_reason);

        for (days, quote_rate) in [(90_f64, 0.04), (180_f64, 0.05)] {
            let t_end = days / 365.0;
            let growth = curve
                .rate_between(0.0, t_end, t_end)
                .expect("implied forward")
                * t_end;
            // Repricing on the deposit's own Act/360 accrual must recover the quote.
            let act360_rate = growth / (days / 360.0);
            assert!(
                (act360_rate - quote_rate).abs() < 1e-8,
                "{days}d deposit reprices to {act360_rate}, expected {quote_rate}"
            );
            // The curve-axis (Act/365F) simple rate is therefore the quote scaled
            // by 365/360 — NOT the raw quote (the old, wrong behaviour).
            let curve_axis_rate = curve
                .rate_between(0.0, t_end, t_end)
                .expect("curve-axis rate");
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
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
            index_tenor: Some(Tenor::quarterly()),
            discount_curve_id: CurveId::new("USD-OIS"),
            method: CalibrationMethod::GlobalSolve {
                use_analytical_jacobian: false,
            },
            interpolation: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::Extend,
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

        let quote = PreparedQuote::new(
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
        );

        let guess = quote.quote.implied_rate();
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
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve");
            let quote = MarketQuote::Rates(RateQuote::Fra {
                id: QuoteId::new("FRA-3X6"),
                index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
                start: crate::quotes::Pillar::Date(base_date + time::Duration::days(90)),
                end: crate::quotes::Pillar::Date(base_date + time::Duration::days(180)),
                rate: 0.04,
            });
            let params = ForwardCurveParams {
                curve_id: CurveId::new("USD-FWD"),
                currency: Currency::USD,
                base_date,
                index_tenor: Some(Tenor::quarterly()),
                discount_curve_id: CurveId::new("USD-OIS"),
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: InterpStyle::Linear,
                extrapolation: ExtrapolationPolicy::Extend,
                conventions: crate::RatesStepConventions {
                    curve_day_count: Some(day_count),
                    ois_compounding: None,
                },
            };

            let (curve, report) = ForwardCurveTarget::solve(
                &params,
                &[quote],
                &MarketContext::new().insert(discount),
                &CalibrationConfig::default(),
            )
            .expect("forward calibration");
            assert!(report.success, "{}", report.convergence_reason);
            curve
        }

        let act365f = calibrate(DayCount::Act365F);
        let act360 = calibrate(DayCount::Act360);

        assert_eq!(act365f.day_count(), DayCount::Act365F);
        assert_eq!(act360.day_count(), DayCount::Act360);
        let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let start = base_date + time::Duration::days(90);
        let end = base_date + time::Duration::days(180);
        for curve in [&act365f, &act360] {
            let growth = curve.df_on_date(start).expect("start DF")
                / curve.df_on_date(end).expect("end DF")
                - 1.0;
            let index_rate = growth / (90.0 / 360.0);
            assert!(
                (index_rate - 0.04).abs() < 1e-8,
                "a curve-clock change must preserve the quoted ACT/360 rate; got {index_rate}"
            );
        }
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

        let grid_365 = act365f.knots();
        let grid_360 = act360.knots();
        assert_eq!(grid_365.len(), 2);
        assert_eq!(grid_360.len(), 2);
        assert!((grid_365[1] - 180.0 / 365.0).abs() < 1e-12);
        assert!((grid_360[1] - 180.0 / 360.0).abs() < 1e-12);
    }

    #[test]
    fn futures_recover_contractual_rate_across_curve_clocks() {
        let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("date");
        let expiry = Date::from_calendar_date(2025, Month::June, 30).expect("expiry");
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.03_f64 * 2.0).exp())])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");
        for contract in ["CME:ED", "CME:SR1"] {
            let quote = MarketQuote::Rates(RateQuote::Futures {
                id: QuoteId::new(contract),
                contract: IrFutureContractId::new(contract),
                expiry,
                price: 96.0,
                convexity_adjustment: 0.0,
            });
            for curve_day_count in [DayCount::Act360, DayCount::Act365F] {
                let params = ForwardCurveParams {
                    curve_id: CurveId::new("USD-FWD"),
                    currency: Currency::USD,
                    base_date,
                    index_tenor: Some(Tenor::quarterly()),
                    discount_curve_id: CurveId::new("USD-OIS"),
                    method: CalibrationMethod::GlobalSolve {
                        use_analytical_jacobian: false,
                    },
                    interpolation: InterpStyle::Linear,
                    extrapolation: ExtrapolationPolicy::Extend,
                    conventions: crate::RatesStepConventions {
                        curve_day_count: Some(curve_day_count),
                        ois_compounding: None,
                    },
                };
                let (curve, report) = ForwardCurveTarget::solve(
                    &params,
                    std::slice::from_ref(&quote),
                    &MarketContext::new().insert(discount.clone()),
                    &CalibrationConfig::default(),
                )
                .expect("future calibration");
                assert!(report.success, "{}", report.convergence_reason);
                let prepared = prepare_rate_calibration_quotes(
                    std::slice::from_ref(&quote),
                    base_date,
                    discount_and_forward_curve_ids("USD-OIS", "USD-FWD"),
                    Some(curve_day_count),
                    1_000_000.0,
                    None,
                )
                .expect("prepared future");
                let prepared = &prepared.quotes[0];
                let future = prepared
                    .instrument
                    .as_any()
                    .downcast_ref::<InterestRateFuture>()
                    .expect("future instrument");
                let (_, start, end) = future.resolve_dates().expect("reference dates");
                let accrual = future
                    .day_count
                    .year_fraction(start, end, DayCountContext::default())
                    .expect("contractual accrual");
                let index_rate = if contract == "CME:ED" {
                    // Term futures are quoted from full-period projection growth.
                    (curve.df_on_date(start).expect("start DF")
                        / curve.df_on_date(end).expect("end DF")
                        - 1.0)
                        / accrual
                } else {
                    future
                        .model_settlement_rate(
                            &MarketContext::new().insert(curve.clone()),
                            base_date,
                        )
                        .expect("arithmetic-average settlement rate")
                };
                assert!(
                    (index_rate - 0.04).abs() < 1e-8,
                    "{contract} on {curve_day_count:?} quotes {index_rate}, expected ACT/360 4%"
                );
            }
        }
    }

    #[test]
    fn forward_replay_quote_retains_absolute_date_pillar() {
        let date = Date::from_calendar_date(2025, Month::September, 17).expect("valid pillar date");
        let quote = RateQuote::Swap {
            id: QuoteId::new("SWAP-DATE"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR-3M"),
            pillar: crate::quotes::Pillar::Date(date),
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
