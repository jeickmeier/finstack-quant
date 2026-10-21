//! Constituent delta calculator for baskets.
//!
//! Computes delta (price sensitivity) for each constituent using finite differences.
//! For each constituent, bumps its price by 1% and measures the impact on basket PV.
//!
//! # Formula
//! ```text
//! ConstituentDelta_i = PV(basket with price_i × 1.01) - PV_base
//! ```
//! i.e. the basket PV change per +1% relative move in constituent `i`'s price,
//! in the basket currency. The metric total is the sum over constituents.
//!
//! Results are stored as a series with labels derived from constituent IDs or tickers.
//!
//! # Notes
//!
//! Instrument-based constituents are handled by replacing the target constituent
//! with a synthetic market data price for the bump scenario. This mirrors a direct
//! price shock without requiring instrument-specific overrides.

use crate::instruments::exotics::basket::types::{BasketAssetType, ConstituentReference};
use crate::instruments::exotics::basket::{Basket, BasketConstituent};
use crate::instruments::Instrument;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PriceId;
use finstack_quant_core::Result;

/// Standard price bump: 1% (0.01)
const PRICE_BUMP_PCT: f64 = 0.01;

/// Constituent delta calculator for baskets.
pub(crate) struct ConstituentDeltaCalculator;

impl MetricCalculator for ConstituentDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let basket: &Basket = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let mut series: Vec<(String, f64)> = Vec::new();
        let mut total_delta = 0.0;

        // For each constituent, bump its price and measure impact
        for (index, constituent) in basket.constituents.iter().enumerate() {
            let label = constituent
                .ticker
                .clone()
                .unwrap_or_else(|| constituent.id.clone());

            let delta =
                bump_and_measure_delta(basket, index, constituent, context, as_of, base_pv)?;

            series.push((label, delta));
            total_delta += delta;
        }

        context.store_bucketed_series(crate::metrics::MetricId::ConstituentDelta, series);

        Ok(total_delta)
    }
}

/// Bump a constituent's price and measure the impact on basket NAV.
fn bump_and_measure_delta(
    basket: &Basket,
    constituent_index: usize,
    constituent: &BasketConstituent,
    context: &MetricContext,
    as_of: finstack_quant_core::dates::Date,
    base_pv: f64,
) -> Result<f64> {
    let mut bumped_ctx = context.curves.as_ref().clone();

    let (current_price, pv_bumped) = match &constituent.reference {
        ConstituentReference::Instrument(_) => {
            let price = instrument_price(basket, constituent_index, constituent, context, as_of)?;
            let bumped_price = price.amount() * (1.0 + PRICE_BUMP_PCT);
            let synthetic_id = synthetic_price_id(basket, constituent);

            bumped_ctx = bumped_ctx.insert_price(
                synthetic_id.as_ref(),
                finstack_quant_core::market_data::scalars::MarketScalar::Price(Money::new(
                    bumped_price,
                    price.currency(),
                )?),
            );

            let bumped_basket =
                basket_with_price_reference(basket, constituent_index, synthetic_id);
            let pv_bumped = bumped_basket.value(&bumped_ctx, as_of)?.amount();
            (price.amount(), pv_bumped)
        }
        ConstituentReference::MarketData { price_id, .. } => {
            let (current_price, new_scalar) = match bumped_ctx.get_price(price_id.as_ref())? {
                MarketScalar::Price(money) => {
                    let price = money.amount();
                    let bumped = Money::new(price * (1.0 + PRICE_BUMP_PCT), money.currency())?;
                    (price, MarketScalar::Price(bumped))
                }
                MarketScalar::Unitless(v) => {
                    let price = Money::new(*v, basket.reporting_currency)?.amount();
                    let bumped = price * (1.0 + PRICE_BUMP_PCT);
                    (price, MarketScalar::Unitless(bumped))
                }
            };
            bumped_ctx = bumped_ctx.insert_price(price_id.as_ref(), new_scalar);

            let pv_bumped = basket.value(&bumped_ctx, as_of)?.amount();
            (current_price, pv_bumped)
        }
    };

    // Constituent delta: basket PV change for a +1% relative move in the
    // constituent price, in currency (per bump, not per unit move).
    // Example: delta = 5.0 means a 1% price increase adds 5.0 to basket PV.
    let bump_size = current_price * PRICE_BUMP_PCT;
    let delta = if bump_size.abs() > 1e-10 {
        pv_bumped - base_pv
    } else {
        0.0
    };

    Ok(delta)
}

fn instrument_price(
    basket: &Basket,
    constituent_index: usize,
    constituent: &BasketConstituent,
    context: &MetricContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<Money> {
    let boxed = basket.boxed_constituent_at(constituent_index)?.ok_or(
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
            id: constituent.id.clone(),
        }),
    )?;
    boxed.value(context.curves.as_ref(), as_of)
}

fn synthetic_price_id(basket: &Basket, constituent: &BasketConstituent) -> PriceId {
    PriceId::from(format!(
        "BASKET::{}::{}",
        basket.id.as_str(),
        constituent.id
    ))
}

/// Copy of `basket` whose constituent at `constituent_index` reads `price_id`.
///
/// The asset type of the synthetic reference is not read by pricing.
fn basket_with_price_reference(
    basket: &Basket,
    constituent_index: usize,
    price_id: PriceId,
) -> Basket {
    let mut bumped_basket = basket.clone();
    let constituent = &mut bumped_basket.constituents[constituent_index];
    constituent.reference = ConstituentReference::MarketData {
        price_id,
        asset_type: BasketAssetType::Derivative,
    };
    bumped_basket
}

#[cfg(test)]
mod tests {
    use finstack_quant_valuations_test_support::date as date_support;
    use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;

    use super::*;
    use crate::metrics::MetricId;
    use discount_forward_curve_support::flat_discount;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;

    /// A basket mixing an instrument-backed bond and a market-data equity.
    fn basket_with_instrument_constituent() -> Basket {
        use crate::instruments::exotics::basket::BasketPricingConfig;
        use crate::instruments::Attributes;
        use finstack_quant_core::types::InstrumentId;
        use time::macros::date;

        let bond = crate::instruments::fixed_income::bond::Bond::fixed(
            "CORP-BOND-001",
            Money::from((1000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("rate"),
            date!(2024 - 01 - 01),
            date!(2034 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let constituents = vec![
            BasketConstituent {
                id: "BOND-CORP".to_string(),
                reference: ConstituentReference::Instrument(Box::new(
                    crate::instruments::json_loader::InstrumentJson::Bond(bond),
                )),
                weight: 0.0,
                units: Some(100.0),
                ticker: Some("CORP".to_string()),
            },
            BasketConstituent {
                id: "EQ-AAPL".to_string(),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new("AAPL-SPOT"),
                    asset_type: BasketAssetType::Equity,
                },
                weight: 1.0,
                units: None,
                ticker: Some("AAPL".to_string()),
            },
        ];
        Basket::builder()
            .id(InstrumentId::new("BASKET-MIXED"))
            .constituents(constituents)
            .expense_ratio(0.001)
            .reporting_currency(Currency::USD)
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .attributes(Attributes::new())
            .pricing_config(BasketPricingConfig::default())
            .build()
            .expect("basket")
    }

    #[test]
    fn test_constituent_delta_mixed_references() {
        let as_of = date_support::date(2024, 1, 2);
        let market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.02))
            .insert_price(
                "AAPL-SPOT",
                MarketScalar::Price(Money::from((150_i64, Currency::USD))),
            );

        let basket = basket_with_instrument_constituent();
        let base_value = basket
            .value(&market, as_of)
            .expect("base basket value should succeed");
        let mut context = MetricContext::new(
            Arc::new(basket),
            Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        );

        let calculator = ConstituentDeltaCalculator;
        let total_delta = calculator
            .calculate(&mut context)
            .expect("constituent delta should compute");

        let series = context
            .computed_series
            .get(&MetricId::ConstituentDelta)
            .expect("bucketed series should be stored");

        assert_eq!(series.len(), 2);
        assert!(series.iter().any(|(label, _)| label == "AAPL"));
        let corp_delta = series
            .iter()
            .find(|(label, _)| label == "CORP")
            .map(|(_, value)| *value)
            .expect("instrument-based delta should be present");
        assert!(corp_delta.abs() > 1e-6);
        assert!(total_delta.abs() >= corp_delta.abs());
    }
}
