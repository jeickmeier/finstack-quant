//! IR Future Option metrics module.
//!
//! Registers standard option greeks (Delta, Gamma, Vega, Theta) and DV01
//! with the metric registry for `IrFutureOption`.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Analytic parallel rate DV01 for `IrFutureOption`.
///
/// The futures price is an exogenous market quote, not derived from a curve,
/// so a generic curve-bump DV01 only reprices the premium discount factor and
/// misses the dominant rate exposure: a +1bp parallel move shifts the futures
/// price by −0.01 (price = 100 − rate). This calculator delegates to
/// [`crate::instruments::IrFutureOption::analytic_rate_dv01`], which includes
/// both the delta and the discounting channel.
#[derive(Debug, Clone, Default)]
struct IrFutureOptionDv01Calculator;

impl MetricCalculator for IrFutureOptionDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let option: &crate::instruments::IrFutureOption = context.instrument_as()?;
        option.analytic_rate_dv01(&context.curves, context.as_of)
    }
}

/// Register IR Future Option metrics with the registry.
///
/// Note: `BucketedDv01` is intentionally NOT registered. Key-rate bumps only
/// reprice the premium discount factor (the futures price is an exogenous
/// quote), so a bucketed profile would report near-zero risk in every bucket
/// and materially understate rate exposure. Use the parallel `Dv01` (analytic,
/// delta-inclusive) and `Delta` instead.
pub(crate) fn register_ir_future_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::IrFutureOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::vega()),
            (Theta, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::theta()),
            (Dv01, IrFutureOptionDv01Calculator),
        ]
    }
}
