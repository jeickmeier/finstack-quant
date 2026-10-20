//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::math::z_spread_discount_factor;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

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
/// use finstack_quant_valuations_fixed_income::bond::Bond;
/// use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricId, MetricContext};
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
        let workout = crate::bond::metrics::quoted_workout_path(bond, curves, as_of, &flows)?;
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
        let periods = if day_count == finstack_quant_core::dates::DayCount::Act365L {
            crate::bond::pricing::time_basis::bond_coupon_periods(bond)?
        } else {
            Vec::new()
        };
        let cached_flows = spread_flows
            .iter()
            .filter(|(date, _)| *date > quote_date)
            .map(
                |(date, amount)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                    // Supply the coupon frequency so ICMA-style curve day
                    // counts (which require a reference frequency) don't
                    // hard-error; ignored by ACT/30-360 conventions.
                    let t = if day_count == finstack_quant_core::dates::DayCount::Act365L {
                        crate::bond::pricing::time_basis::act365l_year_fraction(
                            bond.cashflow_spec.frequency(),
                            &periods,
                            quote_date,
                            *date,
                        )?
                    } else {
                        day_count.year_fraction(
                            quote_date,
                            *date,
                            DayCountContext {
                                frequency: Some(bond.cashflow_spec.frequency()),
                                coupon_period:
                                    finstack_quant_valuations_core::instruments::pricing::coupon_period::icma_reference_period(
                                        day_count,
                                        bond.cashflow_spec.frequency(),
                                        spread_flows.iter().map(|(d, _)| *d),
                                        quote_date,
                                    ),
                                ..DayCountContext::default()
                            },
                        )?
                    };
                    let df_base = disc.df_between_dates(quote_date, *date)?;
                    Ok((t, df_base, amount.amount()))
                },
            )
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        Ok(Self {
            quote_date,
            cached_flows,
            compounds_per_year: bond.cashflow_spec.frequency().compounding_frequency(),
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
        let bond: &Bond = context.instrument_as()?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let target = quote_date_dirty_target(bond, context, &quote_ctx)?;

        // The same quote-date/workout-path repricing kernel used by
        // price_from_z_spread and bond spread duration.
        let kernel = BondZSpreadPricingKernel::new(bond, context.curves.as_ref(), context.as_of)?;
        solve_kernel_spread(
            bond,
            &kernel,
            target,
            kernel.quote_date,
            Z_SPREAD_BASE_BRACKET_BP,
            Z_SPREAD_MAX_BRACKET_BP,
            Z_SPREAD_TOLERANCE,
        )
    }
}

/// Dirty target value in currency at the quote/settlement date.
///
/// Uses the quoted clean price (percent of par) plus accrued at the quote
/// date; without a quote, forward-values the model PV at `as_of` to the quote
/// date, matching the YTM fallback, because the spread kernels discount from
/// the quote date and an un-carried PV target would bias the solved spread by
/// the settlement-period carry.
///
/// # Arguments
///
/// * `bond` - Bond whose quoted clean price and notional set the target.
/// * `context` - Metric context supplying curves, `as_of` and the base model PV.
/// * `quote_ctx` - Quote-date context (settlement date and accrued there).
pub(crate) fn quote_date_dirty_target(
    bond: &Bond,
    context: &MetricContext,
    quote_ctx: &QuoteDateContext,
) -> finstack_quant_core::Result<f64> {
    match bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct
    {
        Some(clean_px) => Ok(quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount())),
        None => crate::bond::pricing::settlement::model_dirty_at_quote_date(
            bond,
            &context.curves,
            context.as_of,
            quote_ctx.quote_date,
            context.base_value.amount(),
        ),
    }
}

/// Solve `kernel.price(s) = target` for the additive spread `s` (decimal).
///
/// Brent starts at `0.0` with a maturity-aware initial bracket (see
/// [`maturity_scaled_bracket`]). A kernel pricing failure (a non-positive
/// discount factor or compounding denominator at a deeply negative spread)
/// aborts the solve and is returned as-is rather than steering the bracket.
///
/// # Arguments
///
/// * `bond` - Bond whose maturity scales the initial bracket.
/// * `kernel` - Spread repricing kernel; its PV is decreasing in the spread.
/// * `target` - Dirty target value in currency at the kernel's quote date.
/// * `bracket_from` - Date from which the bracket's maturity horizon is measured.
/// * `base_bracket_bp` - Bracket half-width in basis points at zero horizon.
/// * `max_bracket_bp` - Cap on the scaled bracket half-width, in basis points.
/// * `tolerance` - Brent tolerance on the spread axis (decimal).
pub(crate) fn solve_kernel_spread(
    bond: &Bond,
    kernel: &BondZSpreadPricingKernel,
    target: f64,
    bracket_from: Date,
    base_bracket_bp: f64,
    max_bracket_bp: f64,
    tolerance: f64,
) -> finstack_quant_core::Result<f64> {
    let bracket = maturity_scaled_bracket(bond, bracket_from, base_bracket_bp, max_bracket_bp)?;
    BrentSolver::new()
        .tolerance(tolerance)
        .initial_bracket_size(Some(bracket))
        .solve_fallible(|z| Ok(kernel.price(z)? - target), 0.0)
}
