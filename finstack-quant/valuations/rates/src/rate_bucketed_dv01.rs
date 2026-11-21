use finstack_quant_core::Result;
use finstack_quant_core::market_data::term_structures::RateCalibrationCurveRole;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_valuations_core::instruments::RatesCurveKind;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
    Dv01CalculatorConfig, UnifiedDv01Calculator, rate_quote_dv01_series,
};
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricId, sensitivity_central_diff,
};
use finstack_quant_valuations_core::recalibration::{QuoteBump, RateMarketRecalibrationRequest};
use std::sync::Arc;

pub(crate) struct RateDv01(pub(crate) bool);

impl MetricCalculator for RateDv01 {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let config = if self.0 {
            Dv01CalculatorConfig::triangular_key_rate()
        } else {
            Dv01CalculatorConfig::parallel_combined()
        };
        let direct = UnifiedDv01Calculator::new(config);
        let curves = direct.collect_curves(context.instrument.as_ref(), &context.curves)?;
        let [(discount_id, RatesCurveKind::Discount), remaining @ ..] = curves.as_slice() else {
            return direct.calculate(context);
        };
        let discount = context.curves.get_discount(discount_id)?;
        let Some(discount_recipe) = discount.rate_calibration() else {
            return direct.calculate(context);
        };
        let forward = if let [(forward_id, RatesCurveKind::Forward)] = remaining {
            let curve = context.curves.get_forward(forward_id)?;
            if curve.rate_calibration().is_none()
                || (context.instrument.key()
                    == finstack_quant_valuations_core::pricer::InstrumentType::Fra
                    && curve
                        .rate_calibration()
                        .is_some_and(crate::fra::metrics::uses_basis_quotes))
            {
                return direct.calculate(context);
            }
            Some((forward_id, curve))
        } else if remaining.is_empty() {
            None
        } else {
            return direct.calculate(context);
        };
        let fra =
            context.instrument.key() == finstack_quant_valuations_core::pricer::InstrumentType::Fra;
        if fra && forward.is_none() {
            return direct.calculate(context);
        }
        let metric_name = if self.0 { "bucketed_dv01" } else { "dv01" };
        let reprice = |bump| {
            let request = match &forward {
                Some((forward_id, _)) => RateMarketRecalibrationRequest::LinkedDiscountForward {
                    market: Arc::clone(&context.curves),
                    discount_curve_id: discount_id.clone(),
                    forward_curve_id: (*forward_id).clone(),
                    bump,
                },
                None => RateMarketRecalibrationRequest::SingleOis {
                    market: Arc::clone(&context.curves),
                    curve_id: discount_id.clone(),
                    bump,
                },
            };
            let bumped =
                crate::rate_recalibration::rebuild_rate_market(context, request, metric_name)?;
            context.reprice_raw(&bumped, context.as_of)
        };
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        if !self.0 {
            return sensitivity_central_diff(
                reprice(QuoteBump::ParallelBp(bump_bp))?,
                reprice(QuoteBump::ParallelBp(-bump_bp))?,
                bump_bp,
            );
        }
        let discount_series =
            rate_quote_dv01_series(discount_id, discount_recipe, bump_bp, reprice)?;
        let mut result = vec![(discount_id.clone(), discount_series)];
        if let Some((id, curve)) = &forward
            && let Some(recipe) = curve.rate_calibration()
        {
            let shared_ois = recipe.ois_compounding.is_some()
                && matches!(
                    &recipe.role,
                    RateCalibrationCurveRole::Projection { discount_curve_id: linked, .. }
                    if linked == discount_id
                );
            if !shared_ois {
                result.push((
                    (*id).clone(),
                    rate_quote_dv01_series(id, recipe, bump_bp, reprice)?,
                ));
            }
        }
        let total = neumaier_sum(
            result
                .iter()
                .flat_map(|(_, series)| series.iter().map(|(_, value)| *value)),
        )?;
        for (id, series) in result {
            context.store_bucketed_series(
                MetricId::composite(&MetricId::BucketedDv01, &[id.as_str()]),
                series,
            );
        }
        Ok(total)
    }
}
