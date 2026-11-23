//! Core attribution enums, bucket share descriptors, and metadata.

#[cfg(doc)]
use super::pnl::PnlAttribution;
use crate::taylor::TaylorAttributionConfig;
use finstack_quant_core::config::RoundingContext;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::{Money, fx::FxPolicyMeta};
use serde::{Deserialize, Serialize};

/// Controls where attribution repricing work spends parallelism.
///
/// `Serial` is the default: typical standalone factor sets are small enough
/// that inner Rayon costs more than it saves. `Parallel` opts into Rayon for
/// independent factor repricings when the caller is not already parallelizing
/// an outer portfolio or batch loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    /// Use Rayon for independent attribution repricings.
    Parallel,
    /// Run independent attribution repricings sequentially.
    #[default]
    Serial,
}

/// Attribution methodology for decomposing P&L.
///
/// Four methodologies are supported:
/// - **Parallel**: Independent factor isolation (may not sum due to cross-effects)
/// - **Waterfall**: Sequential application (guarantees sum = total, order matters)
/// - **MetricsBased**: Linear approximation using existing metrics (fast but approximate)
/// - **Taylor**: Sensitivity-based Taylor expansion (first/second order via bump-and-reprice)
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AttributionMethod {
    /// Independent factor isolation (may not sum due to cross-effects).
    ///
    /// Each factor is isolated independently by restoring T₀ values for that
    /// factor while keeping T₁ values for all others. Residual captures
    /// cross-effects and non-linearities.
    #[default]
    Parallel,

    /// Sequential waterfall attribution (guarantees sum = total, order matters).
    ///
    /// Factors are applied one-by-one in the specified order. Each factor's
    /// P&L is computed with all previous factors at T₁ and remaining at T₀.
    /// Residual is minimal by construction.
    Waterfall(Vec<AttributionFactor>),

    /// Use existing metrics (Theta, DV01, CS01) for approximation.
    ///
    /// Linear approximation using pre-computed sensitivities. Fast but less
    /// accurate for large market moves due to convexity effects.
    MetricsBased,

    /// Sensitivity-based Taylor expansion (first/second order).
    ///
    /// Computes sensitivities at T₀ via bump-and-reprice, then multiplies by
    /// observed market moves to decompose P&L. Optionally includes second-order
    /// gamma/convexity terms.
    Taylor(TaylorAttributionConfig),
}

/// Factor types for P&L attribution.
///
/// Groups `MarketContext` inputs by their economic role:
/// - **RatesCurves**: discount_curves + forward_curves
/// - **CreditCurves**: hazard_curves
/// - **InflationCurves**: inflation_curves + published inflation_indices
/// - **Correlations**: base_correlation_curves
/// - **Fx**: FxMatrix
/// - **Volatility**: surfaces and declared scalar volatility quotes
/// - **MarketScalars**: other prices, non-fixing series and dividends
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AttributionFactor {
    /// Time decay and accruals (Theta).
    Carry,

    /// Interest rate curves (discount & forward).
    RatesCurves,

    /// Credit hazard curves (spread risk).
    CreditCurves,

    /// Inflation curves and published CPI index changes.
    InflationCurves,

    /// Base correlation curves (structured credit).
    Correlations,

    /// FX rate changes.
    Fx,

    /// Implied volatility changes.
    Volatility,

    /// Market scalars (dividends and equity/commodity prices).
    MarketScalars,

    /// Model-specific parameters (prepayment, default, recovery, conversion).
    ModelParameters,
}

impl AttributionFactor {
    /// Return the canonical snake-case serde value for this factor.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Carry => "carry",
            Self::RatesCurves => "rates_curves",
            Self::CreditCurves => "credit_curves",
            Self::InflationCurves => "inflation_curves",
            Self::Correlations => "correlations",
            Self::Fx => "fx",
            Self::Volatility => "volatility",
            Self::MarketScalars => "market_scalars",
            Self::ModelParameters => "model_parameters",
        }
    }
}

/// A money bucket of an attribution result, named by the result field holding
/// it; the instrument and portfolio results use the same fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PnlBucket {
    /// Time decay and accrual (`carry`).
    Carry,
    /// Discount and forward curve moves (`rates_curves_pnl`).
    RatesCurvesPnl,
    /// Credit curve moves (`credit_curves_pnl`).
    CreditCurvesPnl,
    /// Inflation curve moves (`inflation_curves_pnl`).
    InflationCurvesPnl,
    /// Correlation moves (`correlations_pnl`).
    CorrelationsPnl,
    /// FX rate moves on the instrument's pricing (`fx_pnl`).
    FxPnl,
    /// Translation of the result into the reporting currency (`fx_translation_pnl`).
    FxTranslationPnl,
    /// Volatility surface moves (`vol_pnl`).
    VolPnl,
    /// Interaction between factors (`cross_factor_pnl`).
    CrossFactorPnl,
    /// Model parameter changes (`model_params_pnl`).
    ModelParamsPnl,
    /// Market scalar moves (`market_scalars_pnl`).
    MarketScalarsPnl,
    /// Unexplained remainder (`residual`).
    Residual,
}

impl PnlBucket {
    /// Return the canonical snake-case serde value, the result field's name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Carry => "carry",
            Self::RatesCurvesPnl => "rates_curves_pnl",
            Self::CreditCurvesPnl => "credit_curves_pnl",
            Self::InflationCurvesPnl => "inflation_curves_pnl",
            Self::CorrelationsPnl => "correlations_pnl",
            Self::FxPnl => "fx_pnl",
            Self::FxTranslationPnl => "fx_translation_pnl",
            Self::VolPnl => "vol_pnl",
            Self::CrossFactorPnl => "cross_factor_pnl",
            Self::ModelParamsPnl => "model_params_pnl",
            Self::MarketScalarsPnl => "market_scalars_pnl",
            Self::Residual => "residual",
        }
    }
}

/// One money bucket of an attribution result with its share of total P&L.
///
/// Returned by [`PnlAttribution::pct_of_total_by_bucket`] and by the portfolio
/// crate's `PortfolioAttribution::pct_of_total_by_bucket`, one record per bucket
/// in the result struct's field order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PnlBucketShare {
    /// Result field holding this bucket (`carry`, `rates_curves_pnl`, ...,
    /// `residual`); the instrument and portfolio results use the same names.
    pub field: PnlBucket,
    /// The bucket's P&L as attributed, in the `total_pnl` currency.
    pub amount: Money,
    /// Signed share of total P&L in percentage points (`25.0` = 25%); `None`
    /// when `total_pnl` is effectively zero and the share is undefined.
    pub pct_of_total: Option<f64>,
}

/// Attribution metadata.
///
/// Records methodology, dates, repricing count, and residual statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct AttributionMeta {
    /// Attribution method used.
    pub method: AttributionMethod,

    /// Start date (T₀).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub t0: Date,

    /// End date (T₁).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub t1: Date,

    /// Instrument identifier.
    pub instrument_id: String,

    /// Number of top-level valuation calls, including attempted carry-helper
    /// metric, flat-yield and funding valuations. Internal risk-metric bump
    /// valuations are not counted separately.
    pub num_repricings: usize,

    /// Absolute tolerance for residual validation.
    pub tolerance_abs: f64,

    /// Percentage tolerance for residual validation.
    pub tolerance_pct: f64,

    /// Residual as percentage of total P&L.
    pub residual_pct: f64,

    /// Rounding context used for calculations.
    pub rounding: RoundingContext,

    /// FX policy metadata (if FX conversions were applied).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx_policy: Option<FxPolicyMeta>,

    /// Execution policy the attribution ran under (workspace
    /// policy-visibility invariant: results stamp the parallel flag).
    /// `None` for methods without a policy knob (metrics-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_policy: Option<ExecutionPolicy>,

    /// Diagnostic notes and warnings.
    pub notes: Vec<String>,
}

impl AttributionMethod {
    /// Return the canonical snake-case serde variant name for this method.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Waterfall(_) => "waterfall",
            Self::MetricsBased => "metrics_based",
            Self::Taylor(_) => "taylor",
        }
    }

    /// Returns the risk metrics required for this attribution method.
    ///
    /// For `MetricsBased`, returns first-order sensitivities (Theta, aggregate
    /// and bucketed DV01/CS01, Vega, Delta, FX01, Inflation01, Dividend01) plus
    /// second-order terms
    /// (Gamma, Convexity, IrConvexity, Volga, Vanna, CsGamma, InflationConvexity)
    /// needed by the metrics-based attribution algorithm.
    ///
    /// All other methods return an empty vec (they reprice directly rather than
    /// using pre-computed metrics).
    pub fn required_metrics(&self) -> Vec<finstack_quant_valuations::metrics::MetricId> {
        use finstack_quant_valuations::metrics::MetricId;
        match self {
            AttributionMethod::MetricsBased => vec![
                // First-order metrics
                MetricId::Theta,
                MetricId::Dv01,
                MetricId::Cs01,
                // Bucketed rates retain tenor moves under curve twists.
                MetricId::BucketedDv01,
                // Per-tenor par-spread CS01 — drives key-rate credit attribution
                // when available (CDS-family instruments); otherwise the
                // aggregate `Cs01` path is used.
                MetricId::BucketedCs01,
                MetricId::Vega,
                MetricId::Delta,
                MetricId::Fx01,
                MetricId::Inflation01,
                MetricId::Dividend01,
                // Second-order metrics
                MetricId::Gamma,
                MetricId::Convexity,
                MetricId::IrConvexity,
                MetricId::Volga,
                MetricId::Vanna,
                MetricId::CrossGammaRatesCredit,
                MetricId::CrossGammaRatesVol,
                MetricId::CrossGammaSpotVol,
                MetricId::CrossGammaSpotCredit,
                MetricId::CrossGammaFxVol,
                MetricId::CrossGammaFxRates,
                MetricId::CrossGammaCreditVol,
                MetricId::CsGamma,
                MetricId::InflationConvexity,
            ],
            _ => vec![],
        }
    }
}

impl std::fmt::Display for AttributionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributionMethod::Parallel => write!(f, "Parallel"),
            AttributionMethod::Waterfall(_) => write!(f, "Waterfall"),
            AttributionMethod::MetricsBased => write!(f, "MetricsBased"),
            AttributionMethod::Taylor(_) => write!(f, "Taylor"),
        }
    }
}
