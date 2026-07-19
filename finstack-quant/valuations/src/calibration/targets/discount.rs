//! Calibration target construction and shared input validation.
//!
use crate::calibration::api::schema::DiscountCurveParams;
use crate::calibration::config::ResidualWeightingScheme;
use crate::calibration::config::{CalibrationConfig, CalibrationMethod};
use crate::calibration::constants::*;
use crate::calibration::prepared::CalibrationQuote;
use crate::calibration::solver::bootstrap::SequentialBootstrapper;
use crate::calibration::solver::global::GlobalFitOptimizer;
use crate::calibration::solver::traits::{BootstrapTarget, GlobalSolveTarget};
use crate::calibration::targets::util::{
    discount_and_forward_curve_ids, prepare_rate_calibration_quotes_with_ois_override,
    ContextScratch,
};
use crate::calibration::validation::RateBoundsPolicy;
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::ExtractQuotes;
use crate::market::quotes::market_quote::MarketQuote;
use crate::market::quotes::rates::RateQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuote,
    DiscountCurveRateQuoteType, RateCalibrationCurveRole, RateCalibrationFutureContractId,
    RateCalibrationMethod, RateCalibrationOisCompounding, RateCalibrationPillar,
    RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Parameters for constructing a [`DiscountCurveTarget`].
///
/// This struct consolidates all inputs required to execute a discount curve
/// calibration, including base dates, currency, and multi-curve convention IDs.
#[derive(Clone)]
pub(crate) struct DiscountCurveTargetParams {
    /// Base date for the curve (usually the calibration valuation date).
    pub(crate) base_date: Date,
    /// Currency of the curve and its associated instruments.
    pub(crate) currency: Currency,
    /// Identifier for the curve being built.
    pub(crate) curve_id: CurveId,
    /// Effective ID for pricing (usually same as curve_id).
    pub(crate) discount_curve_id: CurveId,
    /// Interpolation style for solving.
    pub(crate) solve_interp: InterpStyle,
    /// Extrapolation policy.
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Calibration configuration.
    pub(crate) config: CalibrationConfig,
    /// Day count convention for mapping dates to year fractions on the curve.
    pub(crate) curve_day_count: DayCount,
    /// Optional spot knot (t_spot, 1.0) if enabled.
    pub(crate) spot_knot: Option<(f64, f64)>,
    /// Residual normalization notional (used to scale PV residuals to per-unit notional).
    ///
    /// Calibration tolerances are interpreted in **per-notional** residual units, so
    /// a realistic notional can be used for instrument construction without making
    /// solver tolerances unrealistically tight in absolute currency terms.
    pub(crate) residual_notional: f64,
    /// Context needed for pricing against OTHER curves (if any).
    pub(crate) base_context: MarketContext,
    /// OIS rate cut-off (business days) the bootstrap-internal swaps are priced
    /// under, derived from the step-level `ois_compounding` override. Stamped
    /// onto every solver and final curve so calibration and pricing agree on
    /// the compounded fast-path decision. `None` for non-cut-off conventions.
    pub(crate) calibration_ois_cutoff_days: Option<i32>,
}

/// Target for discount curve calibration (Bootstrap and Global).
///
/// This struct implements the calibration logic for IR discount curves,
/// supporting both sequential bootstrapping and simultaneous global optimization.
/// It acts as a bridge between the numerical solvers and the financial instrument
/// pricing logic.
///
/// # Examples
///
/// ```ignore
/// // DiscountCurveTarget is internal - use the calibration API instead
/// use finstack_quant_valuations::calibration::{calibrate_discount_curve, DiscountCurveParams};
///
/// let params = DiscountCurveParams { /* ... */ };
/// let curve = calibrate_discount_curve(&params, &quotes)?;
/// ```
pub(crate) struct DiscountCurveTarget {
    /// Base date for the curve.
    pub(crate) base_date: Date,
    /// Currency of the curve.
    pub(crate) currency: Currency,
    /// Identifier for the curve being built.
    pub(crate) curve_id: CurveId,
    /// Effective ID for pricing (usually same as curve_id).
    pub(crate) discount_curve_id: CurveId,
    /// Interpolation style for solving.
    pub(crate) solve_interp: InterpStyle,
    /// Extrapolation policy.
    pub(crate) extrapolation: ExtrapolationPolicy,
    /// Calibration configuration.
    pub(crate) config: CalibrationConfig,
    /// Day count convention for the curve.
    pub(crate) curve_day_count: DayCount,
    /// Optional spot knot (t_spot, 1.0) if enabled.
    pub(crate) spot_knot: Option<(f64, f64)>,
    /// Residual normalization notional.
    pub(crate) residual_notional: f64,
    /// OIS rate cut-off (business days) the bootstrap-internal swaps are priced
    /// under. Stamped onto every solver/final curve. `None` for non-cut-off.
    pub(crate) calibration_ois_cutoff_days: Option<i32>,
    /// Reusable scratch context: holds the base market data plus a slot for the
    /// in-progress curve so each residual evaluation does not have to clone the
    /// full `MarketContext`.
    scratch: ContextScratch,
    /// Optional seed curve used to initialize global solves.
    initial_curve: Option<DiscountCurve>,
}

impl DiscountCurveTarget {
    /// Create a new [`DiscountCurveTarget`] from parameters.
    pub(crate) fn new(params: DiscountCurveTargetParams) -> Self {
        let scratch = ContextScratch::new(params.base_context);
        Self {
            base_date: params.base_date,
            currency: params.currency,
            curve_id: params.curve_id,
            discount_curve_id: params.discount_curve_id,
            solve_interp: params.solve_interp,
            extrapolation: params.extrapolation,
            config: params.config,
            curve_day_count: params.curve_day_count,
            residual_notional: params.residual_notional,
            calibration_ois_cutoff_days: params.calibration_ois_cutoff_days,
            spot_knot: params.spot_knot,
            scratch,
            initial_curve: None,
        }
    }

    /// Compute DF bounds for time t based on rate bounds.
    fn df_bounds_for_time(&self, time: f64) -> (f64, f64) {
        let bounds = self.config.effective_rate_bounds(self.currency);
        let df_lo = (-bounds.max_rate * time).exp();
        let df_hi = (-bounds.min_rate * time).exp();
        (df_lo, df_hi)
    }

    /// Helper to convert zero rate to DF.
    fn zero_rate_to_df(z: f64, t: f64) -> f64 {
        (-z * t).exp()
    }

    /// Generate maturity-aware scan grid.
    fn maturity_aware_scan_grid(
        df_lo: f64,
        df_hi: f64,
        initial_df: f64,
        num_points: usize,
    ) -> Vec<f64> {
        let mut grid = Vec::with_capacity(num_points + 20);

        // Add boundary points
        Self::add_boundary_points(&mut grid, df_lo, df_hi);

        // Add coarse grid points
        let center = initial_df.clamp(df_lo, df_hi);
        let use_log_spacing = df_lo > DF_MIN_HARD && df_hi / df_lo > 10.0;
        Self::add_coarse_grid_points(&mut grid, df_lo, df_hi, num_points, use_log_spacing);

        // Add fine grid around center
        Self::add_fine_grid_around_center(&mut grid, center, df_lo, df_hi);

        // Sort and deduplicate
        grid.sort_by(|a, b| b.total_cmp(a));
        grid.dedup_by(|a, b| (*a - *b).abs() < (df_hi - df_lo) * TOLERANCE_GRID_DEDUP);
        grid
    }

    /// Add boundary points to the grid if valid.
    fn add_boundary_points(grid: &mut Vec<f64>, df_lo: f64, df_hi: f64) {
        if Self::is_valid_df(df_lo) {
            grid.push(df_lo);
        }
        if Self::is_valid_df(df_hi) && (df_hi - df_lo).abs() > TOLERANCE_DUP_KNOTS {
            grid.push(df_hi);
        }
    }

    /// Add coarse grid points between bounds using linear or log spacing.
    fn add_coarse_grid_points(
        grid: &mut Vec<f64>,
        df_lo: f64,
        df_hi: f64,
        num_points: usize,
        use_log_spacing: bool,
    ) {
        let coarse_points = num_points.saturating_sub(10).max(8);

        for i in 1..coarse_points {
            let t = i as f64 / coarse_points as f64;
            let df = if use_log_spacing {
                Self::interpolate_log(df_lo, df_hi, t)
            } else {
                Self::interpolate_linear(df_lo, df_hi, t)
            };

            if Self::is_valid_df_in_range(df, df_lo, df_hi) {
                grid.push(df);
            }
        }
    }

    /// Add fine grid points around the center for better precision.
    fn add_fine_grid_around_center(grid: &mut Vec<f64>, center: f64, df_lo: f64, df_hi: f64) {
        if !Self::is_valid_df(center) {
            return;
        }

        grid.push(center);

        let log_center = center.ln();
        let log_lo = df_lo.max(DF_MIN_HARD).ln();
        let log_hi = df_hi.max(DF_MIN_HARD).ln();

        const LOG_STEPS: [f64; 8] = [1e-4, 2e-4, 5e-4, 1e-3, 2e-3, 5e-3, 1e-2, 2e-2];

        for step in LOG_STEPS {
            Self::try_add_log_offset(grid, log_center, -step, log_lo, log_hi, df_lo, df_hi);
            Self::try_add_log_offset(grid, log_center, step, log_lo, log_hi, df_lo, df_hi);
        }
    }

    /// Try to add a point at log_center + offset if it's valid.
    fn try_add_log_offset(
        grid: &mut Vec<f64>,
        log_center: f64,
        offset: f64,
        log_lo: f64,
        log_hi: f64,
        df_lo: f64,
        df_hi: f64,
    ) {
        let candidate = log_center + offset;
        if candidate < log_lo || candidate > log_hi {
            return;
        }
        let df = candidate.exp();
        if Self::is_valid_df_in_range(df, df_lo, df_hi) {
            grid.push(df);
        }
    }

    /// Check if a discount factor is valid (finite and positive).
    #[inline]
    fn is_valid_df(df: f64) -> bool {
        df.is_finite() && df > 0.0
    }

    /// Check if a discount factor is valid and within the given range (exclusive bounds).
    #[inline]
    fn is_valid_df_in_range(df: f64, lo: f64, hi: f64) -> bool {
        df.is_finite() && df > 0.0 && df > lo && df < hi
    }

    /// Interpolate linearly between two values.
    #[inline]
    fn interpolate_linear(lo: f64, hi: f64, t: f64) -> f64 {
        lo + t * (hi - lo)
    }

    /// Interpolate logarithmically between two values.
    #[inline]
    fn interpolate_log(lo: f64, hi: f64, t: f64) -> f64 {
        let log_lo = lo.ln();
        let log_hi = hi.ln();
        (log_lo + t * (log_hi - log_lo)).exp()
    }

    /// Closed-form proxy for a par instrument's fixed-leg annuity (PV01) at maturity
    /// `t`, used by [`Self::residual_weights`] to put residuals on a common rate-error
    /// scale (item 4).
    ///
    /// The continuously-discounted annuity of a unit-coupon par instrument is
    /// `A(t, r) = ∫₀ᵗ e^{−r·s} ds = (1 − e^{−r·t}) / r`, with the well-defined limit
    /// `A → t` as `r → 0`. This is exact for a continuously-paid coupon and a tight
    /// proxy for the discrete fixed-leg annuity `Σ τ_i·DF_i` of swaps; for a single-
    /// period instrument (deposit / FRA) it reduces to `≈ t`, which is the correct
    /// PV01 there. The proxy needs only the quote's own par rate, so it works inside
    /// `residual_weights` where no calibrated curve is yet available.
    ///
    /// `r` is taken from the quote when it carries a par rate (rate quotes); other
    /// quote kinds fall back to a small representative rate, for which `A ≈ t`.
    fn quote_annuity_proxy(&self, quote: &CalibrationQuote, t: f64) -> f64 {
        // Representative rate for the discount factor in the annuity integral.
        let r = match quote {
            CalibrationQuote::Rates(pq) => match pq.quote.as_ref() {
                // Deposit / FRA / Swap quote `value()` IS the rate (decimal).
                RateQuote::Deposit { rate, .. }
                | RateQuote::Fra { rate, .. }
                | RateQuote::Swap { rate, .. } => *rate,
                // A future quotes a *price* (e.g. 98.5); the implied rate is
                // `(100 − price)/100`. Using the price directly would be nonsense.
                RateQuote::Futures { price, .. } => (100.0 - price) / 100.0,
            },
            // Inflation / xccy-basis quotes do not carry a comparable fixed par rate;
            // a small rate makes the proxy degrade gracefully to `A ≈ t`.
            _ => 0.0,
        };
        // Use the absolute rate: a negative-rate regime (EUR/JPY) still has a
        // well-defined positive annuity, and `(1 − e^{−r·t})/r` is symmetric in the
        // sign of `r` only to second order — `|r|` keeps the proxy stable and positive.
        let r_abs = if r.is_finite() { r.abs() } else { 0.0 };
        let t_pos = t.max(1e-6);
        let annuity = if r_abs < 1e-8 {
            // r → 0 limit: A(t, 0) = t.
            t_pos
        } else {
            (1.0 - (-r_abs * t_pos).exp()) / r_abs
        };
        // Floor strictly positive so the `1/A²` weight is always finite.
        annuity.max(1e-6)
    }

    fn knots_from_params(&self, times: &[f64], params: &[f64]) -> Result<Vec<(f64, f64)>> {
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

        let mut knots = Vec::with_capacity(times.len() + 2);
        knots.push((0.0, 1.0));

        if let Some(spot) = self.spot_knot {
            if spot.0 > 0.0 {
                knots.push(spot);
            }
        }

        let bounds = self.config.effective_rate_bounds(self.currency);
        let mut last_t = self.spot_knot.map(|spot| spot.0).unwrap_or(0.0);

        for (&t, &z) in times.iter().zip(params.iter()) {
            if t <= last_t {
                return Err(finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Non-increasing knot time {:.10} detected (previous {:.10}). \
Global solve requires strictly increasing times.",
                        t, last_t
                    ),
                    category: "global_solve".to_string(),
                });
            }
            last_t = t;
            let clamped_z = z.clamp(bounds.min_rate, bounds.max_rate);
            let mut df = Self::zero_rate_to_df(clamped_z, t);
            df = df.clamp(
                self.config.discount_curve.df_hard_min,
                self.config.discount_curve.df_hard_max,
            );
            knots.push((t, df));
        }

        Ok(knots)
    }

    /// Execute the full calibration for a discount curve step.
    pub(crate) fn solve(
        params: &DiscountCurveParams,
        quotes: &[MarketQuote],
        context: &MarketContext,
        global_config: &CalibrationConfig,
    ) -> Result<(MarketContext, CalibrationReport)> {
        // Apply step-level calibration method preferences to the shared config.
        let mut config = global_config.clone();
        config.calibration_method = params.method.clone();

        // Build rate_calibration sidecar before consuming rates_quotes so that the
        // produced curve carries the original benchmark quotes.  Downstream metrics
        // that re-bump and re-bootstrap (e.g. CDS option IR DV01) require this.
        let raw_rates_quotes: Vec<crate::market::quotes::rates::RateQuote> =
            quotes.extract_quotes();
        if raw_rates_quotes.is_empty() {
            return Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            ));
        }
        // Curve time axis day count:
        //
        // In the prior calibration engine (master-era `calibration/adapters/discount.rs`),
        // the curve day count was a *required* input and (for Bloomberg-style OIS)
        // is typically **Act/365F**, even though many of the underlying instruments
        // (OIS, deposits) accrue on Act/360.
        //
        // Defaulting to the quote accrual day-count (often Act/360) materially shifts
        // the curve's time mapping and can produce large zero-rate/DF differences when
        // compared to vendor curves that report Act/365F continuous zeros.
        let curve_dc = params
            .conventions
            .curve_day_count
            .unwrap_or(finstack_quant_core::dates::DayCount::Act365F);

        let discount_id = params
            .pricing_discount_id
            .as_ref()
            .unwrap_or(&params.curve_id);
        let forward_id = params
            .pricing_forward_id
            .as_ref()
            .unwrap_or(&params.curve_id);
        let rate_calibration_sidecar =
            Self::build_rate_calibration_from_quotes(&raw_rates_quotes, params.currency);
        let rate_calibration_recipe =
            Self::rate_calibration_recipe(params, curve_dc, forward_id.clone(), &raw_rates_quotes);

        // Notional 1_000_000 keeps coupon-money construction off floating-point
        // rounding edges. The OIS-compounding override is threaded through so
        // bootstrap-internal swaps for OvernightRfr indices use the step-level
        // selection instead of the registry default.
        let residual_notional: f64 = 1_000_000.0;
        let prepared = prepare_rate_calibration_quotes_with_ois_override(
            quotes,
            params.base_date,
            discount_and_forward_curve_ids(discount_id.as_ref(), forward_id.as_ref()),
            Some(curve_dc),
            residual_notional,
            params.conventions.ois_compounding.clone(),
        )?;
        let prepared_quotes = prepared.quotes;

        let mut target = DiscountCurveTarget::new(DiscountCurveTargetParams {
            base_date: params.base_date,
            currency: params.currency,
            curve_id: params.curve_id.clone(),
            discount_curve_id: params
                .pricing_discount_id
                .clone()
                .unwrap_or(params.curve_id.clone()),
            solve_interp: params.interpolation,
            extrapolation: params.extrapolation,
            config: config.clone(),
            curve_day_count: curve_dc,
            spot_knot: None,
            residual_notional,
            base_context: context.clone(),
            calibration_ois_cutoff_days: Self::calibration_ois_cutoff_days(
                params.conventions.ois_compounding.as_ref(),
            ),
        });

        // Target-specific validation tolerance for discount curves.
        let success_tolerance = Some(config.discount_curve.validation_tolerance);

        // Optional bootstrap seeding for the global solve to improve convergence and accuracy.
        let mut seed_report: Option<CalibrationReport> = None;
        let mut seed_error: Option<String> = None;
        if matches!(params.method, CalibrationMethod::GlobalSolve { .. })
            && config.discount_curve.bootstrap_seed_global_solve
        {
            let seed_quotes = Clone::clone(&prepared_quotes);

            match SequentialBootstrapper::bootstrap(
                &target,
                &seed_quotes,
                vec![(0.0, 1.0)],
                &config,
                success_tolerance,
                None,
            ) {
                Ok((curve_seed, report)) => {
                    target.initial_curve = Some(curve_seed);
                    seed_report = Some(report);
                }
                Err(e) => {
                    seed_error = Some(format!("{e}"));
                }
            }
        }

        let (mut curve, mut report) = match params.method {
            CalibrationMethod::Bootstrap => {
                let (seed_curve, seed_report) = SequentialBootstrapper::bootstrap(
                    &target,
                    &prepared_quotes,
                    vec![(0.0, 1.0)],
                    &config,
                    success_tolerance,
                    None,
                )?;
                if matches!(
                    params.interpolation,
                    InterpStyle::Linear | InterpStyle::LogLinear
                ) {
                    (seed_curve, seed_report)
                } else {
                    // Non-local interpolation lets later knots move earlier
                    // quote residuals, so a single causal bootstrap sweep is
                    // only a seed. Polish all knots simultaneously to preserve
                    // the caller's requested final interpolation while meeting
                    // the same fit tolerance.
                    target.initial_curve = Some(seed_curve);
                    let refinement_config = CalibrationConfig {
                        calibration_method: CalibrationMethod::GlobalSolve {
                            use_analytical_jacobian: true,
                        },
                        ..config.clone()
                    };
                    GlobalFitOptimizer::optimize(
                        &target,
                        &prepared_quotes,
                        &refinement_config,
                        success_tolerance,
                    )?
                }
            }
            CalibrationMethod::GlobalSolve { .. } => {
                GlobalFitOptimizer::optimize(&target, &prepared_quotes, &config, success_tolerance)?
            }
        };

        // Prefer a high-quality bootstrap seed if it outperforms the global solve.
        if let (CalibrationMethod::GlobalSolve { .. }, Some(seed_rep)) =
            (&params.method, seed_report.as_ref())
        {
            if seed_rep.success
                && seed_rep.max_residual <= config.discount_curve.validation_tolerance
                && seed_rep.max_residual <= report.max_residual
            {
                if let Some(seed_curve) = target.initial_curve {
                    curve = seed_curve;
                    report = seed_rep.clone();
                    report
                        .metadata
                        .insert("adopted_bootstrap_seed".to_string(), "true".to_string());
                }
            }
        }

        // Attach the rate_calibration sidecar (computed from rates_quotes before they were
        // consumed by the prepare loop above).
        let id = curve.id().to_string();
        let mut builder = curve
            .to_builder_with_id(id)
            .rate_calibration_recipe(rate_calibration_recipe);
        if let Some(calibration) = rate_calibration_sidecar {
            builder = builder.rate_calibration(calibration);
        }
        let curve = builder.build()?;

        let new_context = context.clone().insert(curve);

        // Track solver configuration used and any seed diagnostics for transparency.
        report.update_solver_config(config.solver);
        if let Some(seed) = seed_report {
            report.metadata.insert(
                "bootstrap_seed_max_residual".to_string(),
                format!("{:.2e}", seed.max_residual),
            );
            report.metadata.insert(
                "bootstrap_seed_iterations".to_string(),
                seed.iterations.to_string(),
            );
        }
        if let Some(err) = seed_error {
            report
                .metadata
                .insert("bootstrap_seed_error".to_string(), err);
        }

        // Stamp model version and calibration metadata for audit trail.
        report.model_version =
            Some(finstack_quant_core::versions::MULTI_CURVE_OIS_DISCOUNT.to_string());
        report
            .metadata
            .insert("calibration_type".to_string(), "discount_curve".to_string());
        report
            .metadata
            .insert("curve_id".to_string(), params.curve_id.to_string());
        report
            .metadata
            .insert("currency".to_string(), params.currency.to_string());

        Ok((new_context, report))
    }

    /// Build a [`DiscountCurveRateCalibration`] from the solved rate quotes.
    ///
    /// This legacy compatibility sidecar is stamped only when every quote can
    /// be represented without loss: tenor deposits and zero-spread tenor swaps.
    /// Exact replay always uses the curve's typed [`RateCalibrationRecipe`].
    fn build_rate_calibration_from_quotes(
        quotes: &[RateQuote],
        currency: Currency,
    ) -> Option<DiscountCurveRateCalibration> {
        // Infer the index_id from the first deposit or swap quote that has one.
        let index_id = quotes.iter().find_map(|q| match q {
            RateQuote::Deposit { index, .. } | RateQuote::Swap { index, .. } => {
                Some(index.to_string())
            }
            _ => None,
        })?;

        let calibration_quotes: Option<Vec<DiscountCurveRateQuote>> = quotes
            .iter()
            .map(|q| match q {
                RateQuote::Deposit {
                    pillar: crate::market::quotes::ids::Pillar::Tenor(tenor),
                    rate,
                    ..
                } => Some(DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: tenor.to_string(),
                    rate: *rate,
                }),
                RateQuote::Swap {
                    pillar: crate::market::quotes::ids::Pillar::Tenor(tenor),
                    rate,
                    spread_decimal: None,
                    ..
                } => Some(DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Swap,
                    tenor: tenor.to_string(),
                    rate: *rate,
                }),
                _ => None,
            })
            .collect();
        let calibration_quotes = calibration_quotes?;

        Some(DiscountCurveRateCalibration {
            index_id,
            currency,
            quotes: calibration_quotes,
        })
    }

    fn rate_calibration_recipe(
        params: &DiscountCurveParams,
        curve_day_count: DayCount,
        projection_curve_id: CurveId,
        quotes: &[RateQuote],
    ) -> RateCalibrationRecipe {
        RateCalibrationRecipe {
            currency: Some(params.currency),
            method: match &params.method {
                CalibrationMethod::Bootstrap => RateCalibrationMethod::Bootstrap,
                CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian,
                } => RateCalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: *use_analytical_jacobian,
                },
            },
            curve_day_count,
            ois_compounding: params
                .conventions
                .ois_compounding
                .as_ref()
                .map(Self::rate_calibration_ois_compounding),
            role: RateCalibrationCurveRole::Discount {
                projection_curve_id,
            },
            quotes: quotes.iter().map(Self::rate_calibration_quote).collect(),
        }
    }

    fn rate_calibration_quote(quote: &RateQuote) -> RateCalibrationQuote {
        match quote {
            RateQuote::Deposit {
                index,
                pillar,
                rate,
                ..
            } => RateCalibrationQuote::Deposit {
                index_id: index.clone(),
                pillar: Self::rate_calibration_pillar(pillar),
                rate: *rate,
            },
            RateQuote::Fra {
                index,
                start,
                end,
                rate,
                ..
            } => RateCalibrationQuote::Fra {
                index_id: index.clone(),
                start: Self::rate_calibration_pillar(start),
                end: Self::rate_calibration_pillar(end),
                rate: *rate,
            },
            RateQuote::Futures {
                contract,
                expiry,
                price,
                convexity_adjustment,
                vol_surface_id,
                ..
            } => RateCalibrationQuote::Futures {
                contract: RateCalibrationFutureContractId::new(contract.as_str()),
                expiry: *expiry,
                price: *price,
                convexity_adjustment: *convexity_adjustment,
                vol_surface_id: vol_surface_id.clone(),
            },
            RateQuote::Swap {
                index,
                pillar,
                rate,
                spread_decimal,
                ..
            } => RateCalibrationQuote::Swap {
                index_id: index.clone(),
                pillar: Self::rate_calibration_pillar(pillar),
                rate: *rate,
                spread_decimal: *spread_decimal,
            },
        }
    }

    fn rate_calibration_pillar(
        pillar: &crate::market::quotes::ids::Pillar,
    ) -> RateCalibrationPillar {
        match pillar {
            crate::market::quotes::ids::Pillar::Tenor(tenor) => {
                RateCalibrationPillar::Tenor(*tenor)
            }
            crate::market::quotes::ids::Pillar::Date(date) => RateCalibrationPillar::Date(*date),
        }
    }

    fn rate_calibration_ois_compounding(
        compounding: &crate::instruments::rates::irs::FloatingLegCompounding,
    ) -> RateCalibrationOisCompounding {
        use crate::instruments::rates::irs::FloatingLegCompounding;
        match compounding {
            FloatingLegCompounding::Simple => RateCalibrationOisCompounding::Simple,
            FloatingLegCompounding::CompoundedInArrears {
                lookback_days,
                observation_shift,
            } => RateCalibrationOisCompounding::CompoundedInArrears {
                lookback_days: *lookback_days,
                observation_shift: *observation_shift,
            },
            FloatingLegCompounding::CompoundedWithObservationShift { shift_days } => {
                RateCalibrationOisCompounding::CompoundedWithObservationShift {
                    shift_days: *shift_days,
                }
            }
            FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days } => {
                RateCalibrationOisCompounding::CompoundedWithRateCutoff {
                    cutoff_days: *cutoff_days,
                }
            }
        }
    }

    /// Extract the OIS rate cut-off (business days) from the step-level
    /// `ois_compounding` override, when it is a `CompoundedWithRateCutoff`
    /// convention with a positive cut-off.
    ///
    /// This value is stamped onto every solver and final discount curve the
    /// bootstrap produces so that the bootstrap-internal swap repricing and all
    /// downstream single-curve OIS pricing apply the *same* compounded fast-path
    /// decision (see `projected_compounded_float_leg_schedule`).
    fn calibration_ois_cutoff_days(
        ois_compounding: Option<&crate::instruments::rates::irs::FloatingLegCompounding>,
    ) -> Option<i32> {
        use crate::instruments::rates::irs::FloatingLegCompounding;
        match ois_compounding {
            Some(FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days })
                if *cutoff_days > 0 =>
            {
                Some(*cutoff_days)
            }
            _ => None,
        }
    }
}

impl BootstrapTarget for DiscountCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = DiscountCurve;

    fn quote_time(&self, quote: &Self::Quote) -> Result<f64> {
        Ok(quote.pillar_time())
    }

    fn build_curve(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        self.build_curve_for_solver(knots)
    }

    fn build_curve_for_solver(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        if knots.len() < 2 {
            return Err(finstack_quant_core::Error::Calibration {
                message: "Failed to build temp curve: need at least two knots".into(),
                category: "bootstrapping".to_string(),
            });
        }
        if knots.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Failed to build temp curve: non-increasing knot times: {:?}",
                    knots
                        .windows(2)
                        .find(|w| w[1].0 <= w[0].0)
                        .map(|w| (w[0].0, w[1].0))
                ),
                category: "bootstrapping".to_string(),
            });
        }

        // The solver curve should respect the same monotonic/no-arbitrage policy as the final
        // curve. Allowing non-monotone solver curves can make bootstrapping converge to
        // an infeasible (arbitrage) shape and then fail only at the end.
        let config_flag = self.config.discount_curve.allow_non_monotonic_final;
        let policy_allow = match self.config.rate_bounds_policy {
            RateBoundsPolicy::Explicit => self.config.rate_bounds.min_rate < 0.0,
            RateBoundsPolicy::AutoCurrency => {
                matches!(self.currency, Currency::EUR | Currency::JPY | Currency::CHF)
            }
        };
        let allow_non_monotonic = config_flag.unwrap_or(policy_allow);

        if !allow_non_monotonic {
            // Hard guard: even if the underlying curve builder's `build_for_solver()` is lenient,
            // we must not allow the solver to explore arbitrage shapes when the final curve
            // forbids them.
            if knots.windows(2).any(|w| w[1].1 > w[0].1) {
                return Err(finstack_quant_core::Error::Calibration {
                    message: "Discount factors must be non-increasing".to_string(),
                    category: "curve_build".to_string(),
                });
            }
        }

        let mut builder = DiscountCurve::builder(self.discount_curve_id.clone())
            .base_date(self.base_date)
            .day_count(self.curve_day_count)
            .knots(knots.iter().copied())
            .interp(self.solve_interp)
            .extrapolation(self.extrapolation)
            // Stamp the calibration cut-off convention so the bootstrap-internal
            // swap repricing takes the same compounded fast-path branch that
            // downstream single-curve OIS pricing will take against the final
            // curve — keeping calibration and pricing self-consistent.
            .calibration_ois_cutoff_days_opt(self.calibration_ois_cutoff_days);

        builder = if allow_non_monotonic {
            builder.validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            )
        } else {
            builder.validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::MarketStandard,
            )
        };

        builder
            .build_for_solver()
            .map_err(|e| finstack_quant_core::Error::Calibration {
                message: format!("Failed to build temp curve: {}", e),
                category: "bootstrapping".to_string(),
            })
    }

    fn build_curve_final(&self, knots: &[(f64, f64)]) -> Result<Self::Curve> {
        let config_flag = self.config.discount_curve.allow_non_monotonic_final;
        let policy_allow = match self.config.rate_bounds_policy {
            RateBoundsPolicy::Explicit => self.config.rate_bounds.min_rate < 0.0,
            RateBoundsPolicy::AutoCurrency => {
                matches!(self.currency, Currency::EUR | Currency::JPY | Currency::CHF)
            }
        };
        let allow_non_monotonic = config_flag.unwrap_or(policy_allow);

        // Note: MonotoneConvex is now compatible with non-monotone (negative-rate)
        // discount factors — it auto-detects negative discrete forwards and skips
        // its positivity amelioration (user decision
        // Open Question 10) — so no interpolation-style guard is needed here.

        let mut builder = DiscountCurve::builder(self.curve_id.clone())
            .base_date(self.base_date)
            .day_count(self.curve_day_count)
            .knots(knots.iter().copied())
            .interp(self.solve_interp)
            .extrapolation(self.extrapolation)
            // Carry the calibration cut-off convention onto the final curve so
            // downstream single-curve OIS pricing matches the convention the
            // bootstrap-internal swaps were repriced under.
            .calibration_ois_cutoff_days_opt(self.calibration_ois_cutoff_days);

        if allow_non_monotonic {
            builder = builder.validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                    allow_non_monotonic: true,
                    forward_floor: None,
                },
            );
        } else {
            builder = builder.validation(
                finstack_quant_core::market_data::term_structures::ValidationMode::MarketStandard,
            );
        }

        builder
            .build()
            .map_err(|e| finstack_quant_core::Error::Calibration {
                message: e.to_string(),
                category: "curve_build".to_string(),
            })
    }

    fn calculate_residual(&self, curve: &Self::Curve, quote: &Self::Quote) -> Result<f64> {
        self.scratch.with_curve(curve, |ctx| {
            let pv = quote.calibration_value_raw(ctx, self.base_date)?;
            if !self.residual_notional.is_finite() || self.residual_notional <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Invalid residual_notional {}: expected finite positive value",
                    self.residual_notional
                )));
            }
            Ok(pv / self.residual_notional)
        })
    }

    fn initial_guess(&self, quote: &Self::Quote, previous_knots: &[(f64, f64)]) -> Result<f64> {
        let t = self.quote_time(quote)?;
        let (df_lo, df_hi) = self.df_bounds_for_time(t);

        // Try exact match for Deposit/FRA/Swap using simple discounting guess
        // We use pillar_time as proxy for duration.
        // df = 1 / (1 + rate * t)
        if let CalibrationQuote::Rates(pq) = quote {
            use crate::market::quotes::rates::RateQuote;
            match pq.quote.as_ref() {
                RateQuote::Deposit { rate, .. }
                | RateQuote::Fra { rate, .. }
                | RateQuote::Swap { rate, .. } => {
                    // Simple guess: ACT/360 or similar effect using pillar time
                    // For initial guess, accuracy isn't critical, just finding the basin of attraction.
                    // df = 1 / (1 + r * t)
                    let df = 1.0 / (1.0 + rate * t);
                    return Ok(df.clamp(df_lo, df_hi));
                }
                RateQuote::Futures {
                    price,
                    convexity_adjustment,
                    vol_surface_id,
                    ..
                } => {
                    if vol_surface_id.is_some() && convexity_adjustment.is_none() {
                        return Err(finstack_quant_core::Error::Validation(
                            "Discount curve calibration requires a pre-computed convexity_adjustment \
                             for futures quotes; dynamic vol-surface lookup is not wired"
                                .to_string(),
                        ));
                    }
                    // Hull convention: forward = futures - convexity_adjustment
                    let futures_rate = (100.0 - price) / 100.0;
                    let forward_rate = futures_rate - convexity_adjustment.unwrap_or(0.0);
                    let df = 1.0 / (1.0 + forward_rate * t);
                    return Ok(df.clamp(df_lo, df_hi));
                }
            }
        }

        // Try to get a rate for deposit-like guess?
        // Since we abstracted the quote types, let's use a robust extrapolation fallback.
        let mut guess = None;
        let last_two: Vec<(f64, f64)> = previous_knots
            .iter()
            .copied()
            .rev()
            .filter(|(ti, dfi)| *ti > MIN_GRID_SPACING && dfi.is_finite() && *dfi > 0.0)
            .take(2)
            .collect();

        if last_two.len() == 2 {
            let (t1, df1) = last_two[0];
            let (t0, df0) = last_two[1];
            let dt = t1 - t0;
            if dt > MIN_GRID_SPACING {
                let f = (df0.ln() - df1.ln()) / dt;
                let ln_df = df1.ln() - f * (t - t1);
                let df = ln_df.exp();
                if df.is_finite() && df > 0.0 {
                    guess = Some(df);
                }
            }
        } else if last_two.len() == 1 {
            let (t1, df1) = last_two[0];
            if t1 > MIN_GRID_SPACING {
                let df = df1.powf(t / t1);
                if df.is_finite() && df > 0.0 {
                    guess = Some(df);
                }
            }
        }

        // If no previous knots (first point), try a safe default (e.g. rate=0.03 or similar)
        let df = guess.unwrap_or_else(|| {
            // Fallback: 3% rate
            (-0.03 * t).exp()
        });

        Ok(df.clamp(df_lo, df_hi))
    }

    fn scan_points(&self, quote: &Self::Quote, initial_guess: f64) -> Result<Vec<f64>> {
        let time = self.quote_time(quote)?;
        let (df_lo, df_hi) = self.df_bounds_for_time(time);
        let clamped_initial = initial_guess.clamp(df_lo, df_hi);

        let num_points = self.config.discount_curve.scan_grid_points;
        let min_points = self.config.discount_curve.min_scan_grid_points;
        let mut grid = Self::maturity_aware_scan_grid(df_lo, df_hi, clamped_initial, num_points);

        if grid.len() < min_points {
            let num_additional = min_points - grid.len().min(min_points);
            for i in 0..=num_additional {
                let t = i as f64 / num_additional as f64;
                let df = df_lo + t * (df_hi - df_lo);
                if df.is_finite() && df > 0.0 {
                    grid.push(df);
                }
            }
            grid.sort_by(|a, b| b.total_cmp(a));
            grid.dedup_by(|a, b| (*a - *b).abs() < (df_hi - df_lo) * TOLERANCE_GRID_DEDUP);
        }

        Ok(grid)
    }

    fn supports_nearest_first_bracketing(&self) -> bool {
        true
    }

    fn validate_knot(&self, time: f64, value: f64) -> Result<()> {
        if !value.is_finite() || value <= 0.0 {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Non-finite or non-positive discount factor at t={:.6}",
                    time
                ),
                category: "bootstrapping".to_string(),
            });
        }
        let (lo, hi) = self.df_bounds_for_time(time);
        if value < lo || value > hi {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "DF out of bounds at t={:.4}: got {:.6}, range [{:.6}, {:.6}]",
                    time, value, lo, hi
                ),
                category: "bootstrapping".to_string(),
            });
        }
        Ok(())
    }
}

impl GlobalSolveTarget for DiscountCurveTarget {
    type Quote = CalibrationQuote;
    type Curve = DiscountCurve;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        let bounds = self.config.effective_rate_bounds(self.currency);
        let mut entries = Vec::new();
        let seed_curve = self.initial_curve.as_ref();

        for quote in quotes {
            let t = self.quote_time(quote)?;
            if t <= 0.0 {
                continue;
            }

            // Prefer seeded zeros from an initial curve (bootstrap) if available.
            let z = if let Some(curve) = seed_curve {
                let df = if t > 0.0 { curve.df(t) } else { 1.0 };
                if df.is_finite() && df > 0.0 {
                    let implied_z = -df.ln() / t.max(1e-9);
                    implied_z.clamp(bounds.min_rate, bounds.max_rate)
                } else {
                    0.03_f64.clamp(bounds.min_rate, bounds.max_rate)
                }
            } else {
                0.03_f64.clamp(bounds.min_rate, bounds.max_rate)
            };
            entries.push((t, z, quote));
        }

        // Sort entries by pillar time so the global solver sees a strictly increasing knot grid.
        entries.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut times = Vec::with_capacity(entries.len());
        let mut initials = Vec::with_capacity(entries.len());
        let mut active_quotes = Vec::with_capacity(entries.len());
        let mut last_time: Option<f64> = None;

        for (t, z, quote) in entries {
            if let Some(prev) = last_time {
                if (t - prev).abs() <= TOLERANCE_DUP_KNOTS {
                    return Err(finstack_quant_core::Error::Calibration {
                        message: format!(
                            "Duplicate or unsorted knot times detected (prev={:.10}, new={:.10}). \
Ensure quotes map to strictly increasing year fractions.",
                            prev, t
                        ),
                        category: "global_solve".to_string(),
                    });
                }
            }
            last_time = Some(t);
            times.push(t);
            initials.push(z);
            active_quotes.push(quote.clone());
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
        let knots = self.knots_from_params(times, params)?;
        self.build_curve_for_solver(&knots)
    }

    fn build_curve_final_from_params(&self, times: &[f64], params: &[f64]) -> Result<Self::Curve> {
        let knots = self.knots_from_params(times, params)?;
        self.build_curve_final(&knots)
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> Result<()> {
        self.scratch.with_curve(curve, |ctx| {
            for (i, quote) in quotes.iter().enumerate() {
                if i >= residuals.len() {
                    break;
                }
                let pv = quote.calibration_value_raw(ctx, self.base_date)?;
                residuals[i] = pv / self.residual_notional;
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

            // Item 4: par-instrument residuals must enter the least-squares problem on a
            // common *rate-error* scale, weighted by ~1/PV01 (inverse fixed-leg annuity).
            //
            // The residual handed to the solver is `pv / residual_notional`. For a par
            // instrument `PV(r) ≈ A(t)·(r − r_par)·notional`, so the residual is
            // `≈ A(t)·Δr` — a PV01-scaled rate error, NOT a rate error. The global
            // solver minimises `Σ (r_i·√w_i)²`, so to recover the rate-error scale
            // (`r_i·√w_i ≈ Δr`) the weight must be `w_i ≈ 1/A(t)²`.
            //
            // None of the previous schemes did this: `LinearTime` (`w = t`) actively
            // up-weighted the long end — where `A` is largest and a given rate error
            // already produces the largest PV residual — leaving the short end loose.
            //
            // `pv01_inverse_sq = 1/A(t)²` is the PV01-normalisation common to every
            // scheme; the scheme factor then applies the *relative* time emphasis the
            // user selected on top of a correct common scale.
            let annuity = self.quote_annuity_proxy(quote, t);
            let pv01_inverse_sq = 1.0 / (annuity * annuity);

            let scheme_factor = match self.config.discount_curve.weighting_scheme {
                ResidualWeightingScheme::Equal => 1.0,
                ResidualWeightingScheme::LinearTime => t,
                ResidualWeightingScheme::SqrtTime => t.sqrt(),
                ResidualWeightingScheme::InverseDuration => {
                    // Extra inverse-duration tilt on top of the PV01 normalisation.
                    1.0 / t.max(0.1)
                }
            };

            let weight = pv01_inverse_sq * scheme_factor;
            weights_out[i] = if weight.is_finite() {
                weight.max(WEIGHT_MIN_FLOOR)
            } else {
                WEIGHT_MIN_FLOOR
            };
        }
        Ok(())
    }

    /// Compute the Jacobian matrix using an optimized finite-difference scheme.
    ///
    /// # Implementation Notes
    ///
    /// While this method is part of the "Analytical Jacobian" interface, the implementation
    /// uses **efficient row-wise finite differences** rather than closed-form derivatives.
    /// This approach is significantly faster and more accurate than the solver's default
    /// "blind" finite-difference approximation because:
    ///
    /// 1. **Sparsity Exploitation**: Bootstrapped curves have a triangular structure—
    ///    instruments at time `t` are insensitive to knots at times `> t`. We exploit
    ///    this by skipping derivatives that are effectively zero.
    ///
    /// 2. **Configurable Step Size**: Uses `jacobian_step_size` from config rather than
    ///    a generic epsilon, allowing tuning for numerical stability.
    ///
    /// 3. **Curve Caching**: Reuses the base context and avoids redundant curve rebuilds.
    ///
    /// A true analytical Jacobian would require closed-form sensitivities of all instrument
    /// pricing formulas with respect to zero rates, which is not tractable for the full
    /// instrument universe (swaps, futures, FRAs with varying conventions).
    fn jacobian(
        &self,
        params: &[f64],
        times: &[f64],
        quotes: &[Self::Quote],
        jacobian: &mut [Vec<f64>],
    ) -> Result<()> {
        // Validation of dimensions
        if jacobian.len() != quotes.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Jacobian rows {} != quotes {}",
                    jacobian.len(),
                    quotes.len()
                ),
                category: "jacobian".to_string(),
            });
        }
        if params.len() != times.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!("Params {} != Times {}", params.len(), times.len()),
                category: "jacobian".to_string(),
            });
        }
        // Check columns
        if !jacobian.is_empty() && jacobian[0].len() != params.len() {
            return Err(finstack_quant_core::Error::Calibration {
                message: format!(
                    "Jacobian cols {} != params {}",
                    jacobian[0].len(),
                    params.len()
                ),
                category: "jacobian".to_string(),
            });
        }

        // Central finite differences: O(h^2) accuracy vs O(h) for forward FD.
        let fd_eps = self.config.discount_curve.jacobian_step_size;
        let mut params_bumped = params.to_vec();

        // The pillar time of each quote is independent of the bumped parameters,
        // so derive it once rather than recomputing it inside every ±h column
        // loop (previously `2 * n_params * n_quotes` redundant calls).
        let quote_times = quotes
            .iter()
            .map(|q| self.quote_time(q))
            .collect::<Result<Vec<f64>>>()?;

        // Reused across columns: for any quote the +h and -h loops skip exactly
        // the same entries (same `t_cutoff` / `quote_times`), so every entry
        // that is read in the -h loop was freshly written in the +h loop. One
        // buffer therefore suffices instead of allocating per column.
        let mut vals_plus = vec![0.0_f64; quotes.len()];
        let has_local_interpolation = matches!(
            self.solve_interp,
            InterpStyle::Linear | InterpStyle::LogLinear
        );

        for j in 0..params.len() {
            let p_orig = params[j];

            let h = (p_orig.abs() * fd_eps).max(fd_eps);
            let t_cutoff = if j > 0 { times[j - 1] } else { 0.0 };

            // +h evaluation. Insert the bumped curve into the scratch context
            // in place (`with_curve` uses `insert_mut`), avoiding the full
            // `MarketContext` clone + consuming rebuild the previous code did.
            params_bumped[j] = p_orig + h;
            let curve_plus = self.build_curve_for_solver_from_params(times, &params_bumped)?;
            self.scratch.with_curve(&curve_plus, |ctx_plus| {
                for (i, quote) in quotes.iter().enumerate() {
                    if has_local_interpolation && quote_times[i] < t_cutoff - 1e-4 {
                        continue;
                    }
                    let pv = quote.calibration_value_raw(ctx_plus, self.base_date)?;
                    vals_plus[i] = pv / self.residual_notional;
                }
                Ok(())
            })?;

            // -h evaluation
            params_bumped[j] = p_orig - h;
            let curve_minus = self.build_curve_for_solver_from_params(times, &params_bumped)?;
            self.scratch.with_curve(&curve_minus, |ctx_minus| {
                for (i, quote) in quotes.iter().enumerate() {
                    if has_local_interpolation && quote_times[i] < t_cutoff - 1e-4 {
                        jacobian[i][j] = 0.0;
                        continue;
                    }
                    let pv = quote.calibration_value_raw(ctx_minus, self.base_date)?;
                    let val_minus = pv / self.residual_notional;

                    jacobian[i][j] = (vals_plus[i] - val_minus) / (2.0 * h);
                }
                Ok(())
            })?;

            params_bumped[j] = p_orig;
        }

        Ok(())
    }

    /// Returns `true` to indicate an efficient Jacobian is available.
    ///
    /// The discount curve target provides a custom Jacobian implementation that
    /// uses row-wise finite differences and only applies the causal sparsity
    /// shortcut for local interpolation schemes. Global/non-local schemes such
    /// as monotone-convex reprice every quote for every bumped knot.
    ///
    /// See [`jacobian`](Self::jacobian) for implementation details.
    fn supports_efficient_jacobian(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::prepared::CalibrationQuote;
    use crate::calibration::solver::global::GlobalFitOptimizer;
    use crate::calibration::solver::traits::BootstrapTarget;
    use crate::market::build::prepared::PreparedQuote;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCountContext};
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    #[test]
    fn discount_quote_time_uses_swap_payment_date_when_payment_delay_applies() {
        let base_date = Date::from_calendar_date(2024, Month::January, 2).expect("base_date");
        let maturity = base_date + time::Duration::days(365);

        // Native Quote
        // Native Quote
        let quote = crate::market::quotes::rates::RateQuote::Swap {
            id: crate::market::quotes::ids::QuoteId::new("SWP-1Y"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR-OIS"),
            pillar: crate::market::quotes::ids::Pillar::Date(maturity), // 1Y
            rate: 0.02,
            spread_decimal: None,
        };

        // Assume delay 2 days (standard OIS)
        let delay = 2;
        let cal_id = "usny";
        let pay_date = crate::instruments::common_impl::pricing::swap_legs::add_payment_delay(
            maturity,
            delay,
            Some(cal_id),
        )
        .expect("payment delay with usny calendar");

        let expected_yf = DayCount::Act365F
            .year_fraction(base_date, pay_date, DayCountContext::default())
            .expect("expected year_fraction to succeed");

        let target = DiscountCurveTarget::new(DiscountCurveTargetParams {
            base_date,
            currency: Currency::USD,
            curve_id: CurveId::new("USD-OIS"),
            discount_curve_id: CurveId::new("USD-OIS"),
            solve_interp: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
            config: CalibrationConfig::default(),
            curve_day_count: DayCount::Act365F,
            spot_knot: None,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
            calibration_ois_cutoff_days: None,
        });

        // Mock instrument using Deposit with all required fields
        let instrument = std::sync::Arc::new(crate::instruments::rates::deposit::Deposit {
            id: InstrumentId::new("DEP-1Y"),
            quote_rate: Some(rust_decimal::Decimal::try_from(0.02).expect("valid decimal")),
            discount_curve_id: CurveId::new("USD-OIS"),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            spot_lag_days: Some(0),
            bdc: BusinessDayConvention::Following,
            calendar_id: None,
            start_date: base_date,
            maturity: pay_date,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act360,
        });

        let pq = PreparedQuote::new(
            std::sync::Arc::new(quote),
            instrument,
            pay_date,
            expected_yf,
        );
        let cal_quote = CalibrationQuote::Rates(pq);

        let actual = BootstrapTarget::quote_time(&target, &cal_quote).expect("quote_time");
        assert!((actual - expected_yf).abs() < 1e-15);
    }

    #[test]
    fn global_solve_discount_curve_sanity_check() {
        // Sanity check that GlobalFitOptimizer runs with DiscountCurveTarget.
        let base_date = Date::from_calendar_date(2025, Month::December, 10).expect("base_date");
        let mut config = CalibrationConfig::default();
        config.solver = config.solver.with_tolerance(1e-9);
        config.calibration_method = crate::calibration::config::CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: true,
        };

        let target = DiscountCurveTarget::new(DiscountCurveTargetParams {
            base_date,
            currency: Currency::USD,
            curve_id: CurveId::new("USD-OIS"),
            discount_curve_id: CurveId::new("USD-OIS"),
            solve_interp: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
            config: config.clone(),
            curve_day_count: DayCount::Act365F,
            spot_knot: None,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
            calibration_ois_cutoff_days: None,
        });

        // Manual Simple Quote (Deposit)
        let maturity = base_date + time::Duration::days(365);
        let rate = 0.05;
        let p_time = 1.0;

        // RateQuote::Deposit
        let quote = crate::market::quotes::rates::RateQuote::Deposit {
            id: crate::market::quotes::ids::QuoteId::new("DEP-1Y"),
            index: finstack_quant_core::types::IndexId::new("USD-SOFR"),
            pillar: crate::market::quotes::ids::Pillar::Date(maturity),
            rate,
        };

        // Dummy Instrument for PreparedQuote
        let instrument = std::sync::Arc::new(crate::instruments::rates::deposit::Deposit {
            id: InstrumentId::new("DEP-1Y"),
            quote_rate: Some(rust_decimal::Decimal::try_from(rate).expect("valid decimal")),
            discount_curve_id: CurveId::new("USD-OIS"),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            spot_lag_days: Some(0),
            bdc: BusinessDayConvention::Following,
            calendar_id: None,
            start_date: base_date,
            maturity,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act360,
        });

        let pq = PreparedQuote::new(std::sync::Arc::new(quote), instrument, maturity, p_time);
        let quotes = vec![CalibrationQuote::Rates(pq)];

        let (_curve, report) =
            GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("solve");
        println!("Max residual: {}", report.max_residual);
        assert!(report.max_residual < 1e-6);
    }

    #[test]
    fn test_residual_normalization_invariance() {
        // Test that calibration with different notionals produces identical curves
        // This verifies the fix for residual normalization (pv / residual_notional)
        let base_date = Date::from_calendar_date(2025, Month::December, 10).expect("base_date");
        let mut config = CalibrationConfig::default();
        config.solver = config.solver.with_tolerance(1e-9);
        config.calibration_method = crate::calibration::config::CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: true,
        };

        // Helper function to run calibration with a given notional
        let run_calibration = |notional: f64| -> DiscountCurve {
            let target = DiscountCurveTarget::new(DiscountCurveTargetParams {
                base_date,
                currency: Currency::USD,
                curve_id: CurveId::new("USD-OIS"),
                discount_curve_id: CurveId::new("USD-OIS"),
                solve_interp: InterpStyle::Linear,
                extrapolation: ExtrapolationPolicy::FlatZero,
                config: config.clone(),
                curve_day_count: DayCount::Act365F,
                spot_knot: None,
                residual_notional: notional, // Different notionals
                base_context: MarketContext::new(),
                calibration_ois_cutoff_days: None,
            });

            // Create a few deposit quotes with different maturities
            let mut quotes = Vec::new();
            for (days, rate) in [(30, 0.04), (90, 0.045), (180, 0.05), (365, 0.055)] {
                let maturity = base_date + time::Duration::days(days);
                let p_time = DayCount::Act365F
                    .year_fraction(base_date, maturity, DayCountContext::default())
                    .expect("year_fraction");

                let quote = crate::market::quotes::rates::RateQuote::Deposit {
                    id: crate::market::quotes::ids::QuoteId::new(format!("DEP-{}D", days)),
                    index: finstack_quant_core::types::IndexId::new("USD-SOFR"),
                    pillar: crate::market::quotes::ids::Pillar::Date(maturity),
                    rate,
                };

                let instrument = std::sync::Arc::new(crate::instruments::rates::deposit::Deposit {
                    id: InstrumentId::new(format!("DEP-{}D", days)),
                    quote_rate: Some(rust_decimal::Decimal::try_from(rate).expect("valid decimal")),
                    discount_curve_id: CurveId::new("USD-OIS"),
                    instrument_pricing_overrides: Default::default(),
                    metric_pricing_overrides: Default::default(),
                    scenario_pricing_overrides: Default::default(),
                    attributes: Default::default(),
                    spot_lag_days: Some(0),
                    bdc: BusinessDayConvention::Following,
                    calendar_id: None,
                    start_date: base_date,
                    maturity,
                    notional: Money::new(notional, Currency::USD),
                    day_count: DayCount::Act360,
                });

                let pq =
                    PreparedQuote::new(std::sync::Arc::new(quote), instrument, maturity, p_time);
                quotes.push(CalibrationQuote::Rates(pq));
            }

            let (curve, report) =
                GlobalFitOptimizer::optimize(&target, &quotes, &config, None).expect("solve");

            // Ensure calibration succeeded with normalized residuals
            println!(
                "Notional {} - Max residual: {:.2e}",
                notional, report.max_residual
            );
            assert!(
                report.max_residual < 1e-8,
                "Max residual should be < 1e-8 in normalized units"
            );

            curve
        };

        // Run with notional = 1.0
        let curve_notional_1 = run_calibration(1.0);

        // Run with notional = 1,000,000.0
        let curve_notional_1m = run_calibration(1_000_000.0);

        // Compare curves at several points - they should be identical (within numerical tolerance)
        let test_times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        for t in test_times {
            let df_1 = curve_notional_1.df(t);
            let df_1m = curve_notional_1m.df(t);

            println!(
                "t={:.2}: df(notional=1.0)={:.12}, df(notional=1M)={:.12}, diff={:.2e}",
                t,
                df_1,
                df_1m,
                (df_1 - df_1m).abs()
            );

            assert!(
                (df_1 - df_1m).abs() < 1e-12,
                "Discount factors should be identical (within 1e-12) for t={:.2}, but got diff={:.2e}",
                t,
                (df_1 - df_1m).abs()
            );
        }
    }

    #[test]
    fn test_futures_initial_guess_applies_convexity_adjustment() {
        // Verify that a non-zero convexity_adjustment changes the initial guess DF
        // relative to using no adjustment.
        let base_date = Date::from_calendar_date(2025, Month::December, 10).expect("base_date");
        let config = CalibrationConfig::default();

        let target = DiscountCurveTarget::new(DiscountCurveTargetParams {
            base_date,
            currency: Currency::USD,
            curve_id: CurveId::new("USD-OIS"),
            discount_curve_id: CurveId::new("USD-OIS"),
            solve_interp: InterpStyle::Linear,
            extrapolation: ExtrapolationPolicy::FlatZero,
            config,
            curve_day_count: DayCount::Act365F,
            spot_knot: None,
            residual_notional: 1.0,
            base_context: MarketContext::new(),
            calibration_ois_cutoff_days: None,
        });

        let maturity = base_date + time::Duration::days(90);
        let p_time = DayCount::Act365F
            .year_fraction(base_date, maturity, DayCountContext::default())
            .expect("year_fraction");

        // Build two futures quotes: one without adjustment, one with 20bp adjustment
        let make_futures_quote = |adj: Option<f64>| -> CalibrationQuote {
            let quote = crate::market::quotes::rates::RateQuote::Futures {
                id: crate::market::quotes::ids::QuoteId::new("FUT-3M"),
                contract: crate::market::conventions::ids::IrFutureContractId::new("CME:SR3"),
                expiry: maturity,
                price: 95.0, // implies 5% futures rate
                convexity_adjustment: adj,
                vol_surface_id: None,
            };
            // Use a Deposit instrument as a placeholder -- initial_guess only
            // inspects the quote, not the instrument.
            let instrument = std::sync::Arc::new(crate::instruments::rates::deposit::Deposit {
                id: InstrumentId::new("FUT-3M"),
                quote_rate: Some(rust_decimal::Decimal::try_from(0.05).expect("valid decimal")),
                discount_curve_id: CurveId::new("USD-OIS"),
                instrument_pricing_overrides: Default::default(),
                metric_pricing_overrides: Default::default(),
                scenario_pricing_overrides: Default::default(),
                attributes: Default::default(),
                spot_lag_days: Some(0),
                bdc: BusinessDayConvention::Following,
                calendar_id: None,
                start_date: base_date,
                maturity,
                notional: Money::new(1.0, Currency::USD),
                day_count: DayCount::Act360,
            });

            let pq = PreparedQuote::new(std::sync::Arc::new(quote), instrument, maturity, p_time);
            CalibrationQuote::Rates(pq)
        };

        let quote_no_adj = make_futures_quote(None);
        let quote_with_adj = make_futures_quote(Some(0.002)); // 20bp adjustment

        let guess_no_adj =
            BootstrapTarget::initial_guess(&target, &quote_no_adj, &[]).expect("guess no adj");
        let guess_with_adj =
            BootstrapTarget::initial_guess(&target, &quote_with_adj, &[]).expect("guess with adj");

        // With a positive convexity adjustment, forward_rate < futures_rate,
        // so the discount factor should be higher (closer to 1).
        println!(
            "guess_no_adj={:.10}, guess_with_adj={:.10}, diff={:.2e}",
            guess_no_adj,
            guess_with_adj,
            (guess_no_adj - guess_with_adj).abs()
        );

        assert!(
            guess_with_adj > guess_no_adj,
            "Convexity adjustment should increase the DF (lower forward rate); \
             got no_adj={}, with_adj={}",
            guess_no_adj,
            guess_with_adj
        );

        // Verify None defaults to zero (same as explicit 0.0)
        let quote_zero = make_futures_quote(Some(0.0));
        let guess_zero =
            BootstrapTarget::initial_guess(&target, &quote_zero, &[]).expect("guess zero");
        assert!(
            (guess_no_adj - guess_zero).abs() < 1e-15,
            "None adjustment should behave identically to 0.0"
        );
    }

    #[test]
    fn replay_recipe_preserves_supported_mixed_quote_shapes_losslessly() {
        let date = Date::from_calendar_date(2025, Month::September, 17).expect("valid date");
        let index = finstack_quant_core::types::IndexId::new("USD-SOFR-OIS");
        let quotes = vec![
            RateQuote::Deposit {
                id: crate::market::quotes::ids::QuoteId::new("DEP"),
                index: index.clone(),
                pillar: crate::market::quotes::ids::Pillar::Date(date),
                rate: 0.043,
            },
            RateQuote::Fra {
                id: crate::market::quotes::ids::QuoteId::new("FRA"),
                index: index.clone(),
                start: crate::market::quotes::ids::Pillar::Tenor(
                    "3M".parse().expect("valid tenor"),
                ),
                end: crate::market::quotes::ids::Pillar::Date(date),
                rate: 0.041,
            },
            RateQuote::Futures {
                id: crate::market::quotes::ids::QuoteId::new("FUT"),
                contract: crate::market::conventions::ids::IrFutureContractId::new("CME:SR3"),
                expiry: date,
                price: 95.75,
                convexity_adjustment: Some(0.0001),
                vol_surface_id: Some(CurveId::new("USD-SR3-VOL")),
            },
            RateQuote::Swap {
                id: crate::market::quotes::ids::QuoteId::new("SWAP"),
                index,
                pillar: crate::market::quotes::ids::Pillar::Tenor(
                    "5Y".parse().expect("valid tenor"),
                ),
                rate: 0.039,
                spread_decimal: Some(0.00025),
            },
        ];
        let replay_quotes: Vec<_> = quotes
            .iter()
            .map(DiscountCurveTarget::rate_calibration_quote)
            .collect();

        assert!(matches!(
            &replay_quotes[0],
            RateCalibrationQuote::Deposit {
                pillar: RateCalibrationPillar::Date(value),
                ..
            } if *value == date
        ));
        assert!(matches!(
            &replay_quotes[1],
            RateCalibrationQuote::Fra {
                start: RateCalibrationPillar::Tenor(_),
                end: RateCalibrationPillar::Date(value),
                ..
            } if *value == date
        ));
        assert!(matches!(
            &replay_quotes[2],
            RateCalibrationQuote::Futures {
                convexity_adjustment: Some(value),
                ..
            } if (*value - 0.0001).abs() < f64::EPSILON
        ));
        assert!(matches!(
            &replay_quotes[3],
            RateCalibrationQuote::Swap {
                spread_decimal: Some(value),
                ..
            } if (*value - 0.00025).abs() < f64::EPSILON
        ));
        assert!(
            DiscountCurveTarget::build_rate_calibration_from_quotes(&quotes, Currency::USD)
                .is_none(),
            "legacy sidecar must be omitted rather than silently dropping mixed quote fields"
        );
    }
}
