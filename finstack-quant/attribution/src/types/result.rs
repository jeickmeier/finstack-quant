//! Generic / top-level attribution types.
//!
//! This module holds the core attribution enums, the top-level
//! `PnlAttribution` result struct, `AttributionMeta`, helper functions,
//! and unit tests.

use finstack_quant_core::config::RoundingContext;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::{fx::FxPolicyMeta, Money};
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::detail::*;
use crate::taylor::TaylorAttributionConfig;

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

/// Complete P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent factors with optional detailed
/// breakdowns by curve, tenor, FX pair, etc.
///
/// # Examples
///
/// ```no_run
/// use finstack_quant_attribution::{
///     attribute_pnl, AttributionMethod, AttributionRequest, ExecutionPolicy,
/// };
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::money::Money;
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
///         .notional(Money::from((1_000_000_i64, Currency::USD)))
///         .start_date(as_of_t0)
///         .maturity(as_of_t1)
///         .day_count(finstack_quant_core::dates::DayCount::Act360)
///         .discount_curve_id("USD-OIS".into())
///         .build()
///         .expect("deposit builder should succeed"),
/// ) as Arc<dyn Instrument>;
///
/// let request = AttributionRequest {
///     execution_policy: ExecutionPolicy::Parallel,
///     ..AttributionRequest::new(
///         &instrument,
///         &market_t0,
///         &market_t1,
///         as_of_t0,
///         as_of_t1,
///         &config,
///     )
/// };
/// let attribution = attribute_pnl(&AttributionMethod::Parallel, &request)?;
///
/// println!("Total P&L: {}", attribution.total_pnl);
/// println!("Carry: {} ({:.1}%)",
///     attribution.carry,
///     attribution.carry.amount() / attribution.total_pnl.amount() * 100.0
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PnlAttribution {
    /// Total P&L *as reported by this attribution*.
    ///
    /// In the standard total-return convention (the default carry path uses
    /// the internal total-return carry helper), this is
    /// `(val_t1 − val_t0) + economic_cash_between(T0, T1)`. Cashflows received
    /// during the period are added back so that
    /// `total_pnl == carry + factor_sum + residual` holds: the `carry` field
    /// includes income and principal receipts, and reconciliation against the user's
    /// observed val_t1 − val_t0 must subtract the economic cash out of
    /// `total_pnl` to recover the pure mark-to-market move.
    ///
    /// **For a raw mark-to-market view that excludes intra-period cashflows,
    /// read [`Self::mark_to_market_pnl`].** That field, when present, is the
    /// untouched `val_t1 − val_t0` and never absorbs cash receipts.
    pub total_pnl: Money,

    /// Pure mark-to-market change: `val_t1 − val_t0` with **no** intra-period
    /// cashflow adjustment.
    ///
    /// Separates the raw user-input price change from
    /// the total-return view stamped on `total_pnl`. When the attribution path
    /// added period economic cash to `total_pnl` (the standard total-return convention
    /// in parallel / waterfall / taylor attribution), this field still reports
    /// the raw `val_t1 − val_t0` so a downstream consumer that computed their
    /// own total from the underlying valuations can reconcile cleanly.
    ///
    /// Attribution paths that cannot provide the raw mark-to-market change use
    /// `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_to_market_pnl: Option<Money>,

    /// Present value at T₀ on the T₀ market: the opening endpoint of the
    /// attribution, in the `total_pnl` currency.
    ///
    /// `mark_to_market_pnl == pv_t1 − pv_t0`, and `total_pnl` adds the period
    /// cash receipts the method documents. After a target-currency
    /// translation this is the opening value converted at T₀ FX.
    ///
    /// Absent when the result was not produced from two endpoint valuations in
    /// the `total_pnl` currency: a bare [`Self::new`] result, a payload
    /// written before this field existed, or an opening value quoted in a
    /// different currency from the closing value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pv_t0: Option<Money>,

    /// Present value at T₁ on the T₁ market: the closing endpoint of the
    /// attribution, in the `total_pnl` currency.
    ///
    /// After a target-currency translation this is the closing value
    /// converted at T₁ FX. Absent under the same conditions as
    /// [`Self::pv_t0`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pv_t1: Option<Money>,

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

    /// FX rate changes P&L — the **pricing-impact** component of FX moves on
    /// cross-currency instruments (the FX matrix feeding into the instrument's
    /// own pricer). For pure single-currency instruments this is zero.
    pub fx_pnl: Money,

    /// FX translation P&L — the **reporting-currency** component of FX moves.
    ///
    /// Populated only when the attribution call passed an explicit
    /// `target_currency` that differs from the instrument's native pricing currency
    /// (`val_t1.currency()`). In that case the native-currency P&L is
    /// translated into `target_currency` using `market_t0`'s FX at T₀ and
    /// `market_t1`'s FX at T₁; the difference between those two translated
    /// totals lands here.
    ///
    /// For attribution calls that report in native currency (the default —
    /// `target_currency = None`), this is always zero in the `total_pnl` currency.
    ///
    /// The field is always constructed in the same currency as `total_pnl`.
    pub fx_translation_pnl: Money,

    /// Implied volatility changes P&L.
    pub vol_pnl: Money,

    /// Cross-factor interaction P&L (rates×credit, spot×vol, FX×rates, etc.).
    ///
    /// Stored as the **additive contribution to the attributed sum**: for the
    /// parallel method this is the negated mixed second difference
    /// `V(a@T₀) + V(b@T₀) − V(all-T₁) − V(ab@T₀)` summed over factor pairs,
    /// so that extracting cross terms drives the residual toward zero; for
    /// the metrics-based method it is the Taylor cross-gamma term, which is
    /// additive by construction. A positive value means factor co-movement
    /// added P&L beyond the sum of the isolated factor effects.
    pub cross_factor_pnl: Money,

    /// Model parameters P&L.
    pub model_params_pnl: Money,

    /// Market scalars P&L.
    pub market_scalars_pnl: Money,

    /// Residual P&L (total - sum of attributed factors).
    pub residual: Money,

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
    /// `credit_curves_pnl` itself is unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_factor_detail: Option<CreditFactorAttribution>,

    /// Optional credit-factor-hierarchy decomposition of carry.
    ///
    /// Populated only when an `AttributionSpec.credit_factor_model` was
    /// supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_carry_decomposition: Option<CreditCarryDecomposition>,

    /// Ordered repricing steps of a waterfall attribution: the running
    /// present value before and after each factor, in the order applied.
    ///
    /// The step P&Ls chain from [`Self::pv_t0`] to the fully rolled value and
    /// sum to `mark_to_market_pnl − residual`. After a target-currency
    /// translation every step value is converted at T₁ FX, so the first
    /// `pv_before` equals `pv_t0 + fx_translation_pnl`.
    ///
    /// Empty for every other method, for a composite aggregate, and for
    /// payloads written before this field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waterfall_steps: Vec<WaterfallStep>,

    /// The working behind each factor of a sensitivity-based attribution, in
    /// the order computed: sensitivity, observed market move, first-order
    /// P&L and second-order P&L (or the repriced value for factors isolated
    /// by a reprice).
    ///
    /// Populated by the Taylor and metrics-based methods. Each factor bucket
    /// equals the sum of `explained_pnl + gamma_pnl` over its rows, with two
    /// metrics-based exceptions that have no rows: `carry` is read directly
    /// from the T₀ carry metrics (see `carry_detail`), and `cross_factor_pnl`
    /// is itemized by pair in `cross_factor_detail`. Empty for
    /// the parallel and waterfall methods, for a composite aggregate, and for
    /// payloads written before this field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivity_steps: Vec<SensitivityStep>,

    /// Attribution metadata.
    pub meta: AttributionMeta,

    /// True if residual computation encountered non-finite (NaN/Inf) inputs,
    /// or if any factor P&L value is non-finite. When `true`, `residual` and
    /// `meta.residual_pct` are not meaningful and downstream callers should
    /// treat the attribution as invalid.
    pub result_invalid: bool,
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
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub t0: Date,

    /// End date (T₁).
    #[serde(with = "finstack_quant_core::wire::date")]
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

// Share the signed-factor walk between immutable currency validation and
// mutable scaling/FX conversion. Headline amounts and absolute diagnostics
// have different transformation rules and are handled by their callers.
macro_rules! visit_factor_money {
    ($attribution:ident, $visit:ident, $values:ident $(, $mutability:tt)?) => {{
        for (name, money) in [
            ("carry", & $($mutability)? $attribution.carry),
            ("rates_curves", & $($mutability)? $attribution.rates_curves_pnl),
            ("credit_curves", & $($mutability)? $attribution.credit_curves_pnl),
            ("inflation_curves", & $($mutability)? $attribution.inflation_curves_pnl),
            ("correlations", & $($mutability)? $attribution.correlations_pnl),
            ("fx", & $($mutability)? $attribution.fx_pnl),
            ("vol", & $($mutability)? $attribution.vol_pnl),
            ("cross_factor", & $($mutability)? $attribution.cross_factor_pnl),
            ("model_params", & $($mutability)? $attribution.model_params_pnl),
            ("market_scalars", & $($mutability)? $attribution.market_scalars_pnl),
        ] {
            $visit(name, money)?;
        }
        if let Some(detail) = & $($mutability)? $attribution.carry_detail {
            $visit("carry_detail.total", & $($mutability)? detail.total)?;
            for (name, line) in [
                ("carry_detail.coupon_income", & $($mutability)? detail.coupon_income),
                ("carry_detail.roll_down", & $($mutability)? detail.roll_down),
            ] {
                if let Some(line) = line {
                    $visit(name, & $($mutability)? line.total)?;
                    for money in [
                        & $($mutability)? line.rates_part,
                        & $($mutability)? line.credit_part,
                    ].into_iter().flatten() {
                        $visit(name, money)?;
                    }
                }
            }
            for (name, money) in [
                ("carry_detail.pull_to_par", & $($mutability)? detail.pull_to_par),
                ("carry_detail.funding_cost", & $($mutability)? detail.funding_cost),
            ] {
                if let Some(money) = money {
                    $visit(name, money)?;
                }
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.rates_detail {
            for money in detail.by_curve.$values().chain(detail.by_tenor.$values()) {
                $visit("rates_detail", money)?;
            }
            $visit("rates_detail.discount_total", & $($mutability)? detail.discount_total)?;
            $visit("rates_detail.forward_total", & $($mutability)? detail.forward_total)?;
        }
        if let Some(detail) = & $($mutability)? $attribution.credit_detail {
            for money in detail.by_curve.$values().chain(detail.by_tenor.$values()) {
                $visit("credit_detail", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.inflation_detail {
            for money in detail.by_curve.$values() {
                $visit("inflation_detail", money)?;
            }
            if let Some(by_tenor) = & $($mutability)? detail.by_tenor {
                for money in by_tenor.$values() {
                    $visit("inflation_detail.by_tenor", money)?;
                }
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.correlations_detail {
            for money in detail.by_curve.$values() {
                $visit("correlations_detail", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.fx_detail {
            for money in detail.by_pair.$values() {
                $visit("fx_detail", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.vol_detail {
            for money in detail.by_surface.$values() {
                $visit("vol_detail", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.cross_factor_detail {
            $visit("cross_factor_detail.total", & $($mutability)? detail.total)?;
            for money in detail.by_pair.$values() {
                $visit("cross_factor_detail.by_pair", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.model_params_detail {
            for money in [
                & $($mutability)? detail.prepayment,
                & $($mutability)? detail.default_rate,
                & $($mutability)? detail.recovery_rate,
                & $($mutability)? detail.conversion_ratio,
            ].into_iter().flatten() {
                $visit("model_params_detail", money)?;
            }
            for money in detail.other.$values() {
                $visit("model_params_detail.other", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.scalars_detail {
            for money in detail.dividends.$values()
                .chain(detail.inflation.$values())
                .chain(detail.equity_prices.$values())
                .chain(detail.commodity_prices.$values()) {
                $visit("scalars_detail", money)?;
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.credit_factor_detail {
            $visit("credit_factor_detail.generic_pnl", & $($mutability)? detail.generic_pnl)?;
            $visit("credit_factor_detail.adder_pnl_total", & $($mutability)? detail.adder_pnl_total)?;
            $visit("credit_factor_detail.curve_shape_pnl", & $($mutability)? detail.curve_shape_pnl)?;
            for level in & $($mutability)? detail.levels {
                $visit("credit_factor_detail.levels", & $($mutability)? level.total)?;
                for money in level.by_bucket.$values() {
                    $visit("credit_factor_detail.levels.by_bucket", money)?;
                }
            }
            if let Some(by_issuer) = & $($mutability)? detail.adder_pnl_by_issuer {
                for money in by_issuer.$values() {
                    $visit("credit_factor_detail.adder_pnl_by_issuer", money)?;
                }
            }
        }
        if let Some(detail) = & $($mutability)? $attribution.credit_carry_decomposition {
            $visit("credit_carry_decomposition.rates_carry_total", & $($mutability)? detail.rates_carry_total)?;
            $visit("credit_carry_decomposition.credit_carry_total", & $($mutability)? detail.credit_carry_total)?;
            $visit("credit_carry_decomposition.generic", & $($mutability)? detail.credit_by_level.generic)?;
            $visit("credit_carry_decomposition.adder_total", & $($mutability)? detail.credit_by_level.adder_total)?;
            for level in & $($mutability)? detail.credit_by_level.levels {
                $visit("credit_carry_decomposition.levels", & $($mutability)? level.total)?;
                for money in level.by_bucket.$values() {
                    $visit("credit_carry_decomposition.levels.by_bucket", money)?;
                }
            }
            if let Some(by_issuer) = & $($mutability)? detail.credit_by_level.adder_by_issuer {
                for money in by_issuer.$values() {
                    $visit("credit_carry_decomposition.adder_by_issuer", money)?;
                }
            }
        }
        for step in & $($mutability)? $attribution.waterfall_steps {
            $visit("waterfall_steps.pv_before", & $($mutability)? step.pv_before)?;
            $visit("waterfall_steps.pv_after", & $($mutability)? step.pv_after)?;
            $visit("waterfall_steps.step_pnl", & $($mutability)? step.step_pnl)?;
        }
        for step in & $($mutability)? $attribution.sensitivity_steps {
            $visit("sensitivity_steps.explained_pnl", & $($mutability)? step.explained_pnl)?;
            for money in [
                & $($mutability)? step.sensitivity,
                & $($mutability)? step.repriced_pv,
                & $($mutability)? step.gamma_pnl,
            ].into_iter().flatten() {
                $visit("sensitivity_steps", money)?;
            }
            for bucket in & $($mutability)? step.buckets {
                $visit("sensitivity_steps.buckets", & $($mutability)? bucket.sensitivity)?;
            }
        }
        Ok::<(), Error>(())
    }};
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
        let zero = Money::from((0_i64, total_pnl.currency()));

        Self {
            total_pnl,
            // Raw mark-to-market is the caller-supplied `total_pnl` before
            // any total-return adjustment. `apply_total_return_carry` leaves
            // this field untouched when it mutates `total_pnl`.
            mark_to_market_pnl: Some(total_pnl),
            pv_t0: None,
            pv_t1: None,
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
            // `fx_translation_pnl` is constructed in the same currency as
            // `total_pnl`; it stays zero unless a `target_currency` translation is
            // explicitly applied by the per-method attribution code.
            fx_translation_pnl: zero,
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
            waterfall_steps: Vec::new(),
            sensitivity_steps: Vec::new(),
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
                execution_policy: None,
                notes: Vec::new(),
            },
        }
    }

    /// Apply `f` to every signed `Money` leaf: the per-factor aggregates
    /// (`carry` … `market_scalars_pnl`) and every leaf of every populated
    /// detail struct.
    ///
    /// The walk also covers the monetary fields of `waterfall_steps` and
    /// `sensitivity_steps` (step values, sensitivities and step P&Ls).
    ///
    /// Deliberately excluded: `total_pnl`, `mark_to_market_pnl`, `pv_t0`,
    /// `pv_t1`, `fx_translation_pnl` and `residual` (callers derive or
    /// recompute them) and the diagnostic absolute value
    /// `credit_factor_detail.adder_magnitude` (must stay non-negative).
    /// [`Self::scale`] and target-currency translation both walk this one
    /// visitor, so a new detail field is added in exactly one place.
    ///
    /// # Arguments
    ///
    /// * `f` - Mutation applied to each leaf in place; the first error aborts
    ///   the walk, leaving already-visited leaves mutated.
    ///
    /// # Errors
    ///
    /// Propagates the first error returned by `f`.
    pub fn for_each_money_mut(
        &mut self,
        mut f: impl FnMut(&mut Money) -> Result<()>,
    ) -> Result<()> {
        let mut visit = |_: &str, money: &mut Money| f(money);
        visit_factor_money!(self, visit, values_mut, mut)
    }

    /// Scale every monetary amount by a dimensionless position multiplier.
    ///
    /// Negative values reverse signed P&L exposure. Absolute diagnostics use
    /// the multiplier's magnitude so they remain non-negative. Scaling is
    /// atomic: an invalid multiplier or unrepresentable product leaves every
    /// amount and metadata field unchanged.
    ///
    /// # Arguments
    ///
    /// * `factor` - Finite dimensionless position multiplier, including zero
    ///   or a negative value for a short position. Every scaled amount must
    ///   remain within `Money`'s Decimal representation range.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] for a non-finite
    /// `factor`, or the checked monetary multiplication error if the scalar
    /// or any product cannot be represented. The original result is unchanged.
    pub fn scale(&mut self, factor: f64) -> Result<()> {
        if !factor.is_finite() {
            return Err(Error::Validation(
                "attribution scale factor must be finite".to_string(),
            ));
        }
        let mut scaled = self.clone();
        scaled.total_pnl = scaled.total_pnl.checked_mul_f64(factor)?;
        for money in [
            &mut scaled.mark_to_market_pnl,
            &mut scaled.pv_t0,
            &mut scaled.pv_t1,
        ]
        .into_iter()
        .flatten()
        {
            *money = money.checked_mul_f64(factor)?;
        }
        scaled.fx_translation_pnl = scaled.fx_translation_pnl.checked_mul_f64(factor)?;
        scaled.residual = scaled.residual.checked_mul_f64(factor)?;
        scaled.for_each_money_mut(|money| {
            *money = money.checked_mul_f64(factor)?;
            Ok(())
        })?;
        if let Some(money) = scaled
            .credit_factor_detail
            .as_mut()
            .and_then(|detail| detail.adder_magnitude.as_mut())
        {
            *money = money.checked_mul_f64(factor.abs())?;
        }
        *self = scaled;
        Ok(())
    }

    /// Validate that every monetary amount uses `total_pnl`'s currency.
    ///
    /// Includes headline P&L, residual, all nested factor and carry details,
    /// and absolute diagnostic amounts, so amount-only exports can use one
    /// report-currency label safely.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if any monetary
    /// amount uses a currency different from `total_pnl`.
    pub fn validate_currencies(&self) -> Result<()> {
        let expected = self.total_pnl.currency();
        let visit = |name: &str, money: &Money| {
            if money.currency() != expected {
                return Err(Error::Validation(format!(
                    "Currency mismatch in '{}' amount: expected {}, got {}",
                    name,
                    expected,
                    money.currency()
                )));
            }
            Ok(())
        };
        visit("residual", &self.residual)?;
        visit("fx_translation", &self.fx_translation_pnl)?;
        for (name, money) in [
            ("mark_to_market_pnl", &self.mark_to_market_pnl),
            ("pv_t0", &self.pv_t0),
            ("pv_t1", &self.pv_t1),
        ] {
            if let Some(money) = money {
                visit(name, money)?;
            }
        }
        if let Some(money) = self
            .credit_factor_detail
            .as_ref()
            .and_then(|detail| detail.adder_magnitude.as_ref())
        {
            visit("credit_factor_detail.adder_magnitude", money)?;
        }
        visit_factor_money!(self, visit, values)
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
    ///
    /// # Errors
    ///
    /// Returns the currency-validation or monetary-addition error. Before a
    /// currency error is returned, the method marks the result invalid, resets
    /// the residual to zero in the report currency, and records a diagnostic
    /// note.
    pub fn compute_residual(&mut self) -> Result<()> {
        // Validate currencies first
        if let Err(e) = self.validate_currencies() {
            let note = format!(
                "Currency validation failed during residual computation: {}",
                e
            );
            self.meta.notes.push(note);
            self.residual = Money::from((0_i64, self.total_pnl.currency()));
            self.meta.residual_pct = 0.0;
            // Surface the failure so `residual_within_tolerance`
            // does NOT report a "clean" 0 residual after a validation error.
            self.result_invalid = true;
            return Err(e);
        }

        let mut attributed_sum = self.carry;
        for (amount, label) in [
            (self.rates_curves_pnl, "rates curves P&L"),
            (self.credit_curves_pnl, "credit curves P&L"),
            (self.inflation_curves_pnl, "inflation curves P&L"),
            (self.correlations_pnl, "correlations P&L"),
            (self.fx_pnl, "FX P&L"),
            (self.vol_pnl, "vol P&L"),
            (self.cross_factor_pnl, "cross-factor P&L"),
            (self.model_params_pnl, "model params P&L"),
            (self.market_scalars_pnl, "market scalars P&L"),
            (self.fx_translation_pnl, "FX translation P&L"),
        ] {
            attributed_sum = add_factor(attributed_sum, amount, label, &mut self.meta.notes)?;
        }

        self.residual = match self.total_pnl.checked_sub(attributed_sum) {
            Ok(r) => r,
            Err(e) => {
                let note = format!("Failed to compute residual: {}", e);
                self.meta.notes.push(note);
                self.residual = Money::from((0_i64, self.total_pnl.currency()));
                self.meta.residual_pct = 0.0;
                // Flag invalid so tolerance checks fail.
                self.result_invalid = true;
                return Err(e);
            }
        };

        // Residual percentage; 0.0 when total_pnl is effectively zero.
        self.meta.residual_pct = self.pct_of_total(self.residual.amount()).unwrap_or(0.0);

        Ok(())
    }

    /// Share of total P&L, in percent, that an amount represents.
    ///
    /// Computes `amount / total_pnl × 100`, the per-factor percentage shown by
    /// [`Self::explain`] and attribution reports. The zero test on
    /// `total_pnl` uses the attribution's stored `RoundingContext`, the same
    /// test as `meta.residual_pct`.
    ///
    /// # Arguments
    ///
    /// * `amount` - P&L amount in `total_pnl` currency units, normally one
    ///   factor field (`carry`, `rates_curves_pnl`, `residual`, …) or
    ///   `total_pnl` itself.
    ///
    /// # Returns
    ///
    /// Signed percentage points (`25.0` = 25% of total P&L), or `None` when
    /// `total_pnl` is effectively zero and the share is undefined.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_attribution::{AttributionMethod, PnlAttribution};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use time::macros::date;
    ///
    /// let mut pnl = PnlAttribution::new(
    ///     Money::from((200_i64, Currency::USD)),
    ///     "BOND-1",
    ///     date!(2025 - 01 - 15),
    ///     date!(2025 - 01 - 16),
    ///     AttributionMethod::Parallel,
    /// );
    /// assert_eq!(pnl.pct_of_total(50.0), Some(25.0));
    /// pnl.total_pnl = Money::from((0_i64, Currency::USD));
    /// assert_eq!(pnl.pct_of_total(50.0), None);
    /// ```
    #[must_use]
    pub fn pct_of_total(&self, amount: f64) -> Option<f64> {
        let total = &self.total_pnl;
        if self
            .meta
            .rounding
            .is_effectively_zero_money(total.amount(), total.currency())
        {
            None
        } else {
            Some(amount / total.amount() * 100.0)
        }
    }

    /// Check if residual is within tolerance.
    ///
    /// Tolerance is either percentage-based (relative to total P&L) or
    /// absolute, whichever is larger.
    ///
    /// # Arguments
    ///
    /// * `pct_tolerance` - Percentage tolerance (e.g., `Some(0.1)` for 0.1%);
    ///   `None` uses the run's configured `meta.tolerance_pct`.
    /// * `abs_tolerance` - Absolute tolerance in `total_pnl` currency units
    ///   (e.g., `Some(100.0)` for $100); `None` uses `meta.tolerance_abs`.
    ///
    /// # Returns
    ///
    /// `true` if residual is within tolerance.
    pub fn residual_within_tolerance(
        &self,
        pct_tolerance: Option<f64>,
        abs_tolerance: Option<f64>,
    ) -> bool {
        let pct_tolerance = pct_tolerance.unwrap_or(self.meta.tolerance_pct);
        let abs_tolerance = abs_tolerance.unwrap_or(self.meta.tolerance_abs);
        // If residual computation failed, `residual` was reset to 0
        // and a clean tolerance check would falsely succeed. Refuse to claim
        // "within tolerance" when the attribution is flagged invalid.
        if self.result_invalid {
            return false;
        }

        let abs_residual = self.residual.amount().abs();
        let abs_total = self.total_pnl.amount().abs();

        // Tolerance is the larger of percentage-based or absolute. The
        // "is total_pnl effectively zero?" gate uses the attribution's stored
        // RoundingContext (MI4) rather than a hardcoded 1e-10 — keeps the
        // semantic consistent with the rest of `PnlAttribution`'s zero checks.
        let total_is_zero = self
            .meta
            .rounding
            .is_effectively_zero_money(abs_total, self.total_pnl.currency());
        let tolerance = if total_is_zero {
            abs_tolerance
        } else {
            (abs_total * pct_tolerance / 100.0).max(abs_tolerance)
        };

        abs_residual <= tolerance
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

        let fmt = |amount: &Money| -> String {
            let pct = self.pct_of_total(amount.amount()).unwrap_or(0.0);
            format!("{} ({:.1}%)", amount, pct)
        };

        let show = |m: &Money| -> bool {
            show_zeros || !rc.is_effectively_zero_money(m.amount(), m.currency())
        };

        let child = |label: &dyn std::fmt::Display, pnl: &Money| format!("  │   ├─ {label}: {pnl}");
        let mut carry_lines = Vec::new();
        if let Some(detail) = &self.carry_detail {
            if let Some(coupon_income) = &detail.coupon_income {
                carry_lines.push(child(&"Coupon Income", &coupon_income.total));
            }
            if let Some(pull_to_par) = &detail.pull_to_par {
                carry_lines.push(child(&"Pull-to-Par", pull_to_par));
            }
            if let Some(roll_down) = &detail.roll_down {
                carry_lines.push(child(&"Roll-Down", &roll_down.total));
            }
            if let Some(funding_cost) = &detail.funding_cost {
                carry_lines.push(format!("  │   └─ Funding Cost: {funding_cost}"));
            }
        }
        let by_curve = |by_curve: Option<
            &indexmap::IndexMap<finstack_quant_core::types::CurveId, Money>,
        >|
         -> Vec<String> {
            by_curve
                .into_iter()
                .flatten()
                .map(|(curve_id, pnl)| child(curve_id, pnl))
                .collect()
        };
        let cross_lines = self
            .cross_factor_detail
            .iter()
            .flat_map(|detail| &detail.by_pair)
            .map(|(pair, pnl)| child(pair, pnl))
            .collect();

        let mut lines = vec![format!("Total P&L: {}", self.total_pnl)];
        for (label, amount, children) in [
            ("Carry", &self.carry, carry_lines),
            (
                "Rates Curves",
                &self.rates_curves_pnl,
                by_curve(self.rates_detail.as_ref().map(|d| &d.by_curve)),
            ),
            (
                "Credit Curves",
                &self.credit_curves_pnl,
                by_curve(self.credit_detail.as_ref().map(|d| &d.by_curve)),
            ),
            ("Inflation Curves", &self.inflation_curves_pnl, Vec::new()),
            ("Correlations", &self.correlations_pnl, Vec::new()),
            ("FX", &self.fx_pnl, Vec::new()),
            ("FX Translation", &self.fx_translation_pnl, Vec::new()),
            ("Vol", &self.vol_pnl, Vec::new()),
            ("Cross-Factor", &self.cross_factor_pnl, cross_lines),
            ("Model Params", &self.model_params_pnl, Vec::new()),
            ("Market Scalars", &self.market_scalars_pnl, Vec::new()),
        ] {
            if show(amount) {
                lines.push(format!("  ├─ {label}: {}", fmt(amount)));
                lines.extend(children);
            }
        }

        lines.push(format!("  └─ Residual: {}", fmt(&self.residual)));

        lines.join("\n")
    }
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

fn add_factor(sum: Money, value: Money, label: &str, notes: &mut Vec<String>) -> Result<Money> {
    sum.checked_add(value).map_err(|e| {
        let note = format!("Failed to add {}: {}", label, e);
        notes.push(note);
        e
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;
    use std::collections::BTreeMap;
    use time::macros::date;

    #[test]
    fn pct_of_total_is_signed_share_and_none_for_zero_total() {
        let mut pnl = PnlAttribution::new(
            Money::new(22_733.88, Currency::USD).expect("finite total"),
            "BOND",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::Parallel,
        );
        let total = pnl.total_pnl.amount();
        let carry = pnl.pct_of_total(25_112.0).expect("non-zero total");
        assert!((carry - 25_112.0 / total * 100.0).abs() < 1e-12);
        assert_eq!(pnl.pct_of_total(total), Some(100.0));
        pnl.total_pnl = Money::from((0_i64, Currency::USD));
        assert_eq!(pnl.pct_of_total(5.0), None);
        assert_eq!(pnl.pct_of_total(0.0), None);
    }

    fn attribution_with_all_details() -> serde_json::Value {
        let mut value = serde_json::to_value(PnlAttribution::new(
            Money::from((1_i64, Currency::USD)),
            "ALL-DETAILS",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::Parallel,
        ))
        .expect("attribution wire payload");
        let money = serde_json::json!({"amount": "1", "currency": "USD"});
        let source = serde_json::json!({
            "total": money, "rates_part": money, "credit_part": money,
        });
        let level = serde_json::json!({
            "level_name": "rating", "total": money, "by_bucket": {"IG": money},
        });
        let details = serde_json::json!({
            "carry_detail": {"total": money, "coupon_income": source,
                "pull_to_par": money, "roll_down": source, "funding_cost": money},
            "rates_detail": {"by_curve": {"USD-OIS": money},
                "by_tenor": {"USD-OIS|1Y": money}, "discount_total": money, "forward_total": money},
            "credit_detail": {"by_curve": {"ACME-HZD": money}, "by_tenor": {"ACME-HZD|1Y": money}},
            "inflation_detail": {"by_curve": {"US-CPI": money}, "by_tenor": {"US-CPI|1Y": money}},
            "correlations_detail": {"by_curve": {"CDX-BC": money}},
            "fx_detail": {"by_pair": {"EUR/USD": money}},
            "vol_detail": {"by_surface": {"SPX-VOL": money}},
            "cross_factor_detail": {"total": money, "by_pair": {"Rates×Credit": money}},
            "model_params_detail": {"prepayment": money, "default_rate": money,
                "recovery_rate": money, "conversion_ratio": money, "other": {"parameter": money}},
            "scalars_detail": {"dividends": {"SPX": money}, "inflation": {"US-CPI": money},
                "equity_prices": {"SPX": money}, "commodity_prices": {"OIL": money}},
            "credit_factor_detail": {"model_id": "test/0", "generic_pnl": money,
                "levels": [level], "adder_pnl_total": money, "curve_shape_pnl": money,
                "adder_pnl_by_issuer": {"ACME": money}, "adder_magnitude": money},
            "credit_carry_decomposition": {"model_id": "test/0", "rates_carry_total": money,
                "credit_carry_total": money, "credit_by_level": {"generic": money,
                    "levels": [level], "adder_total": money, "adder_by_issuer": {"ACME": money}}},
        });
        value
            .as_object_mut()
            .expect("attribution object")
            .extend(details.as_object().expect("detail object").clone());
        value
    }

    // Traverse the public wire shape independently of the implementation's
    // monetary visitor, so an omitted nested Money field fails these checks.
    fn money_paths(value: &serde_json::Value, prefix: &str, paths: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if object.contains_key("amount") && object.contains_key("currency") {
                    paths.push(prefix.to_owned());
                } else {
                    for (key, child) in object {
                        let key = key.replace('~', "~0").replace('/', "~1");
                        money_paths(child, &format!("{prefix}/{key}"), paths);
                    }
                }
            }
            serde_json::Value::Array(array) => {
                for (index, child) in array.iter().enumerate() {
                    money_paths(child, &format!("{prefix}/{index}"), paths);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_currency_validation_checks_every_money_leaf() {
        let payload = attribution_with_all_details();
        let attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
        attribution
            .validate_currencies()
            .expect("all amounts use USD");
        let mut paths = Vec::new();
        money_paths(&payload, "", &mut paths);
        for path in paths {
            let mut mismatched = payload.clone();
            *mismatched.pointer_mut(&format!("{path}/currency")).unwrap() =
                serde_json::json!("EUR");
            let attribution: PnlAttribution = serde_json::from_value(mismatched).unwrap();
            assert!(attribution.validate_currencies().is_err(), "missed {path}");
            assert!(
                crate::long_rows::pnl_attribution_wide_row(&attribution).is_err(),
                "wide row accepted {path}"
            );
        }
    }

    #[test]
    fn test_scale_rejects_invalid_scalars_without_mutation() {
        for factor in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e100] {
            let payload = attribution_with_all_details();
            let mut attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
            assert!(attribution.scale(factor).is_err());
            assert_eq!(serde_json::to_value(attribution).unwrap(), payload);
        }
    }

    #[test]
    fn test_scale_updates_every_money_leaf_and_preserves_currencies() {
        let payload = attribution_with_all_details();
        let mut paths = Vec::new();
        money_paths(&payload, "", &mut paths);
        for factor in [-2.0, 0.0, 0.5] {
            let mut attribution: PnlAttribution = serde_json::from_value(payload.clone()).unwrap();
            attribution.scale(factor).expect("representable scale");
            let scaled = serde_json::to_value(attribution).unwrap();
            for path in &paths {
                let amount = |value: &serde_json::Value| {
                    value
                        .pointer(&format!("{path}/amount"))
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                };
                let multiplier = if path == "/credit_factor_detail/adder_magnitude" {
                    factor.abs()
                } else {
                    factor
                };
                assert_eq!(
                    amount(&scaled),
                    amount(&payload) * multiplier,
                    "missed {path}"
                );
                assert_eq!(
                    scaled.pointer(&format!("{path}/currency")),
                    payload.pointer(&format!("{path}/currency"))
                );
            }
            assert_eq!(scaled["meta"], payload["meta"]);
        }
    }

    #[test]
    fn test_scale_is_atomic_when_any_money_leaf_overflows() {
        let payload = attribution_with_all_details();
        let mut paths = Vec::new();
        money_paths(&payload, "", &mut paths);
        for path in paths {
            let mut overflowing = payload.clone();
            *overflowing.pointer_mut(&format!("{path}/amount")).unwrap() =
                serde_json::json!(rust_decimal::Decimal::MAX.to_string());
            let mut attribution: PnlAttribution = serde_json::from_value(overflowing).unwrap();
            let original = serde_json::to_value(&attribution).unwrap();
            assert!(attribution.scale(2.0).is_err(), "missed {path}");
            assert_eq!(
                serde_json::to_value(attribution).unwrap(),
                original,
                "mutated {path}"
            );
        }
    }

    #[test]
    fn test_metrics_based_defaults_include_bucketed_rates() {
        use finstack_quant_valuations::metrics::MetricId;
        let metrics = crate::default_attribution_metrics();
        assert!(metrics.contains(&MetricId::BucketedDv01));
        assert!(metrics.contains(&MetricId::BucketedCs01));
        assert_eq!(metrics, AttributionMethod::MetricsBased.required_metrics());
    }

    #[test]
    fn test_carry_detail_scale_and_explain_include_decomposition_fields() {
        let mut attribution = PnlAttribution::new(
            Money::from((10_i64, Currency::USD)),
            "BOND-1",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::MetricsBased,
        );
        attribution.carry = Money::from((10_i64, Currency::USD));
        attribution.carry_detail = Some(CarryDetail {
            total: Money::from((10_i64, Currency::USD)),
            coupon_income: Some(SourceLine::scalar(Money::from((3_i64, Currency::USD)))),
            pull_to_par: Some(Money::from((4_i64, Currency::USD))),
            roll_down: Some(SourceLine::scalar(Money::from((5_i64, Currency::USD)))),
            funding_cost: Some(Money::from((2_i64, Currency::USD))),
        });

        attribution.scale(0.5).expect("finite representable scale");

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

        attribution.carry_detail = Some(detail);
        let explanation = attribution.explain_verbose();
        assert!(explanation.contains("Coupon Income"));
        assert!(explanation.contains("Pull-to-Par"));
        assert!(explanation.contains("Roll-Down"));
        assert!(explanation.contains("Funding Cost"));
        assert!(!explanation.contains("Theta"));
    }

    #[test]
    fn explain_renders_nonzero_factors_with_their_children() {
        let usd = |amount: i64| Money::from((amount, Currency::USD));
        let mut attribution = PnlAttribution::new(
            usd(100),
            "BOND-1",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::Parallel,
        );
        attribution.carry = usd(40);
        attribution.carry_detail = Some(CarryDetail {
            total: usd(40),
            coupon_income: Some(SourceLine::scalar(usd(30))),
            pull_to_par: None,
            roll_down: None,
            funding_cost: Some(usd(5)),
        });
        attribution.rates_curves_pnl = usd(50);
        attribution.rates_detail = Some(RatesCurvesAttribution {
            by_curve: IndexMap::from([(
                finstack_quant_core::types::CurveId::new("USD-OIS"),
                usd(50),
            )]),
            by_tenor: IndexMap::new(),
            discount_total: usd(50),
            forward_total: usd(0),
        });
        attribution
            .compute_residual()
            .expect("same-currency residual");

        let expected = [
            format!("Total P&L: {}", usd(100)),
            format!("  ├─ Carry: {} (40.0%)", usd(40)),
            format!("  │   ├─ Coupon Income: {}", usd(30)),
            format!("  │   └─ Funding Cost: {}", usd(5)),
            format!("  ├─ Rates Curves: {} (50.0%)", usd(50)),
            format!("  │   ├─ USD-OIS: {}", usd(50)),
            format!("  └─ Residual: {} (10.0%)", usd(10)),
        ]
        .join("\n");
        assert_eq!(attribution.explain(), expected);
        assert!(attribution
            .explain_verbose()
            .contains("  ├─ Market Scalars: "));
    }

    #[test]
    fn test_cross_factor_detail_serde_roundtrip() {
        let detail = CrossFactorDetail {
            total: Money::from((500_i64, Currency::USD)),
            by_pair: {
                let mut map = IndexMap::new();
                map.insert(
                    "Rates×Credit".to_string(),
                    Money::from((300_i64, Currency::USD)),
                );
                map.insert(
                    "Spot×Vol".to_string(),
                    Money::from((200_i64, Currency::USD)),
                );
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
        let total = Money::from((1000_i64, Currency::USD));
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::from((600_i64, Currency::USD));
        attr.credit_curves_pnl = Money::from((200_i64, Currency::USD));
        attr.cross_factor_pnl = Money::from((150_i64, Currency::USD));

        attr.compute_residual().unwrap();

        assert!((attr.residual.amount() - 50.0).abs() < 1e-10);
    }

    /// `mark_to_market_pnl` must capture the raw `val_t1 − val_t0`
    /// supplied at construction time and stay frozen even if `total_pnl` is
    /// later mutated by `apply_total_return_carry`. This is what lets a
    /// consumer reconcile against their own computation of the price change.
    #[test]
    fn test_mark_to_market_pnl_captured_at_construction_and_preserved() {
        use crate::helpers::apply_total_return_carry;
        let raw_pnl = Money::from((1000_i64, Currency::USD));
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
        let coupon = Money::from((50_i64, Currency::USD));
        let theta = Money::from((30_i64, Currency::USD));
        let carry_inputs = crate::helpers::TotalReturnCarryInputs {
            cash_paid: coupon,
            income_cash_paid: coupon,
            delta_accrued: None,
            flat_window_diff: None,
            funding_cost: None,
            num_repricings: 0,
            warnings: Vec::new(),
        };
        apply_total_return_carry(&mut attr, theta, carry_inputs).expect("carry add");

        assert_eq!(attr.total_pnl.amount(), 1050.0);
        assert_eq!(
            attr.mark_to_market_pnl.map(|m| m.amount()),
            Some(1000.0),
            "mark_to_market_pnl must not absorb coupon adjustment"
        );
    }

    /// When compute_residual fails because a factor is in a
    /// different currency from total_pnl, the attribution must report itself
    /// as invalid, and `residual_within_tolerance` must refuse to claim
    /// success even though `residual == 0` was set by the failure path.
    #[test]
    fn test_failed_residual_marks_invalid_and_blocks_tolerance_check() {
        let total = Money::from((1000_i64, Currency::USD));
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 01),
            date!(2025 - 01 - 02),
            AttributionMethod::Parallel,
        );
        // Deliberately mismatched currency so validate_currencies errors out.
        attr.rates_curves_pnl = Money::from((600_i64, Currency::EUR));

        let err = attr.compute_residual();
        assert!(err.is_err(), "currency mismatch must error");
        assert!(attr.result_invalid, "result_invalid must be set on failure");
        // residual was reset to zero — without the result_invalid guard the
        // tolerance check below would falsely succeed.
        assert_eq!(attr.residual.amount(), 0.0);
        assert!(
            !attr.residual_within_tolerance(Some(0.1), Some(1.0)),
            "tolerance check MUST fail when result_invalid is set, regardless of residual value"
        );
        assert!(
            !attr.residual_within_tolerance(None, None),
            "stored-tolerance check MUST also fail when result_invalid is set"
        );
    }

    /// Scaling must include every `credit_factor_detail` component, including
    /// `curve_shape_pnl` and the `adder_magnitude` diagnostic, while preserving
    /// `generic + Σlevels + adder + curve_shape ≡ credit_curves_pnl`.
    #[test]
    fn test_scale_preserves_credit_factor_detail_reconciliation() {
        let usd = Currency::USD;
        let mut attr = PnlAttribution::new(
            Money::from((100_i64, usd)),
            "BOND-SCALE",
            date!(2025 - 01 - 15),
            date!(2025 - 02 - 15),
            AttributionMethod::Parallel,
        );
        attr.credit_curves_pnl = Money::from((100_i64, usd));
        attr.credit_factor_detail = Some(CreditFactorAttribution {
            model_id: "test/0".to_string(),
            generic_pnl: Money::from((40_i64, usd)),
            levels: vec![LevelPnl {
                level_name: "rating".to_string(),
                total: Money::from((30_i64, usd)),
                by_bucket: BTreeMap::new(),
            }],
            adder_pnl_total: Money::from((20_i64, usd)),
            curve_shape_pnl: Money::from((10_i64, usd)),
            adder_pnl_by_issuer: None,
            adder_magnitude: Some(Money::from((20_i64, usd))),
        });

        let reconciles = |a: &PnlAttribution| {
            let d = a.credit_factor_detail.as_ref().expect("detail");
            let sum = d.generic_pnl.amount()
                + d.levels.iter().map(|l| l.total.amount()).sum::<f64>()
                + d.adder_pnl_total.amount()
                + d.curve_shape_pnl.amount();
            (sum - a.credit_curves_pnl.amount()).abs() < 1e-9
        };
        assert!(reconciles(&attr), "fixture must reconcile before scaling");

        attr.scale(0.5).expect("finite representable scale");
        let d = attr.credit_factor_detail.as_ref().expect("detail");
        assert_eq!(
            d.curve_shape_pnl.amount(),
            5.0,
            "curve_shape_pnl must scale"
        );
        assert_eq!(
            d.adder_magnitude.map(|m| m.amount()),
            Some(10.0),
            "adder_magnitude must scale"
        );
        assert!(
            reconciles(&attr),
            "reconciliation invariant must survive scaling"
        );

        // A negative scale (short position) must keep adder_magnitude non-negative.
        attr.scale(-1.0).expect("finite representable scale");
        assert_eq!(
            attr.credit_factor_detail
                .as_ref()
                .and_then(|d| d.adder_magnitude)
                .map(|m| m.amount()),
            Some(10.0),
            "adder_magnitude is a |.| diagnostic — stays non-negative under negative scale"
        );
        assert!(
            reconciles(&attr),
            "reconciliation survives a negative scale"
        );
    }
}
