//! Equity-option greeks under a selected (non-native) pricing model.
//!
//! The native provider prices its Black-Scholes model only. When the metric
//! context selected another registered model, these helpers differentiate
//! that model's valuation function through the context's repricing dispatch,
//! holding the model's explicit parameters fixed: a surface bump is not a
//! Heston parameter bump or an implicit recalibration.

use crate::instruments::common_impl::traits::OptionGreekKind;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Equity's native provider evaluates Black-Scholes. Other registered models
/// must differentiate their selected valuation function, holding their explicit
/// model parameters fixed. In particular, a surface bump is not a Heston
/// parameter bump or an implicit recalibration.
pub(crate) fn selected_equity_greek(
    kind: OptionGreekKind,
    context: &mut MetricContext,
) -> Result<f64> {
    use crate::instruments::EquityOption;
    use crate::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVega, GenericFdVolga,
    };

    let option: &EquityOption = context.instrument_as()?;
    if option
        .exercise
        .as_ref()
        .is_some_and(|exercise| exercise.date <= context.as_of)
    {
        let g = option.greeks(&context.curves, context.as_of)?;
        return Ok(match kind {
            OptionGreekKind::Delta => g.delta,
            OptionGreekKind::Gamma => g.gamma,
            OptionGreekKind::Vega => g.vega,
            OptionGreekKind::Theta => g.theta,
            OptionGreekKind::Rho => g.rho,
            _ => 0.0,
        });
    }
    match kind {
        OptionGreekKind::Delta => GenericFdDelta::<EquityOption>::default().calculate(context),
        OptionGreekKind::Gamma => GenericFdGamma::<EquityOption>::default().calculate(context),
        OptionGreekKind::Vega => GenericFdVega::<EquityOption>::default().calculate(context),
        OptionGreekKind::Vanna => GenericFdVanna::<EquityOption>::default().calculate(context),
        OptionGreekKind::Volga => GenericFdVolga::<EquityOption>::default().calculate(context),
        OptionGreekKind::Theta => selected_equity_theta(context),
        OptionGreekKind::Rho => crate::metrics::UnifiedDv01Calculator::new(
            crate::metrics::Dv01CalculatorConfig::parallel_discount_only(),
        )
        .calculate(context),
        OptionGreekKind::ForeignRho => Err(finstack_quant_core::Error::Validation(
            "Foreign rho is not defined for an equity option".into(),
        )),
    }
}

fn selected_equity_theta(context: &MetricContext) -> Result<f64> {
    use std::sync::Arc;
    let option: &crate::instruments::EquityOption = context.instrument_as()?;
    if context.as_of >= option.expiry
        || option
            .exercise
            .as_ref()
            .is_some_and(|exercise| exercise.date <= context.as_of)
    {
        return Ok(0.0);
    }
    // Equity theta is per reporting day, rather than period-total carry.
    // Estimate one calendar day's selected-model roll in an isolated context
    // and normalize to the same 365/252 reporting basis as the native provider.
    // A separate context also prevents the period-carry component metrics from
    // being cached with the units of this per-day sensitivity.
    let mut overrides = context
        .get_metric_pricing_overrides()
        .cloned()
        .unwrap_or_else(|| option.metric_pricing_overrides.clone());
    let days_per_year = overrides.theta_days_per_year();
    if (option.expiry - context.as_of).whole_days() == 1 {
        // A forward stencil would cross into the observed-expiry state and
        // require an exercise observation that does not yet exist. Use a
        // backward calendar roll while both valuation points remain live.
        let previous_date = context
            .as_of
            .checked_sub(time::Duration::days(1))
            .ok_or_else(|| finstack_quant_core::Error::Validation("theta date underflow".into()))?;
        let previous_market = context.curves.roll_forward(-1)?;
        let current = context.reprice_raw(&context.curves, context.as_of)?;
        let previous = context.reprice_raw(&previous_market, previous_date)?;
        return Ok((current - previous) * 365.0 / days_per_year);
    }
    overrides.theta_period = Some(finstack_quant_core::dates::Tenor::daily());
    let mut daily = MetricContext::new(
        Arc::clone(&context.instrument),
        Arc::clone(&context.curves),
        context.as_of,
        context.base_value,
        context.config_arc(),
    );
    daily.set_pricer_dispatch(context.clone_pricer_dispatch());
    daily.set_metric_pricing_overrides(Some(overrides));
    Ok(
        crate::metrics::sensitivities::theta::GenericThetaAny.calculate(&mut daily)? * 365.0
            / days_per_year,
    )
}
