use crate::instruments::fixed_income::bond::pricing::engine::tree::{bond_tree_config, TreePricer};
use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
use crate::instruments::fixed_income::bond::pricing::settlement::settlement_date;
use crate::instruments::fixed_income::bond::CallPutSchedule;
use crate::instruments::Bond;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

#[derive(Debug, Clone, Default)]
pub(crate) struct BondVegaCalculator;

fn has_embedded_options(bond: &Bond) -> bool {
    bond.call_put
        .as_ref()
        .map(|cp| cp.has_options())
        .unwrap_or(false)
}

fn resolve_oas_decimal(bond: &Bond, context: &MetricContext) -> finstack_core::Result<f64> {
    if let Some(oas) = context.computed.get(&MetricId::Oas) {
        return Ok(*oas);
    }
    if let Some(oas) = bond.pricing_overrides.market_quotes.quoted_oas {
        return Ok(oas);
    }
    if let Some(clean_price) = bond.pricing_overrides.market_quotes.quoted_clean_price {
        let pricer = TreePricer::with_config(bond_tree_config(bond)?);
        return Ok(pricer.calculate_oas(
            bond,
            context.curves.as_ref(),
            context.as_of,
            clean_price,
        )? / 10_000.0);
    }
    Ok(0.0)
}

fn holder_option_value_at_vol(
    bond: &Bond,
    context: &MetricContext,
    oas_decimal: f64,
    volatility: f64,
) -> finstack_core::Result<f64> {
    let mut bumped = bond.clone();
    bumped.pricing_overrides.market_quotes.implied_volatility = Some(volatility);
    let quote_date = settlement_date(&bumped, context.as_of)?;
    let price_with_options =
        price_from_oas(&bumped, context.curves.as_ref(), quote_date, oas_decimal)?;
    bumped.call_put = Some(CallPutSchedule::default());
    let price_straight = price_from_oas(&bumped, context.curves.as_ref(), quote_date, oas_decimal)?;
    Ok(price_with_options - price_straight)
}

impl MetricCalculator for BondVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        if !has_embedded_options(bond) {
            return Ok(0.0);
        }

        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let bump = defaults.vol_bump_pct;
        let base_vol = bond_tree_config(bond)?.volatility;
        let source_vol = bond
            .pricing_overrides
            .market_quotes
            .implied_volatility
            .filter(|source_vol| source_vol.is_finite() && *source_vol > 0.0);
        let model_bump = source_vol.map_or(bump, |source_vol| bump * base_vol / source_vol);
        let down_vol = (base_vol - model_bump).max(1e-8);
        let up_vol = base_vol + model_bump;
        let oas_decimal = resolve_oas_decimal(bond, context)?;

        let up = holder_option_value_at_vol(bond, context, oas_decimal, up_vol)?;
        let down = holder_option_value_at_vol(bond, context, oas_decimal, down_vol)?;
        let effective_model_bump = up_vol - down_vol;
        if effective_model_bump.abs() < f64::EPSILON {
            return Ok(0.0);
        }

        // Bloomberg OAS screens display bond vega in price points for a 1 vol point
        // move in the source volatility quote. When a source implied vol is
        // provided alongside a converted tree vol, `model_bump` maps that quote
        // bump into the tree's volatility units.
        let source_width = source_vol
            .filter(|_| base_vol.is_finite() && base_vol.abs() > f64::EPSILON)
            .map_or(effective_model_bump, |source_vol| {
                effective_model_bump * source_vol / base_vol
            });
        if source_width.abs() < f64::EPSILON {
            return Ok(0.0);
        }

        Ok((up - down) / bond.notional.amount() * 100.0 / source_width * 0.01)
    }
}
