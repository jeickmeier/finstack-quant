//! Equity pricer engine.
//!
//! Provides deterministic PV for `Equity` instruments. The PV is
//! `price_per_share * effective_shares` in the instrument's quote currency.
//!
//! All arithmetic uses the core `Money` type to respect rounding policy and
//! currency safety requirements.

use crate::instruments::equity::Equity;
// (no pricer registry integration here)
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Stateless pricing engine for `Equity` instruments.
#[derive(Debug, Default, Clone, Copy)]
pub struct EquityPricer;

impl EquityPricer {
    /// Resolve price per share for the equity.
    ///
    /// Priority:
    /// 1) `inst.price_quote` if set
    /// 2) `MarketContext::price` using instrument-provided overrides and fallbacks:
    ///    explicit `price_id`, attribute hints, ticker, instrument id, `{ticker}-SPOT`, then `EQUITY-SPOT`
    ///    - If `Price`, convert to `inst.currency` via FX matrix
    ///    - If `Unitless`, treat as amount in `inst.currency`
    pub fn price_per_share(
        &self,
        inst: &Equity,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        inst.price_per_share(curves, as_of)
    }

    /// Compute present value in the instrument's currency.
    ///
    /// Parameters:
    /// - `inst`: reference to the `Equity` instrument
    /// - `curves`: market context (unused currently; placeholder for quotes)
    /// - `as_of`: valuation date (unused currently)
    pub fn pv(&self, inst: &Equity, curves: &MarketContext, as_of: Date) -> Result<Money> {
        let px = self.price_per_share(inst, curves, as_of)?;
        Ok(Money::new(
            px.amount() * inst.effective_shares(),
            inst.currency,
        ))
    }

    /// Resolve dividend yield (annualized, decimal) for the equity.
    ///
    /// Attempts to read from market context using the key format
    /// "{ticker}-DIVYIELD". When not present, defaults to 0.0.
    pub fn dividend_yield(&self, inst: &Equity, curves: &MarketContext) -> Result<f64> {
        inst.dividend_yield(curves)
    }

    /// Build forward price per share using continuous-compound approximation:
    /// F(t) = S0 × exp((r - q) × t)
    ///
    /// - S0 resolved via `price_per_share` (respects instrument overrides)
    /// - r pulled from discount curve configured on instrument
    /// - q from `dividend_yield` (0.0 when absent)
    pub fn forward_price_per_share(
        &self,
        inst: &Equity,
        curves: &MarketContext,
        as_of: Date,
        t: f64,
    ) -> Result<Money> {
        inst.forward_price_per_share(curves, as_of, t)
    }

    /// Forward total value for the position (per-share forward × shares).
    pub fn forward_value(
        &self,
        inst: &Equity,
        curves: &MarketContext,
        as_of: Date,
        t: f64,
    ) -> Result<Money> {
        let per_share = self.forward_price_per_share(inst, curves, as_of, t)?;
        Ok(Money::new(
            per_share.amount() * inst.effective_shares(),
            inst.currency,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::market_data::{
        context::MarketContext, term_structures::DiscountCurve,
    };
    use time::Month;

    fn create_test_equity() -> Equity {
        Equity::new("AAPL", "Apple Inc.", Currency::USD).with_price(150.0)
    }

    fn create_test_market_context() -> MarketContext {
        let base_date = create_date(2025, Month::January, 1).expect("should succeed");
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.85)])
            .build()
            .expect("should succeed");

        MarketContext::new().insert(discount_curve)
    }

    #[test]
    fn test_equity_pricing_with_valid_market_data() {
        let equity = create_test_equity();
        let market = create_test_market_context();
        let as_of =
            finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                .expect("valid date");

        let value = equity.value(&market, as_of).expect("should succeed");
        assert!(value.amount() > 0.0);
    }

    #[test]
    fn test_equity_pricing_without_discount_curve() {
        let equity = create_test_equity();
        let empty_market = MarketContext::new(); // No discount curve
        let as_of =
            finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                .expect("valid date");

        // Should still price correctly even without discount curve
        let value = equity.value(&empty_market, as_of).expect("should succeed");
        assert!(value.amount() > 0.0);
    }

    #[test]
    fn test_equity_pricing_with_different_currencies() {
        let eur_equity = Equity::new("SAP", "SAP SE", Currency::EUR).with_price(120.0);

        let market = MarketContext::new(); // No discount curve for EUR
        let as_of =
            finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                .expect("valid date");

        let value = eur_equity.value(&market, as_of).expect("should succeed");
        assert_eq!(value.currency(), Currency::EUR);
        assert!(value.amount() > 0.0);
    }

    #[test]
    fn test_equity_pricing_error_message_quality() {
        let equity = create_test_equity();
        let market = create_test_market_context();
        let as_of =
            finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
                .expect("valid date");

        // Test that any errors have meaningful messages
        match equity.value(&market, as_of) {
            Ok(value) => {
                assert!(value.amount() >= 0.0);
            }
            Err(error) => {
                let error_msg = format!("{}", error);
                assert!(!error_msg.is_empty());
                // Error messages should be descriptive
                assert!(error_msg.len() > 10);
            }
        }
    }
}
