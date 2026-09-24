//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use std::cell::RefCell;

/// Z-spread solver tolerance on the spread axis (decimal, not bp).
///
/// `1e-10` (0.001 bp) keeps the price residual below `$0.001` per $1M face for
/// a duration-8 bond (`price error ≈ duration × notional × tolerance`), the
/// same order as the YTM solver so yield and spread metrics share precision.
const Z_SPREAD_TOLERANCE: f64 = 1e-10;

/// Base half-width of the Z-spread initial bracket, in basis points.
///
/// ±1000 bp covers IG, HY and most distressed names for short maturities.
const Z_SPREAD_BASE_BRACKET_BP: f64 = 1000.0;

/// Cap on the maturity-scaled Z-spread bracket half-width, in basis points.
const Z_SPREAD_MAX_BRACKET_BP: f64 = 3000.0;

/// Maturity-aware initial Brent bracket half-width, in decimal units.
///
/// The bracket scales from `base_bracket_bp` at zero maturity to twice that
/// at 30 years and beyond (`base × (1 + min(years/30, 1))`), capped at
/// `max_bracket_bp`. Years are measured ACT/365F from `as_of` to maturity;
/// this is a numerical heuristic, independent of the coupon day count.
///
/// # Arguments
///
/// * `bond` - Bond whose maturity sets the horizon.
/// * `as_of` - Quote/settlement date the horizon starts from; on or after
///   maturity the base bracket is returned.
/// * `base_bracket_bp` - Half-width in basis points for a zero-length horizon.
/// * `max_bracket_bp` - Maximum half-width in basis points after scaling.
pub(crate) fn maturity_scaled_bracket(
    bond: &Bond,
    as_of: Date,
    base_bracket_bp: f64,
    max_bracket_bp: f64,
) -> finstack_quant_core::Result<f64> {
    if as_of >= bond.maturity {
        return Ok(base_bracket_bp / 10_000.0);
    }
    let years = finstack_quant_core::dates::DayCount::Act365F
        .year_fraction(as_of, bond.maturity, DayCountContext::default())?
        .max(0.0);
    let maturity_scale = 1.0 + (years / 30.0).min(1.0);
    Ok((base_bracket_bp * maturity_scale).min(max_bracket_bp) / 10_000.0)
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
/// Uses Brent's method with tolerance `1e-10` and a maturity-aware initial
/// bracket: ±1000 bp for short-dated bonds, widened up to ±3000 bp for
/// long-dated bonds.
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
#[derive(Debug, Clone, Copy, Default)]
pub struct ZSpreadCalculator;

pub(crate) fn bond_z_spread_compounding_frequency(bond: &Bond) -> f64 {
    let years = bond.cashflow_spec.frequency().to_years();
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

pub(crate) struct BondZSpreadPricingKernel {
    pub(crate) quote_date: Date,
    cached_flows: Vec<(f64, f64, f64)>,
    compounds_per_year: f64,
}

impl BondZSpreadPricingKernel {
    pub(crate) fn new(
        bond: &Bond,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Self> {
        let quote_ctx = QuoteDateContext::new(bond, curves, as_of)?;
        let flows = quote_ctx.entitled_flows(bond, curves, as_of)?;
        let workout = crate::instruments::fixed_income::bond::metrics::quoted_workout_path(
            bond, curves, as_of, &flows,
        )?;
        let (spread_flows, quote_date) = match workout {
            Some((_, workout_flows, workout_quote_date)) => (workout_flows, workout_quote_date),
            None if bond.has_exercise_rights() => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Z-spread pricing for option-bearing bond '{}' requires an explicit quoted clean price to select a workout path; use OAS for model-based optional pricing",
                    bond.id.as_str()
                )));
            }
            None => (flows, quote_ctx.quote_date),
        };
        let disc = curves.get_discount(&bond.discount_curve_id)?;
        let day_count = disc.day_count();
        let cached_flows = spread_flows
            .iter()
            .filter(|(date, _)| *date > quote_date)
            .map(
                |(date, amount)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                    // Supply the coupon frequency so ICMA-style curve day
                    // counts (which require a reference frequency) don't
                    // hard-error; ignored by ACT/30-360 conventions.
                    let t = day_count.year_fraction(
                        quote_date,
                        *date,
                        DayCountContext {
                            frequency: Some(bond.cashflow_spec.frequency()),
                            coupon_period: crate::instruments::fixed_income::bond::pricing::quote_conversions::icma_reference_period(
                                day_count, bond.cashflow_spec.frequency(), spread_flows.iter().map(|(d, _)| *d), quote_date,
                            ),
                            ..DayCountContext::default()
                        },
                    )?;
                    let df_base = disc.df_between_dates(quote_date, *date)?;
                    Ok((t, df_base, amount.amount()))
                },
            )
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        Ok(Self {
            quote_date,
            cached_flows,
            compounds_per_year: bond_z_spread_compounding_frequency(bond),
        })
    }

    pub(crate) fn price(&self, z: f64) -> finstack_quant_core::Result<f64> {
        let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for (t, df_base, amount) in &self.cached_flows {
            let df_z = z_spread_discount_factor(*df_base, *t, z, self.compounds_per_year)?;
            pv.add(*amount * df_z);
        }
        Ok(pv.total())
    }
}

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Get bond and compute quote-date context
        let bond: &Bond = context.instrument_as()?;

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Determine dirty market value in currency at quote_date
        let target_value_currency: f64 = if let Some(clean_px) = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
        {
            // Use accrued at quote_date for dirty price calculation
            quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount())
        } else {
            // Fallback: forward-value the model PV (at `as_of`) to the
            // quote/settlement date, matching the YTM fallback. The kernel
            // discounts from `quote_date`, so an un-carried PV target would
            // bias the solved spread by the settlement-period carry.
            crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
                bond,
                &context.curves,
                context.as_of,
                quote_ctx.quote_date,
                context.base_value.amount(),
            )?
        };

        // Build the same quote-date/workout-path repricing kernel used by
        // price_from_z_spread and bond spread duration.
        let pricing_kernel =
            BondZSpreadPricingKernel::new(bond, context.curves.as_ref(), context.as_of)?;
        let quote_date = pricing_kernel.quote_date;

        // Capture the first z-spread pricing error encountered inside the
        // objective so a bad-curve-data failure is surfaced as the real cause
        // rather than an opaque `SolverConvergenceFailed`.
        let pricing_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);

        // Objective: PV_z(z) - target_value_currency = 0
        //
        // When `z_spread_discount_factor` returns `Err` (non-positive DF or
        // non-positive compounding denominator), we capture the first error and
        // return a large positive residual (+1e12) so Brent can report a
        // convergence failure instead of silently propagating NaN/Inf values
        // into the PV accumulator.
        let objective = |z: f64| -> f64 {
            match pricing_kernel.price(z) {
                Ok(pv) => pv - target_value_currency,
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
                    1e12
                }
            }
        };

        // Solve using Brent with a maturity-aware bracket and production-grade
        // tolerance. Initial guess is 0.0 (0 bp).
        let bracket = maturity_scaled_bracket(
            bond,
            quote_date,
            Z_SPREAD_BASE_BRACKET_BP,
            Z_SPREAD_MAX_BRACKET_BP,
        )?;
        let solver = BrentSolver::new()
            .tolerance(Z_SPREAD_TOLERANCE)
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
