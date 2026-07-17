//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use std::borrow::Cow;
use std::cell::RefCell;

/// Configuration for Z-spread solver with maturity-aware bracket sizing.
///
/// Controls convergence tolerance and initial search bracket width for the
/// Z-spread root-finding algorithm. The bracket width scales with bond maturity
/// to handle both short-dated and long-dated bonds efficiently.
///
/// # Tolerance Design Rationale
///
/// The Z-spread tolerance is specified on the **spread axis** (in decimal, not bp).
/// The default `1e-10` (0.001 bp) is chosen to ensure:
///
/// 1. **Price accuracy**: For a 10Y bond with duration ~8, a spread tolerance
///    of `1e-10` translates to price error < `$0.00001` per $1000 face.
///
/// 2. **Consistency with YTM**: Same order of magnitude as YTM solver tolerance
///    ensures consistent precision across all yield/spread metrics.
///
/// ## Tolerance-to-Price Sensitivity
///
/// The relationship between spread tolerance and price accuracy:
///
/// ```text
/// Price Error ≈ Duration × Notional × Spread Tolerance
///
/// Example: Duration = 8, Notional = $1,000,000, Tolerance = 1e-10
/// Price Error ≈ 8 × 1,000,000 × 1e-10 = $0.0008
/// ```
///
/// ## Recommended Tolerances by Use Case
///
/// | Use Case | Tolerance | Spread Precision | Price Error ($1M) |
/// |----------|-----------|------------------|-------------------|
/// | Regulatory | `1e-12` | < 0.0001 bp | < $0.0001 |
/// | Trading | `1e-10` | < 0.01 bp | < $0.01 |
/// | Screening | `1e-8` | < 1 bp | < $1 |
///
/// # Maturity-Aware Bracketing
///
/// The initial bracket scales with maturity to handle both IG and HY:
///
/// ```text
/// bracket = min(base_bracket × (1 + years/30), max_bracket)
/// ```
///
/// This ensures:
/// - Short-dated IG bonds: tight ±500-1000 bp bracket → fast convergence
/// - Long-dated HY bonds: wider ±1500-3000 bp bracket → robust coverage
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::metrics::price_yield_spread::ZSpreadSolverConfig;
///
/// // Default production configuration
/// let default = ZSpreadSolverConfig::default();
///
/// // Tighter tolerance for regulatory reporting
/// let regulatory = ZSpreadSolverConfig {
///     tolerance: 1e-12,
///     base_bracket_bp: 1000.0,
///     max_bracket_bp: 3000.0,
/// };
///
/// // Wider bracket for distressed debt screening
/// let distressed = ZSpreadSolverConfig {
///     tolerance: 1e-8,
///     base_bracket_bp: 2000.0,
///     max_bracket_bp: 5000.0,
/// };
/// ```
#[derive(Debug, Clone)]
pub(crate) struct ZSpreadSolverConfig {
    /// Convergence tolerance for the Z-spread solver (on the spread axis, decimal).
    ///
    /// Default: `1e-10` (~0.01 bp precision), which typically achieves price
    /// residuals well below `$0.01` per $1M face for all credit qualities.
    ///
    /// # Interpretation
    ///
    /// The solver stops when the price residual (model vs target) is less than
    /// `tolerance × duration × notional`, ensuring proportional accuracy.
    pub tolerance: f64,

    /// Base half-width of the initial search bracket, in basis points.
    ///
    /// Short-dated IG credit typically has spreads in 50-300 bp range, but
    /// we default to ±1000 bp to comfortably cover HY (300-800 bp) and
    /// distressed (800+ bp) names without manual configuration.
    ///
    /// # Maturity Scaling
    ///
    /// The actual bracket is scaled by maturity:
    /// `actual_bracket = base_bracket × (1 + years/30)`
    pub base_bracket_bp: f64,

    /// Maximum half-width of the initial search bracket after maturity scaling.
    ///
    /// Caps the bracket for very long-dated bonds (30Y+) to prevent excessive
    /// search domains that could slow convergence.
    pub max_bracket_bp: f64,
}

impl Default for ZSpreadSolverConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-10,
            // Short-dated bonds: ±1000 bp is generous and covers HY/distressed
            base_bracket_bp: 1000.0,
            // Allow widening for long maturities, but cap to a realistic range
            max_bracket_bp: 3000.0,
        }
    }
}

/// Z-spread metric calculator for vanilla bonds.
///
/// Calculates the zero-volatility spread (Z-spread) as the constant additive spread
/// to the base discount curve that makes the discounted value of future cashflows
/// equal to the bond's dirty market price. The spread is applied on the
/// **periodically-compounded zero rate**: the base discount factor is converted
/// to its zero rate at the quote frequency `m`, the spread `z` is added, and the
/// flow is re-discounted at frequency `m` (see `z_spread_discount_factor`). It is
/// not a continuous `exp(-z·t)` shift.
///
/// Uses Brent's method with a maturity-aware initial bracket and a configurable
/// tolerance. The default configuration is tuned for production use:
/// - `tolerance = 1e-10`
/// - short-dated bonds: ±1000 bp initial bracket
/// - long-dated/distressed: widened up to ±3000 bp
///
/// # Dependencies
///
/// Requires `Accrued` metric to be computed first.
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Z-spread is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct ZSpreadCalculator {
    config: ZSpreadSolverConfig,
}

impl ZSpreadCalculator {
    /// Create a Z-spread calculator with default production-grade solver
    /// settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute a maturity-aware initial bracket in decimal units.
    ///
    /// Short-dated bonds use the base bracket (e.g., ±1000 bp). Longer
    /// maturities widen the bracket smoothly up to `max_bracket_bp`.
    fn initial_bracket_decimal(
        &self,
        bond: &Bond,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        if as_of >= bond.maturity {
            return Ok(self.config.base_bracket_bp / 10_000.0);
        }
        let dc = bond.cashflow_spec.day_count();
        let years = dc
            .year_fraction(
                as_of,
                bond.maturity,
                DayCountContext {
                    frequency: Some(bond.cashflow_spec.frequency()),
                    ..Default::default()
                },
            )?
            .max(0.0);

        // Scale between 1x and 2x base over 0–30y, then clamp.
        let maturity_scale = 1.0 + (years / 30.0).min(1.0);
        let bracket_bp =
            (self.config.base_bracket_bp * maturity_scale).min(self.config.max_bracket_bp);

        Ok(bracket_bp / 10_000.0)
    }
}

pub(crate) fn bond_z_spread_compounding_frequency(bond: &Bond) -> f64 {
    let years = bond.cashflow_spec.frequency().to_years_simple();
    if years > 0.0 && years.is_finite() {
        (1.0 / years).round().max(1.0)
    } else {
        1.0
    }
}

/// Compute the z-spread discount factor for a single cashflow.
///
/// Returns `Err` in two degenerate cases that must not silently propagate
/// non-finite values into PV accumulators:
///
/// - `df_base <= 0` or non-finite: the base discount curve has produced an
///   invalid discount factor (curve-data error, not a solvable point).
/// - `denom = 1 + (base_rate + z) / m <= 0`: the total spread-adjusted rate
///   produces a non-positive compounding base. This can only happen for
///   extremely negative spreads that are outside any realistic range; returning
///   `INFINITY` (the old behaviour) would silently corrupt PV sums and confuse
///   the Brent bracket search with non-finite residuals.
pub(crate) fn z_spread_discount_factor(
    df_base: f64,
    t: f64,
    z: f64,
    compounds_per_year: f64,
) -> finstack_quant_core::Result<f64> {
    if t <= 0.0 {
        return Ok(df_base);
    }
    if !df_base.is_finite() || df_base <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "z_spread_discount_factor: non-positive or non-finite base discount factor ({df_base}); \
             this is a curve-data error"
        )));
    }
    let m = compounds_per_year.max(1.0);
    let base_rate = m * (df_base.powf(-1.0 / (m * t)) - 1.0);
    let denom = 1.0 + (base_rate + z) / m;
    if denom <= 0.0 || !denom.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "z_spread_discount_factor: non-positive compounding denominator ({denom}) \
             for z={z:.6e}, base_rate={base_rate:.6e}, m={m}; \
             spread is too negative for this cashflow"
        )));
    }
    Ok(denom.powf(-m * t))
}

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Get bond and compute quote-date context
        let bond: &Bond = context.instrument_as()?;

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Determine dirty market value in currency at quote_date
        let target_value_ccy: f64 = if let Some(clean_px) = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
        {
            // Use accrued at quote_date for dirty price calculation
            quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount())
        } else {
            // Fallback to base PV if no market quote
            context.base_value.amount()
        };

        // OPTIMIZATION: Pre-calculate cashflow times and base discount factors
        // to avoid repeated date logic and curve lookups inside the solver loop.
        //
        // Use quote_date consistently as the time origin for z-spread calculation.
        // Both year fractions and discount factors use the same origin (quote_date).
        // Note: quote_date is the settlement date, which is the correct anchor
        // for market-convention z-spread calculations. This ensures the z-spread
        // shift (applied via `z_spread_discount_factor` on the periodically-
        // compounded zero rate) is anchored to the same date as the base DFs.
        let flows = bond.pricing_dated_cashflows(&context.curves, context.as_of)?;
        let disc = context.curves.get_discount(&bond.discount_curve_id)?;
        let (spread_flows, quote_date) = if let Some((_, workout_flows, workout_quote_date)) =
            crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
                bond,
                context.curves.as_ref(),
                context.as_of,
                &flows,
            )? {
            (Cow::Owned(workout_flows), workout_quote_date)
        } else {
            (Cow::Borrowed(flows.as_slice()), quote_ctx.quote_date)
        };
        let compounds_per_year = bond_z_spread_compounding_frequency(bond);
        // Keep z-spread time axis on the discount-curve basis for consistency with
        // existing solver calibration and parity tests.
        let dc = disc.day_count();

        // Cache (time_from_quote_date, df_from_quote_date, amount) for each future cashflow
        let cached_flows: Vec<(f64, f64, f64)> = spread_flows
            .iter()
            .filter(|(d, _)| *d > quote_date)
            .map(|(d, amt)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                // Year fraction from quote_date (settlement) for z-spread shift
                let t = dc.year_fraction(quote_date, *d, DayCountContext::default())?;
                // Discount factor from quote_date (settlement) — same origin as t
                let df_base = disc.df_between_dates(quote_date, *d)?;
                Ok((t, df_base, amt.amount()))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        // Capture the first z-spread pricing error encountered inside the
        // objective so a bad-curve-data failure is surfaced as the real cause
        // rather than an opaque `SolverConvergenceFailed`.
        let pricing_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);

        // Objective: PV_z(z) - target_value_ccy = 0
        //
        // When `z_spread_discount_factor` returns `Err` (non-positive DF or
        // non-positive compounding denominator), we capture the first error and
        // return a large positive residual (+1e12) so Brent can report a
        // convergence failure instead of silently propagating NaN/Inf values
        // into the PV accumulator.
        let objective = |z: f64| -> f64 {
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
            for (t, df_base, amt) in &cached_flows {
                match z_spread_discount_factor(*df_base, *t, z, compounds_per_year) {
                    Ok(df_z) => pv.add(amt * df_z),
                    Err(e) => {
                        let mut slot = pricing_error.borrow_mut();
                        if slot.is_none() {
                            *slot = Some(e);
                        }
                        drop(slot);
                        // Large positive residual: price diverges to +∞ when the
                        // spread is extremely negative (below the compounding
                        // floor). This keeps the residual monotone and prevents
                        // Brent from manufacturing a fake sign-changing bracket.
                        return 1e12;
                    }
                }
            }
            pv.total() - target_value_ccy
        };

        // Solve using Brent with a maturity-aware bracket and production-grade
        // tolerance. Initial guess is 0.0 (0 bp).
        let bracket = self.initial_bracket_decimal(bond, quote_date)?;
        let solver = BrentSolver::new()
            .tolerance(self.config.tolerance)
            .initial_bracket_size(Some(bracket));
        let z = solver.solve(objective, 0.0)?;

        // Surface any pricing error that occurred inside the objective instead
        // of returning a potentially meaningless spread.
        if let Some(err) = pricing_error.into_inner() {
            return Err(err);
        }

        Ok(z)
    }
}
