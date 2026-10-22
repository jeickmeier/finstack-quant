//! Calibration target construction and shared input validation.
//!
use crate::api::schema::InflationCurveParams;
use crate::build::prepared::PreparedQuote;
use crate::config::{CalibrationConfig, CalibrationMethod};
use crate::constants::WEIGHT_MIN_FLOOR;
use crate::quotes::InflationQuote;
use crate::quotes::{ExtractQuotes, MarketQuote};
use crate::solver::bootstrap;
use crate::solver::global;
use crate::solver::traits::TimeGridAndGuesses;
use crate::solver::traits::{BootstrapTarget, GlobalSolveTarget};
use crate::targets::util::{
    scheme_factor, sorted_knot_grid, validate_global_knot_grid, ContextScratch,
};
use crate::CalibrationReport;

use finstack_quant_core::dates::DateExt;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{InflationLag, SeriesInterpolation};
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::market::conventions::ConventionRegistry;
use finstack_quant_valuations_rates::inflation_swap::{InflationSwap, YoYInflationSwap};
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
    /// Reusable scratch context (see [`ContextScratch`]).
    scratch: ContextScratch,
}

/// Parse an inflation observation lag the delivered curve can represent:
/// a month lag, or none (an empty spec). `Some(0)` preserves monthly anchoring.
///
/// # Arguments
///
/// * `spec` - Month-lag label such as `3M`, `none`, or an empty string. Day lags
///   are rejected because the delivered curve cannot represent them.
///
/// # Errors
///
/// Returns an error if `spec` does not parse or is a day-based lag, which an
/// inflation curve cannot carry.
pub(crate) fn parse_observation_lag(spec: &str) -> Result<Option<u8>> {
    if spec.trim().is_empty() {
        return Ok(None);
    }
    let lag = spec.parse::<InflationLag>()?;
    match lag {
        InflationLag::Months(months) => Ok(Some(months)),
        InflationLag::None => Ok(None),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "Inflation calibration observation_lag '{spec}' is unsupported: use a month lag or no lag; inflation curves cannot represent day-based observation lags"
        ))),
    }
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
        Self {
            params,
            scratch: ContextScratch::new(base_context.clone()),
            base_context,
            config,
        }
    }

    /// Pre-build per-quote instruments so solver residual evaluation is allocation-free.
    pub(crate) fn prepare_quotes(
        &self,
        quotes: Vec<InflationQuote>,
    ) -> Result<Vec<PreparedQuote<InflationQuote>>> {
        quotes.into_iter().map(|q| self.prepare_single(q)).collect()
    }

    fn prepare_single(&self, raw: InflationQuote) -> Result<PreparedQuote<InflationQuote>> {
        let maturity = raw.maturity();
        let rate = raw.rate();
        let index_name = raw.index();
        let convention_id = raw.convention();
        let frequency = match &raw {
            InflationQuote::InflationSwap { .. } => None,
            InflationQuote::YoYInflationSwap { frequency, .. } => Some(*frequency),
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
        let lag = parse_observation_lag(&self.params.observation_lag)?;
        if let Ok(index) = self
            .base_context
            .get_inflation_index(self.params.curve_id.as_str())
        {
            if index.interpolation() != conventions.interpolation {
                return Err(finstack_quant_core::Error::Validation(
                    "Inflation quote interpolation differs from the observed index convention"
                        .into(),
                ));
            }
        }
        let base_cpi = self.effective_base_cpi()?;

        let swap: std::sync::Arc<dyn finstack_quant_valuations_core::instruments::Instrument> =
            if let Some(frequency) = frequency {
                let instrument = YoYInflationSwap::builder()
                    .id("CALIB_YOY".into())
                    .notional(Money::new(self.params.notional, self.params.currency)?)
                    .start_date(base_date)
                    .maturity(maturity)
                    .fixed_rate(finstack_quant_core::decimal::f64_to_decimal(rate)?)
                    .frequency(frequency)
                    .base_cpi_opt(Some(base_cpi))
                    .inflation_index_id(self.params.curve_id.clone())
                    .discount_curve_id(self.params.discount_curve_id.clone())
                    .day_count(conventions.day_count)
                    .side(PayReceive::Pay)
                    .lag_opt(Some(lag.map_or(InflationLag::None, InflationLag::Months)))
                    .interpolation_opt(Some(conventions.interpolation))
                    .business_day_convention(conventions.business_day_convention)
                    .calendar_id_opt(Some(conventions.calendar_id.clone().into()))
                    .build()
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                Arc::new(instrument)
            } else {
                let instrument = InflationSwap::builder()
                    .id("CALIB_ZCIS".into())
                    .notional(Money::new(self.params.notional, self.params.currency)?)
                    .start_date(base_date)
                    .maturity(maturity)
                    .fixed_rate(finstack_quant_core::decimal::f64_to_decimal(rate)?)
                    .inflation_index_id(self.params.curve_id.clone())
                    .discount_curve_id(self.params.discount_curve_id.clone())
                    .day_count(conventions.day_count)
                    .side(PayReceive::Pay)
                    .lag_opt(Some(lag.map_or(InflationLag::None, InflationLag::Months)))
                    .interpolation_opt(Some(conventions.interpolation))
                    .base_cpi_opt(Some(base_cpi))
                    .business_day_convention(conventions.business_day_convention)
                    .calendar_id_opt(Some(conventions.calendar_id.clone().into()))
                    .build()
                    .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;
                Arc::new(instrument)
            };

        // The final required monthly anchor bounds projection exposure. A daily
        // reference index after day one consumes the next monthly observation.
        let fixing_date = if let Some(months) = lag {
            let first = maturity
                .replace_day(1)
                .map_err(|_| finstack_quant_core::InputError::InvalidDateRange)?;
            let anchor = first.add_months(-i32::from(months))?;
            if conventions.interpolation == SeriesInterpolation::Linear && maturity.day() > 1 {
                anchor.add_months(1)?
            } else {
                anchor
            }
        } else {
            Self::apply_lag(maturity, lag)?
        };
        let pillar_time = DayCount::Act365F.year_fraction(
            self.reference_date()?,
            fixing_date,
            DayCountContext::default(),
        )?;

        Ok(PreparedQuote::new(
            Arc::new(raw),
            swap,
            maturity,
            pillar_time,
        ))
    }

    /// Preserve the calibration lag for instruments that inherit curve metadata.
    fn indexation_lag_months(&self) -> Result<u32> {
        Ok(u32::from(
            parse_observation_lag(&self.params.observation_lag)?.unwrap_or(0),
        ))
    }

    /// Apply an observation lag to a date.
    fn apply_lag(
        date: finstack_quant_core::dates::Date,
        lag: Option<u8>,
    ) -> finstack_quant_core::Result<finstack_quant_core::dates::Date> {
        match lag {
            Some(months) => date.add_months(-i32::from(months)),
            None => Ok(date),
        }
    }

    /// Date represented by the lagged reference CPI at zero curve time.
    fn reference_date(&self) -> Result<finstack_quant_core::dates::Date> {
        Self::apply_lag(
            self.params.base_date,
            parse_observation_lag(&self.params.observation_lag)?,
        )
    }

    /// Build the CPI curve from knots that already include the `(0, base_cpi)` point.
    fn curve_from_knots(&self, base_cpi: f64, knots: Vec<(f64, f64)>) -> Result<InflationCurve> {
        InflationCurve::builder(self.params.curve_id.to_string())
            .base_cpi(base_cpi)
            .base_date(self.reference_date()?)
            .indexation_lag_months(self.indexation_lag_months()?)
            .knots(knots)
            .interp(self.params.interpolation)
            .build()
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

    /// Execute the full calibration for an inflation curve step.
    pub(crate) fn solve(
        params: &InflationCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(InflationCurve, CalibrationReport)> {
        let inflation_quotes: Vec<InflationQuote> = quotes.extract_quotes();

        if inflation_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }

        let config = global_config.clone();

        let target = InflationCurveTarget::new(params.clone(), context.clone(), config.clone());
        let prepared_quotes = target.prepare_quotes(inflation_quotes)?;

        // Target-specific validation tolerance for inflation curves.
        let success_tolerance = config.inflation_curve.validation_tolerance;

        let (curve, mut report) = match params.method {
            CalibrationMethod::Bootstrap => bootstrap::bootstrap(
                &target,
                &prepared_quotes,
                Vec::new(),
                &config,
                success_tolerance,
            )?,
            CalibrationMethod::GlobalSolve {
                use_analytical_jacobian,
            } => global::optimize(
                &target,
                &prepared_quotes,
                &config,
                success_tolerance,
                use_analytical_jacobian,
            )?,
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

        Ok((curve, report))
    }

    /// Compute CPI bounds for a given time based on reasonable inflation rate bounds.
    fn cpi_bounds_for_time(&self, time: f64) -> Result<(f64, f64)> {
        let base_cpi = self.effective_base_cpi()?;
        let min_inflation = -0.10_f64;
        let max_inflation = 0.50_f64;
        let cpi_lo = (base_cpi * (1.0 + min_inflation).powf(time))
            .max(self.config.inflation_curve.cpi_hard_min);
        let cpi_hi = (base_cpi * (1.0 + max_inflation).powf(time))
            .min(self.config.inflation_curve.cpi_hard_max);
        Ok((cpi_lo, cpi_hi))
    }

    /// Validate a CPI knot value.
    fn validate_cpi_knot(&self, time: f64, value: f64) -> Result<()> {
        let cpi_min = self.config.inflation_curve.cpi_hard_min;
        let cpi_max = self.config.inflation_curve.cpi_hard_max;
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
    type Quote = PreparedQuote<InflationQuote>;
    type Curve = InflationCurve;

    fn quote_value(&self, quote: &Self::Quote) -> Option<f64> {
        Some(quote.quote.rate())
    }

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        quote.quote.id().as_str().to_string()
    }

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        Ok(quote.pillar_time)
    }

    fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
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

        self.curve_from_knots(base_cpi, full_knots)
    }

    fn residual_units(&self) -> crate::report::ResidualUnits {
        crate::report::ResidualUnits::PvPerUnitNotional
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        let base_date = self.params.base_date;
        // Context needs the curve being calibrated + discount curve
        self.scratch.with_curve(curve, |ctx| {
            let pv = quote.instrument.value_raw(ctx, base_date)?;
            Ok(pv / self.params.notional)
        })
    }

    fn initial_guess(&self, quote: &Self::Quote, _previous_knots: &[(f64, f64)]) -> Result<f64> {
        let t = self.quote_time(quote)?;
        let rate = quote.quote.rate();
        let base_cpi = self.effective_base_cpi()?;
        Ok(base_cpi * (1.0 + rate).powf(t))
    }

    fn validate_knot(&self, time: f64, value: f64) -> Result<()> {
        self.validate_cpi_knot(time, value)
    }
}

impl GlobalSolveTarget for InflationCurveTarget {
    type Quote = PreparedQuote<InflationQuote>;
    type Curve = InflationCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<TimeGridAndGuesses<Self::Quote>> {
        let base_cpi = self.effective_base_cpi()?;

        let mut entries = Vec::with_capacity(quotes.len());

        for quote in quotes {
            let t = self.quote_time(quote)?;
            if !t.is_finite() || t <= 0.0 {
                continue;
            }

            let rate = quote.quote.rate();

            // Initial guess: CPI = base_cpi * (1 + rate)^t
            let cpi_guess = base_cpi * (1.0 + rate).powf(t);
            let (cpi_lo, cpi_hi) = self.cpi_bounds_for_time(t)?;
            let clamped_guess = cpi_guess.clamp(cpi_lo, cpi_hi);

            entries.push((t, clamped_guess, quote.clone()));
        }

        sorted_knot_grid(entries, "inflation")
    }

    fn build_curve_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        validate_global_knot_grid(times, params, "inflation ")?;

        let base_cpi = self.effective_base_cpi()?;
        let mut knots = Vec::with_capacity(times.len() + 1);
        knots.push((0.0, base_cpi));

        for (&t, &cpi) in times.iter().zip(params.iter()) {
            self.validate_cpi_knot(t, cpi)?;
            knots.push((t, cpi));
        }

        self.curve_from_knots(base_cpi, knots)
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

        self.scratch.with_curve(curve, |ctx| {
            for (i, quote) in quotes.iter().enumerate() {
                let pv = quote.instrument.value_raw(ctx, self.params.base_date)?;
                residuals[i] = pv / self.params.notional;
            }
            Ok(())
        })
    }

    fn quote_value(&self, quote: &Self::Quote) -> Option<f64> {
        Some(quote.quote.rate())
    }

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        quote.quote.id().as_str().to_string()
    }

    fn residual_weights(&self, quotes: &[Self::Quote], weights_out: &mut [f64]) -> Result<()> {
        for (i, quote) in quotes.iter().enumerate() {
            let t = self.quote_time(quote)?.max(1e-6);

            // Use inflation-curve-specific weighting scheme, not discount curve's.
            weights_out[i] = scheme_factor(&self.config.inflation_curve.weighting_scheme, t)
                .max(WEIGHT_MIN_FLOOR);
        }
        Ok(())
    }
}
