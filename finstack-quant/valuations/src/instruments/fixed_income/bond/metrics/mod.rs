//! Bond-specific metric calculators split into per-metric modules.
//!
//! This module provides metric calculators for bond-specific risk and valuation metrics.
//!
//! # Available Metrics
//!
//! ## Price and Yield Metrics
//! - **Accrued Interest**: Interest accrued since last coupon payment
//! - **Clean Price**: Quoted price excluding accrued interest
//! - **Dirty Price**: Clean price plus accrued interest
//! - **Yield to Maturity (YTM)**: Internal rate of return
//! - **Yield to Worst (YTW)**: Minimum yield across call/put/maturity paths
//!
//! ## Risk Metrics
//! - **Macaulay Duration**: Weighted average time to cashflows
//! - **Modified Duration**: Interest rate sensitivity measure
//! - **Convexity**: Curvature of price/yield relationship
//! - **DV01**: Dollar value of 1bp rate change
//! - **CS01**: Credit spread sensitivity
//!
//! ## Spread Metrics
//! - **Z-Spread**: Zero-volatility spread over discount curve
//! - **OAS**: Option-adjusted spread (for callable/putable bonds)
//! - **I-Spread**: Interpolated spread (YTM - par swap rate)
//! - **Discount Margin**: Spread measure for floating-rate notes
//! - **Asset Swap Spreads**: Par and market asset swap spreads
//!
//! # Examples
//!
//! ```text
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//! use finstack_quant_valuations::instruments::fixed_income::bond::metrics::register_bond_metrics;
//! use finstack_quant_valuations::metrics::{MetricRegistry, MetricId};
//!
//! let mut registry = MetricRegistry::new();
//! register_bond_metrics(&mut registry);
//!
//! // Use registry to compute metrics for bonds
//! ```
//!
//! # See Also
//!
//! - [`register_bond_metrics`] for registering all bond metrics
//! - [`crate::metrics`] for the metrics framework

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Accrued interest calculator
pub(crate) mod accrued;
/// Convexity calculator
pub(crate) mod convexity;
/// CS01 calculators with z-spread fallback for bonds without credit curves
pub(crate) mod cs01;
/// Macaulay duration calculator
pub(crate) mod duration_macaulay;
/// Modified duration calculator
pub(crate) mod duration_modified;
/// Option-aware bond DV01 calculator
pub(crate) mod dv01;
/// Effective duration and convexity for bonds with embedded options
pub(crate) mod effective;
/// Price, yield, and spread metrics
pub(crate) mod price_yield_spread;
/// Quote-reproducing risk view for bond bump-based sensitivities
mod risk_view;
/// Weighted Average Life calculator
pub(crate) mod wal;
/// Yield-basis DV01 calculator
pub(crate) mod yield_dv01;

pub(crate) use accrued::AccruedInterestCalculator;
pub(crate) use convexity::ConvexityCalculator;
pub(crate) use cs01::{BondBucketedCs01Calculator, BondCs01Calculator};
pub(crate) use duration_macaulay::MacaulayDurationCalculator;
pub(crate) use duration_modified::ModifiedDurationCalculator;
pub(crate) use dv01::{BondBucketedDv01Calculator, BondDv01Calculator};
pub use price_yield_spread::{
    AssetSwapMarketCalculator, AssetSwapParCalculator, DiscountMarginCalculator, ZSpreadCalculator,
};
pub(crate) use price_yield_spread::{
    BondVegaCalculator, CleanPriceCalculator, DirtyPriceCalculator, EmbeddedOptionValueCalculator,
    ISpreadCalculator, OasCalculator, YtmCalculator, YtwCalculator,
};
pub(crate) use wal::BondWalCalculator;
pub(crate) use yield_dv01::YieldDv01Calculator;

type BondCashflowPath = Vec<(Date, Money)>;
type QuotedWorkoutPath = (f64, BondCashflowPath, Date);

pub(crate) fn bond_risk_basis(
    context: &crate::metrics::MetricContext,
) -> crate::instruments::BondRiskBasis {
    context
        .get_metric_overrides()
        .map_or_else(crate::instruments::BondRiskBasis::default, |overrides| {
            overrides.bond_risk_basis_or_default()
        })
}

pub(crate) fn quoted_workout_path(
    bond: &crate::instruments::Bond,
    curves: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
    flows: &[(Date, Money)],
) -> finstack_quant_core::Result<Option<QuotedWorkoutPath>> {
    if !bond.call_put.as_ref().is_some_and(|cp| cp.has_options()) {
        return Ok(None);
    }

    let Some(clean_px) = bond.pricing_overrides.market_quotes.quoted_clean_price else {
        return Ok(None);
    };

    let quote_ctx =
        crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
            bond, curves, as_of,
        )?;
    let dirty_now = Money::new(
        quote_ctx.dirty_from_clean_pct(clean_px, bond.notional.amount()),
        bond.notional.currency(),
    );
    let schedule = bond.full_cashflow_schedule(curves)?;
    let (workout_yield, workout_flows) =
        crate::instruments::fixed_income::bond::pricing::quote_conversions::solve_ytw_from_flows(
            bond,
            flows,
            quote_ctx.quote_date,
            dirty_now,
            Some(&schedule),
        )?;

    Ok(Some((workout_yield, workout_flows, quote_ctx.quote_date)))
}

/// Registers all bond metrics to a registry.
///
/// This function registers all bond-specific metric calculators to the provided
/// metric registry, enabling computation of price, yield, duration, convexity,
/// and spread metrics for bonds.
///
/// # Arguments
///
/// * `registry` - The metric registry to register bond metrics into
///
/// # Examples
///
/// ```text
/// use finstack_quant_valuations::instruments::fixed_income::bond::metrics::register_bond_metrics;
/// use finstack_quant_valuations::metrics::MetricRegistry;
///
/// let mut registry = MetricRegistry::new();
/// register_bond_metrics(&mut registry);
/// ```
pub(crate) fn register_bond_metrics(registry: &mut crate::metrics::MetricRegistry) {
    use crate::metrics::{
        make_credit_bumper, make_rates_bumper, CrossFactorCalculator, CrossFactorPair, MetricId,
    };
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::CrossGammaRatesCredit,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::RatesCredit,
            make_rates_bumper,
            make_credit_bumper,
        )),
        &[InstrumentType::Bond],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Bond,
        metrics: [
            (Accrued, AccruedInterestCalculator),
            (DirtyPrice, DirtyPriceCalculator),
            (CleanPrice, CleanPriceCalculator),
            (Ytm, YtmCalculator),
            (Ytw, YtwCalculator),

            (DurationMac, MacaulayDurationCalculator),
            (DurationMod, ModifiedDurationCalculator),
            (YieldDv01, YieldDv01Calculator),
            (Convexity, ConvexityCalculator),

            (Oas, OasCalculator),
            (EmbeddedOptionValue, EmbeddedOptionValueCalculator),
            (Vega, BondVegaCalculator),
            (ZSpread, ZSpreadCalculator::default()),
            (ISpread, ISpreadCalculator::default()),
            (DiscountMargin, DiscountMarginCalculator::default()),
            (ASWPar, AssetSwapParCalculator::default()),
            (ASWMarket, AssetSwapMarketCalculator::default()),

            (WAL, BondWalCalculator),

            // Theta is now registered universally in metrics::standard_registry()

            (Dv01, BondDv01Calculator),
            (BucketedDv01, BondBucketedDv01Calculator),

            (Cs01, BondCs01Calculator),
            (BucketedCs01, BondBucketedCs01Calculator),

        ]
    };
}
