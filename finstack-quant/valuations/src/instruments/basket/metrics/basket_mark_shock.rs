//! Fixed one-percent relative shock to the basket's prepared marks.

use super::super::pricer::marked_constituent;
use super::super::{Basket, ConstituentReference};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;
use finstack_quant_core::money::Money;
use std::collections::BTreeMap;

pub(super) struct BasketMarkShockCalculator;

impl MetricCalculator for BasketMarkShockCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket = context.instrument_as::<Basket>()?;
        let zero = Money::from((0_i64, basket.reporting_currency));
        let mut groups = BTreeMap::new();
        let mut total = zero;
        for constituent in &basket.constituents {
            let mark = marked_constituent(basket, constituent, &context.curves, context.as_of)?;
            let shocked = mark.checked_mul_f64(1.01)?;
            if mark != zero && shocked == mark {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "basket mark shock cannot be represented for {}",
                    constituent.id
                )));
            }
            let contribution = shocked
                .checked_sub(mark)?
                .checked_mul_f64(constituent.units.unwrap_or(constituent.weight))?
                .checked_mul_f64(basket.notional.amount())?;
            let key = match &constituent.reference {
                ConstituentReference::MarketData { price_id, .. } => {
                    ("price", price_id.to_string())
                }
                ConstituentReference::Instrument(_) => ("instrument", constituent.id.clone()),
            };
            let value = groups.entry(key).or_insert(zero);
            *value = value.checked_add(contribution)?;
            total = total.checked_add(contribution)?;
        }
        for ((kind, id), amount) in groups {
            context.computed.insert(
                MetricId::composite(&MetricId::BasketMarkShock, &[kind, &id]),
                amount.amount(),
            );
        }
        Ok(total.amount())
    }
}
