//! Metrics framework for clean separation of pricing and financial measures.
//!
//! This module provides a trait-based architecture for computing financial
//! metrics independently from core pricing logic. Metrics can be computed
//! on-demand, have dependencies, and are cached for efficiency.
//!
//! # Metric Contract
//!
//! Metric values are returned as raw `f64` values, but they are not unitless.
//! The authoritative semantic contract for each measure lives on
//! [`crate::metrics::MetricId`],
//! including:
//!
//! - units such as currency, currency per bp, decimal rate, or vol point
//! - bump conventions such as per-1bp or per-1 vol point
//! - sign conventions for long-holder, payer/receiver, or spot-up interpretations
//! - distinctions between similarly named measures such as `Dv01`, `Pv01`,
//!   `YieldDv01`, and `Cs01Hazard`
//!
//! Consumers should interpret values from
//! [`crate::results::ValuationResult::measures`] through
//! [`crate::metrics::MetricId`] rather than assuming every `f64` is a currency
//! amount.
//!
//! # Key Features
//!
//! - **Trait-based design**: `MetricCalculator` trait for custom metric implementations
//! - **Dependency management**: Automatic computation ordering based on metric dependencies
//! - **Caching**: Built-in caching of intermediate results like cashflows and discount factors
//! - **Instrument-specific**: Metrics can be registered for specific instrument types
//! - **Standard registry**: Pre-configured registry with common financial metrics
//!
//! # Quick Start Examples
//!
//! ## Example 1: Computing Bucketed DV01 for a Bond
//!
//! ```rust,no_run
//! use finstack_valuations::instruments::{Bond, Instrument, PricingOptions};
//! use finstack_valuations::metrics::MetricId;
//! use finstack_core::market_data::context::MarketContext;
//! use time::macros::date;
//!
//! # fn main() -> finstack_core::Result<()> {
//! // Setup: create an example bond and an (empty) market context.
//! // Note: real runs require a populated market context with required curves.
//! let as_of = date!(2025-01-01);
//! let bond = Bond::example().unwrap();
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::BucketedDv01];
//!
//! // Price with metrics
//! let result = bond.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! // Access results
//! let pv = result.value.amount();
//! println!("Bond PV: ${:.2}", pv);
//!
//! // Get total DV01 (scalar)
//! if let Some(total_dv01) = result.measures.get(MetricId::BucketedDv01.as_str()) {
//!     println!("Total DV01: ${:.2} per bp", total_dv01);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 2: Computing Parallel DV01 for an Interest Rate Swap
//!
//! ```rust,no_run
//! use finstack_valuations::instruments::{Instrument, InterestRateSwap, PricingOptions};
//! use finstack_valuations::metrics::MetricId;
//! use finstack_core::market_data::context::MarketContext;
//! use time::macros::date;
//!
//! # fn main() -> finstack_core::Result<()> {
//! let as_of = date!(2025-01-01);
//! let swap = InterestRateSwap::example_standard()?;
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::Dv01]; // Parallel DV01
//!
//! let result = swap.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! if let Some(dv01) = result.measures.get(MetricId::Dv01.as_str()) {
//!     println!("Swap DV01: ${:.2} per bp", dv01);
//!     // Negative DV01 means swap loses value when rates rise
//!     // (typical for receiver swaps)
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 3: Computing Theta (Time Decay) for an Option
//!
//! ```rust,no_run
//! use finstack_valuations::instruments::{EquityOption, Instrument, PricingOptions};
//! use finstack_valuations::metrics::MetricId;
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::create_date;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let expiry = create_date(2024, Month::July, 1)?; // 6-month option
//!
//! let option = EquityOption::european_call(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     expiry,
//!     Money::new(100.0, Currency::USD),
//! )?;
//! let market = MarketContext::new();
//! let metrics = vec![MetricId::Theta];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! if let Some(theta) = result.measures.get(MetricId::Theta.as_str()) {
//!     println!("Option theta per day: ${:.2}", theta);
//!     // Negative theta = option loses value over time (time decay)
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 4: Computing Multiple Greeks for an Option
//!
//! ```rust,no_run
//! use finstack_valuations::instruments::{EquityOption, Instrument, PricingOptions};
//! use finstack_valuations::metrics::MetricId;
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::create_date;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let option = EquityOption::european_call(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     create_date(2024, Month::July, 1)?,
//!     Money::new(100.0, Currency::USD),
//! )?;
//! let market = MarketContext::new();
//! let metrics = vec![
//!     MetricId::Delta,
//!     MetricId::Gamma,
//!     MetricId::Vega,
//!     MetricId::Theta,
//!     MetricId::Rho,
//! ];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! println!("Option Greeks:");
//! println!("  PV:    ${:.2}", result.value.amount());
//! println!(
//!     "  Delta: {:.4}",
//!     result.measures.get(MetricId::Delta.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Gamma: {:.4}",
//!     result.measures.get(MetricId::Gamma.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Vega:  {:.4}",
//!     result.measures.get(MetricId::Vega.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Theta: {:.4}",
//!     result.measures.get(MetricId::Theta.as_str()).unwrap_or(&0.0)
//! );
//! println!(
//!     "  Rho:   {:.4}",
//!     result.measures.get(MetricId::Rho.as_str()).unwrap_or(&0.0)
//! );
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! - **`MetricId`**: Strongly-typed identifiers for all metrics
//! - **`MetricCalculator`**: Trait for implementing custom metrics
//! - **`MetricContext`**: Context containing instrument, market data, and cached results
//! - **`MetricRegistry`**: Registry for managing calculators and dependencies
//! - **Risk metrics**: Specialized calculators for DV01, bucketed risk, and time decay
//!
//! # References
//!
//! - Fixed-income risk measures: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Curve construction and sensitivities: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - VaR and Expected Shortfall interpretation: `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//!

// Internal submodules (organized by concern)

// calculators module removed - GenericPv was the only calculator and has been removed
mod core;
pub mod risk;
pub(crate) mod sensitivities;
mod shared;

// Core surface (supported)
pub use core::ids::{MetricGroup, MetricId};
pub use core::registry::MetricRegistry;
pub use core::traits::{MetricCalculator, MetricContext, Structured2D};
/// Format a standard risk bucket (years) as a human-readable label.
pub use sensitivities::config::format_bucket_label;
pub use sensitivities::cross_factor::{CrossFactorCalculator, CrossFactorPair};

// -----------------------------------------------------------------------------
// Crate-internal re-exports (NOT part of the public API)
// -----------------------------------------------------------------------------
//
// These are used across `finstack-valuations` (instrument metric registries, etc.) but are not
// supported as a stable downstream API. Keep them `pub(crate)` so we can refactor module layout
// without creating public breakage surface.
pub(crate) use core::finite_difference::{
    bump_discount_curve_parallel, bump_scalar_price, bump_sizes, bump_surface_vol_absolute,
    central_diff_by_half_bump, central_diff_scalar_relative, replace_scalar_value,
    scalar_numeric_value, scaled_central_diff_by_width,
};
pub(crate) use sensitivities::breakeven::BreakevenCalculator;
pub(crate) use sensitivities::carry_decomposition::{
    CarryComponentLookup, CarryDecompositionCalculator,
};
pub(crate) use sensitivities::config::from_finstack_config_or_default as resolve_sensitivities_config;
pub(crate) use sensitivities::cross_factor::{
    make_credit_bumper, make_fx_bumper, make_rates_bumper, make_spot_bumper, make_vol_bumper,
};
pub(crate) use sensitivities::cs01::{
    GenericBucketedCs01, GenericBucketedCs01Hazard, GenericParallelCs01, GenericParallelCs01Hazard,
};
pub(crate) use sensitivities::dv01::{Dv01CalculatorConfig, UnifiedDv01Calculator};
pub(crate) use sensitivities::fd_greeks::{
    GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVega, GenericFdVolga, HasDayCount,
    HasExpiry, HasPricingOverrides,
};
pub(crate) use sensitivities::option_greeks::OptionGreekCalculator;
pub(crate) use sensitivities::theta::{
    calculate_theta_date, GenericThetaAny, ThetaComponentLookup,
};
pub(crate) use sensitivities::vega::KeyRateVega;
pub(crate) use shared::df::{GenericDfEndCalculator, GenericDfStartCalculator};

// -----------------------------------------------------------------------------
// Macros
// -----------------------------------------------------------------------------

/// Define a trivial metric calculator that delegates to an instrument method or closure.
#[macro_export]
macro_rules! define_metric_calculator {
    (
        $(#[$meta:meta])*
        $name:ident,
        instrument = $instrument:ty,
        calc = |$inst:ident, $ctx:ident| $body:expr
        $(, deps = [$($dep:expr),* $(,)?])?
    ) => {
        $(#[$meta])*
        pub(crate) struct $name;

        impl $crate::metrics::MetricCalculator for $name {
            fn calculate(
                &self,
                $ctx: &mut $crate::metrics::MetricContext,
            ) -> finstack_core::Result<f64> {
                let $inst: &$instrument = $ctx.instrument_as()?;
                let value: finstack_core::Result<f64> = { $body };
                value
            }

            fn dependencies(&self) -> &[$crate::metrics::MetricId] {
                static DEPS: &[$crate::metrics::MetricId] = &[$($($dep),*)?];
                DEPS
            }
        }
    };
}

// -----------------------------------------------------------------------------
// Error helper functions
// -----------------------------------------------------------------------------

/// Create a NotFound error for missing metrics.
///
/// Use this when a metric dependency or required metric is not available.
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_valuations::metrics::{metric_not_found, MetricId};
/// use finstack_core::Result;
/// use finstack_core::HashMap;
///
/// fn get_metric(id: MetricId, results: &HashMap<MetricId, f64>) -> Result<f64> {
///     results.get(&id).copied().ok_or_else(|| metric_not_found(id))
/// }
/// ```
use crate::metrics::risk::{GenericExpectedShortfall, GenericHVar, VarConfig};
use std::sync::OnceLock;

/// Create a typed NotFound error for a missing metric.
#[inline]
pub fn metric_not_found(metric: MetricId) -> finstack_core::Error {
    finstack_core::InputError::NotFound {
        id: format!("metric:{metric:?}"),
    }
    .into()
}

/// Create a NotFound error for missing context fields.
///
/// Use this when a required field is not present in a context or configuration.
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_valuations::metrics::context_not_found;
/// use finstack_core::types::CurveId;
/// use finstack_core::Result;
///
/// struct PricingContext {
///     discount_curve_id: Option<CurveId>,
/// }
///
/// impl PricingContext {
///     fn discount_curve_id(&self) -> Option<&CurveId> {
///         self.discount_curve_id.as_ref()
///     }
/// }
///
/// fn get_curve_id(context: &PricingContext) -> Result<&CurveId> {
///     context
///         .discount_curve_id()
///         .ok_or_else(|| context_not_found("discount_curve_id"))
/// }
/// ```
#[inline]
pub fn context_not_found(field: &str) -> finstack_core::Error {
    finstack_core::InputError::NotFound {
        id: format!("context.{field}"),
    }
    .into()
}

static STANDARD_REGISTRY: OnceLock<MetricRegistry> = OnceLock::new();
/// Creates a standard metric registry with all built-in metrics.
///
/// This registry includes metrics for:
/// - **Bonds**: YTM, duration, convexity, accrued interest, credit spreads
/// - **Interest Rate Swaps**: DV01, annuity factors, par rates
/// - **Deposits**: Discount factors, par rates, year fractions
/// - **Risk**: Bucketed DV01, time decay (theta)
///
/// See unit tests and `examples/` for usage.
pub fn standard_registry() -> &'static MetricRegistry {
    STANDARD_REGISTRY.get_or_init(|| {
        let mut registry = MetricRegistry::new();

        register_universal_metrics(&mut registry);
        register_credit_cs01_metrics(&mut registry);
        register_equity_instrument_metrics(&mut registry);
        register_fixed_income_instrument_metrics(&mut registry);
        register_rates_instrument_metrics(&mut registry);
        register_credit_derivative_instrument_metrics(&mut registry);
        register_fx_instrument_metrics(&mut registry);
        register_commodity_instrument_metrics(&mut registry);
        register_exotic_instrument_metrics(&mut registry);

        registry
    })
}

fn register_equity_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::equity::spot::metrics::register_equity_metrics(registry);
    crate::instruments::equity::equity_option::metrics::register_equity_option_metrics(registry);
    crate::instruments::equity::equity_trs::metrics::register_equity_trs_metrics(registry);
    crate::instruments::equity::variance_swap::metrics::register_variance_swap_metrics(registry);
    crate::instruments::equity::pe_fund::register_private_markets_fund_metrics(registry);
    crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(registry);
    crate::instruments::equity::vol_index_future::metrics::register_vol_index_future_metrics(
        registry,
    );
    crate::instruments::equity::vol_index_option::metrics::register_vol_index_option_metrics(
        registry,
    );
    crate::instruments::equity::equity_index_future::metrics::register_equity_index_future_metrics(
        registry,
    );
    crate::instruments::equity::real_estate::metrics::register_real_estate_metrics(registry);
    crate::instruments::equity::real_estate::metrics::register_levered_real_estate_metrics(
        registry,
    );
    crate::instruments::exotics::basket::metrics::register_basket_metrics(registry);
}

fn register_fixed_income_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::fixed_income::bond::metrics::register_bond_metrics(registry);
    crate::instruments::fixed_income::bond_future::metrics::register_bond_future_metrics(registry);
    crate::instruments::fixed_income::convertible::metrics::register_convertible_metrics(registry);
    crate::instruments::fixed_income::mbs_passthrough::metrics::register_mbs_passthrough_metrics(
        registry,
    );
    crate::instruments::fixed_income::tba::metrics::register_tba_metrics(registry);
    crate::instruments::fixed_income::dollar_roll::metrics::register_dollar_roll_metrics(registry);
    crate::instruments::fixed_income::cmo::metrics::register_cmo_metrics(registry);
    crate::instruments::fixed_income::inflation_linked_bond::metrics::register_ilb_metrics(
        registry,
    );
    crate::instruments::fixed_income::structured_credit::metrics::register_structured_credit_metrics(registry);
    crate::instruments::fixed_income::term_loan::metrics::register_term_loan_metrics(registry);
    crate::instruments::fixed_income::revolving_credit::metrics::register_revolving_credit_metrics(
        registry,
    );
    crate::instruments::fixed_income::fi_trs::metrics::register_fi_trs_metrics(registry);
}

fn register_rates_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::rates::irs::metrics::register_irs_metrics(registry);
    crate::instruments::rates::deposit::metrics::register_deposit_metrics(registry);
    crate::instruments::rates::fra::metrics::register_fra_metrics(registry);
    crate::instruments::rates::ir_future::metrics::register_ir_future_metrics(registry);
    crate::instruments::rates::ir_future_option::metrics::register_ir_future_option_metrics(
        registry,
    );
    crate::instruments::rates::inflation_swap::metrics::register_inflation_swap_metrics(registry);
    crate::instruments::rates::inflation_cap_floor::metrics::register_inflation_cap_floor_metrics(
        registry,
    );
    crate::instruments::rates::cap_floor::metrics::register_cap_floor_metrics(registry);
    crate::instruments::rates::swaption::metrics::register_swaption_metrics(registry);
    crate::instruments::rates::xccy_swap::metrics::register_xccy_swap_metrics(registry);
    crate::instruments::rates::repo::metrics::register_repo_metrics(registry);
    crate::instruments::rates::basis_swap::metrics::register_basis_swap_metrics(registry);
    crate::instruments::rates::cms_option::metrics::register_cms_option_metrics(registry);
    crate::instruments::rates::cms_swap::metrics::register_cms_swap_metrics(registry);
    crate::instruments::rates::cms_spread_option::metrics::register_cms_spread_option_metrics(
        registry,
    );
}

fn register_credit_derivative_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::credit_derivatives::cds::metrics::register_cds_metrics(registry);
    crate::instruments::credit_derivatives::cds_index::metrics::register_cds_index_metrics(
        registry,
    );
    crate::instruments::credit_derivatives::cds_tranche::metrics::register_cds_tranche_metrics(
        registry,
    );
    crate::instruments::credit_derivatives::cds_option::metrics::register_cds_option_metrics(
        registry,
    );
}

fn register_fx_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::fx::fx_spot::metrics::register_fx_spot_metrics(registry);
    crate::instruments::fx::fx_swap::metrics::register_fx_swap_metrics(registry);
    crate::instruments::fx::fx_forward::metrics::register_fx_forward_metrics(registry);
    crate::instruments::fx::ndf::metrics::register_ndf_metrics(registry);
    crate::instruments::fx::fx_option::metrics::register_fx_option_metrics(registry);
    crate::instruments::fx::fx_variance_swap::metrics::register_fx_variance_swap_metrics(registry);
    crate::instruments::fx::fx_barrier_option::metrics::register_fx_barrier_option_metrics(
        registry,
    );
    crate::instruments::fx::fx_digital_option::metrics::register_fx_digital_option_metrics(
        registry,
    );
    crate::instruments::fx::fx_touch_option::metrics::register_fx_touch_option_metrics(registry);
}

fn register_commodity_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::commodity::commodity_forward::metrics::register_commodity_forward_metrics(
        registry,
    );
    crate::instruments::commodity::commodity_swap::metrics::register_commodity_swap_metrics(
        registry,
    );
    crate::instruments::commodity::commodity_option::metrics::register_commodity_option_metrics(
        registry,
    );
    crate::instruments::commodity::commodity_asian_option::metrics::register_commodity_asian_option_metrics(registry);
    crate::instruments::commodity::commodity_swaption::metrics::register_commodity_swaption_metrics(
        registry,
    );
    crate::instruments::commodity::commodity_spread_option::metrics::register_commodity_spread_option_metrics(registry);
}

fn register_exotic_instrument_metrics(registry: &mut MetricRegistry) {
    crate::instruments::exotics::lookback_option::metrics::register_lookback_option_metrics(
        registry,
    );
    crate::instruments::exotics::asian_option::metrics::register_asian_option_metrics(registry);
    crate::instruments::equity::autocallable::metrics::register_autocallable_metrics(registry);
    crate::instruments::exotics::barrier_option::metrics::register_barrier_option_metrics(registry);
    crate::instruments::equity::cliquet_option::metrics::register_cliquet_option_metrics(registry);
    crate::instruments::fx::quanto_option::metrics::register_quanto_option_metrics(registry);
    crate::instruments::rates::range_accrual::metrics::register_range_accrual_metrics(registry);
    crate::instruments::rates::tarn::metrics::register_tarn_metrics(registry);
    crate::instruments::rates::snowball::metrics::register_snowball_metrics(registry);
    crate::instruments::rates::callable_range_accrual::metrics::register_callable_range_accrual_metrics(registry);
}

fn register_universal_metrics(registry: &mut MetricRegistry) {
    registry.register_metric(MetricId::Theta, std::sync::Arc::new(GenericThetaAny), &[]);
    registry.register_metric(
        MetricId::ThetaCarry,
        std::sync::Arc::new(ThetaComponentLookup(MetricId::ThetaCarry)),
        &[],
    );
    registry.register_metric(
        MetricId::ThetaRollDown,
        std::sync::Arc::new(ThetaComponentLookup(MetricId::ThetaRollDown)),
        &[],
    );
    registry.register_metric(
        MetricId::CarryTotal,
        std::sync::Arc::new(CarryDecompositionCalculator),
        &[],
    );
    registry.register_metric(
        MetricId::CouponIncome,
        std::sync::Arc::new(CarryComponentLookup(MetricId::CouponIncome)),
        &[],
    );
    registry.register_metric(
        MetricId::PullToPar,
        std::sync::Arc::new(CarryComponentLookup(MetricId::PullToPar)),
        &[],
    );
    registry.register_metric(
        MetricId::RollDown,
        std::sync::Arc::new(CarryComponentLookup(MetricId::RollDown)),
        &[],
    );
    registry.register_metric(
        MetricId::FundingCost,
        std::sync::Arc::new(CarryComponentLookup(MetricId::FundingCost)),
        &[],
    );
    registry.register_metric(
        MetricId::Breakeven,
        std::sync::Arc::new(BreakevenCalculator),
        &[],
    );
    registry.register_metric(
        MetricId::HVar,
        std::sync::Arc::new(GenericHVar::new(VarConfig::var_95())),
        &[],
    );
    registry.register_metric(
        MetricId::ExpectedShortfall,
        std::sync::Arc::new(GenericExpectedShortfall::new(VarConfig::var_95())),
        &[],
    );
}

fn register_credit_cs01_metrics(registry: &mut MetricRegistry) {
    // BucketedCs01 (par-spread rebootstrap)
    registry.register_metric(
        MetricId::BucketedCs01,
        std::sync::Arc::new(
            GenericBucketedCs01::<crate::instruments::CreditDefaultSwap>::default(),
        ),
        &[crate::pricer::InstrumentType::CDS],
    );
    registry.register_metric(
        MetricId::BucketedCs01,
        std::sync::Arc::new(GenericBucketedCs01::<
            crate::instruments::credit_derivatives::cds_index::CDSIndex,
        >::default()),
        &[crate::pricer::InstrumentType::CDSIndex],
    );
    registry.register_metric(
        MetricId::BucketedCs01,
        std::sync::Arc::new(GenericBucketedCs01::<
            crate::instruments::credit_derivatives::cds_option::CDSOption,
        >::default()),
        &[crate::pricer::InstrumentType::CDSOption],
    );

    // Cs01Hazard (direct hazard-rate bump, parallel)
    registry.register_metric(
        MetricId::Cs01Hazard,
        std::sync::Arc::new(GenericParallelCs01Hazard::<
            crate::instruments::CreditDefaultSwap,
        >::default()),
        &[crate::pricer::InstrumentType::CDS],
    );
    // Cs01Hazard for CDSTranche and RevolvingCredit are registered locally
    // by their respective metrics modules with custom wrappers.

    // BucketedCs01Hazard (direct hazard-rate bump, bucketed)
    registry.register_metric(
        MetricId::BucketedCs01Hazard,
        std::sync::Arc::new(GenericBucketedCs01Hazard::<
            crate::instruments::CreditDefaultSwap,
        >::default()),
        &[crate::pricer::InstrumentType::CDS],
    );
    registry.register_metric(
        MetricId::BucketedCs01Hazard,
        std::sync::Arc::new(GenericBucketedCs01Hazard::<
            crate::instruments::credit_derivatives::cds_index::CDSIndex,
        >::default()),
        &[crate::pricer::InstrumentType::CDSIndex],
    );
    // BucketedCs01Hazard for CDSTranche and RevolvingCredit are registered
    // locally by their respective metrics modules with custom wrappers. CDS
    // options intentionally expose only quoted-spread CS01, not hazard-rate CS01.
}
