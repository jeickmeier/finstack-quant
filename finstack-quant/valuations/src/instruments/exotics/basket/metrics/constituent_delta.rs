//! Constituent delta calculator for baskets.
//!
//! Computes delta (price sensitivity) for each constituent using finite differences.
//! For each constituent, bumps its price by 1% and measures the impact on basket NAV.
//!
//! # Formula
//! ```text
//! ConstituentDelta_i = (PV(basket with bumped constituent_i) - PV_base) / bump_size
//! ```
//! Where bump_size is 1% (0.01) of the constituent price.
//!
//! Results are stored as a series with labels derived from constituent IDs or tickers.
//!
//! # Notes
//!
//! Instrument-based constituents are handled by replacing the target constituent
//! with a synthetic market data price for the bump scenario. This mirrors a direct
//! price shock without requiring instrument-specific overrides.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::basket::types::{AssetType, ConstituentReference};
use crate::instruments::exotics::basket::{Basket, BasketConstituent};
use crate::metrics::{MetricCalculator, MetricContext};
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
        for constituent in &basket.constituents {
            let label = constituent
                .ticker
                .clone()
                .unwrap_or_else(|| constituent.id.clone());

            let delta = bump_and_measure_delta(basket, constituent, context, as_of, base_pv)?;

            series.push((label, delta));
            total_delta += delta;
        }

        // Store as bucketed series
        context.store_bucketed_series(crate::metrics::MetricId::ConstituentDelta, series);

        Ok(total_delta)
    }
}

/// Helper to get the price (as Money) of a constituent.
fn get_constituent_price_money(
    basket: &Basket,
    constituent: &BasketConstituent,
    context: &MetricContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<Money> {
    match &constituent.reference {
        ConstituentReference::Instrument(instr_json) => {
            let price = instrument_price_and_type(instr_json, context, as_of)?.0;
            Ok(price)
        }
        ConstituentReference::MarketData { price_id, .. } => {
            let scalar = context.curves.get_price(price_id.as_ref())?;
            match scalar {
                finstack_quant_core::market_data::scalars::MarketScalar::Price(money) => Ok(*money),
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => {
                    Ok(Money::new(*v, basket.currency))
                }
            }
        }
    }
}

/// Bump a constituent's price and measure the impact on basket NAV.
fn bump_and_measure_delta(
    basket: &Basket,
    constituent: &BasketConstituent,
    context: &MetricContext,
    as_of: finstack_quant_core::dates::Date,
    base_pv: f64,
) -> Result<f64> {
    let mut bumped_ctx = context.curves.as_ref().clone();

    let (current_price, pv_bumped) = match &constituent.reference {
        ConstituentReference::Instrument(instr_json) => {
            let (price, asset_type) = instrument_price_and_type(instr_json, context, as_of)?;
            let bumped_price = price.amount() * (1.0 + PRICE_BUMP_PCT);
            let synthetic_id = synthetic_price_id(basket, constituent);

            bumped_ctx = bumped_ctx.insert_price(
                synthetic_id.as_ref(),
                finstack_quant_core::market_data::scalars::MarketScalar::Price(Money::new(
                    bumped_price,
                    price.currency(),
                )),
            );

            let bumped_basket =
                basket_with_price_reference(basket, constituent, synthetic_id, asset_type);
            let pv_bumped = bumped_basket.value(&bumped_ctx, as_of)?.amount();
            (price.amount(), pv_bumped)
        }
        ConstituentReference::MarketData { price_id, .. } => {
            let current_price = get_constituent_price_money(basket, constituent, context, as_of)?;
            let bumped_price = current_price.amount() * (1.0 + PRICE_BUMP_PCT);
            let current_scalar = bumped_ctx.get_price(price_id.as_ref())?;
            let new_scalar = match current_scalar {
                finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => {
                    finstack_quant_core::market_data::scalars::MarketScalar::Price(
                        finstack_quant_core::money::Money::new(bumped_price, m.currency()),
                    )
                }
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(_) => {
                    finstack_quant_core::market_data::scalars::MarketScalar::Unitless(bumped_price)
                }
            };
            bumped_ctx = bumped_ctx.insert_price(price_id.as_ref(), new_scalar);

            let pv_bumped = basket.value(&bumped_ctx, as_of)?.amount();
            (current_price.amount(), pv_bumped)
        }
    };

    // Constituent delta: sensitivity of basket PV to a 1% move in constituent price.
    //
    // Formula: delta = (PV_bumped - PV_base) / (price * 1%) * price
    //        = (PV_bumped - PV_base) / 1%
    //
    // Units: currency units per 1% move in the constituent price.
    // Example: delta = 5.0 means a 1% price increase adds 5.0 to basket PV.
    let bump_size = current_price * PRICE_BUMP_PCT;
    let delta = if bump_size.abs() > 1e-10 {
        (pv_bumped - base_pv) / PRICE_BUMP_PCT
    } else {
        0.0
    };

    Ok(delta)
}

fn instrument_price_and_type(
    instr_json: &crate::instruments::json_loader::InstrumentJson,
    context: &MetricContext,
    as_of: finstack_quant_core::dates::Date,
) -> Result<(Money, AssetType)> {
    let boxed = instr_json.clone().into_boxed()?;
    let price = boxed.value(context.curves.as_ref(), as_of)?;
    let asset_type = asset_type_for_instrument_key(boxed.key());
    Ok((price, asset_type))
}

fn asset_type_for_instrument_key(key: crate::pricer::InstrumentType) -> AssetType {
    use crate::pricer::InstrumentType;

    match key {
        InstrumentType::Equity
        | InstrumentType::EquityOption
        | InstrumentType::EquityIndexFuture
        | InstrumentType::EquityTotalReturnSwap => AssetType::Equity,
        InstrumentType::Bond
        | InstrumentType::BondFuture
        | InstrumentType::InflationLinkedBond
        | InstrumentType::AgencyMbsPassthrough
        | InstrumentType::AgencyTba
        | InstrumentType::AgencyCmo
        | InstrumentType::Loan
        | InstrumentType::TermLoan
        | InstrumentType::RevolvingCredit
        | InstrumentType::Deposit
        | InstrumentType::Repo => AssetType::Bond,
        InstrumentType::CommodityForward | InstrumentType::CommoditySwap => AssetType::Commodity,
        _ => AssetType::Derivative,
    }
}

fn synthetic_price_id(basket: &Basket, constituent: &BasketConstituent) -> PriceId {
    PriceId::from(format!(
        "BASKET::{}::{}",
        basket.id.as_str(),
        constituent.id
    ))
}

fn basket_with_price_reference(
    basket: &Basket,
    target: &BasketConstituent,
    price_id: PriceId,
    asset_type: AssetType,
) -> Basket {
    let mut bumped_basket = basket.clone();
    if let Some(constituent) = bumped_basket
        .constituents
        .iter_mut()
        .find(|c| c.id == target.id)
    {
        constituent.reference = ConstituentReference::MarketData {
            price_id,
            asset_type,
        };
    }
    bumped_basket
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use crate::metrics::MetricId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use test_utils::flat_discount;

    #[test]
    fn test_constituent_delta_mixed_references() {
        let as_of = test_utils::date(2024, 1, 2);
        let market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.02))
            .insert_price(
                "AAPL-SPOT",
                MarketScalar::Price(Money::new(150.0, Currency::USD)),
            );

        let basket = Basket::example_with_instruments().expect("Basket example is valid");
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
