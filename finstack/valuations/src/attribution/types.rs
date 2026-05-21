//! Core data structures for P&L attribution.
//!
//! This module provides types for decomposing multi-period P&L changes into
//! constituent factors: carry, curve shifts, credit spreads, FX, volatility,
//! cross-factor interactions, model parameters, and market scalars.

use finstack_core::config::RoundingContext;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::{fx::FxPolicyMeta, Money};
use finstack_core::types::{CurveId, IssuerId};
use finstack_core::{Error, Result};
use indexmap::IndexMap;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::attribution::taylor::TaylorAttributionConfig;

/// Attribution methodology for decomposing P&L.
///
/// Four methodologies are supported:
/// - **Parallel**: Independent factor isolation (may not sum due to cross-effects)
/// - **Waterfall**: Sequential application (guarantees sum = total, order matters)
/// - **MetricsBased**: Linear approximation using existing metrics (fast but approximate)
/// - **Taylor**: Sensitivity-based Taylor expansion (first/second order via bump-and-reprice)
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
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
/// Maps cleanly to `MarketContext` structure:
/// - **RatesCurves**: discount_curves + forward_curves
/// - **CreditCurves**: hazard_curves
/// - **InflationCurves**: inflation_curves
/// - **Correlations**: base_correlation_curves
/// - **Fx**: FxMatrix
/// - **Volatility**: surfaces (VolSurface)
/// - **MarketScalars**: prices, series, inflation_indices, dividends
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum AttributionFactor {
    /// Time decay and accruals (Theta).
    Carry,

    /// Interest rate curves (discount & forward).
    RatesCurves,

    /// Credit hazard curves (spread risk).
    CreditCurves,

    /// Inflation curves.
    InflationCurves,

    /// Base correlation curves (structured credit).
    Correlations,

    /// FX rate changes.
    Fx,

    /// Implied volatility changes.
    Volatility,

    /// Model-specific parameters (prepayment, default, recovery, conversion).
    ModelParameters,

    /// Market scalars (dividends, equity/commodity prices, inflation indices).
    MarketScalars,
}

/// Complete P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent factors with optional detailed
/// breakdowns by curve, tenor, FX pair, etc.
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::attribution::attribute_pnl_parallel;
/// use finstack_valuations::instruments::Instrument;
/// use finstack_valuations::instruments::rates::deposit::Deposit;
/// use finstack_core::config::FinstackConfig;
/// use finstack_core::currency::Currency;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::money::Money;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of_t0 = date!(2025-01-15);
/// let as_of_t1 = date!(2025-01-16);
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
/// let config = FinstackConfig::default();
///
/// let instrument = Arc::new(
///     Deposit::builder()
///         .id("DEP-1D".into())
///         .notional(Money::new(1_000_000.0, Currency::USD))
///         .start_date(as_of_t0)
///         .maturity(as_of_t1)
///         .day_count(finstack_core::dates::DayCount::Act360)
///         .discount_curve_id("USD-OIS".into())
///         .build()
///         .expect("deposit builder should succeed"),
/// ) as Arc<dyn Instrument>;
///
/// let attribution = attribute_pnl_parallel(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
///     None,
/// )?;
///
/// println!("Total P&L: {}", attribution.total_pnl);
/// println!("Carry: {} ({:.1}%)",
///     attribution.carry,
///     attribution.carry.amount() / attribution.total_pnl.amount() * 100.0
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PnlAttribution {
    /// Total P&L *as reported by this attribution*.
    ///
    /// In the standard total-return convention (the default carry path uses
    /// [`crate::attribution::helpers::apply_total_return_carry`]), this is
    /// `(val_t1 − val_t0) + coupon_income_between(T0, T1)`. Cashflows received
    /// during the period are added back so that
    /// `total_pnl == carry + factor_sum + residual` holds: the `carry` field
    /// includes the coupon income, and reconciliation against the user's
    /// observed val_t1 − val_t0 must subtract the coupon_income out of
    /// `total_pnl` to recover the pure mark-to-market move.
    ///
    /// **For a raw mark-to-market view that excludes intra-period cashflows,
    /// read [`Self::mark_to_market_pnl`].** That field, when present, is the
    /// untouched `val_t1 − val_t0` and never absorbs coupon income.
    pub total_pnl: Money,

    /// Pure mark-to-market change: `val_t1 − val_t0` with **no** intra-period
    /// cashflow adjustment.
    ///
    /// Added in audit rec #2 to separate the raw user-input price change from
    /// the total-return view stamped on `total_pnl`. When the attribution path
    /// added coupon_income to `total_pnl` (the standard total-return convention
    /// in parallel / waterfall / taylor attribution), this field still reports
    /// the raw `val_t1 − val_t0` so a downstream consumer that computed their
    /// own total from the underlying valuations can reconcile cleanly.
    ///
    /// Additive serde extension: pre-existing JSON without this field
    /// deserializes with `None`. New attribution runs populate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_to_market_pnl: Option<Money>,

    /// Carry P&L (theta + accruals).
    pub carry: Money,

    /// Interest rate curves P&L.
    pub rates_curves_pnl: Money,

    /// Credit hazard curves P&L.
    pub credit_curves_pnl: Money,

    /// Inflation curves P&L.
    pub inflation_curves_pnl: Money,

    /// Base correlation curves P&L.
    pub correlations_pnl: Money,

    /// FX rate changes P&L.
    pub fx_pnl: Money,

    /// Implied volatility changes P&L.
    pub vol_pnl: Money,

    /// Cross-factor interaction P&L (rates×credit, spot×vol, FX×rates, etc.).
    pub cross_factor_pnl: Money,

    /// Model parameters P&L.
    pub model_params_pnl: Money,

    /// Market scalars P&L.
    pub market_scalars_pnl: Money,

    /// Residual P&L (total - sum of attributed factors).
    pub residual: Money,

    // Detailed breakdowns
    /// Detailed carry decomposition (theta + roll-down).
    pub carry_detail: Option<CarryDetail>,

    /// Detailed rates curves attribution (by curve and tenor).
    pub rates_detail: Option<RatesCurvesAttribution>,

    /// Detailed credit curves attribution (by curve and tenor).
    pub credit_detail: Option<CreditCurvesAttribution>,

    /// Detailed inflation curves attribution (by curve, optional tenor).
    pub inflation_detail: Option<InflationCurvesAttribution>,

    /// Detailed correlations attribution (by curve).
    pub correlations_detail: Option<CorrelationsAttribution>,

    /// Detailed FX attribution (by currency pair).
    pub fx_detail: Option<FxAttribution>,

    /// Detailed volatility attribution (by surface).
    pub vol_detail: Option<VolAttribution>,

    /// Detailed cross-factor attribution (by factor-pair label).
    pub cross_factor_detail: Option<CrossFactorDetail>,

    /// Detailed model parameters attribution.
    pub model_params_detail: Option<ModelParamsAttribution>,

    /// Detailed market scalars attribution.
    pub scalars_detail: Option<ScalarsAttribution>,

    /// Optional credit-factor-hierarchy decomposition of `credit_curves_pnl`.
    ///
    /// Populated only when an `AttributionSpec.credit_factor_model` was supplied
    /// to the attribution call. When present, this is purely additive detail —
    /// `credit_curves_pnl` itself is unchanged. Old JSON without this field
    /// deserializes with `None` (additive serde extension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_factor_detail: Option<CreditFactorAttribution>,

    /// Optional credit-factor-hierarchy decomposition of carry (PR-8b §7.2).
    ///
    /// Populated only when an `AttributionSpec.credit_factor_model` was
    /// supplied. Additive: when absent, behavior is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_carry_decomposition: Option<CreditCarryDecomposition>,

    /// Attribution metadata.
    pub meta: AttributionMeta,

    /// True if residual computation encountered non-finite (NaN/Inf) inputs,
    /// or if any factor P&L value is non-finite. When `true`, `residual` and
    /// `meta.residual_pct` are not meaningful and downstream callers should
    /// treat the attribution as invalid. Defaults to `false`.
    #[serde(default)]
    pub result_invalid: bool,
}

/// Zero USD `Money` — serde default for [`CreditFactorAttribution::curve_shape_pnl`]
/// so attributions serialized before that field was added still deserialize.
fn zero_money_usd() -> Money {
    Money::new(0.0, Currency::USD)
}

/// Hierarchy-level decomposition of credit P&L, opt-in via
/// `AttributionSpec.credit_factor_model`.
///
/// The reconciliation invariant
///
/// ```text
/// generic_pnl + Σ_levels(level.total) + adder_pnl_total + curve_shape_pnl ≡ credit_curves_pnl
/// ```
///
/// holds at absolute tolerance `1e-8` for both metrics-based and Taylor methods.
/// `curve_shape_pnl` is the non-parallel hazard-curve residual (audit item #1);
/// for a purely parallel credit move it is zero and the invariant reduces to
/// the historical `generic + Σ levels + adder` form.
///
/// **Single-instrument scope**: when produced via the valuations-layer
/// per-instrument attribution wire (`metrics_based`, `taylor`), each call
/// processes a single instrument. Therefore `LevelPnl.by_bucket` will contain
/// at most one entry per call (the issuer's bucket at that level).
/// Portfolio-level multi-bucket aggregation is provided at the portfolio layer
/// (PR-8 onward).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditFactorAttribution {
    /// Deterministic traceability ID for the calibrated model. Format:
    /// `format!("{}/{:016x}", model.as_of, fnv1a64(serde_json::to_string(model)))`.
    pub model_id: String,
    /// P&L attributed to the generic (PC) credit factor:
    /// `-Σ_i CS01_i × β_i^PC × ΔF_PC`.
    pub generic_pnl: Money,
    /// One entry per [`finstack_core::factor_model::credit_hierarchy::HierarchyDimension`]
    /// in the spec order recorded by the model's hierarchy.
    pub levels: Vec<LevelPnl>,
    /// Total adder P&L: `-Σ_i CS01_i × Δadder_i`. This is the **parallel**
    /// issuer-idiosyncratic move only; non-parallel curve-shape risk is
    /// reported separately in [`Self::curve_shape_pnl`].
    pub adder_pnl_total: Money,
    /// P&L attributed to the **non-parallel** part of the hazard-curve move
    /// (steepening / twist / term-structure roll) — the curve-shape residual.
    ///
    /// Audit item #1: previously this residual was absorbed into
    /// `adder_pnl_total`, which mislabeled curve-shape risk as
    /// issuer-idiosyncratic. It is now a distinct component. The reconciliation
    /// invariant is correspondingly:
    /// `generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`.
    ///
    /// `#[serde(default)]` keeps backward compatibility: older serialized
    /// attributions (no `curve_shape_pnl`) deserialize with a zero value.
    #[serde(default = "zero_money_usd")]
    pub curve_shape_pnl: Money,
    /// Optional per-issuer adder breakdown (gated by
    /// `CreditFactorDetailOptions.include_per_issuer_adder`, default off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, Money>>")]
    pub adder_pnl_by_issuer: Option<BTreeMap<IssuerId, Money>>,
    /// Diagnostic: absolute magnitude of the per-issuer adder step P&L
    /// (`|adder_pnl_total|`). Surfaced so downstream consumers can detect
    /// when the adder is absorbing significant non-parallel curve moves.
    /// A `tracing::warn!` is also emitted by the cascade builder when the
    /// adder magnitude exceeds `credit_cascade::ADDER_MAGNITUDE_WARN_RATIO`
    /// of total credit P&L.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adder_magnitude: Option<Money>,
}

/// P&L contribution from a single hierarchy level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LevelPnl {
    /// Human-readable level name (e.g. `"rating"`, `"region"`, `"sector"`,
    /// or a custom dimension key).
    pub level_name: String,
    /// Aggregate P&L for this level across all buckets.
    pub total: Money,
    /// Optional per-bucket breakdown keyed by dotted bucket path
    /// (e.g. `"IG.EU.FIN"`). Empty when
    /// `CreditFactorDetailOptions.include_per_bucket_breakdown == false`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_bucket: BTreeMap<String, Money>,
}

/// Detailed attribution for interest rate curves.
///
/// Provides aggregate and per-curve/per-tenor breakdown for discount
/// and forward curves.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RatesCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor).
    pub by_tenor: IndexMap<(CurveId, String), Money>,

    /// Total discount curves P&L.
    pub discount_total: Money,

    /// Total forward curves P&L.
    pub forward_total: Money,
}

/// Detailed attribution for credit hazard curves.
///
/// Provides per-curve and per-tenor breakdown for credit spread risk.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor).
    pub by_tenor: IndexMap<(CurveId, String), Money>,
}

/// Detailed attribution for inflation curves.
///
/// Provides per-curve breakdown with optional tenor detail for
/// term-structured inflation curves.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InflationCurvesAttribution {
    /// P&L by curve ID.
    pub by_curve: IndexMap<CurveId, Money>,

    /// P&L by (curve_id, tenor) for term-structured inflation curves.
    pub by_tenor: Option<IndexMap<(CurveId, String), Money>>,
}

/// Detailed attribution for base correlation curves.
///
/// Used for structured credit products (CDO tranches, synthetic credit).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CorrelationsAttribution {
    /// P&L by correlation curve ID.
    pub by_curve: IndexMap<CurveId, Money>,
}

/// Detailed attribution for FX rate changes.
///
/// Provides per-currency-pair breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FxAttribution {
    /// P&L by (from_currency, to_currency) pair.
    pub by_pair: IndexMap<(Currency, Currency), Money>,
}

/// Detailed attribution for implied volatility changes.
///
/// Provides per-surface breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VolAttribution {
    /// P&L by volatility surface ID.
    pub by_surface: IndexMap<CurveId, Money>,
}

/// Detailed attribution for cross-factor interaction terms.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CrossFactorDetail {
    /// Total cross-factor P&L across all populated pairs.
    pub total: Money,

    /// P&L by human-readable factor-pair label.
    #[serde(default)]
    pub by_pair: IndexMap<String, Money>,
}

/// Detailed attribution for model-specific parameters.
///
/// Extensible structure for instrument-specific model parameters
/// (prepayment speeds, default rates, recovery rates, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelParamsAttribution {
    /// Prepayment speed changes (for MBS/ABS).
    pub prepayment: Option<Money>,

    /// Default rate changes (for structured credit).
    pub default_rate: Option<Money>,

    /// Recovery rate changes (for credit instruments).
    pub recovery_rate: Option<Money>,

    /// Conversion ratio changes (for convertible bonds).
    pub conversion_ratio: Option<Money>,

    /// Other model-specific parameters.
    #[serde(default)]
    pub other: IndexMap<String, Money>,
}

/// One source-line of carry, optionally split into rates / credit components.
///
/// Used for `CarryDetail.coupon_income` and `CarryDetail.roll_down`. When no
/// `CreditFactorModel` is supplied to attribution, `rates_part` and
/// `credit_part` are both `None` and `total` carries the scalar value (legacy
/// behavior). When a model is supplied, the two parts sum to `total` at
/// 1e-8 absolute tolerance (PR-8b §7.1, §7.4 invariants 1 & 2).
///
/// # JSON backward-compat
///
/// Pre-PR-8b JSON encoded `coupon_income` / `roll_down` as a bare `Money`
/// object (`{"amount": ..., "currency": ...}`). The custom `Deserialize`
/// implementation below accepts either:
///   - a legacy `Money` shape → `SourceLine { total: <money>, rates_part: None, credit_part: None }`
///   - the full `SourceLine` shape (`total` + optional `rates_part` / `credit_part`)
///
/// New JSON serialization always uses the `SourceLine` shape.
#[derive(Debug, Clone, Serialize, PartialEq, schemars::JsonSchema)]
pub struct SourceLine {
    /// Total signed amount for this line (always populated).
    pub total: Money,
    /// Rates-only contribution (populated only when a credit factor model
    /// drove the split).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rates_part: Option<Money>,
    /// Credit-only contribution (populated only when a credit factor model
    /// drove the split).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_part: Option<Money>,
}

impl SourceLine {
    /// Build a scalar (no-model) source line: only `total` is populated.
    pub fn scalar(total: Money) -> Self {
        Self {
            total,
            rates_part: None,
            credit_part: None,
        }
    }

    /// Build a split source line. The caller is responsible for ensuring
    /// `rates_part + credit_part == total` to within 1e-8.
    pub fn split(total: Money, rates_part: Money, credit_part: Money) -> Self {
        Self {
            total,
            rates_part: Some(rates_part),
            credit_part: Some(credit_part),
        }
    }
}

// Custom Deserialize that accepts either the legacy `Money` shape (object
// with `amount` and `currency`) or the new `SourceLine` shape (object with
// `total` and optional `rates_part` / `credit_part`). Disambiguation is by
// presence of the `total` key — `Money` has no `total` field.
impl<'de> Deserialize<'de> for SourceLine {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Read into a JSON Value first so we can route on shape. This is
        // fine for an `Option<SourceLine>` field: serde only invokes us when
        // a value is present.
        let v = serde_json::Value::deserialize(deserializer)?;
        if let Some(obj) = v.as_object() {
            if obj.contains_key("total") {
                // New shape.
                #[derive(Deserialize)]
                struct Helper {
                    total: Money,
                    #[serde(default)]
                    rates_part: Option<Money>,
                    #[serde(default)]
                    credit_part: Option<Money>,
                }
                let h: Helper = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                return Ok(SourceLine {
                    total: h.total,
                    rates_part: h.rates_part,
                    credit_part: h.credit_part,
                });
            }
            if obj.contains_key("amount") && obj.contains_key("currency") {
                // Legacy bare-Money shape.
                let m: Money = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                return Ok(SourceLine::scalar(m));
            }
        }
        Err(serde::de::Error::custom(
            "expected SourceLine ({total, rates_part?, credit_part?}) or legacy Money ({amount, currency})",
        ))
    }
}

/// Detailed carry decomposition.
///
/// When available, breaks carry into sub-components:
/// - **coupon_income**: Net cashflows (coupons, interest) received during the period
/// - **pull_to_par**: PV convergence toward par (time effect at flat yield)
/// - **roll_down**: Curve shape benefit from aging along a sloped curve
/// - **funding_cost**: Cost of financing the position
/// - **theta**: Total pre-funding carry (before decomposition into sub-components)
///
/// In metrics-based attribution, these fields are populated from pre-computed
/// carry decomposition metrics when available. In repricing-based attribution
/// methods, only a partial breakdown may be available.
///
/// `coupon_income` and `roll_down` are typed as [`SourceLine`] so that
/// callers with a `CreditFactorModel` may further split them into rates and
/// credit components (PR-8b §7.1).
///
/// # Reference
///
/// Bloomberg PORT decomposes carry into Carry (coupon/funding), Curve Roll-Down,
/// and Shift as distinct P&L components.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CarryDetail {
    /// Total carry P&L (sum of all components).
    pub total: Money,

    /// Coupon/interest income received during the period (with optional
    /// rates / credit split).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupon_income: Option<SourceLine>,

    /// PV convergence toward par (time effect at flat yield). Unsplit (v1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_to_par: Option<Money>,

    /// Curve shape benefit from aging along a sloped curve, with optional
    /// rates / credit split.
    ///
    /// This field includes slide/rolldown effects separate from pure pull-to-par.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll_down: Option<SourceLine>,

    /// Cost of financing the position. Pure rates, never split.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_cost: Option<Money>,

    /// Total pre-funding carry (before decomposition into sub-components).
    /// Residual catch-all, unsplit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<Money>,
}

/// Factor-cut decomposition of carry under a calibrated `CreditFactorModel`
/// (PR-8b §7.2). Populated only when an `AttributionSpec.credit_factor_model`
/// was supplied. Purely additive — does not modify any existing field.
///
/// # Reconciliation invariants (§7.4, all at 1e-8 absolute tolerance)
///
/// - `credit_carry_total ≡ Σ_lines SourceLine.credit_part` (lines = coupon + roll)
/// - `credit_carry_total ≡ generic + Σ_levels(level.total) + adder_total`
/// - `rates_carry_total ≡ Σ_lines SourceLine.rates_part − funding_cost`
///
/// # Attribution method coverage
///
/// Note: Parallel and Waterfall attribution methods do not currently populate
/// `carry_detail` (only `coupon_income` and `theta` from `apply_total_return_carry`
/// for some paths). Therefore `credit_carry_decomposition` is typically emitted
/// only on the MetricsBased / Taylor path. Future PRs may extend Parallel/Waterfall
/// to populate carry — the decomposition logic is method-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCarryDecomposition {
    /// Deterministic traceability id of the model used (matches
    /// `CreditFactorAttribution.model_id`).
    pub model_id: String,
    /// Sum of rates components across split source lines, minus funding cost.
    pub rates_carry_total: Money,
    /// Sum of credit components across split source lines.
    pub credit_carry_total: Money,
    /// Per-factor breakdown of `credit_carry_total`.
    pub credit_by_level: CreditCarryByLevel,
}

/// Per-factor breakdown of credit carry (PR-8b §7.2).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditCarryByLevel {
    /// Generic (PC) factor contribution to credit carry.
    pub generic: Money,
    /// One entry per hierarchy level, in spec order.
    pub levels: Vec<LevelCarry>,
    /// Sum of issuer-specific adder carry contributions.
    pub adder_total: Money,
    /// Optional per-issuer adder breakdown (gated by
    /// `CreditFactorDetailOptions.include_per_issuer_adder`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<BTreeMap<String, Money>>")]
    pub adder_by_issuer: Option<BTreeMap<IssuerId, Money>>,
}

/// Carry contribution from a single hierarchy level.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LevelCarry {
    /// Human-readable level name (e.g. `"rating"`, `"region"`).
    pub level_name: String,
    /// Aggregate carry for this level across all buckets.
    pub total: Money,
    /// Optional per-bucket breakdown keyed by dotted bucket path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub by_bucket: BTreeMap<String, Money>,
}

/// Detailed attribution for market scalars.
///
/// Includes dividends, equity/commodity prices, inflation indices, etc.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScalarsAttribution {
    /// Dividend changes by equity ID.
    #[serde(default)]
    pub dividends: IndexMap<CurveId, Money>,

    /// Inflation index changes.
    #[serde(default)]
    pub inflation: IndexMap<CurveId, Money>,

    /// Equity price changes.
    #[serde(default)]
    pub equity_prices: IndexMap<CurveId, Money>,

    /// Commodity price changes.
    #[serde(default)]
    pub commodity_prices: IndexMap<CurveId, Money>,
}

/// Attribution metadata.
///
/// Records methodology, dates, repricing count, and residual statistics.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AttributionMeta {
    /// Attribution method used.
    pub method: AttributionMethod,

    /// Start date (T₀).
    #[schemars(with = "String")]
    pub t0: Date,

    /// End date (T₁).
    #[schemars(with = "String")]
    pub t1: Date,

    /// Instrument identifier.
    pub instrument_id: String,

    /// Number of repricings performed.
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

    /// Diagnostic notes and warnings.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl PnlAttribution {
    /// Create a new P&L attribution with required fields.
    ///
    /// # Arguments
    ///
    /// * `total_pnl` - Total P&L (val_t1 - val_t0)
    /// * `instrument_id` - Instrument identifier
    /// * `t0` - Start date
    /// * `t1` - End date
    /// * `method` - Attribution methodology
    ///
    /// # Returns
    ///
    /// New `PnlAttribution` with all factor P&Ls initialized to zero.
    pub fn new(
        total_pnl: Money,
        instrument_id: impl Into<String>,
        t0: Date,
        t1: Date,
        method: AttributionMethod,
    ) -> Self {
        let zero = Money::new(0.0, total_pnl.currency());

        Self {
            total_pnl,
            // Audit rec #2: the raw mark-to-market is whatever the caller
            // passed as `total_pnl` *before* any total-return adjustment.
            // Stored as Some() at construction; downstream apply_total_return_carry
            // leaves this untouched even as it mutates total_pnl.
            mark_to_market_pnl: Some(total_pnl),
            carry: zero,
            rates_curves_pnl: zero,
            credit_curves_pnl: zero,
            inflation_curves_pnl: zero,
            correlations_pnl: zero,
            fx_pnl: zero,
            vol_pnl: zero,
            cross_factor_pnl: zero,
            model_params_pnl: zero,
            market_scalars_pnl: zero,
            residual: total_pnl, // Initially all P&L is residual
            carry_detail: None,
            rates_detail: None,
            credit_detail: None,
            inflation_detail: None,
            correlations_detail: None,
            fx_detail: None,
            vol_detail: None,
            cross_factor_detail: None,
            model_params_detail: None,
            scalars_detail: None,
            credit_factor_detail: None,
            credit_carry_decomposition: None,
            result_invalid: false,
            meta: AttributionMeta {
                method,
                t0,
                t1,
                instrument_id: instrument_id.into(),
                num_repricings: 0,
                tolerance_abs: 1.0,
                tolerance_pct: 0.01,
                residual_pct: 100.0,
                rounding: RoundingContext::default(),
                fx_policy: None,
                notes: Vec::new(),
            },
        }
    }

    /// Create a new P&L attribution with explicit rounding context.
    ///
    /// # Arguments
    ///
    /// * `total_pnl` - Total P&L (val_t1 - val_t0)
    /// * `instrument_id` - Instrument identifier
    /// * `t0` - Start date
    /// * `t1` - End date
    /// * `method` - Attribution methodology
    /// * `rounding` - Rounding context to stamp
    ///
    /// # Returns
    ///
    /// New `PnlAttribution` with all factor P&Ls initialized to zero.
    pub fn new_with_rounding(
        total_pnl: Money,
        instrument_id: impl Into<String>,
        t0: Date,
        t1: Date,
        method: AttributionMethod,
        rounding: RoundingContext,
    ) -> Self {
        let mut attr = Self::new(total_pnl, instrument_id, t0, t1, method);
        attr.meta.rounding = rounding;
        attr
    }

    /// Scale all attribution values by a factor.
    ///
    /// Useful for scaling per-unit attribution to position quantity.
    pub fn scale(&mut self, factor: f64) {
        self.total_pnl *= factor;
        if let Some(m) = self.mark_to_market_pnl.as_mut() {
            *m *= factor;
        }
        self.carry *= factor;
        self.rates_curves_pnl *= factor;
        self.credit_curves_pnl *= factor;
        self.inflation_curves_pnl *= factor;
        self.correlations_pnl *= factor;
        self.fx_pnl *= factor;
        self.vol_pnl *= factor;
        self.cross_factor_pnl *= factor;
        self.model_params_pnl *= factor;
        self.market_scalars_pnl *= factor;
        self.residual *= factor;

        if let Some(d) = &mut self.carry_detail {
            d.total *= factor;
            scale_source_line_opt(&mut d.coupon_income, factor);
            scale_money_opt(&mut d.pull_to_par, factor);
            scale_money_opt(&mut d.theta, factor);
            scale_source_line_opt(&mut d.roll_down, factor);
            scale_money_opt(&mut d.funding_cost, factor);
        }
        if let Some(d) = &mut self.rates_detail {
            scale_money_map(&mut d.by_curve, factor);
            scale_money_map(&mut d.by_tenor, factor);
            d.discount_total *= factor;
            d.forward_total *= factor;
        }
        if let Some(d) = &mut self.credit_detail {
            scale_money_map(&mut d.by_curve, factor);
            scale_money_map(&mut d.by_tenor, factor);
        }
        if let Some(d) = &mut self.inflation_detail {
            scale_money_map(&mut d.by_curve, factor);
            if let Some(bt) = &mut d.by_tenor {
                scale_money_map(bt, factor);
            }
        }
        if let Some(d) = &mut self.correlations_detail {
            scale_money_map(&mut d.by_curve, factor);
        }
        if let Some(d) = &mut self.fx_detail {
            scale_money_map(&mut d.by_pair, factor);
        }
        if let Some(d) = &mut self.vol_detail {
            scale_money_map(&mut d.by_surface, factor);
        }
        if let Some(d) = &mut self.cross_factor_detail {
            d.total *= factor;
            scale_money_map(&mut d.by_pair, factor);
        }
        if let Some(d) = &mut self.model_params_detail {
            scale_money_opt(&mut d.prepayment, factor);
            scale_money_opt(&mut d.default_rate, factor);
            scale_money_opt(&mut d.recovery_rate, factor);
            scale_money_opt(&mut d.conversion_ratio, factor);
            scale_money_map(&mut d.other, factor);
        }
        if let Some(d) = &mut self.scalars_detail {
            scale_money_map(&mut d.dividends, factor);
            scale_money_map(&mut d.inflation, factor);
            scale_money_map(&mut d.equity_prices, factor);
            scale_money_map(&mut d.commodity_prices, factor);
        }
        if let Some(d) = &mut self.credit_factor_detail {
            d.generic_pnl *= factor;
            d.adder_pnl_total *= factor;
            for level in &mut d.levels {
                level.total *= factor;
                for v in level.by_bucket.values_mut() {
                    *v *= factor;
                }
            }
            if let Some(by_issuer) = &mut d.adder_pnl_by_issuer {
                for v in by_issuer.values_mut() {
                    *v *= factor;
                }
            }
        }
        if let Some(d) = &mut self.credit_carry_decomposition {
            d.rates_carry_total *= factor;
            d.credit_carry_total *= factor;
            d.credit_by_level.generic *= factor;
            d.credit_by_level.adder_total *= factor;
            for level in &mut d.credit_by_level.levels {
                level.total *= factor;
                for v in level.by_bucket.values_mut() {
                    *v *= factor;
                }
            }
            if let Some(by_issuer) = &mut d.credit_by_level.adder_by_issuer {
                for v in by_issuer.values_mut() {
                    *v *= factor;
                }
            }
        }
    }

    /// Validate that all factor currencies match total_pnl currency.
    ///
    /// # Returns
    ///
    /// Ok(()) if all currencies match, Err otherwise.
    pub fn validate_currencies(&self) -> Result<()> {
        let expected = self.total_pnl.currency();

        let factors = [
            ("carry", self.carry.currency()),
            ("rates_curves", self.rates_curves_pnl.currency()),
            ("credit_curves", self.credit_curves_pnl.currency()),
            ("inflation_curves", self.inflation_curves_pnl.currency()),
            ("correlations", self.correlations_pnl.currency()),
            ("fx", self.fx_pnl.currency()),
            ("vol", self.vol_pnl.currency()),
            ("cross_factor", self.cross_factor_pnl.currency()),
            ("model_params", self.model_params_pnl.currency()),
            ("market_scalars", self.market_scalars_pnl.currency()),
        ];

        for (name, ccy) in &factors {
            if *ccy != expected {
                return Err(Error::Validation(format!(
                    "Currency mismatch in '{}' factor: expected {}, got {}",
                    name, expected, ccy
                )));
            }
        }

        Ok(())
    }

    /// Compute residual as total_pnl minus sum of all attributed factors.
    ///
    /// Updates both `residual` and `residual_pct` fields.
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Err if currency mismatch detected.
    ///
    /// # Notes
    ///
    /// On error, sets residual to zero and adds a diagnostic note to metadata.
    pub fn compute_residual(&mut self) -> Result<()> {
        // Validate currencies first
        if let Err(e) = self.validate_currencies() {
            let note = format!(
                "Currency validation failed during residual computation: {}",
                e
            );
            self.meta.notes.push(note);
            self.residual = Money::new(0.0, self.total_pnl.currency());
            self.meta.residual_pct = 0.0;
            // Audit rec #3: surface the failure so `residual_within_tolerance`
            // does NOT report a "clean" 0 residual after a validation error.
            self.result_invalid = true;
            return Err(e);
        }

        // Sum all attributed factors (safe now that currencies are validated)
        let mut attributed_sum = self.carry;
        attributed_sum = add_factor(
            attributed_sum,
            self.rates_curves_pnl,
            "rates curves P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.credit_curves_pnl,
            "credit curves P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.inflation_curves_pnl,
            "inflation curves P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.correlations_pnl,
            "correlations P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(attributed_sum, self.fx_pnl, "FX P&L", &mut self.meta.notes)?;
        attributed_sum = add_factor(
            attributed_sum,
            self.vol_pnl,
            "vol P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.cross_factor_pnl,
            "cross-factor P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.model_params_pnl,
            "model params P&L",
            &mut self.meta.notes,
        )?;
        attributed_sum = add_factor(
            attributed_sum,
            self.market_scalars_pnl,
            "market scalars P&L",
            &mut self.meta.notes,
        )?;

        self.residual = match self.total_pnl.checked_sub(attributed_sum) {
            Ok(r) => r,
            Err(e) => {
                let note = format!("Failed to compute residual: {}", e);
                self.meta.notes.push(note);
                self.residual = Money::new(0.0, self.total_pnl.currency());
                self.meta.residual_pct = 0.0;
                // Audit rec #3: as above — flag invalid so tolerance checks fail.
                self.result_invalid = true;
                return Err(e);
            }
        };

        // Compute residual percentage (handle zero total_pnl) via RoundingContext
        let rc = &self.meta.rounding;
        self.meta.residual_pct =
            if !rc.is_effectively_zero_money(self.total_pnl.amount(), self.total_pnl.currency()) {
                (self.residual.amount() / self.total_pnl.amount()) * 100.0
            } else {
                0.0
            };

        Ok(())
    }

    /// Check if residual is within tolerance.
    ///
    /// Tolerance is either percentage-based (relative to total P&L) or
    /// absolute, whichever is larger.
    ///
    /// # Arguments
    ///
    /// * `pct_tolerance` - Percentage tolerance (e.g., 0.1 for 0.1%)
    /// * `abs_tolerance` - Absolute tolerance (e.g., 100.0 for $100)
    ///
    /// # Returns
    ///
    /// `true` if residual is within tolerance.
    pub fn residual_within_tolerance(&self, pct_tolerance: f64, abs_tolerance: f64) -> bool {
        // Audit rec #3: if residual computation failed, `residual` was reset to 0
        // and a clean tolerance check would falsely succeed. Refuse to claim
        // "within tolerance" when the attribution is flagged invalid.
        if self.result_invalid {
            return false;
        }

        let abs_residual = self.residual.amount().abs();
        let abs_total = self.total_pnl.amount().abs();

        // Tolerance is the larger of percentage-based or absolute
        let tolerance = if abs_total > 1e-10 {
            (abs_total * pct_tolerance / 100.0).max(abs_tolerance)
        } else {
            abs_tolerance
        };

        abs_residual <= tolerance
    }

    /// Check if residual is within the stored tolerance thresholds.
    ///
    /// Uses the tolerance_abs and tolerance_pct from metadata.
    ///
    /// # Returns
    ///
    /// `true` if residual is within the stored tolerances.
    pub fn residual_within_meta_tolerance(&self) -> bool {
        self.residual_within_tolerance(self.meta.tolerance_pct, self.meta.tolerance_abs)
    }

    /// Generate a structured tree explanation of P&L attribution.
    ///
    /// Creates a human-readable tree showing the total P&L broken down by factor.
    /// Zero-valued factors are omitted for a clean presentation. Use
    /// [`Self::explain_verbose`] to include all factors regardless of value.
    ///
    /// # Returns
    ///
    /// Multi-line string with tree structure.
    ///
    /// # Examples
    ///
    /// ```text
    /// Total P&L: $125,430
    ///   ├─ Carry: $45,000 (35.8%)
    ///   ├─ Rates Curves: $65,000 (51.7%)
    ///   │   ├─ USD-OIS: $50,000
    ///   │   └─ EUR-OIS: $15,000
    ///   ├─ Credit Curves: $5,000 (4.0%)
    ///   ├─ FX: $12,000 (9.5%)
    ///   ├─ Vol: $2,000 (1.6%)
    ///   └─ Residual: -$1,570 (-1.2%)
    /// ```
    pub fn explain(&self) -> String {
        self.explain_impl(false)
    }

    /// Generate a verbose tree explanation showing all factors including zeros.
    ///
    /// Unlike [`Self::explain`], this method shows every attribution factor
    /// regardless of whether its value is zero. Useful for debugging and
    /// verifying that all factors are being computed.
    pub fn explain_verbose(&self) -> String {
        self.explain_impl(true)
    }

    fn explain_impl(&self, show_zeros: bool) -> String {
        let rc = &self.meta.rounding;

        let fmt = |amount: &Money, total: &Money| -> String {
            let pct = if !rc.is_effectively_zero_money(total.amount(), total.currency()) {
                (amount.amount() / total.amount()) * 100.0
            } else {
                0.0
            };
            format!("{} ({:.1}%)", amount, pct)
        };

        let show = |m: &Money| -> bool {
            show_zeros || !rc.is_effectively_zero_money(m.amount(), m.currency())
        };

        let mut lines = Vec::new();
        lines.push(format!("Total P&L: {}", self.total_pnl));

        if show(&self.carry) {
            lines.push(format!("  ├─ Carry: {}", fmt(&self.carry, &self.total_pnl)));
            if let Some(ref detail) = self.carry_detail {
                if let Some(ref coupon_income) = detail.coupon_income {
                    lines.push(format!("  │   ├─ Coupon Income: {}", coupon_income.total));
                }
                if let Some(ref pull_to_par) = detail.pull_to_par {
                    lines.push(format!("  │   ├─ Pull-to-Par: {}", pull_to_par));
                }
                if let Some(ref theta) = detail.theta {
                    lines.push(format!("  │   ├─ Theta: {}", theta));
                }
                if let Some(ref roll_down) = detail.roll_down {
                    lines.push(format!("  │   ├─ Roll-Down: {}", roll_down.total));
                }
                if let Some(ref funding_cost) = detail.funding_cost {
                    lines.push(format!("  │   └─ Funding Cost: {}", funding_cost));
                }
            }
        }

        if show(&self.rates_curves_pnl) {
            lines.push(format!(
                "  ├─ Rates Curves: {}",
                fmt(&self.rates_curves_pnl, &self.total_pnl)
            ));
            if let Some(ref detail) = self.rates_detail {
                for (curve_id, pnl) in &detail.by_curve {
                    lines.push(format!("  │   ├─ {}: {}", curve_id, pnl));
                }
            }
        }

        if show(&self.credit_curves_pnl) {
            lines.push(format!(
                "  ├─ Credit Curves: {}",
                fmt(&self.credit_curves_pnl, &self.total_pnl)
            ));
            if let Some(ref detail) = self.credit_detail {
                for (curve_id, pnl) in &detail.by_curve {
                    lines.push(format!("  │   ├─ {}: {}", curve_id, pnl));
                }
            }
        }

        if show(&self.inflation_curves_pnl) {
            lines.push(format!(
                "  ├─ Inflation Curves: {}",
                fmt(&self.inflation_curves_pnl, &self.total_pnl)
            ));
        }

        if show(&self.correlations_pnl) {
            lines.push(format!(
                "  ├─ Correlations: {}",
                fmt(&self.correlations_pnl, &self.total_pnl)
            ));
        }

        if show(&self.fx_pnl) {
            lines.push(format!("  ├─ FX: {}", fmt(&self.fx_pnl, &self.total_pnl)));
        }

        if show(&self.vol_pnl) {
            lines.push(format!("  ├─ Vol: {}", fmt(&self.vol_pnl, &self.total_pnl)));
        }

        if show(&self.cross_factor_pnl) {
            lines.push(format!(
                "  ├─ Cross-Factor: {}",
                fmt(&self.cross_factor_pnl, &self.total_pnl)
            ));
            if let Some(ref detail) = self.cross_factor_detail {
                for (pair_label, pnl) in &detail.by_pair {
                    lines.push(format!("  │   ├─ {}: {}", pair_label, pnl));
                }
            }
        }

        if show(&self.model_params_pnl) {
            lines.push(format!(
                "  ├─ Model Params: {}",
                fmt(&self.model_params_pnl, &self.total_pnl)
            ));
        }

        if show(&self.market_scalars_pnl) {
            lines.push(format!(
                "  ├─ Market Scalars: {}",
                fmt(&self.market_scalars_pnl, &self.total_pnl)
            ));
        }

        lines.push(format!(
            "  └─ Residual: {}",
            fmt(&self.residual, &self.total_pnl)
        ));

        lines.join("\n")
    }
}

impl AttributionMethod {
    /// Returns the risk metrics required for this attribution method.
    ///
    /// For `MetricsBased`, returns first-order sensitivities (Theta, DV01, CS01,
    /// Vega, Delta, FX01, Inflation01, Dividend01) plus second-order terms
    /// (Gamma, Convexity, IrConvexity, Volga, Vanna, CsGamma, InflationConvexity)
    /// needed by the metrics-based attribution algorithm.
    ///
    /// All other methods return an empty vec (they reprice directly rather than
    /// using pre-computed metrics).
    pub fn required_metrics(&self) -> Vec<crate::metrics::MetricId> {
        use crate::metrics::MetricId;
        match self {
            AttributionMethod::MetricsBased => vec![
                // First-order metrics
                MetricId::Theta,
                MetricId::Dv01,
                MetricId::Cs01,
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

impl std::fmt::Display for AttributionFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributionFactor::Carry => write!(f, "Carry"),
            AttributionFactor::RatesCurves => write!(f, "RatesCurves"),
            AttributionFactor::CreditCurves => write!(f, "CreditCurves"),
            AttributionFactor::InflationCurves => write!(f, "InflationCurves"),
            AttributionFactor::Correlations => write!(f, "Correlations"),
            AttributionFactor::Fx => write!(f, "Fx"),
            AttributionFactor::Volatility => write!(f, "Volatility"),
            AttributionFactor::ModelParameters => write!(f, "ModelParameters"),
            AttributionFactor::MarketScalars => write!(f, "MarketScalars"),
        }
    }
}

fn scale_money_map<K: std::hash::Hash + Eq>(map: &mut IndexMap<K, Money>, factor: f64) {
    for v in map.values_mut() {
        *v *= factor;
    }
}

fn scale_money_opt(opt: &mut Option<Money>, factor: f64) {
    if let Some(v) = opt {
        *v *= factor;
    }
}

fn scale_source_line_opt(opt: &mut Option<SourceLine>, factor: f64) {
    if let Some(v) = opt {
        v.total *= factor;
        scale_money_opt(&mut v.rates_part, factor);
        scale_money_opt(&mut v.credit_part, factor);
    }
}

fn add_factor(sum: Money, value: Money, label: &str, notes: &mut Vec<String>) -> Result<Money> {
    sum.checked_add(value).map_err(|e| {
        let note = format!("Failed to add {}: {}", label, e);
        notes.push(note.clone());
        e
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use time::macros::date;

    #[test]
    fn test_carry_detail_scale_and_explain_include_decomposition_fields() {
        let mut attribution = PnlAttribution::new(
            Money::new(10.0, Currency::USD),
            "BOND-1",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::MetricsBased,
        );
        attribution.carry = Money::new(10.0, Currency::USD);
        attribution.carry_detail = Some(CarryDetail {
            total: Money::new(10.0, Currency::USD),
            coupon_income: Some(SourceLine::scalar(Money::new(3.0, Currency::USD))),
            pull_to_par: Some(Money::new(4.0, Currency::USD)),
            roll_down: Some(SourceLine::scalar(Money::new(5.0, Currency::USD))),
            funding_cost: Some(Money::new(2.0, Currency::USD)),
            theta: Some(Money::new(12.0, Currency::USD)),
        });

        attribution.scale(0.5);

        let detail = attribution.carry_detail.clone().expect("carry detail");
        assert_eq!(detail.total.amount(), 5.0);
        assert_eq!(
            detail
                .coupon_income
                .as_ref()
                .expect("coupon income")
                .total
                .amount(),
            1.5
        );
        assert_eq!(
            detail.pull_to_par.as_ref().expect("pull to par").amount(),
            2.0
        );
        assert_eq!(
            detail.roll_down.as_ref().expect("roll down").total.amount(),
            2.5
        );
        assert_eq!(
            detail.funding_cost.as_ref().expect("funding cost").amount(),
            1.0
        );
        assert_eq!(detail.theta.as_ref().expect("theta").amount(), 6.0);

        attribution.carry_detail = Some(detail);
        let explanation = attribution.explain_verbose();
        assert!(explanation.contains("Coupon Income"));
        assert!(explanation.contains("Pull-to-Par"));
        assert!(explanation.contains("Roll-Down"));
        assert!(explanation.contains("Funding Cost"));
        assert!(explanation.contains("Theta"));
    }

    #[test]
    fn test_cross_factor_detail_serde_roundtrip() {
        let detail = CrossFactorDetail {
            total: Money::new(500.0, Currency::USD),
            by_pair: {
                let mut map = IndexMap::new();
                map.insert("Rates×Credit".to_string(), Money::new(300.0, Currency::USD));
                map.insert("Spot×Vol".to_string(), Money::new(200.0, Currency::USD));
                map
            },
        };

        let json = serde_json::to_string(&detail).unwrap();
        let parsed: CrossFactorDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total.amount(), 500.0);
        assert_eq!(parsed.by_pair.len(), 2);
    }

    #[test]
    fn test_compute_residual_includes_cross_factor() {
        let total = Money::new(1000.0, Currency::USD);
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::new(600.0, Currency::USD);
        attr.credit_curves_pnl = Money::new(200.0, Currency::USD);
        attr.cross_factor_pnl = Money::new(150.0, Currency::USD);

        attr.compute_residual().unwrap();

        assert!((attr.residual.amount() - 50.0).abs() < 1e-10);
    }

    /// Audit rec #2: `mark_to_market_pnl` must capture the raw `val_t1 − val_t0`
    /// supplied at construction time and stay frozen even if `total_pnl` is
    /// later mutated by `apply_total_return_carry`. This is what lets a
    /// consumer reconcile against their own computation of the price change.
    #[test]
    fn test_mark_to_market_pnl_captured_at_construction_and_preserved() {
        use crate::attribution::helpers::apply_total_return_carry;
        let raw_pnl = Money::new(1000.0, Currency::USD);
        let mut attr = PnlAttribution::new(
            raw_pnl,
            "TEST-MTM",
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );

        // Sanity: mark_to_market_pnl mirrors total_pnl at construction.
        assert_eq!(attr.mark_to_market_pnl.map(|m| m.amount()), Some(1000.0));

        // Simulate total-return adjustment (coupon income paid in period).
        let coupon = Money::new(50.0, Currency::USD);
        let theta = Money::new(30.0, Currency::USD);
        apply_total_return_carry(&mut attr, theta, coupon).expect("carry add");

        // total_pnl is now total-return (1000 + 50 = 1050); mark-to-market
        // must still report the raw pre-coupon price change (1000).
        assert_eq!(attr.total_pnl.amount(), 1050.0);
        assert_eq!(
            attr.mark_to_market_pnl.map(|m| m.amount()),
            Some(1000.0),
            "mark_to_market_pnl must not absorb coupon adjustment"
        );
    }

    /// Audit rec #3: when compute_residual fails because a factor is in a
    /// different currency from total_pnl, the attribution must report itself
    /// as invalid, and `residual_within_tolerance` must refuse to claim
    /// success even though `residual == 0` was set by the failure path.
    #[test]
    fn test_failed_residual_marks_invalid_and_blocks_tolerance_check() {
        let total = Money::new(1000.0, Currency::USD);
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );
        // Deliberately mismatched currency so validate_currencies errors out.
        attr.rates_curves_pnl = Money::new(600.0, Currency::EUR);

        let err = attr.compute_residual();
        assert!(err.is_err(), "currency mismatch must error");
        assert!(attr.result_invalid, "result_invalid must be set on failure");
        // residual was reset to zero — without the result_invalid guard the
        // tolerance check below would falsely succeed.
        assert_eq!(attr.residual.amount(), 0.0);
        assert!(
            !attr.residual_within_tolerance(0.1, 1.0),
            "tolerance check MUST fail when result_invalid is set, regardless of residual value"
        );
        assert!(
            !attr.residual_within_meta_tolerance(),
            "meta-tolerance check MUST also fail when result_invalid is set"
        );
    }
}
