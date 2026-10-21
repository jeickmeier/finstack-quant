//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::pricing::quote_conversions::{solve_kernel_spread, BondZSpreadPricingKernel};
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
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

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let target = super::quote_date_dirty_target(bond, context, &quote_ctx)?;

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
