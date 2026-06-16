//! Generic Historical VaR metric calculator.
//!
//! Integrates Historical VaR into the standard metrics framework as a
//! `MetricCalculator` that can be registered and computed alongside other
//! risk metrics like DV01, Theta, etc.

use crate::metrics::core::traits::{MetricCalculator, MetricContext};
use crate::metrics::risk::{calculate_var_with_pricing, VarConfig, VarResult};
use crate::metrics::MetricId;
use finstack_quant_core::Result;

fn calculate_var_result(
    context: &mut MetricContext,
    default_config: &VarConfig,
    missing_history_message: &str,
) -> Result<VarResult> {
    let history = context.get_market_history().ok_or_else(|| {
        finstack_quant_core::Error::Validation(missing_history_message.to_string())
    })?;
    let config = context
        .get_metric_overrides()
        .and_then(|overrides| overrides.var_config.clone())
        .unwrap_or_else(|| default_config.clone());
    let (pricing_model, pricer_registry) = context.clone_pricer_dispatch();
    calculate_var_with_pricing(
        &[context.instrument.as_ref()],
        &context.curves,
        history,
        context.as_of,
        &config,
        pricing_model,
        pricer_registry,
    )
}

fn cache_num_scenarios(context: &mut MetricContext, result: &VarResult) {
    context.computed.insert(
        MetricId::custom("hvar_num_scenarios"),
        result.num_scenarios as f64,
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
/// ```ignore
/// use finstack_quant_valuations::metrics::{MetricId, MetricRegistry};
/// use finstack_quant_valuations::metrics::risk::{GenericHVar, VarConfig};
/// use std::sync::Arc;
///
/// // Create VaR calculator with 95% confidence
/// let var_calc = GenericHVar::new(VarConfig::var_95());
///
/// // Register in metric registry
/// let mut registry = MetricRegistry::new();
/// registry.register_metric(MetricId::HVar, Arc::new(var_calc), &[]);
/// ```
pub struct GenericHVar {
    config: VarConfig,
}

impl GenericHVar {
    /// Create a new VaR calculator with the given configuration.
    pub fn new(config: VarConfig) -> Self {
        Self { config }
    }
}

impl MetricCalculator for GenericHVar {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // If ES already computed (it populates HVar), return the cached value.
        if let Some(&var) = context.computed.get(&MetricId::HVar) {
            return Ok(var);
        }

        let result = calculate_var_result(
            context,
            &self.config,
            "Market history required for VaR calculation. Provide it via Instrument::price_with_metrics(...) with PricingOptions::with_market_history(...)",
        )?;

        context
            .computed
            .insert(MetricId::ExpectedShortfall, result.expected_shortfall);
        cache_num_scenarios(context, &result);

        Ok(result.var)
    }
}

/// Generic Expected Shortfall (ES / CVaR) calculator that works with any instrument.
///
/// This is the companion to [`GenericHVar`]. It computes the same historical simulation
/// distribution but returns **Expected Shortfall** as the primary metric value.
///
/// Notes:
/// - If both `MetricId::HVar` and `MetricId::ExpectedShortfall` are requested, whichever is
///   computed first will populate the other in `context.computed` so the second computation
///   will be skipped by the registry (deterministic and avoids duplicated repricing).
pub struct GenericExpectedShortfall {
    config: VarConfig,
}

impl GenericExpectedShortfall {
    /// Create a new Expected Shortfall calculator with the given configuration.
    pub fn new(config: VarConfig) -> Self {
        Self { config }
    }
}

impl MetricCalculator for GenericExpectedShortfall {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // If HVar already computed (it populates ES), return the cached value.
        if let Some(&es) = context.computed.get(&MetricId::ExpectedShortfall) {
            return Ok(es);
        }

        let result = calculate_var_result(
            context,
            &self.config,
            "Market history required for VaR/ES calculation. Provide it via Instrument::price_with_metrics(...) with PricingOptions::with_market_history(...)",
        )?;

        context.computed.insert(MetricId::HVar, result.var);
        cache_num_scenarios(context, &result);

        Ok(result.expected_shortfall)
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/metrics_risk_test_utils.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::metrics::risk::calculate_var;
    use std::sync::Arc;
    use test_utils::{history_from_rate_shifts, sample_as_of, standard_bond, usd_ois_market};
    use time::Duration;

    #[test]
    fn test_generic_hvar_creation() {
        let var_calc = GenericHVar::new(VarConfig::var_95());
        assert_eq!(var_calc.config.confidence_level, 0.95);

        let var_calc = GenericHVar::new(VarConfig::var_99());
        assert_eq!(var_calc.config.confidence_level, 0.99);

        let var_calc = GenericHVar::new(VarConfig::new(0.975));
        assert_eq!(var_calc.config.confidence_level, 0.975);
    }

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
            &[&bond],
            market.as_ref(),
            history.as_ref(),
            as_of,
            &VarConfig::var_95(),
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
        assert!(var > 0.0, "VaR should be positive");
        assert!(es >= var, "ES should be >= VaR");
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
}
