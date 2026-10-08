//! Generic Historical VaR metric calculator.
//!
//! Integrates Historical VaR into the standard metrics framework as a
//! `MetricCalculator` that can be registered and computed alongside other
//! risk metrics like DV01, Theta, etc.

use crate::metrics::core::traits::{MetricCalculator, MetricContext};
use crate::metrics::risk::{calculate_var, VarConfig, VarResult};
use crate::metrics::MetricId;
use finstack_quant_core::Result;

fn calculate_var_result(
    context: &mut MetricContext,
    missing_history_message: &str,
) -> Result<VarResult> {
    let history = context.get_market_history().ok_or_else(|| {
        finstack_quant_core::Error::Validation(missing_history_message.to_string())
    })?;
    let config = context
        .get_metric_pricing_overrides()
        .and_then(|overrides| overrides.var_config.clone())
        .unwrap_or_else(VarConfig::var_95);
    let dispatch = context.clone_pricer_dispatch();
    calculate_var(
        &[context.instrument.as_ref()],
        &context.curves,
        history,
        context.as_of,
        &config,
        dispatch,
        context.get_recalibration_provider(),
    )
}

fn cache_var_diagnostics(context: &mut MetricContext, result: &VarResult) {
    context.computed.insert(
        MetricId::custom("hvar_num_scenarios"),
        result.num_scenarios as f64,
    );
    // Taylor-approximation degradation flags (0.0/1.0). Bindings consume only
    // the measures map, so this is the sole channel through which a caller can
    // see that FX/vol scenario components were skipped and VaR understates
    // that risk (see `VarResult::skipped_fx` / `skipped_vol`).
    context.computed.insert(
        MetricId::custom("hvar_skipped_fx"),
        f64::from(result.skipped_fx),
    );
    context.computed.insert(
        MetricId::custom("hvar_skipped_vol"),
        f64::from(result.skipped_vol),
    );
}

/// Generic Historical VaR calculator that works with any instrument.
///
/// This calculator integrates Historical VaR into the standard metrics
/// framework. It requires a `MarketHistory` to be provided at the pricing
/// boundary (see [`crate::instruments::common_impl::traits::Instrument::price_with_metrics`]).
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::metrics::{MetricId, MetricRegistry};
/// use finstack_quant_valuations::metrics::risk::GenericHVar;
/// use std::sync::Arc;
///
/// // Configuration comes from PricingOptions metric overrides (95% by default).
/// let var_calc = GenericHVar;
///
/// // Register in metric registry
/// let mut registry = MetricRegistry::new();
/// registry.register_metric(MetricId::HVar, Arc::new(var_calc), &[]);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct GenericHVar;

impl MetricCalculator for GenericHVar {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // If ES already computed (it populates HVar), return the cached value.
        if let Some(&var) = context.computed.get(&MetricId::HVar) {
            return Ok(var);
        }

        let result = calculate_var_result(
            context,
            "Market history required for VaR calculation. Provide it via Instrument::price_with_metrics(...) with PricingOptions::with_market_history(...)",
        )?;

        context
            .computed
            .insert(MetricId::ExpectedShortfall, result.expected_shortfall);
        cache_var_diagnostics(context, &result);

        Ok(result.var)
    }
}

/// Generic Expected Shortfall (ES / CVaR) calculator that works with any instrument.
///
/// This is the companion to [`GenericHVar`]. It computes the same historical simulation
/// distribution but returns **Expected Shortfall** as the primary metric value.
///
/// Configuration is resolved once from the pricing request; without an override
/// both metrics use the same 95% configuration.
///
/// Notes:
/// - If both `MetricId::HVar` and `MetricId::ExpectedShortfall` are requested, whichever is
///   computed first will populate the other in `context.computed` so the second computation
///   will be skipped by the registry (deterministic and avoids duplicated repricing).
#[derive(Debug, Clone, Copy, Default)]
pub struct GenericExpectedShortfall;

impl MetricCalculator for GenericExpectedShortfall {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // If HVar already computed (it populates ES), return the cached value.
        if let Some(&es) = context.computed.get(&MetricId::ExpectedShortfall) {
            return Ok(es);
        }

        let result = calculate_var_result(
            context,
            "Market history required for VaR/ES calculation. Provide it via Instrument::price_with_metrics(...) with PricingOptions::with_market_history(...)",
        )?;

        context.computed.insert(MetricId::HVar, result.var);
        cache_var_diagnostics(context, &result);

        Ok(result.expected_shortfall)
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/metrics_risk_test_utils.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::pricer::PricingDispatch;
    use std::sync::Arc;
    use test_utils::{history_from_rate_shifts, sample_as_of, standard_bond, usd_ois_market};
    use time::Duration;

    #[test]
    fn test_hvar_via_metrics_framework() -> Result<()> {
        let as_of = sample_as_of();
        let maturity = as_of + Duration::days(365 * 5);
        let bond = standard_bond("TEST-BOND", as_of, maturity);

        // Use enough scenarios so ES != VaR at 95% (tail size >= 2).
        let mut shifts: Vec<(finstack_quant_core::dates::Date, f64)> = Vec::new();
        for i in 1..=25_i64 {
            let d = as_of - Duration::days(i);
            // Mix signs and magnitudes to ensure a non-degenerate tail.
            let shift = if i % 2 == 0 {
                0.0004 * (i as f64)
            } else {
                -0.0003 * (i as f64)
            };
            shifts.push((d, shift));
        }
        let history = Arc::new(history_from_rate_shifts(as_of, &shifts));

        let market = Arc::new(usd_ois_market(as_of)?);

        // Compute a reference result directly from the VaR engine.
        let expected = calculate_var(
            &[&bond as &dyn Instrument],
            market.as_ref(),
            history.as_ref(),
            as_of,
            &VarConfig::var_95(),
            PricingDispatch::InstrumentDefault,
            None,
        )?;

        // Calculate VaR + ES via metrics framework
        use crate::instruments::PricingOptions;
        let opts = PricingOptions::default().with_market_history(history);
        let result_ordered = bond.price_with_metrics(
            market.as_ref(),
            as_of,
            &[MetricId::HVar, MetricId::ExpectedShortfall],
            opts,
        )?;

        let var = *result_ordered
            .measures
            .get("hvar")
            .expect("VaR should be computed");
        let es = *result_ordered
            .measures
            .get("expected_shortfall")
            .expect("ES should be computed");
        assert!(
            var < 0.0,
            "VaR should be negative (losses-negative convention)"
        );
        assert!(es <= var, "ES should lie at or beyond VaR in the loss tail");
        assert!(
            (var - expected.var).abs() < 1e-10,
            "VaR mismatch: got {var}, expected {}",
            expected.var
        );
        assert!(
            (es - expected.expected_shortfall).abs() < 1e-10,
            "ES mismatch: got {es}, expected {}",
            expected.expected_shortfall
        );
        let n = *result_ordered
            .measures
            .get("hvar_num_scenarios")
            .expect("scenario count should be exposed");
        assert_eq!(n as usize, expected.num_scenarios);

        // Also verify reversed ordering doesn't break ES vs VaR wiring.
        let history2 = Arc::new(history_from_rate_shifts(as_of, &shifts));
        let opts2 = PricingOptions::default().with_market_history(history2);
        let result_reversed = bond.price_with_metrics(
            market.as_ref(),
            as_of,
            &[MetricId::ExpectedShortfall, MetricId::HVar],
            opts2,
        )?;
        let var2 = *result_reversed
            .measures
            .get("hvar")
            .expect("VaR should be computed");
        let es2 = *result_reversed
            .measures
            .get("expected_shortfall")
            .expect("ES should be computed");
        assert!((var2 - expected.var).abs() < 1e-10);
        assert!((es2 - expected.expected_shortfall).abs() < 1e-10);

        Ok(())
    }

    #[test]
    fn request_config_applies_to_both_historical_risk_metrics_in_either_order() -> Result<()> {
        let as_of = sample_as_of();
        let mut bond = standard_bond("RISK-CONFIG", as_of, as_of + Duration::days(365 * 5));
        let config = VarConfig::var_99();
        bond.metric_pricing_overrides.var_config = Some(config.clone());
        let shifts: Vec<_> = (1..=101)
            .map(|i| (as_of - Duration::days(i), i as f64 * 0.0001))
            .collect();
        let history = Arc::new(history_from_rate_shifts(as_of, &shifts));
        let market = usd_ois_market(as_of)?;
        let expected = calculate_var(
            &[&bond as &dyn Instrument],
            &market,
            &history,
            as_of,
            &config,
            PricingDispatch::InstrumentDefault,
            None,
        )?;
        let default = calculate_var(
            &[&bond as &dyn Instrument],
            &market,
            &history,
            as_of,
            &VarConfig::var_95(),
            PricingDispatch::InstrumentDefault,
            None,
        )?;
        assert_ne!(expected.var, default.var);
        for metrics in [
            [MetricId::HVar, MetricId::ExpectedShortfall],
            [MetricId::ExpectedShortfall, MetricId::HVar],
        ] {
            let result = bond.price_with_metrics(
                &market,
                as_of,
                &metrics,
                crate::instruments::PricingOptions::default()
                    .with_market_history(Arc::clone(&history)),
            )?;
            assert!((result.measures["hvar"] - expected.var).abs() < 1e-10);
            assert!(
                (result.measures["expected_shortfall"] - expected.expected_shortfall).abs() < 1e-10
            );
        }
        Ok(())
    }
}
