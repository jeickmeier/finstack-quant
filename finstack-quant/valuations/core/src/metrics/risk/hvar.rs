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
}

/// Generic Historical VaR calculator that works with any instrument.
///
/// This calculator integrates Historical VaR into the standard metrics
/// framework. It requires a `MarketHistory` to be provided at the pricing
/// boundary (see [`crate::instruments::Instrument::price_with_metrics`]).
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
/// use finstack_quant_valuations_core::metrics::risk::GenericHVar;
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
