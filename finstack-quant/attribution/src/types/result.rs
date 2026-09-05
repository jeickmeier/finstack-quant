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
/// Maps cleanly to `MarketContext` structure:
/// - **RatesCurves**: discount_curves + forward_curves
/// - **CreditCurves**: hazard_curves
/// - **InflationCurves**: inflation_curves
/// - **Correlations**: base_correlation_curves
/// - **Fx**: FxMatrix
/// - **Volatility**: surfaces (VolSurface)
/// - **MarketScalars**: prices, series, inflation_indices, dividends
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

    /// Inflation curves.
    InflationCurves,

    /// Base correlation curves (structured credit).
    Correlations,

    /// FX rate changes.
    Fx,

    /// Implied volatility changes.
    Volatility,

    /// Market scalars (dividends, equity/commodity prices, inflation indices).
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
    /// Separates the raw user-input price change from
    /// the total-return view stamped on `total_pnl`. When the attribution path
    /// added coupon_income to `total_pnl` (the standard total-return convention
    /// in parallel / waterfall / taylor attribution), this field still reports
    /// the raw `val_t1 − val_t0` so a downstream consumer that computed their
    /// own total from the underlying valuations can reconcile cleanly.
    ///
    /// Attribution paths that cannot provide the raw mark-to-market change use
    /// `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
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

    /// Execution policy the attribution ran under (workspace
    /// policy-visibility invariant: results stamp the parallel flag).
    /// `None` for methods without a policy knob (metrics-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_policy: Option<ExecutionPolicy>,

    /// Diagnostic notes and warnings.
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
        let zero = Money::from((0_i64, total_pnl.currency()));

        Self {
            total_pnl,
            // Raw mark-to-market is the caller-supplied `total_pnl` before
            // any total-return adjustment. `apply_total_return_carry` leaves
            // this field untouched when it mutates `total_pnl`.
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
    pub(crate) fn new_with_rounding(
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

    /// Apply `f` to every signed `Money` leaf: the per-factor aggregates
    /// (`carry` … `market_scalars_pnl`) and every leaf of every populated
    /// detail struct.
    ///
    /// Deliberately excluded: `total_pnl`, `mark_to_market_pnl`,
    /// `fx_translation_pnl` and `residual` (callers derive or recompute
    /// them) and the diagnostic absolute value
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
        fn values<'m>(
            values: impl Iterator<Item = &'m mut Money>,
            f: &mut impl FnMut(&mut Money) -> Result<()>,
        ) -> Result<()> {
            for v in values {
                f(v)?;
            }
            Ok(())
        }
        fn opt(
            opt: &mut Option<Money>,
            f: &mut impl FnMut(&mut Money) -> Result<()>,
        ) -> Result<()> {
            opt.as_mut().map_or(Ok(()), f)
        }
        fn source_line(
            line: &mut Option<SourceLine>,
            f: &mut impl FnMut(&mut Money) -> Result<()>,
        ) -> Result<()> {
            if let Some(line) = line {
                f(&mut line.total)?;
                opt(&mut line.rates_part, f)?;
                opt(&mut line.credit_part, f)?;
            }
            Ok(())
        }
        for m in [
            &mut self.carry,
            &mut self.rates_curves_pnl,
            &mut self.credit_curves_pnl,
            &mut self.inflation_curves_pnl,
            &mut self.correlations_pnl,
            &mut self.fx_pnl,
            &mut self.vol_pnl,
            &mut self.cross_factor_pnl,
            &mut self.model_params_pnl,
            &mut self.market_scalars_pnl,
        ] {
            f(m)?;
        }

        if let Some(d) = &mut self.carry_detail {
            f(&mut d.total)?;
            source_line(&mut d.coupon_income, &mut f)?;
            opt(&mut d.pull_to_par, &mut f)?;
            source_line(&mut d.roll_down, &mut f)?;
            opt(&mut d.funding_cost, &mut f)?;
        }
        if let Some(d) = &mut self.rates_detail {
            values(d.by_curve.values_mut(), &mut f)?;
            values(d.by_tenor.values_mut(), &mut f)?;
            f(&mut d.discount_total)?;
            f(&mut d.forward_total)?;
        }
        if let Some(d) = &mut self.credit_detail {
            values(d.by_curve.values_mut(), &mut f)?;
            values(d.by_tenor.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.inflation_detail {
            values(d.by_curve.values_mut(), &mut f)?;
            if let Some(bt) = &mut d.by_tenor {
                values(bt.values_mut(), &mut f)?;
            }
        }
        if let Some(d) = &mut self.correlations_detail {
            values(d.by_curve.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.fx_detail {
            values(d.by_pair.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.vol_detail {
            values(d.by_surface.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.cross_factor_detail {
            f(&mut d.total)?;
            values(d.by_pair.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.model_params_detail {
            opt(&mut d.prepayment, &mut f)?;
            opt(&mut d.default_rate, &mut f)?;
            opt(&mut d.recovery_rate, &mut f)?;
            opt(&mut d.conversion_ratio, &mut f)?;
            values(d.other.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.scalars_detail {
            values(d.dividends.values_mut(), &mut f)?;
            values(d.inflation.values_mut(), &mut f)?;
            values(d.equity_prices.values_mut(), &mut f)?;
            values(d.commodity_prices.values_mut(), &mut f)?;
        }
        if let Some(d) = &mut self.credit_factor_detail {
            f(&mut d.generic_pnl)?;
            f(&mut d.adder_pnl_total)?;
            // `curve_shape_pnl` is a signed P&L component and load-bearing for
            // the `generic + Σlevels + adder + curve_shape ≡ credit_curves_pnl`
            // reconciliation invariant — it MUST move with the rest.
            f(&mut d.curve_shape_pnl)?;
            for level in &mut d.levels {
                f(&mut level.total)?;
                values(level.by_bucket.values_mut(), &mut f)?;
            }
            if let Some(by_issuer) = &mut d.adder_pnl_by_issuer {
                values(by_issuer.values_mut(), &mut f)?;
            }
        }
        if let Some(d) = &mut self.credit_carry_decomposition {
            f(&mut d.rates_carry_total)?;
            f(&mut d.credit_carry_total)?;
            f(&mut d.credit_by_level.generic)?;
            f(&mut d.credit_by_level.adder_total)?;
            for level in &mut d.credit_by_level.levels {
                f(&mut level.total)?;
                values(level.by_bucket.values_mut(), &mut f)?;
            }
            if let Some(by_issuer) = &mut d.credit_by_level.adder_by_issuer {
                values(by_issuer.values_mut(), &mut f)?;
            }
        }
        Ok(())
    }

    /// Scale all attribution values by a factor.
    ///
    /// Useful for scaling per-unit attribution to position quantity.
    ///
    /// A non-finite `factor` flags the attribution invalid and leaves the
    /// values untouched instead of panicking inside `Money`'s arithmetic
    /// (this crate forbids panics on public APIs).
    ///
    /// # Arguments
    ///
    /// * `factor` - Dimensionless position multiplier applied in place to attribution amounts; negative values reverse exposure, and non-finite values flag the result invalid without scaling.
    pub fn scale(&mut self, factor: f64) {
        if !factor.is_finite() {
            self.meta.notes.push(format!(
                "PnlAttribution::scale called with non-finite factor ({factor}); \
                 values left unscaled and attribution flagged invalid"
            ));
            self.result_invalid = true;
            return;
        }
        self.total_pnl *= factor;
        if let Some(m) = self.mark_to_market_pnl.as_mut() {
            *m *= factor;
        }
        self.fx_translation_pnl *= factor;
        self.residual *= factor;
        // `*=` on Money is infallible, so the visitor cannot fail here.
        let _ = self.for_each_money_mut(|m| {
            *m *= factor;
            Ok(())
        });
        // `adder_magnitude` is a diagnostic absolute value (= |adder_pnl_total|);
        // scale by |factor| so it stays non-negative for short positions.
        if let Some(m) = self
            .credit_factor_detail
            .as_mut()
            .and_then(|d| d.adder_magnitude.as_mut())
        {
            *m *= factor.abs();
        }
    }

    /// Validate that all factor currencies match total_pnl currency.
    ///
    /// # Returns
    ///
    /// Ok(()) if all currencies match, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if any factor P&L or
    /// optional detail amount uses a currency different from `total_pnl`.
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
            ("fx_translation", self.fx_translation_pnl.currency()),
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
        attributed_sum = add_factor(
            attributed_sum,
            self.fx_translation_pnl,
            "FX translation P&L",
            &mut self.meta.notes,
        )?;

        // MI5: guard against a non-finite accumulator before calling
        // `checked_sub` — `Money::new` panics on NaN/Inf inside `checked_sub`,
        // so if attributed_sum has hit ±∞ from a runaway factor we flag the
        // attribution invalid instead of unwinding the caller's stack.
        if !attributed_sum.amount().is_finite() {
            let note = format!(
                "Non-finite attributed sum ({:?}) during residual computation; \
                 attribution flagged invalid",
                attributed_sum.amount()
            );
            self.meta.notes.push(note);
            self.residual = Money::from((0_i64, self.total_pnl.currency()));
            self.meta.residual_pct = 0.0;
            self.result_invalid = true;
            return Ok(());
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
    /// For `MetricsBased`, returns first-order sensitivities (Theta, DV01, CS01,
    /// Vega, Delta, FX01, Inflation01, Dividend01) plus second-order terms
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

        attribution.carry_detail = Some(detail);
        let explanation = attribution.explain_verbose();
        assert!(explanation.contains("Coupon Income"));
        assert!(explanation.contains("Pull-to-Par"));
        assert!(explanation.contains("Roll-Down"));
        assert!(explanation.contains("Funding Cost"));
        assert!(!explanation.contains("Theta"));
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
            delta_accrued: None,
            flat_window_diff: None,
            funding_cost: None,
            warnings: Vec::new(),
            invalid: false,
        };
        apply_total_return_carry(&mut attr, theta, carry_inputs).expect("carry add");

        // total_pnl is now total-return (1000 + 50 = 1050); mark-to-market
        // must still report the raw pre-coupon price change (1000).
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
            !attr.residual_within_tolerance(0.1, 1.0),
            "tolerance check MUST fail when result_invalid is set, regardless of residual value"
        );
        assert!(
            !attr.residual_within_tolerance(attr.meta.tolerance_pct, attr.meta.tolerance_abs),
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

        attr.scale(0.5);
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
        attr.scale(-1.0);
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
