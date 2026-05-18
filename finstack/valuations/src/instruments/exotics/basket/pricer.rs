//! Basket pricing engine.
//!
//! This module contains all the pricing logic for basket instruments, separated from
//! the type definitions. It handles NAV calculations, constituent valuation, expense
//! drag, and currency conversions.

use super::types::{Basket, BasketConstituent, BasketPricingConfig, ConstituentReference};
use finstack_core::{
    currency::Currency,
    dates::Date,
    market_data::context::MarketContext,
    money::{fx::FxQuery, Money},
    Result,
};

/// Internal valuation mode used to interpret weights/units per call site.
#[derive(Debug, Clone)]
enum ValueMode {
    /// Return per-share contribution; units require shares to be present.
    PerShare { shares: Option<f64> },
    /// Return total contribution; prefers units, else uses AUM, else shares.
    Total {
        shares: Option<f64>,
        aum: Option<f64>,
    },
}

/// Basket calculation engine that handles all pricing logic.
///
/// This calculator is stateless and can be reused across multiple basket valuations.
/// It encapsulates all the complex logic for valuing basket constituents, handling
/// different value modes, applying expense drag, and managing currency conversions.
#[derive(Debug, Clone)]
pub struct BasketCalculator {
    config: BasketPricingConfig,
}

impl BasketCalculator {
    /// Create a new calculator with the given configuration.
    pub fn new(config: BasketPricingConfig) -> Self {
        Self { config }
    }

    /// Create a calculator with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(BasketPricingConfig::default())
    }

    /// Calculate Net Asset Value per share.
    ///
    /// # Arguments
    /// * `basket` - The basket instrument to value
    /// * `context` - Market context with pricing data
    /// * `as_of` - Valuation date
    /// * `shares_outstanding` - Total shares outstanding for per-share calculation
    pub fn nav(
        &self,
        basket: &Basket,
        context: &MarketContext,
        as_of: Date,
        shares_outstanding: f64,
    ) -> Result<Money> {
        let mut per_share = 0.0;
        for constituent in &basket.constituents {
            let c = self.value_constituent(
                basket,
                constituent,
                context,
                as_of,
                ValueMode::PerShare {
                    shares: Some(shares_outstanding),
                },
            )?;
            per_share += c.amount();
        }

        // Apply expense ratio drag to per-share value
        let expense_drag = self.calculate_expense_drag(basket, per_share, as_of)?;
        let per_share_after_fees = per_share - expense_drag;
        Ok(Money::new(per_share_after_fees, basket.currency))
    }

    /// Calculate total basket value (gross, without per-share division).
    ///
    /// # Arguments
    /// * `basket` - The basket instrument to value
    /// * `context` - Market context with pricing data
    /// * `as_of` - Valuation date
    /// * `shares_outstanding` - Optional shares outstanding for weight-based calculations
    pub fn basket_value(
        &self,
        basket: &Basket,
        context: &MarketContext,
        as_of: Date,
        shares_outstanding: Option<f64>,
    ) -> Result<Money> {
        let mut total = 0.0;
        for constituent in &basket.constituents {
            let c = self.value_constituent(
                basket,
                constituent,
                context,
                as_of,
                ValueMode::Total {
                    shares: shares_outstanding,
                    aum: None,
                },
            )?;
            total += c.amount();
        }
        let expense_drag = self.calculate_expense_drag(basket, total, as_of)?;
        Ok(Money::new(total - expense_drag, basket.currency))
    }

    /// Calculate Net Asset Value per share using an explicit AUM.
    ///
    /// When constituents lack `units`, contributions are computed as
    /// `weight × AUM (in basket currency)`.
    ///
    /// # Arguments
    /// * `basket` - The basket instrument to value
    /// * `context` - Market context with pricing data
    /// * `as_of` - Valuation date
    /// * `aum` - Assets under management amount
    /// * `shares_outstanding` - Total shares outstanding for per-share calculation
    pub fn nav_with_aum(
        &self,
        basket: &Basket,
        context: &MarketContext,
        as_of: Date,
        aum: Money,
        shares_outstanding: f64,
    ) -> Result<Money> {
        let aum_basket = self.to_basket_currency(basket, aum, basket.currency, context, as_of)?;
        let total = self.basket_value_with_aum(basket, context, as_of, aum_basket)?;
        let nav_value = if shares_outstanding > 0.0 {
            total.amount() / shares_outstanding
        } else {
            total.amount()
        };
        Ok(Money::new(nav_value, basket.currency))
    }

    /// Calculate total basket value using an explicit AUM for weight-based constituents.
    ///
    /// For an all-weight-based basket the total invested value is
    /// `Σ weightᵢ · AUM`. When the weights sum to ~1.0 this is just the AUM,
    /// but a partially-invested or misconfigured basket (weights summing to,
    /// say, 0.5 or 1.2) must be scaled by the actual weight sum rather than
    /// returning the AUM verbatim (W-10). The plain `aum_amount` shortcut is
    /// kept only when the weight sum is within a tight tolerance of 1.0, where
    /// it avoids floating-point drift.
    pub fn basket_value_with_aum(
        &self,
        basket: &Basket,
        context: &MarketContext,
        as_of: Date,
        aum_basket: Money,
    ) -> Result<Money> {
        let aum_amount = aum_basket.amount();
        // If all constituents are weight-based (no explicit units), the total
        // invested value is `weight_sum × AUM`.
        let all_weight_based = basket.constituents.iter().all(|c| c.units.is_none());
        let total = if all_weight_based {
            let weight_sum: f64 = basket.constituents.iter().map(|c| c.weight).sum();
            // Within 10bp of fully invested → treat as exactly the AUM to
            // avoid floating-point drift (matches `Basket::validate`'s
            // tolerance). Otherwise scale by the actual weight sum so a
            // partially-invested or misweighted basket is valued correctly.
            if (weight_sum - 1.0).abs() <= 0.001 {
                aum_amount
            } else {
                aum_amount * weight_sum
            }
        } else {
            let mut sum = 0.0;
            for constituent in &basket.constituents {
                let c = self.value_constituent(
                    basket,
                    constituent,
                    context,
                    as_of,
                    ValueMode::Total {
                        shares: None,
                        aum: Some(aum_amount),
                    },
                )?;
                sum += c.amount();
            }
            sum
        };
        let expense_drag = self.calculate_expense_drag(basket, total, as_of)?;
        Ok(Money::new(total - expense_drag, basket.currency))
    }

    // ----- Internal Helper Methods -----

    /// Value a single constituent based on the given mode.
    fn value_constituent(
        &self,
        basket: &Basket,
        constituent: &BasketConstituent,
        context: &MarketContext,
        as_of: Date,
        mode: ValueMode,
    ) -> Result<Money> {
        let out = match mode {
            ValueMode::PerShare { shares } => {
                // Resolve price then allocate per share
                let raw_value = self.get_constituent_price(basket, constituent, context, as_of)?;
                let base_value =
                    self.to_basket_currency(basket, raw_value, basket.currency, context, as_of)?;
                if let Some(units) = constituent.units {
                    let s = shares.ok_or(finstack_core::Error::Input(
                        finstack_core::InputError::Invalid,
                    ))?;
                    if s <= 0.0 {
                        return Err(finstack_core::Error::Input(
                            finstack_core::InputError::Invalid,
                        ));
                    }
                    (base_value * units) / s
                } else {
                    base_value * constituent.weight
                }
            }
            ValueMode::Total { shares, aum } => {
                if let Some(units) = constituent.units {
                    // Price × units (convert to basket currency first)
                    let raw_value =
                        self.get_constituent_price(basket, constituent, context, as_of)?;
                    let base_value = self.to_basket_currency(
                        basket,
                        raw_value,
                        basket.currency,
                        context,
                        as_of,
                    )?;
                    base_value * units
                } else if let Some(a) = aum {
                    Money::new(a * constituent.weight, basket.currency)
                } else if let Some(s) = shares {
                    // Weight-only contribution scaled by shares × price
                    let raw_value =
                        self.get_constituent_price(basket, constituent, context, as_of)?;
                    let base_value = self.to_basket_currency(
                        basket,
                        raw_value,
                        basket.currency,
                        context,
                        as_of,
                    )?;
                    base_value * constituent.weight * s
                } else {
                    return Err(finstack_core::Error::Input(
                        finstack_core::InputError::Invalid,
                    ));
                }
            }
        };
        Ok(out)
    }

    /// Get the price for a single constituent.
    fn get_constituent_price(
        &self,
        basket: &Basket,
        constituent: &BasketConstituent,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        match &constituent.reference {
            ConstituentReference::Instrument(instr_json) => {
                // Clone the InstrumentJson, convert to boxed instrument, and price it
                let boxed_instrument = instr_json.as_ref().clone().into_boxed()?;
                boxed_instrument.value(context, as_of)
            }
            ConstituentReference::MarketData { price_id, .. } => {
                let scalar = context.get_price(price_id.as_ref())?;
                match scalar {
                    finstack_core::market_data::scalars::MarketScalar::Price(money) => Ok(*money),
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => {
                        // For unitless scalars, use the basket currency by default
                        Ok(Money::new(*v, basket.currency))
                    }
                }
            }
        }
    }

    /// Calculate expense drag based on the portfolio value.
    ///
    /// Computes a **single-day** accrual of the annual expense ratio for mark-to-market
    /// purposes. The formula is:
    ///
    /// ```text
    /// drag = portfolio_value × expense_ratio / days_in_year
    /// ```
    ///
    /// This represents one day's worth of management fees. For multi-day holding
    /// period calculations, callers should scale the result by the number of
    /// holding days or integrate over the period.
    ///
    /// # Parameters
    ///
    /// * `basket` - The basket instrument (provides `expense_ratio`)
    /// * `portfolio_value` - Current portfolio value to apply the expense rate to
    /// * `_as_of` - Valuation date (reserved for future use with time-dependent fees)
    fn calculate_expense_drag(
        &self,
        basket: &Basket,
        portfolio_value: f64,
        _as_of: Date,
    ) -> Result<f64> {
        // Single-day accrual of expense ratio for mark-to-market
        let daily_expense_rate = basket.expense_ratio / self.config.days_in_year;
        Ok(portfolio_value * daily_expense_rate)
    }

    /// Convert money to basket currency using FX rates.
    #[inline]
    fn to_basket_currency(
        &self,
        _basket: &Basket,
        money: Money,
        target: Currency,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        if money.currency() == target {
            return Ok(money);
        }

        let fx = context.fx().ok_or(finstack_core::Error::Input(
            finstack_core::InputError::NotFound {
                id: "fx".to_string(),
            },
        ))?;

        let rate = fx
            .rate(FxQuery::with_policy(
                money.currency(),
                target,
                as_of,
                self.config.fx_policy,
            ))?
            .rate;

        Ok(Money::new(money.amount() * rate, target))
    }
}

impl Default for BasketCalculator {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::exotics::basket::types::{
        AssetType, BasketConstituent, ConstituentReference,
    };
    use finstack_core::types::{InstrumentId, PriceId};

    fn weight_based_basket(weights: &[f64]) -> Basket {
        let constituents: Vec<BasketConstituent> = weights
            .iter()
            .enumerate()
            .map(|(i, &w)| BasketConstituent {
                id: format!("CONST-{i}"),
                reference: ConstituentReference::MarketData {
                    price_id: PriceId::new(format!("PX-{i}")),
                    asset_type: AssetType::Equity,
                },
                weight: w,
                units: None,
                ticker: None,
            })
            .collect();
        Basket {
            id: InstrumentId::new("W10-BASKET"),
            constituents,
            // Zero expense ratio so the assertion isolates the weight-sum fix.
            expense_ratio: 0.0,
            currency: Currency::USD,
            notional: Money::new(1.0, Currency::USD),
            discount_curve_id: "USD-OIS".into(),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::new(),
            pricing_config: BasketPricingConfig::default(),
        }
    }

    /// W-10: an all-weight-based basket whose weights do NOT sum to 1.0 must
    /// have its AUM scaled by the actual weight sum, not returned verbatim. A
    /// basket only 50%-invested is worth half the AUM in positions.
    #[test]
    fn w10_partially_invested_basket_scales_aum_by_weight_sum() {
        let calc = BasketCalculator::with_defaults();
        let context = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let aum = Money::new(1_000_000.0, Currency::USD);

        // Weights sum to 0.5 — only half the AUM is invested.
        let basket = weight_based_basket(&[0.3, 0.2]);
        let value = calc
            .basket_value_with_aum(&basket, &context, as_of, aum)
            .expect("basket value");
        assert!(
            (value.amount() - 500_000.0).abs() < 1e-6,
            "a 50%-invested weight-based basket must be worth 0.5 × AUM = \
             500000, got {} — the AUM was returned verbatim",
            value.amount()
        );
    }

    /// W-10: an over-weighted basket (weights summing above 1.0) is likewise
    /// scaled — a 1.2× levered weight set is worth 1.2 × AUM.
    #[test]
    fn w10_over_weighted_basket_scales_aum_up() {
        let calc = BasketCalculator::with_defaults();
        let context = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let aum = Money::new(1_000_000.0, Currency::USD);

        let basket = weight_based_basket(&[0.7, 0.5]); // sums to 1.2
        let value = calc
            .basket_value_with_aum(&basket, &context, as_of, aum)
            .expect("basket value");
        assert!(
            (value.amount() - 1_200_000.0).abs() < 1e-6,
            "a 1.2×-weighted basket must be worth 1.2 × AUM = 1200000, got {}",
            value.amount()
        );
    }

    /// W-10: a correctly-weighted basket (weights summing to ~1.0) still
    /// returns the AUM exactly — the tight tolerance keeps the no-drift
    /// shortcut for the common, well-formed case.
    #[test]
    fn w10_fully_invested_basket_returns_aum_exactly() {
        let calc = BasketCalculator::with_defaults();
        let context = MarketContext::new();
        let as_of = Date::from_calendar_date(2025, time::Month::January, 1).expect("date");
        let aum = Money::new(1_000_000.0, Currency::USD);

        let basket = weight_based_basket(&[0.6, 0.4]); // sums to 1.0
        let value = calc
            .basket_value_with_aum(&basket, &context, as_of, aum)
            .expect("basket value");
        assert!((value.amount() - 1_000_000.0).abs() < 1e-9);
    }
}
