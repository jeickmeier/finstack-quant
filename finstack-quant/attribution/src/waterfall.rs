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
//!   3. Final PV should equal T₁ PV (residual ≈ 0 by construction)
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
use super::factors::*;
use super::helpers::*;
use super::model_params;
use super::types::*;
use crate::AttributionRequest;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::recalibration::RecalibrationProvider;
use std::sync::Arc;

/// Default waterfall order for factor attribution.
///
/// # Doctrine
///
/// The first factor is **always** [`AttributionFactor::Carry`]. The waterfall
/// path enforces this at the entry point (see
/// [`crate::attribute_pnl`]) so the date-roll P&L — "price today's
/// position with yesterday's market" — is the foundational first step of every
/// attribution, before any factor-level market move is layered in. Any
/// user-supplied factor order that does not start with `Carry` will be rejected
/// with an `Error::Validation`.
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
/// * `model_params_t0` - Optional opening model-parameter snapshot used to
///   isolate parameter P&L; `None` omits that source detail.
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
/// ```no_run
/// use finstack_quant_attribution::{
///     attribute_pnl, default_waterfall_order, AttributionMethod, AttributionRequest,
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
/// let request = AttributionRequest::new(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
/// );
/// let attribution = attribute_pnl(
///     &AttributionMethod::Waterfall(default_waterfall_order()),
///     &request,
/// )?;
///
/// // Residual should be minimal
/// assert!(attribution.residual_within_tolerance(0.01, 1.0));
/// # Ok(())
/// # }
/// ```
///
/// # Credit-factor cascade
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
#[tracing::instrument(skip_all, fields(instrument_id = %request.instrument.id(), method = "waterfall"))]
pub(crate) fn attribute_pnl_waterfall(
    request: &AttributionRequest<'_>,
    factor_order: Vec<AttributionFactor>,
) -> Result<PnlAttribution> {
    let AttributionRequest {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        strict_validation,
        model_params_t0,
        credit_factor_model,
        credit_factor_detail_options,
        prepared_endpoints,
        ..
    } = *request;
    if factor_order.is_empty() {
        return Err(Error::Validation(
            "Waterfall attribution requires non-empty factor_order".to_string(),
        ));
    }
    // Doctrine: the date-roll Carry — pricing today's position with yesterday's
    // market — is the foundational first step of every attribution. Reject any
    // ordering that does not start with `Carry` so the doctrine is enforced
    // rather than convention-only. (See `default_waterfall_order` doc.)
    if factor_order[0] != AttributionFactor::Carry {
        return Err(Error::Validation(format!(
            "Waterfall attribution requires factor_order[0] == AttributionFactor::Carry \
             (yesterday's market rolled forward is always the first step); got {:?}",
            factor_order[0]
        )));
    }
    // Reject duplicate factors: factor P&L is recorded by assignment, so a
    // repeated factor would find the market already rolled, measure ~0, and
    // silently overwrite the genuine first-pass P&L (the real move would leak
    // into the residual); a repeated Carry would additionally double-count
    // coupon income into `total_pnl` via `apply_total_return_carry`.
    {
        let mut seen = std::collections::HashSet::new();
        for factor in &factor_order {
            if !seen.insert(factor) {
                return Err(Error::Validation(format!(
                    "Waterfall attribution factor_order contains duplicate factor {factor:?}; \
                     each factor may appear at most once"
                )));
            }
        }
    }
    validate_attribution_period(as_of_t0, as_of_t1)?;

    // Step 1: Price at T₀
    // Use T₀ model parameters for T₀ valuation if available
    let instrument_t0 = if let Some(params) = model_params_t0 {
        model_params::with_model_params(instrument, params)?
    } else {
        Arc::clone(instrument)
    };
    let (val_t0, val_t1) = if let Some(endpoints) = prepared_endpoints {
        endpoints
    } else {
        (
            reprice_instrument(&instrument_t0, market_t0, as_of_t0)?,
            reprice_instrument(instrument, market_t1, as_of_t1)?,
        )
    };

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
        Some(config),
    );
    // Policy-visibility invariant: stamp the execution policy the
    // attribution ran under (workspace rule: results carry the parallel flag).
    attribution.meta.execution_policy = Some(ExecutionPolicy::Serial);
    // Waterfall factor P&Ls are path-dependent; stamp the executed order.
    attribution.meta.notes.push(format!(
        "waterfall order: {}",
        factor_order
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" -> ")
    ));

    // Plan a credit cascade if a model was supplied. Use the canonical single
    // CreditCurves step when planning yields None (no issuer tag, no hazard
    // dependencies, etc.).
    let cascade: Option<CreditCascade> = match credit_factor_model {
        Some(model) => plan_credit_cascade(model, instrument, market_t0, market_t1)?,
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
    let mut credit_step_pnls: Vec<finstack_quant_core::money::Money> = Vec::new();

    // Build hybrid market: start with all T₀, progressively apply T₁
    let mut ctx = WaterfallContext {
        target_instrument: instrument,
        current_instrument: instrument_t0,
        source_market: market_t0,
        current_market: market_t0.clone(),
        current_val: val_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        strict_validation,
        num_repricings: 2, // T₀ and T₁ repricings already performed
        factor_use: InstrumentFactorUse::of(instrument.as_ref()),
        recalibration_provider: Arc::new(CachedRecalibrationProvider::new()),
    };

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

        match factor {
            AttributionFactor::Carry => {
                let theta = factor_pnl;
                let carry_inputs = total_return_carry_inputs(
                    ctx.current_instrument.as_ref(),
                    &ctx.current_market,
                    ctx.as_of_t0,
                    ctx.as_of_t1,
                    factor_pnl.currency(),
                )?;
                // Merge diagnostics BEFORE moving carry_inputs into apply_total_return_carry.
                for w in &carry_inputs.warnings {
                    attribution.meta.notes.push(w.clone());
                }
                // `total_return_carry_inputs` performed extra `price_with_metrics`
                // repricings (Accrued×2, YTM, flat-curve value×2) — count one.
                ctx.count_extra_repricing();

                apply_total_return_carry(&mut attribution, theta, carry_inputs)?;
            }
            AttributionFactor::RatesCurves => attribution.rates_curves_pnl = factor_pnl,
            AttributionFactor::CreditCurves => attribution.credit_curves_pnl = factor_pnl,
            AttributionFactor::InflationCurves => attribution.inflation_curves_pnl = factor_pnl,
            AttributionFactor::Correlations => attribution.correlations_pnl = factor_pnl,
            AttributionFactor::Fx => {
                attribution.fx_pnl = factor_pnl;
                // Stamp FX policy when FX factor is applied
                let target_currency = attribution.fx_pnl.currency();
                stamp_fx_policy(
                    &mut attribution,
                    target_currency,
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
            )?;
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
    factor_use: InstrumentFactorUse,
    source_market: &'a MarketContext,
    current_market: MarketContext,
    current_val: Money,
    market_t1: &'a MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    strict_validation: bool,
    num_repricings: usize,
    recalibration_provider: Arc<dyn RecalibrationProvider>,
}

impl<'a> WaterfallContext<'a> {
    fn num_repricings(&self) -> usize {
        self.num_repricings
    }

    /// Count a pricing performed outside `apply_factor` (e.g. the carry
    /// helper's `price_with_metrics` call) so `meta.num_repricings` reflects
    /// true pricing cost.
    fn count_extra_repricing(&mut self) {
        self.num_repricings += 1;
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
        let mut total = Money::from((0_i64, base_currency));

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
                    // cascade end-state match the single Credit step
                    // exactly so `Σ steps ≡ credit_curves_pnl` still holds.
                    snap_hazard_to_t1(&credit_base, self.market_t1, &cascade.hazard_curve_ids)
                }
                // Generic / Level(k) / Adder all apply a *parallel* par-spread
                // bp bump; accumulate and re-bootstrap once from the fixed base.
                CreditStepKind::Generic | CreditStepKind::Level(_) | CreditStepKind::Adder => {
                    cumulative_bp += step.delta_bp;
                    shift_credit_curves_par_spread(
                        self.source_market,
                        &credit_base,
                        &cascade.hazard_curve_ids,
                        discount_id,
                        cumulative_bp,
                        self.recalibration_provider.as_ref(),
                    )?
                }
            };
            let new_val = reprice_instrument(&self.current_instrument, &new_market, self.as_of_t1)?;
            self.num_repricings += 1;
            let step_pnl =
                compute_pnl(prev_val, new_val, base_currency, &new_market, self.as_of_t1)?;
            step_pnls.push(step_pnl);
            total = total.checked_add(step_pnl).map_err(|_| {
                finstack_quant_core::Error::Validation(
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

        if !self.factor_use.uses_attribution_factor(factor) {
            return Ok(Money::from((0_i64, base_currency)));
        }

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
                Ok(Money::from((0_i64, base_currency)))
            }
        }
    }

    fn build_market_for_factor(&self, factor: &AttributionFactor) -> Result<MarketContext> {
        let flags = match factor {
            AttributionFactor::Carry => return Ok(self.current_market.clone()),
            AttributionFactor::RatesCurves => MarketRestoreFlags::RATES,
            AttributionFactor::CreditCurves => MarketRestoreFlags::CREDIT,
            AttributionFactor::InflationCurves => MarketRestoreFlags::INFLATION,
            AttributionFactor::Correlations => MarketRestoreFlags::CORRELATION,
            AttributionFactor::Fx => MarketRestoreFlags::FX,
            AttributionFactor::Volatility => MarketRestoreFlags::VOL,
            AttributionFactor::MarketScalars => MarketRestoreFlags::SCALARS,
            AttributionFactor::ModelParameters => {
                return Err(Error::internal(
                    "model parameter restoration is not implemented for attribution waterfall",
                ))
            }
        };
        let dependencies = self.current_instrument.market_dependencies()?;
        let family_t1 = MarketSnapshot::extract_with_credit_roles(
            self.market_t1,
            flags,
            &dependencies.curves.credit_curves,
        );
        Ok(MarketSnapshot::restore_market(
            &self.current_market,
            &family_t1,
            flags,
        ))
    }

    /// Add a factor's P&L delta using `Money`'s decimal arithmetic.
    ///
    /// Accumulation is exact within the decimal precision envelope; a currency
    /// mismatch returns a validation error.
    fn update_current_value(&mut self, prev_val: Money, delta: Money) -> Result<()> {
        self.current_val = prev_val
            .checked_add(delta)
            .map_err(|_| Error::Validation("Currency mismatch in waterfall".to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
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

        let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-001", Money::from((1000_i64, Currency::USD))),
        );

        let market_t0 = MarketContext::new();
        let market_t1 = MarketContext::new();
        let config = FinstackConfig::default();

        // Empty order should fail
        let result = crate::attribute_pnl(
            &AttributionMethod::Waterfall(vec![]),
            &crate::AttributionRequest {
                strict_validation: false,
                model_params_t0: None,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &config,
                )
            },
        );

        assert!(result.is_err());
    }

    /// Confirm that Money's Decimal-backed arithmetic does not drift over
    /// many small same-currency adds, even at large notional. A Kahan/Neumaier
    /// accumulator turned out to be redundant because `Money::checked_add` is
    /// exact within the Decimal envelope — this test pins that invariant so a
    /// future Decimal→f64 substitution would be caught.
    #[test]
    fn waterfall_money_decimal_accumulator_is_exact_at_large_notional() {
        use finstack_quant_core::market_data::context::MarketContext;
        let market = MarketContext::new();
        let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-DEC", Money::from((0_i64, Currency::USD))),
        );
        let mut ctx = WaterfallContext {
            target_instrument: &instrument,
            current_instrument: Arc::clone(&instrument),
            factor_use: InstrumentFactorUse::of(instrument.as_ref()),
            source_market: &market,
            current_market: market.clone(),
            current_val: Money::new(1.0e9, Currency::USD).expect("valid money fixture"),
            market_t1: &market,
            as_of_t0: date!(2025 - 01 - 15),
            as_of_t1: date!(2025 - 01 - 16),
            strict_validation: false,
            num_repricings: 0,
            recalibration_provider: Arc::new(CachedRecalibrationProvider::new()),
        };

        // Apply 30 increments of 0.1; ideal answer is exactly 1_000_000_003.
        for _ in 0..30 {
            let prev = ctx.current_val;
            ctx.update_current_value(
                prev,
                Money::new(0.1, Currency::USD).expect("valid money fixture"),
            )
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
        use finstack_quant_core::market_data::context::MarketContext;
        let market = MarketContext::new();
        let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> = Arc::new(
            TestInstrument::new("TEST-CCY", Money::from((0_i64, Currency::USD))),
        );
        let mut ctx = WaterfallContext {
            target_instrument: &instrument,
            current_instrument: Arc::clone(&instrument),
            factor_use: InstrumentFactorUse::of(instrument.as_ref()),
            source_market: &market,
            current_market: market.clone(),
            current_val: Money::from((100_i64, Currency::USD)),
            market_t1: &market,
            as_of_t0: date!(2025 - 01 - 15),
            as_of_t1: date!(2025 - 01 - 16),
            strict_validation: false,
            num_repricings: 0,
            recalibration_provider: Arc::new(CachedRecalibrationProvider::new()),
        };

        let result =
            ctx.update_current_value(ctx.current_val, Money::from((10_i64, Currency::EUR)));
        assert!(result.is_err(), "mixed-currency add must fail");
    }
}
