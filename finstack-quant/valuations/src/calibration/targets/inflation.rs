use crate::calibration::api::schema::InflationCurveParams;
use crate::calibration::config::{CalibrationConfig, CalibrationMethod, ResidualWeightingScheme};
use crate::calibration::constants::{TOLERANCE_DUP_KNOTS, WEIGHT_MIN_FLOOR};
use crate::calibration::prepared::CalibrationQuote;
use crate::calibration::solver::bootstrap::SequentialBootstrapper;
use crate::calibration::solver::global::GlobalFitOptimizer;
use crate::calibration::solver::traits::{BootstrapTarget, GlobalSolveTarget};
use crate::calibration::CalibrationReport;
use crate::market::quotes::inflation::InflationQuote;
use crate::market::quotes::market_quote::{ExtractQuotes, MarketQuote};

use crate::instruments::common_impl::parameters::legs::PayReceive;
use crate::instruments::rates::inflation_swap::{InflationSwap, YoYInflationSwap};
use crate::market::build::prepared::PreparedQuote;
use crate::market::conventions::registry::ConventionRegistry;
use finstack_quant_core::dates::DateExt;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rust_decimal::Decimal;
use std::cell::RefCell;
use std::sync::Arc;

// CPI hard bounds are now configurable via InflationCurveSolveConfig.
// See `config.rs` for the default values (1.0 and 10_000.0).

/// Bootstrapper for inflation curves from inflation swap quotes.
///
/// Implements sequential bootstrapping and global optimization of inflation curves
/// using zero-coupon inflation swap (ZCIS) and year-on-year (YoY) swap quotes
/// with different maturities. The bootstrapper prices synthetic inflation swaps
/// to solve for CPI values that match market quotes.
///
/// # Supported Methods
/// - **Bootstrap**: Sequential solving, one knot at a time (default).
/// - **GlobalSolve**: Simultaneous Levenberg-Marquardt fit of all CPI knots.
pub(crate) struct InflationCurveTarget {
    /// Parameters for the inflation curve (ID, interpolation, etc).
    pub(crate) params: InflationCurveParams,
    /// Baseline market context containing discount curves.
    pub(crate) base_context: MarketContext,
    /// Global calibration settings (used for solver controls and weights).
    pub(crate) config: CalibrationConfig,
    /// Optional reusable context for sequential solvers to reduce memory pressure.
    reuse_context: Option<RefCell<MarketContext>>,
}

impl InflationCurveTarget {
    /// Creates a new inflation curve bootstrapper.
    ///
    /// # Arguments
    ///
    /// * `params` - Parameters defining the inflation curve structure
    /// * `base_context` - Market context containing discount curves
    /// * `config` - Global calibration settings (solver controls and weights)
    ///
    /// # Returns
    ///
    /// A new `InflationCurveTarget` instance ready for calibration.
    pub(crate) fn new(
        params: InflationCurveParams,
        base_context: MarketContext,
        config: CalibrationConfig,
    ) -> Self {
        let reuse_context = if matches!(config.calibration_method, CalibrationMethod::Bootstrap)
            || !config.use_parallel
        {
            Some(RefCell::new(base_context.clone()))
        } else {
            None
        };
        Self {
            params,
            base_context,
            config,
            reuse_context,
        }
    }

    /// Pre-build per-quote instruments so solver residual evaluation is allocation-free.
    pub(crate) fn prepare_quotes(
        &self,
        quotes: Vec<InflationQuote>,
    ) -> Result<Vec<CalibrationQuote>> {
        quotes.into_iter().map(|q| self.prepare_single(q)).collect()
    }

    fn prepare_single(&self, raw: InflationQuote) -> Result<CalibrationQuote> {
        let (maturity, rate, index_name, frequency, convention_id) = match &raw {
            InflationQuote::InflationSwap {
                maturity,
                rate,
                index,
                convention,
                ..
            } => (*maturity, *rate, index.as_str(), None, convention),
            InflationQuote::YoYInflationSwap {
                maturity,
                rate,
                index,
                frequency,
                convention,
                ..
            } => (
                *maturity,
                *rate,
                index.as_str(),
                Some(*frequency),
                convention,
            ),
        };

        // Load conventions
        let conventions =
            ConventionRegistry::try_global()?.require_inflation_swap(convention_id)?;

        if index_name != self.params.index && index_name != self.params.curve_id.as_str() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Quote index {} does not match calibrator index {}",
                index_name, self.params.index
            )));
        }

        let base_date = self.params.base_date;
        let has_index_fixings = self
            .base_context
            .get_inflation_index(self.params.curve_id.as_str())
            .is_ok();

        let (lag, base_cpi) = if let Ok(index) = self
            .base_context
            .get_inflation_index(self.params.curve_id.as_str())
        {
            let base_cpi = index.value_on(base_date).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to resolve base CPI from inflation index '{}': {}",
                    self.params.curve_id.as_str(),
                    e
                ))
            })?;
            (index.lag(), base_cpi)
        } else {
            // Use conventions lag if available, otherwise params
            (
                self.parse_lag(&conventions.inflation_lag.to_string())
                    .or_else(|_| self.parse_lag(&self.params.observation_lag))?,
                self.params.base_cpi,
            )
        };

        let swap: std::sync::Arc<dyn crate::instruments::common_impl::traits::Instrument> =
            if let Some(freq) = frequency {
                let instrument = YoYInflationSwap::builder()
                    .id("CALIB_YOY".into())
                    .notional(Money::new(self.params.notional, self.params.currency))
                    .start_date(base_date)
                    .maturity(maturity)
                    .fixed_rate(Decimal::try_from(rate).map_err(|_| {
                        finstack_quant_core::Error::Input(
                            finstack_quant_core::InputError::ConversionOverflow,
                        )
                    })?)
                    .frequency(freq)
                    .inflation_index_id(self.params.curve_id.clone())
                    .discount_curve_id(self.params.discount_curve_id.clone())
                    .day_count(conventions.day_count)
                    .side(PayReceive::Pay)
                    .lag_override_opt(if has_index_fixings { None } else { Some(lag) })
                    .bdc(conventions.business_day_convention)
                    .calendar_id_opt(Some(conventions.calendar_id.clone().into()))
                    .build()
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                Arc::new(instrument)
            } else {
                let instrument = InflationSwap::builder()
                    .id("CALIB_ZCIS".into())
                    .notional(Money::new(self.params.notional, self.params.currency))
                    .start_date(base_date)
                    .maturity(maturity)
                    .fixed_rate(Decimal::try_from(rate).map_err(|_| {
                        finstack_quant_core::Error::Input(
                            finstack_quant_core::InputError::ConversionOverflow,
                        )
                    })?)
                    .inflation_index_id(self.params.curve_id.clone())
                    .discount_curve_id(self.params.discount_curve_id.clone())
                    .day_count(conventions.day_count)
                    .side(PayReceive::Pay)
                    .lag_override_opt(if has_index_fixings { None } else { Some(lag) })
                    .base_cpi_opt(if has_index_fixings {
                        None
                    } else {
                        Some(base_cpi)
                    })
                    .bdc(conventions.business_day_convention)
                    .calendar_id_opt(Some(conventions.calendar_id.clone().into()))
                    .build()
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                Arc::new(instrument)
            };

        // Calculate pillar time (lagged)
        let fixing_date = Self::apply_lag(maturity, lag);
        let pillar_time = DayCount::Act365F.year_fraction(
            self.params.base_date,
            fixing_date,
            DayCountContext::default(),
        )?;

        let pq = PreparedQuote::new(Arc::new(raw), swap, maturity, pillar_time);
        Ok(CalibrationQuote::Inflation(pq))
    }

    /// Parse an observation lag string (e.g. "3M").
    fn parse_lag(&self, spec: &str) -> Result<InflationLag> {
        let s = spec.trim();
        if s.is_empty() {
            return Ok(InflationLag::None);
        }
        let upper = s.to_ascii_uppercase();
        if upper == "NONE" || upper == "0" || upper == "0M" || upper == "0D" {
            return Ok(InflationLag::None);
        }
        if let Some(num) = upper.strip_suffix('M') {
            let months: u8 = num.trim().parse().map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "Invalid observation_lag '{spec}': expected like '3M'"
                ))
            })?;
            return Ok(InflationLag::Months(months));
        }
        if let Some(num) = upper.strip_suffix('D') {
            let days: u16 = num.trim().parse().map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "Invalid observation_lag '{spec}': expected like '90D'"
                ))
            })?;
            return Ok(InflationLag::Days(days));
        }
        Err(finstack_quant_core::Error::Validation(format!(
            "Invalid observation_lag '{spec}': expected like '3M' or '90D'"
        )))
    }

    /// Apply an observation lag to a date.
    fn apply_lag(
        date: finstack_quant_core::dates::Date,
        lag: InflationLag,
    ) -> finstack_quant_core::dates::Date {
        match lag {
            InflationLag::Months(m) => date.add_months(-(m as i32)),
            InflationLag::Days(d) => date - time::Duration::days(d as i64),
            _ => date,
        }
    }

    /// Resolve the effective base CPI level (from index or params).
    fn effective_base_cpi(&self) -> Result<f64> {
        if let Ok(index) = self
            .base_context
            .get_inflation_index(self.params.curve_id.as_str())
        {
            return index.value_on(self.params.base_date).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to resolve base CPI from inflation index '{}': {}",
                    self.params.curve_id.as_str(),
                    e
                ))
            });
        }
        Ok(self.params.base_cpi)
    }

    fn with_temp_context<F, T>(&self, curve: &InflationCurve, op: F) -> Result<T>
    where
        F: FnOnce(&MarketContext) -> Result<T>,
    {
        if let Some(ctx_cell) = &self.reuse_context {
            let mut ctx = ctx_cell.borrow_mut();
            ctx.insert_mut(curve.clone());
            op(&ctx)
        } else {
            let mut temp_context = self.base_context.clone();
            temp_context.insert_mut(curve.clone());
            op(&temp_context)
        }
    }

    /// Execute the full calibration for an inflation curve step.
    pub(crate) fn solve(
        params: &InflationCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        let inflation_quotes: Vec<InflationQuote> = quotes.extract_quotes();

        if inflation_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let mut config = global_config.clone();
        config.calibration_method = params.method.clone();

        let target = InflationCurveTarget::new(params.clone(), context.clone(), config.clone());
        let prepared_quotes = target.prepare_quotes(inflation_quotes)?;

        // Target-specific validation tolerance for inflation curves.
        let success_tolerance = Some(config.inflation_curve.validation_tolerance);

        let (curve, mut report) = match params.method {
            CalibrationMethod::Bootstrap => SequentialBootstrapper::bootstrap(
                &target,
                &prepared_quotes,
                Vec::new(),
                &config,
                success_tolerance,
                None,
            )?,
            CalibrationMethod::GlobalSolve { .. } => {
                GlobalFitOptimizer::optimize(&target, &prepared_quotes, &config, success_tolerance)?
            }
        };

        report.update_solver_config(config.solver);
        report.metadata.insert(
            "calibration_type".to_string(),
            "inflation_curve".to_string(),
        );
        report
            .metadata
            .insert("curve_id".to_string(), params.curve_id.to_string());
        report
            .metadata
            .insert("index".to_string(), params.index.clone());

        let new_context = context.clone().insert(curve);
        Ok((new_context, report))
    }

    fn cpi_hard_min(&self) -> f64 {
        self.config.inflation_curve.cpi_hard_min
    }

    fn cpi_hard_max(&self) -> f64 {
        self.config.inflation_curve.cpi_hard_max
    }

    /// Compute CPI bounds for a given time based on reasonable inflation rate bounds.
    fn cpi_bounds_for_time(&self, time: f64) -> Result<(f64, f64)> {
        let base_cpi = self.effective_base_cpi()?;
        let min_inflation = -0.10_f64;
        let max_inflation = 0.50_f64;
        let cpi_lo = (base_cpi * (1.0 + min_inflation).powf(time)).max(self.cpi_hard_min());
        let cpi_hi = (base_cpi * (1.0 + max_inflation).powf(time)).min(self.cpi_hard_max());
        Ok((cpi_lo, cpi_hi))
    }

    /// Validate a CPI knot value.
    fn validate_cpi_knot(&self, time: f64, value: f64) -> Result<()> {
        let cpi_min = self.cpi_hard_min();
        let cpi_max = self.cpi_hard_max();
        if !value.is_finite() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Non-finite CPI value for {} at t={:.6}",
                    self.params.curve_id, time
                ),
                category: "bootstrapping".to_string(),
            });
        }
        if value < cpi_min {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "CPI value too low for {} at t={:.6}: {:.6} (min {:.6})",
                    self.params.curve_id, time, value, cpi_min
                ),
                category: "bootstrapping".to_string(),
            });
        }
        if value > cpi_max {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "CPI value too high for {} at t={:.6}: {:.6} (max {:.6})",
                    self.params.curve_id, time, value, cpi_max
                ),
                category: "bootstrapping".to_string(),
            });
        }
        Ok(())
    }
}

impl BootstrapTarget for InflationCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = InflationCurve;

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        Ok(quote.pillar_time())
    }

    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        // knots are (time, cpi)
        // Ensure base point (0.0, base_cpi) is included or added
        let base_cpi = self.effective_base_cpi()?;
        let mut full_knots: Vec<(f64, f64)> = knots
            .iter()
            .copied()
            .filter(|(t, _)| t.abs() > 1e-8)
            .collect();
        full_knots.push((0.0, base_cpi));
        full_knots.sort_by(|a, b| a.0.total_cmp(&b.0));

        InflationCurve::builder(self.params.curve_id.to_string())
            .base_cpi(base_cpi)
            .base_date(self.params.base_date)
            .knots(full_knots)
            .interp(self.params.interpolation)
            .build()
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        let base_date = self.params.base_date;
        // Context needs the curve being calibrated + discount curve
        self.with_temp_context(curve, |ctx| {
            let pv = quote.calibration_value_raw(ctx, base_date)?;
            Ok(pv / self.params.notional)
        })
    }

    fn initial_guess(&self, quote: &Self::Quote, _previous_knots: &[(f64, f64)]) -> Result<f64> {
        let t = self.quote_time(quote)?;
        // We know it's Inflation variant
        let rate = match quote {
            CalibrationQuote::Inflation(pq) => match pq.quote.as_ref() {
                InflationQuote::InflationSwap { rate, .. }
                | InflationQuote::YoYInflationSwap { rate, .. } => *rate,
            },
            _ => 0.02, // Fallback if mismatched type (shouldn't happen)
        };
        let base_cpi = self.effective_base_cpi()?;
        Ok(base_cpi * (1.0 + rate).powf(t))
    }

    fn validate_knot(&self, time: f64, value: f64) -> Result<()> {
        self.validate_cpi_knot(time, value)
    }
}

impl GlobalSolveTarget for InflationCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = InflationCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        let base_cpi = self.effective_base_cpi()?;

        let mut entries = Vec::with_capacity(quotes.len());

        for quote in quotes {
            let t = self.quote_time(quote)?;
            if !t.is_finite() || t <= 0.0 {
                continue;
            }

            // Extract inflation rate from quote for initial guess
            let rate = match quote {
                CalibrationQuote::Inflation(pq) => match pq.quote.as_ref() {
                    InflationQuote::InflationSwap { rate, .. }
                    | InflationQuote::YoYInflationSwap { rate, .. } => *rate,
                },
                _ => 0.02, // Fallback
            };

            // Initial guess: CPI = base_cpi * (1 + rate)^t
            let cpi_guess = base_cpi * (1.0 + rate).powf(t);
            let (cpi_lo, cpi_hi) = self.cpi_bounds_for_time(t)?;
            let clamped_guess = cpi_guess.clamp(cpi_lo, cpi_hi);

            entries.push((t, clamped_guess, quote.clone()));
        }

        // Sort by time
        entries.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut times = Vec::with_capacity(entries.len());
        let mut initials = Vec::with_capacity(entries.len());
        let mut active_quotes = Vec::with_capacity(entries.len());
        let mut last_time: Option<f64> = None;

        for (t, cpi, quote) in entries {
            if let Some(prev) = last_time {
                if (t - prev).abs() <= TOLERANCE_DUP_KNOTS {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Duplicate or unsorted inflation knot times detected (prev={:.10}, new={:.10}). \
Ensure quotes map to strictly increasing year fractions.",
                            prev, t
                        ),
                        category: "global_solve".to_string(),
                    });
                }
            }
            last_time = Some(t);
            times.push(t);
            initials.push(cpi);
            active_quotes.push(quote);
        }

        Ok((times, initials, active_quotes))
    }

    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        self.build_curve_for_solver_from_params(times, params)
    }

    fn build_curve_for_solver_from_params(
        &self,
        times: &[f64],
        params: &[f64],
    ) -> Result<Self::Curve> {
        if times.len() != params.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve dimension mismatch: {} times vs {} params",
                    times.len(),
                    params.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        let base_cpi = self.effective_base_cpi()?;
        let mut knots = Vec::with_capacity(times.len() + 1);
        knots.push((0.0, base_cpi));

        let mut last_t = 0.0;
        for (&t, &cpi) in times.iter().zip(params.iter()) {
            if t <= last_t {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Non-increasing inflation knot time {:.10} detected (previous {:.10}). \
Global solve requires strictly increasing times.",
                        t, last_t
                    ),
                    category: "global_solve".to_string(),
                });
            }
            self.validate_cpi_knot(t, cpi)?;
            last_t = t;
            knots.push((t, cpi));
        }

        InflationCurve::builder(self.params.curve_id.to_string())
            .base_cpi(base_cpi)
            .base_date(self.params.base_date)
            .knots(knots)
            .interp(self.params.interpolation)
            .build()
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        if residuals.len() < quotes.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve residuals buffer too small: got {} need {}",
                    residuals.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        self.with_temp_context(curve, |ctx| {
            for (i, quote) in quotes.iter().enumerate() {
                let pv = quote.calibration_value_raw(ctx, self.params.base_date)?;
                residuals[i] = pv / self.params.notional;
            }
            Ok(())
        })
    }

    fn residual_key(&self, quote: &Self::Quote, idx: usize) -> String {
        let q = quote.get_instrument();
        format!("{}-{:03}", q.id(), idx)
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        if quotes.len() != weights_out.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Global solve requires weights.len() == quotes.len(); got {} vs {}.",
                    weights_out.len(),
                    quotes.len()
                ),
                category: "global_solve".to_string(),
            });
        }

        for (i, quote) in quotes.iter().enumerate() {
            let t = self.quote_time(quote)?.max(1e-6);

            // Use inflation-curve-specific weighting scheme, not discount curve's.
            let weight = match self.config.inflation_curve.weighting_scheme {
                ResidualWeightingScheme::Equal => 1.0,
                ResidualWeightingScheme::LinearTime => t,
                ResidualWeightingScheme::SqrtTime => t.sqrt(),
                ResidualWeightingScheme::InverseDuration => 1.0 / t.max(0.1),
            };

            weights_out[i] = weight.max(WEIGHT_MIN_FLOOR);
        }
        Ok(())
    }
}
