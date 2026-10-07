//! Provenance stamped on a valuation result: which model, dates, market data and
//! bump sizes produced it.

use crate::instruments::MarketDependencies;
use crate::pricer::ModelKey;
use finstack_quant_core::dates::Date;

/// Finite-difference bump sizes in force for a metric request.
///
/// These are the values after layering the `valuations.sensitivities.v1`
/// configuration extension and the instrument's
/// `metric_pricing_overrides.bump_config` over the library defaults. Bumped
/// sensitivities are reported per unit bump (per 1bp, per 1 vol point), so
/// these sizes describe how the difference was taken, not the reporting unit.
///
/// A calculator with a fixed, documented shock of its own (for example an
/// analytic greek, or a metric whose rustdoc names its shock) does not read
/// these values.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SensitivityBumps {
    /// Parallel interest-rate bump in basis points (1.0 = 1bp).
    pub rate_bump_bp: f64,
    /// Credit-spread bump in basis points (1.0 = 1bp).
    pub credit_spread_bump_bp: f64,
    /// Spot bump as a decimal fraction of spot (0.01 = 1%).
    pub spot_bump_decimal: f64,
    /// Absolute volatility bump in decimal volatility (0.01 = 1 vol point).
    pub vol_bump_decimal: f64,
    /// Yield bump in basis points for numerical yield duration and convexity.
    /// Absent when each calculator keeps its own default shock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ytm_bump_bp: Option<f64>,
    /// Whether spot and volatility bumps are rescaled by volatility, time to
    /// expiry and moneyness instead of applied at the fixed sizes above.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adaptive_bumps: bool,
}

/// How a valuation result was produced.
///
/// Stamped by [`crate::pricer::PricerRegistry::price_with_metrics`] on every
/// result so the number can be reproduced from the result plus the archived
/// market: the model that priced it, the date the caller asked for, the market
/// data the instrument declares, any scenario adjustment applied to the value,
/// and the bump sizes behind its sensitivities.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ValuationProvenance {
    /// Registered pricing model that produced `value`.
    pub model: ModelKey,
    /// Valuation date the caller requested.
    ///
    /// The result's `as_of` is the effective date after the instrument resolved
    /// it (for example to the market's spot date); the two differ only when
    /// the instrument moved it.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub requested_as_of: Date,
    /// Curves, surfaces, scalars, FX pairs and series the instrument declares
    /// it reads. Identifiers refer to the `MarketContext` the result was
    /// priced against.
    pub market_dependencies: MarketDependencies,
    /// Scenario price shock already applied to `value`, as a decimal
    /// (`-0.10` = the model value was multiplied by 0.90). Absent when the
    /// value is the unadjusted model value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_price_shock_decimal: Option<f64>,
    /// Bump sizes in force for the requested metrics. Absent when no metric
    /// was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_bumps: Option<SensitivityBumps>,
}
