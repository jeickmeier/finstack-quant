//! Forward curve PV01 for interest rate options (per 1bp parallel bump of forward curve).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::Result;

/// Forward PV01 calculator (per 1bp parallel forward curve bump)
pub(crate) struct ForwardPv01Calculator;

impl MetricCalculator for ForwardPv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &CapFloor = context.instrument_as()?;

        // Base PV from context
        let base = context.base_value.amount();

        // Get the original forward curve
        let original_fwd = context.curves.get_forward(&option.forward_curve_id)?;

        // Use shared sensitivity config to keep forward PV01 bump aligned with DV01 settings.
        let bump_bp = crate::metrics::resolve_sensitivities_config(context.config())?.rate_bump_bp;
        let bump_amount = bump_bp * 0.0001;

        let bumped_rates: Vec<(f64, f64)> = original_fwd
            .knots()
            .iter()
            .copied()
            .zip(original_fwd.forwards().iter().copied())
            .map(|(t, r)| (t, r + bump_amount))
            .collect();

        // Build bumped curve with ORIGINAL ID so instrument can find it
        let bumped_fwd =
            ForwardCurve::builder(option.forward_curve_id.clone(), original_fwd.tenor())
                .base_date(original_fwd.base_date())
                .reset_lag(original_fwd.reset_lag())
                .day_count(original_fwd.day_count())
                .knots(bumped_rates)
                .build()?;

        // Create new context with bumped curve (replaces original with same ID)
        let bumped_ctx = context.curves.as_ref().clone().insert(bumped_fwd);

        // Reprice with bumped forward curve
        let bumped = option.value(&bumped_ctx, context.as_of)?;

        // Normalize to per-1bp semantics even when configured bump differs.
        Ok((bumped.amount() - base) / bump_bp)
    }
}
