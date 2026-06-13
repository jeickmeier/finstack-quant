use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_core::dates::Date;
use finstack_core::money::Money;
use std::borrow::Cow;

/// Calculates yield-basis DV01 for bonds.
///
/// This differs from the generic `MetricId::Dv01` risk metric:
/// - `Dv01`: parallel curve-bump sensitivity to the market discount/projection curves
/// - `YieldDv01`: price sensitivity to a 1bp move in the bond's own quoted yield
///
/// For straight bonds, this is the direct dollar analogue of modified duration:
/// `YieldDv01 = -Price_yield_basis * ModifiedDuration * 1bp`.
///
/// For optioned bonds on the default Workout basis, this uses the same workout
/// yield/cashflow path as `DurationMod`; CallableOas remains an explicit opt-in
/// curve-bump basis.
pub(crate) struct YieldDv01Calculator;

pub(crate) fn yield_basis_dv01(
    bond: &Bond,
    context: &MetricContext,
    duration_mod: f64,
    ytm: f64,
) -> finstack_core::Result<f64> {
    let flows: &Vec<(Date, Money)> = context
        .cashflows
        .as_ref()
        .ok_or_else(|| crate::metrics::context_not_found("cashflows"))?;

    let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
    let (yield_rate, risk_flows, quote_date) =
        if let Some((workout_yield, workout_flows, workout_quote_date)) =
            super::quoted_workout_path(bond, context.curves.as_ref(), context.as_of, flows)?
        {
            (workout_yield, Cow::Owned(workout_flows), workout_quote_date)
        } else {
            (ytm, Cow::Borrowed(flows.as_slice()), quote_ctx.quote_date)
        };
    let price = crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_ytm(
        bond,
        risk_flows.as_ref(),
        quote_date,
        yield_rate,
    )?;

    Ok(-(price * duration_mod * 0.0001))
}

impl MetricCalculator for YieldDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        // Ytm is required alongside DurationMod for the price-from-yield repricing.
        // Declaring it explicitly keeps this calculator robust to changes in which
        // metrics are requested in the batch.
        &[MetricId::DurationMod, MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let duration_mod = context
            .computed
            .get(&MetricId::DurationMod)
            .copied()
            .ok_or_else(|| crate::metrics::metric_not_found(MetricId::DurationMod))?;
        let ytm = context
            .computed
            .get(&MetricId::Ytm)
            .copied()
            .ok_or_else(|| crate::metrics::metric_not_found(MetricId::Ytm))?;

        yield_basis_dv01(bond, context, duration_mod, ytm)
    }
}
