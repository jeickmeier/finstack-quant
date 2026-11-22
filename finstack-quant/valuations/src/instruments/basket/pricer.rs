//! Basket NAV: constituent valuation, FX conversion and expense drag.

use super::types::{Basket, BasketConstituent, BasketExpense, ConstituentReference};
use finstack_quant_core::{
    Result,
    dates::Date,
    market_data::context::MarketContext,
    money::{Money, fx::FxQuery},
};

/// Net asset value per basket unit after the elected accrued expense.
pub(crate) fn nav(basket: &Basket, market: &MarketContext, as_of: Date) -> Result<Money> {
    let mut total = Money::from((0_i64, basket.reporting_currency));
    for constituent in &basket.constituents {
        let mark = marked_constituent(basket, constituent, market, as_of)?;
        total = total
            .checked_add(mark.checked_mul_f64(constituent.units.unwrap_or(constituent.weight))?)?;
    }
    match basket.expense {
        BasketExpense::None => Ok(total),
        BasketExpense::Accrued { amount } => total.checked_sub(amount),
    }
}

/// Resolve a constituent's mark in the basket reporting currency.
pub(crate) fn marked_constituent(
    basket: &Basket,
    constituent: &BasketConstituent,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let mark = constituent_price(basket, constituent, market, as_of)?;
    to_basket_currency(basket, mark, market, as_of)
}

/// Price of one constituent in its own currency.
fn constituent_price(
    basket: &Basket,
    constituent: &BasketConstituent,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    match &constituent.reference {
        ConstituentReference::Instrument(json) => json.as_dyn_instrument().value(context, as_of),
        ConstituentReference::MarketData { price_id, .. } => {
            let scalar = context.get_price(price_id.as_ref())?;
            if let Some(money) = scalar.get_price() {
                Ok(money)
            } else {
                let v = scalar.get_unitless().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "Unsupported MarketScalar variant".to_string(),
                    )
                })?;
                // For unitless scalars, use the basket currency by default
                Ok(Money::new(v, basket.reporting_currency)?)
            }
        }
    }
}

/// Convert `money` into the basket's reporting currency under its FX policy.
#[inline]
fn to_basket_currency(
    basket: &Basket,
    money: Money,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let target = basket.reporting_currency;
    if money.currency() == target {
        return Ok(money);
    }

    let fx = context.fx().ok_or(finstack_quant_core::Error::Input(
        finstack_quant_core::InputError::NotFound {
            id: "fx".to_string(),
        },
    ))?;

    let rate = fx
        .rate(FxQuery::new(
            money.currency(),
            target,
            as_of,
            basket.pricing_config.fx_policy,
        ))?
        .rate;

    Money::from_model_value(money.amount() * rate, target)
}
