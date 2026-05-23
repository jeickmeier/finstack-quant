//! Waterfall P&L attribution methodology.
//!
//! Sequential factor application approach where factors are applied one-by-one
//! in a specified order. Each factor's P&L is computed after applying all
//! previous factors.
//!
//! # Algorithm
//!
//! 1. Start with PV at T₀
//! 2. For each factor in `factor_order`:
//!    - Apply that factor's T₁ state while keeping remaining factors at T₀
//!    - Reprice and record delta
//!    - Keep that factor at T₁ for remaining steps
//! 3. Final PV should equal T₁ PV (residual ≈ 0 by construction)
//!
//! # Default Order
//!
//! If no order specified:
//! 1. Carry
//! 2. RatesCurves
//! 3. CreditCurves
//! 4. InflationCurves
//! 5. Correlations
//! 6. Fx
//! 7. Volatility
//! 8. ModelParameters
//! 9. MarketScalars
//!
//! # Notes
//!
//! - Order matters! Different orders produce different factor attributions
//! - Residual is minimal by construction (should be within numeric precision)
//! - Recommended for risk reporting where sum must equal total

use super::credit_cascade::{
    build_credit_factor_attribution, plan_credit_cascade, shift_credit_curves_par_spread,
    snap_hazard_to_t1, CreditCascade, CreditStepKind,
};
use super::credit_factor::CreditFactorDetailOptions;
use super::factors::*;
use super::helpers::*;
use super::model_params;
use super::types::*;
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::{Error, Result};
use finstack_factor_model::credit_hierarchy::CreditFactorModel;
use finstack_valuations::instruments::Instrument;
use std::sync::Arc;

/// Default waterfall order for factor attribution.
///
/// # Returns
///
/// Vector of attribution factors in recommended sequential order.
pub fn default_waterfall_order() -> Vec<AttributionFactor> {
    vec![
        AttributionFactor::Carry,
        AttributionFactor::RatesCurves,
        AttributionFactor::CreditCurves,
        AttributionFactor::InflationCurves,
        AttributionFactor::Correlations,
        AttributionFactor::Fx,
        AttributionFactor::Volatility,
        AttributionFactor::ModelParameters,
        AttributionFactor::MarketScalars,
    ]
}

/// Perform waterfall P&L attribution for an instrument.
///
/// Factors are applied sequentially in the specified order. Each factor's
/// P&L is computed after applying all previous factors at T₁.
///
/// # Arguments
///
/// * `instrument` - Instrument to attribute
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `as_of_t0` - Valuation date at T₀
/// * `as_of_t1` - Valuation date at T₁
/// * `config` - Finstack configuration
/// * `factor_order` - Ordered list of factors to apply
/// * `strict_validation` - If true, propagate errors instead of soft failures
///
/// # Returns
///
/// Complete P&L attribution with factor decomposition.
///
/// # Errors
///
/// Returns error if:
/// - Pricing fails at any step
/// - Currency conversion fails
/// - Factor order is empty
/// - (If strict_validation) Model parameter modification/repricing fails
///
/// # Examples
///
/// ```ignore
/// use finstack_attribution::{
///     attribute_pnl_waterfall, default_waterfall_order
/// };
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
/// let attribution = attribute_pnl_waterfall(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
///     default_waterfall_order(),
///     true, // Strict validation
///     None,
/// )?;
///
/// // Residual should be minimal
/// assert!(attribution.residual_within_tolerance(0.01, 1.0));
/// # Ok(())
/// # }
/// ```
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip_all, fields(instrument_id = %instrument.id(), method = "waterfall"))]
pub fn attribute_pnl_waterfall(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    factor_order: Vec<AttributionFactor>,
    strict_validation: bool,
    model_params_t0: Option<&model_params::ModelParamsSnapshot>,
) -> Result<PnlAttribution> {
    attribute_pnl_waterfall_with_credit_model(
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        factor_order,
        strict_validation,
        model_params_t0,
        None,
        &CreditFactorDetailOptions::default(),
    )
}

/// Waterfall attribution with optional `CreditFactorModel`.
///
/// When `credit_factor_model` is `Some(_)` and the order contains
/// [`AttributionFactor::CreditCurves`], the single credit step is replaced by
/// the per-issuer hierarchy cascade (`generic → level_0 → … → level_{L-1} →
/// adder`). Each step bumps the instrument's hazard curves by the issuer's
/// `β·ΔF` (or `Δadder` / snap-to-T1 for the final adder), reprices, and
/// captures step-level credit P&L. The aggregate `credit_curves_pnl` matches
/// the no-model single-step result byte-identically because the adder step
/// snaps the running hazard curves to T1.
///
/// The reconciliation invariant
/// `generic_pnl + Σ levels.total + adder_pnl_total ≡ credit_curves_pnl`
/// holds at 1e-8 by construction.
///
/// See `credit_cascade::plan_credit_cascade` for the multi-curve issuer averaging caveat.
///
/// # Performance
///
/// When a `CreditFactorModel` is supplied with `L` hierarchy levels, the credit
/// cascade performs `L + 2` additional repricings (PC, one per level, and
/// Adder) compared to the single-step credit reprice without a model. For
/// typical L = 1–3 and portfolios of thousands of instruments this is
/// acceptable; consider `MetricsBased` or `Taylor` for cost-sensitive use cases
/// (they remain linear, no reprice).
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    skip_all,
    fields(instrument_id = %instrument.id(), method = "waterfall")
)]
pub fn attribute_pnl_waterfall_with_credit_model(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    _config: &FinstackConfig,
    factor_order: Vec<AttributionFactor>,
    strict_validation: bool,
    model_params_t0: Option<&model_params::ModelParamsSnapshot>,
    credit_factor_model: Option<&CreditFactorModel>,
    credit_factor_detail_options: &CreditFactorDetailOptions,
) -> Result<PnlAttribution> {
    if factor_order.is_empty() {
        return Err(Error::Validation(
            "Waterfall attribution requires non-empty factor_order".to_string(),
        ));
    }
    validate_attribution_period(as_of_t0, as_of_t1)?;

    // Step 1: Price at T₀
    // Use T₀ model parameters for T₀ valuation if available
    let instrument_t0 = if let Some(params) = model_params_t0 {
        model_params::with_model_params(instrument, params)?
    } else {
        Arc::clone(instrument)
    };
    let val_t0 = reprice_instrument(&instrument_t0, market_t0, as_of_t0)?;

    // Also price at T₁ for total P&L calculation
    let val_t1 = reprice_instrument(instrument, market_t1, as_of_t1)?;

    let total_pnl = compute_pnl_with_fx(
        val_t0,
        val_t1,
        val_t1.currency(),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )?;

    let mut attribution = init_attribution(
        total_pnl,
        instrument.id(),
        as_of_t0,
        as_of_t1,
        AttributionMethod::Waterfall(factor_order.clone()),
        Some(_config),
    );

    // Plan a credit cascade if a model was supplied. Falls back to the legacy
    // single CreditCurves step when planning yields None (no issuer tag, no
    // hazard deps, etc.).
    let cascade: Option<CreditCascade> = match credit_factor_model {
        Some(model) => {
            plan_credit_cascade(model, instrument, market_t0, market_t1, as_of_t0, as_of_t1)?
        }
        None => None,
    };
    if credit_factor_model.is_some() && cascade.is_none() {
        tracing::warn!(
            instrument_id = instrument.id(),
            method = "waterfall",
            "Credit factor model supplied but credit cascade could not be planned"
        );
        attribution.meta.notes.push(format!(
            "credit_factor_model supplied but no resolvable issuer/hazard cascade for {}; credit_factor_detail omitted",
            instrument.id()
        ));
    }
    let mut credit_step_pnls: Vec<finstack_core::money::Money> = Vec::new();

    // Build hybrid market: start with all T₀, progressively apply T₁
    let mut ctx = WaterfallContext {
        target_instrument: instrument,
        current_instrument: instrument_t0,
        current_market: market_t0.clone(),
        current_val: val_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        strict_validation,
        num_repricings: 2, // T₀ and T₁ repricings already performed
    };

    // Apply each factor in sequence
    for factor in factor_order {
        // Intercept CreditCurves with the cascade when a model is supplied.
        if matches!(factor, AttributionFactor::CreditCurves) {
            if let Some(c) = &cascade {
                let total = ctx.apply_credit_cascade(c, &mut credit_step_pnls)?;
                attribution.credit_curves_pnl = total;
                continue;
            }
        }

        let factor_pnl = ctx.apply_factor(&factor)?;

        // Record factor P&L
        match factor {
            AttributionFactor::Carry => {
                let theta = factor_pnl;
                let carry_inputs = total_return_carry_inputs(
                    ctx.current_instrument.as_ref(),
                    &ctx.current_market,
                    market_t0,
                    ctx.as_of_t0,
                    ctx.as_of_t1,
                    factor_pnl.currency(),
                );

                apply_total_return_carry(
                    &mut attribution,
                    theta,
                    carry_inputs.coupon_income,
                    carry_inputs.roll_down,
                )?;
            }
            AttributionFactor::RatesCurves => attribution.rates_curves_pnl = factor_pnl,
            AttributionFactor::CreditCurves => attribution.credit_curves_pnl = factor_pnl,
            AttributionFactor::InflationCurves => attribution.inflation_curves_pnl = factor_pnl,
            AttributionFactor::Correlations => attribution.correlations_pnl = factor_pnl,
            AttributionFactor::Fx => {
                attribution.fx_pnl = factor_pnl;
                // Stamp FX policy when FX factor is applied
                let target_ccy = attribution.fx_pnl.currency();
                stamp_fx_policy(
                    &mut attribution,
                    target_ccy,
                    "Waterfall FX attribution with full translation",
                );
            }
            AttributionFactor::Volatility => attribution.vol_pnl = factor_pnl,
            AttributionFactor::ModelParameters => {
                attribution.model_params_pnl = factor_pnl;
                // Add note if factor P&L is zero (likely skipped)
                if factor_pnl.amount().abs() < 1e-10 {
                    attribution
                        .meta
                        .notes
                        .push("Model parameters attribution returned zero".to_string());
                }
            }
            AttributionFactor::MarketScalars => attribution.market_scalars_pnl = factor_pnl,
        }
    }

    // Populate credit_factor_detail when the cascade ran.
    if let (Some(c), Some(model)) = (&cascade, credit_factor_model) {
        if !credit_step_pnls.is_empty() {
            let detail = build_credit_factor_attribution(
                model,
                c,
                credit_factor_detail_options,
                &credit_step_pnls,
            );
            attribution.credit_factor_detail = Some(detail);
        }
    }

    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "waterfall",
        ctx.num_repricings(),
        0.01,
        0.001, // Waterfall should have very small residual
    );

    Ok(attribution)
}

struct WaterfallContext<'a> {
    target_instrument: &'a Arc<dyn Instrument>,
    current_instrument: Arc<dyn Instrument>,
    current_market: MarketContext,
    current_val: Money,
    market_t1: &'a MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    strict_validation: bool,
    num_repricings: usize,
}

impl<'a> WaterfallContext<'a> {
    fn num_repricings(&self) -> usize {
        self.num_repricings
    }

    /// Apply the credit cascade as a sequence of per-step bumps replacing the
    /// single CreditCurves step. Returns the *aggregate* credit P&L (sum of
    /// all step P&Ls); per-step amounts are appended to `step_pnls` in order.
    fn apply_credit_cascade(
        &mut self,
        cascade: &CreditCascade,
        step_pnls: &mut Vec<Money>,
    ) -> Result<Money> {
        let _span = tracing::info_span!("waterfall_credit_cascade").entered();
        let base_currency = self.current_val.currency();
        let mut total = Money::new(0.0, base_currency);

        // Credit base: the running market *before* the credit cascade — its
        // credit curves are still T0. Par-spread re-bootstrap bumps do not
        // compose cleanly under chaining, so each parallel step's market is
        // built as a SINGLE par-spread bump of the cumulative bp from this
        // fixed base (not chained off the previous step's re-bootstrapped
        // curve). The marginal step P&L `val(cumᵏ) − val(cumᵏ⁻¹)` still
        // telescopes to `val(T1 credit) − val(T0 credit) ≡ credit_curves_pnl`.
        let credit_base = self.current_market.clone();
        let discount_id = cascade.discount_curve_id.as_ref();
        let mut cumulative_bp = 0.0_f64;

        for (idx, step) in cascade.steps.iter().enumerate() {
            let prev_val = self.current_val;
            let new_market = match step.kind {
                CreditStepKind::CurveShape => {
                    // Snap to T1 hazard. After the parallel Generic / Level /
                    // Adder bumps this step absorbs whatever non-parallel
                    // (steepening / twist) residual remains, and makes the
                    // cascade end-state match the legacy single Credit step
                    // exactly so `Σ steps ≡ credit_curves_pnl` still holds.
                    snap_hazard_to_t1(&credit_base, self.market_t1, &cascade.hazard_curve_ids)
                }
                // Generic / Level(k) / Adder all apply a *parallel* par-spread
                // bp bump; accumulate and re-bootstrap once from the fixed base.
                CreditStepKind::Generic | CreditStepKind::Level(_) | CreditStepKind::Adder => {
                    cumulative_bp += step.delta_bp;
                    shift_credit_curves_par_spread(
                        &credit_base,
                        &cascade.hazard_curve_ids,
                        discount_id,
                        cumulative_bp,
                    )?
                }
            };
            let new_val = reprice_instrument(&self.current_instrument, &new_market, self.as_of_t1)?;
            self.num_repricings += 1;
            let step_pnl =
                compute_pnl(prev_val, new_val, base_currency, &new_market, self.as_of_t1)?;
            step_pnls.push(step_pnl);
            total = total.checked_add(step_pnl).map_err(|_| {
                finstack_core::Error::Validation(
                    "Currency mismatch summing credit cascade steps".to_string(),
                )
            })?;
            self.current_market = new_market;
            self.update_current_value(prev_val, step_pnl)?;
            tracing::trace!(
                cascade_step = idx,
                step_label = %step.label,
                delta_bp = step.delta_bp,
                step_pnl = step_pnl.amount(),
                "applied credit cascade step"
            );
        }
        Ok(total)
    }

    fn apply_factor(&mut self, factor: &AttributionFactor) -> Result<Money> {
        let _span = tracing::info_span!("waterfall_factor", factor = %factor).entered();
        let prev_val = self.current_val;
        let base_currency = prev_val.currency();

        if matches!(factor, AttributionFactor::ModelParameters) {
            return self.apply_model_params(prev_val, base_currency, factor);
        }

        // Carry only changes the date, not the market — skip the clone
        if matches!(factor, AttributionFactor::Carry) {
            let new_val = reprice_instrument(
                &self.current_instrument,
                &self.current_market,
                self.as_of_t1,
            )?;
            self.num_repricings += 1;
            let factor_pnl = compute_pnl(
                prev_val,
                new_val,
                base_currency,
                &self.current_market,
                self.as_of_t1,
            )?;
            self.update_current_value(prev_val, factor_pnl)?;
            return Ok(factor_pnl);
        }

        let new_market = self.build_market_for_factor(factor)?;
        let new_val = reprice_instrument(&self.current_instrument, &new_market, self.as_of_t1)?;
        self.num_repricings += 1;

        let factor_pnl = if matches!(factor, AttributionFactor::Fx) {
            compute_pnl_with_fx(
                prev_val,
                new_val,
                base_currency,
                &self.current_market,
                &new_market,
                self.as_of_t0,
                self.as_of_t1,
            )?
        } else {
            compute_pnl(prev_val, new_val, base_currency, &new_market, self.as_of_t1)?
        };

        self.current_market = new_market;
        self.update_current_value(prev_val, factor_pnl)?;
        Ok(factor_pnl)
    }

    fn apply_model_params(
        &mut self,
        prev_val: Money,
        base_currency: Currency,
        factor: &AttributionFactor,
    ) -> Result<Money> {
        match reprice_instrument(self.target_instrument, &self.current_market, self.as_of_t1) {
            Ok(new_val) => {
                self.num_repricings += 1;
                let factor_pnl = compute_pnl(
                    prev_val,
                    new_val,
                    base_currency,
                    &self.current_market,
                    self.as_of_t1,
                )?;
                self.current_instrument = Arc::clone(self.target_instrument);
                self.update_current_value(prev_val, factor_pnl)?;
                Ok(factor_pnl)
            }
            Err(e) => {
                if self.strict_validation {
                    return Err(e);
                }
                tracing::warn!(
                    error = %e,
                    factor = ?factor,
                    instrument_id = %self.target_instrument.id(),
                    "Waterfall attribution: repricing with T₁ model parameters failed, returning zero P&L"
                );
                Ok(Money::new(0.0, base_currency))
            }
        }
    }

    fn build_market_for_factor(&self, factor: &AttributionFactor) -> Result<MarketContext> {
        match factor {
            AttributionFactor::Carry => Ok(self.current_market.clone()),
            AttributionFactor::RatesCurves => {
                let rates_t1 = MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::RATES);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &rates_t1,
                    CurveRestoreFlags::RATES,
                ))
            }
            AttributionFactor::CreditCurves => {
                let credit_t1 = MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::CREDIT);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &credit_t1,
                    CurveRestoreFlags::CREDIT,
                ))
            }
            AttributionFactor::InflationCurves => {
                let inflation_t1 =
                    MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::INFLATION);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &inflation_t1,
                    CurveRestoreFlags::INFLATION,
                ))
            }
            AttributionFactor::Correlations => {
                let corr_t1 =
                    MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::CORRELATION);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &corr_t1,
                    CurveRestoreFlags::CORRELATION,
                ))
            }
            AttributionFactor::Fx => {
                let fx_t1 = MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::FX);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &fx_t1,
                    CurveRestoreFlags::FX,
                ))
            }
            AttributionFactor::Volatility => {
                let vol_t1 = MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::VOL);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &vol_t1,
                    CurveRestoreFlags::VOL,
                ))
            }
            AttributionFactor::MarketScalars => {
                let scalars_t1 =
                    MarketSnapshot::extract(self.market_t1, CurveRestoreFlags::SCALARS);
                Ok(MarketSnapshot::restore_market(
                    &self.current_market,
                    &scalars_t1,
                    CurveRestoreFlags::SCALARS,
                ))
            }
            AttributionFactor::ModelParameters => Err(Error::internal(
                "model parameter restoration is not implemented for attribution waterfall",
            )),
        }
    }

    /// Update the current accumulated value by adding a factor's P&L delta.
    ///
    /// # Numerical Precision (audit rec #7)
    ///
    /// The audit recommended a Kahan/Neumaier-compensated accumulator on the
    /// suspicion that the ~9·ε drift of naive `f64 +` would matter at $1B+
    /// notional. On closer inspection that drift does not exist on this code
    /// path: `Money::checked_add` is backed by `rust_decimal::Decimal`
    /// arithmetic, which is **exact** within its 28-digit precision envelope.
    /// The only IEEE-754 rounding step is the single `Decimal → f64`
    /// conversion when a downstream consumer calls `.amount()`. Compensating
    /// a chain of decimal additions for f64 rounding errors that don't occur
    /// would be net noise.
    ///
    /// If a future change replaces the `Money` storage with `f64`, or
    /// inserts an f64 short-circuit on this hot path, the right fix would be
    /// `finstack_core::math::NeumaierAccumulator` on the amount channel with
    /// the currency check still gating the operation.
    fn update_current_value(&mut self, prev_val: Money, delta: Money) -> Result<()> {
        self.current_val = prev_val
            .checked_add(delta)
            .map_err(|_| Error::Validation("Currency mismatch in waterfall".to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use test_utils::TestInstrument;
    use time::macros::date;

    #[test]
    fn test_default_waterfall_order() {
        let order = default_waterfall_order();
        assert_eq!(order.len(), 9);
        assert_eq!(order[0], AttributionFactor::Carry);
        assert_eq!(order[1], AttributionFactor::RatesCurves);
    }

    #[test]
    fn test_waterfall_requires_order() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        let instrument: Arc<dyn finstack_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-001", Money::new(1000.0, Currency::USD)),
        );

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = FinstackConfig::default();

        // Empty order should fail
        let result = attribute_pnl_waterfall(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            vec![],
            false, // strict validation off
            None,
        );

        assert!(result.is_err());
    }

    /// Audit rec #7 verification: confirm that Money's Decimal-backed
    /// arithmetic does not drift over many small same-currency adds, even at
    /// large notional. The audit recommendation of a Kahan/Neumaier
    /// accumulator turned out to be redundant because `Money::checked_add` is
    /// exact within the Decimal envelope — this test pins that invariant so a
    /// future Decimal→f64 substitution would be caught.
    #[test]
    fn waterfall_money_decimal_accumulator_is_exact_at_large_notional() {
        use finstack_core::market_data::context::MarketContext;
        let market = MarketContext::new();
        let instrument: Arc<dyn finstack_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-DEC", Money::new(0.0, Currency::USD)),
        );
        let mut ctx = WaterfallContext {
            target_instrument: &instrument,
            current_instrument: Arc::clone(&instrument),
            current_market: market.clone(),
            current_val: Money::new(1.0e9, Currency::USD),
            market_t1: &market,
            as_of_t0: date!(2025 - 01 - 15),
            as_of_t1: date!(2025 - 01 - 16),
            strict_validation: false,
            num_repricings: 0,
        };

        // Apply 30 increments of 0.1; ideal answer is exactly 1_000_000_003.
        for _ in 0..30 {
            let prev = ctx.current_val;
            ctx.update_current_value(prev, Money::new(0.1, Currency::USD))
                .expect("same-currency add must succeed");
        }

        // Decimal arithmetic is exact for these inputs; the only rounding
        // step is the final Decimal → f64 conversion, bounded by ~1 ULP at
        // 1e9 (≈ 1.2e-7). Allow 1e-6 to absorb that single rounding.
        let got = ctx.current_val.amount();
        assert!(
            (got - 1_000_000_003.0).abs() < 1e-6,
            "Money/Decimal accumulator unexpectedly drifted: got {got}, want 1_000_000_003"
        );
    }

    /// Audit rec #7: currency mismatch must still surface as a hard error —
    /// the waterfall accumulator must never silently coerce currencies.
    #[test]
    fn waterfall_currency_mismatch_still_errors() {
        use finstack_core::market_data::context::MarketContext;
        let market = MarketContext::new();
        let instrument: Arc<dyn finstack_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-CCY", Money::new(0.0, Currency::USD)),
        );
        let mut ctx = WaterfallContext {
            target_instrument: &instrument,
            current_instrument: Arc::clone(&instrument),
            current_market: market.clone(),
            current_val: Money::new(100.0, Currency::USD),
            market_t1: &market,
            as_of_t0: date!(2025 - 01 - 15),
            as_of_t1: date!(2025 - 01 - 16),
            strict_validation: false,
            num_repricings: 0,
        };

        let result = ctx.update_current_value(ctx.current_val, Money::new(10.0, Currency::EUR));
        assert!(result.is_err(), "mixed-currency add must fail");
    }
}
