//! Basket NAV: constituent valuation, FX conversion and expense drag.

use super::types::{Basket, BasketConstituent, ConstituentReference};
use finstack_quant_core::{
    dates::Date,
    market_data::context::MarketContext,
    money::{fx::FxQuery, Money},
    Result,
};

/// Net asset value per share of `basket`, after one day of expense drag.
///
/// Each constituent contributes `price × units / shares_outstanding` when it
/// holds explicit units and `price × weight` otherwise, with every price
/// converted into the basket's reporting currency.
///
/// # Arguments
///
/// * `basket` - Basket to value; its `pricing_config` supplies the fee day
///   basis and the FX policy.
/// * `market` - Market context supplying constituent prices and FX.
/// * `as_of` - Valuation date for constituent pricing and FX lookups.
/// * `shares_outstanding` - Shares that unit-based holdings are spread over;
///   must be positive when any constituent holds units.
///
/// # Errors
///
/// Returns an error when a constituent price or FX rate is unavailable, or the
/// share count is not positive while a constituent holds units.
pub(crate) fn nav(
    basket: &Basket,
    market: &MarketContext,
    as_of: Date,
    shares_outstanding: f64,
) -> Result<Money> {
    let mut per_share = 0.0;
    for (index, constituent) in basket.constituents.iter().enumerate() {
        let raw_value = constituent_price(basket, index, constituent, market, as_of)?;
        let base_value = to_basket_currency(basket, raw_value, market, as_of)?;
        let c = if let Some(units) = constituent.units {
            if shares_outstanding <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "basket share count must be positive".to_string(),
                ));
            }
            base_value
                .checked_mul_f64(units)?
                .checked_div_f64(shares_outstanding)?
        } else {
            base_value.checked_mul_f64(constituent.weight)?
        };
        per_share += c.amount();
    }

    // Apply expense ratio drag to per-share value
    let expense_drag = expense_drag(basket, per_share);
    let per_share_after_fees = per_share - expense_drag;
    Money::new(per_share_after_fees, basket.reporting_currency)
}

/// Price of one constituent in its own currency.
fn constituent_price(
    basket: &Basket,
    constituent_index: usize,
    constituent: &BasketConstituent,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    match &constituent.reference {
        ConstituentReference::Instrument(_) => {
            let boxed_instrument = basket.boxed_constituent_at(constituent_index)?.ok_or(
                finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                    id: constituent.id.clone(),
                }),
            )?;
            boxed_instrument.value(context, as_of)
        }
        ConstituentReference::MarketData { price_id, .. } => {
            let scalar = context.get_price(price_id.as_ref())?;
            match scalar {
                finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => Ok(*money),
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => {
                    // For unitless scalars, use the basket currency by default
                    Ok(Money::new(*v, basket.reporting_currency)?)
                }
            }
        }
    }
}

/// One day of expense drag on `portfolio_value`.
///
/// ```text
/// drag = portfolio_value × expense_ratio / days_in_year
/// ```
fn expense_drag(basket: &Basket, portfolio_value: f64) -> f64 {
    let daily_expense_rate = basket.expense_ratio / basket.pricing_config.days_in_year;
    portfolio_value * daily_expense_rate
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

    Money::new(money.amount() * rate, target)
}
