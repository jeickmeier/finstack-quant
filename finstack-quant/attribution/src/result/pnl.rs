//! Top-level [`PnlAttribution`] result container and reconciliation methods.

use super::types::{AttributionFactor, AttributionMeta, AttributionMethod};
use crate::{
    CarryDetail, CorrelationsAttribution, CreditCarryDecomposition, CreditCurvesAttribution,
    CreditFactorAttribution, CrossFactorDetail, FxAttribution, InflationCurvesAttribution,
    ModelParamsAttribution, RatesCurvesAttribution, ScalarsAttribution, SensitivityStep,
    VolAttribution, WaterfallStep,
};
use finstack_quant_core::config::RoundingContext;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

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
    /// * `t0` - Valuation date opening the attribution window; the T₀ market
    ///   and instrument state are observed on it.
    /// * `t1` - Valuation date closing the window (`t1 >= t0`); the T₁ market
    ///   and instrument state are observed on it.
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

    /// Add another attribution's headline amounts into this one.
    ///
    /// Sums `total_pnl`, every factor field, `cross_factor_pnl`,
    /// `fx_translation_pnl` and `residual` with currency-checked `Money`
    /// arithmetic. `mark_to_market_pnl`, `pv_t0` and `pv_t1` stay present
    /// only when both sides carry them. `result_invalid`, the repricing count
    /// and the notes are merged; detail structs and step rows are not, since
    /// per-instrument breakdowns do not sum into one tree. Use it to aggregate
    /// positions or composite legs that are already in one reporting currency
    /// (see [`crate::translate_to_target_currency`]).
    ///
    /// # Arguments
    ///
    /// * `other` - Attribution to add, in the same currency as `self`.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::CurrencyMismatch`] when an amount
    /// of `other` is in a different currency, or the checked-addition error
    /// when a sum cannot be represented. `self` may then be partially
    /// updated, so callers abort the aggregation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_attribution::{AttributionMethod, PnlAttribution};
    /// use finstack_quant_core::{currency::Currency, money::Money};
    /// use time::macros::date;
    ///
    /// let leg = |carry: i64| {
    ///     let mut attribution = PnlAttribution::new(
    ///         Money::from((carry, Currency::USD)),
    ///         "LEG",
    ///         date!(2025 - 01 - 15),
    ///         date!(2025 - 01 - 16),
    ///         AttributionMethod::Parallel,
    ///     );
    ///     attribution.carry = attribution.total_pnl;
    ///     attribution
    /// };
    /// let mut book = leg(10);
    /// book.accumulate(leg(15))?;
    /// assert_eq!(book.carry, Money::from((25_i64, Currency::USD)));
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn accumulate(&mut self, other: PnlAttribution) -> Result<()> {
        self.total_pnl = self.total_pnl.checked_add(other.total_pnl)?;
        for (target, source) in [
            (&mut self.mark_to_market_pnl, other.mark_to_market_pnl),
            (&mut self.pv_t0, other.pv_t0),
            (&mut self.pv_t1, other.pv_t1),
        ] {
            *target = match (*target, source) {
                (Some(target), Some(source)) => Some(target.checked_add(source)?),
                _ => None,
            };
        }
        for (target, source) in [
            (&mut self.carry, other.carry),
            (&mut self.rates_curves_pnl, other.rates_curves_pnl),
            (&mut self.credit_curves_pnl, other.credit_curves_pnl),
            (&mut self.inflation_curves_pnl, other.inflation_curves_pnl),
            (&mut self.correlations_pnl, other.correlations_pnl),
            (&mut self.fx_pnl, other.fx_pnl),
            (&mut self.vol_pnl, other.vol_pnl),
            (&mut self.cross_factor_pnl, other.cross_factor_pnl),
            (&mut self.model_params_pnl, other.model_params_pnl),
            (&mut self.market_scalars_pnl, other.market_scalars_pnl),
            (&mut self.fx_translation_pnl, other.fx_translation_pnl),
            (&mut self.residual, other.residual),
        ] {
            *target = target.checked_add(source)?;
        }
        self.result_invalid |= other.result_invalid;
        self.meta.num_repricings += other.meta.num_repricings;
        self.meta.notes.extend(other.meta.notes);
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

    /// P&L amount attributed to one factor.
    ///
    /// `Carry` reads [`Self::carry`]; every other factor reads its own
    /// `*_pnl` field (`RatesCurves` → [`Self::rates_curves_pnl`], …).
    ///
    /// # Arguments
    ///
    /// * `factor` - Attribution factor whose headline amount is returned, in
    ///   the attribution's reporting currency.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_attribution::{AttributionFactor, AttributionMethod, PnlAttribution};
    /// use finstack_quant_core::{currency::Currency, money::Money};
    /// use time::macros::date;
    ///
    /// let mut attribution = PnlAttribution::new(
    ///     Money::from((100_i64, Currency::USD)),
    ///     "BOND-1",
    ///     date!(2025 - 01 - 15),
    ///     date!(2025 - 01 - 16),
    ///     AttributionMethod::Parallel,
    /// );
    /// attribution.rates_curves_pnl = Money::from((60_i64, Currency::USD));
    /// assert_eq!(
    ///     attribution.get_factor_pnl(&AttributionFactor::RatesCurves),
    ///     Money::from((60_i64, Currency::USD)),
    /// );
    /// ```
    #[must_use]
    pub fn get_factor_pnl(&self, factor: &AttributionFactor) -> Money {
        match factor {
            AttributionFactor::Carry => self.carry,
            AttributionFactor::RatesCurves => self.rates_curves_pnl,
            AttributionFactor::CreditCurves => self.credit_curves_pnl,
            AttributionFactor::InflationCurves => self.inflation_curves_pnl,
            AttributionFactor::Correlations => self.correlations_pnl,
            AttributionFactor::Fx => self.fx_pnl,
            AttributionFactor::Volatility => self.vol_pnl,
            AttributionFactor::ModelParameters => self.model_params_pnl,
            AttributionFactor::MarketScalars => self.market_scalars_pnl,
        }
    }

    /// Mutable twin of [`Self::get_factor_pnl`], used by the engines to land
    /// a factor's P&L in its field.
    pub(crate) fn factor_pnl_mut(&mut self, factor: &AttributionFactor) -> &mut Money {
        match factor {
            AttributionFactor::Carry => &mut self.carry,
            AttributionFactor::RatesCurves => &mut self.rates_curves_pnl,
            AttributionFactor::CreditCurves => &mut self.credit_curves_pnl,
            AttributionFactor::InflationCurves => &mut self.inflation_curves_pnl,
            AttributionFactor::Correlations => &mut self.correlations_pnl,
            AttributionFactor::Fx => &mut self.fx_pnl,
            AttributionFactor::Volatility => &mut self.vol_pnl,
            AttributionFactor::ModelParameters => &mut self.model_params_pnl,
            AttributionFactor::MarketScalars => &mut self.market_scalars_pnl,
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
