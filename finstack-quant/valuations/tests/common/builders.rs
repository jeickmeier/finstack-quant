//! Shared builders for test markets and instruments.
//!
//! These builders reduce boilerplate across risk tests by providing
//! consistent default configurations for common test scenarios.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, SettlementType};

// =============================================================================
// Market Context Builders
// =============================================================================

/// Builder for creating test market contexts with common configurations.
pub struct TestMarketBuilder {
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: Option<f64>,
    discount_curve_id: String,
    vol_surface_id: String,
    spot_id: String,
    div_yield_id: Option<String>,
    tenor_years: f64,
}

impl TestMarketBuilder {
    /// Create a new test market builder.
    pub fn new(as_of: Date) -> Self {
        Self {
            as_of,
            spot: 100.0,
            vol: 0.25,
            rate: 0.05,
            div_yield: None,
            discount_curve_id: "USD-OIS".to_string(),
            vol_surface_id: "SPOT_VOL".to_string(),
            spot_id: "SPOT".to_string(),
            div_yield_id: None,
            tenor_years: 2.0,
        }
    }

    /// Set the spot price.
    pub fn spot(mut self, spot: f64) -> Self {
        self.spot = spot;
        self
    }

    /// Set the volatility level (flat across surface).
    pub fn vol(mut self, vol: f64) -> Self {
        self.vol = vol;
        self
    }

    /// Set the risk-free rate.
    pub fn rate(mut self, rate: f64) -> Self {
        self.rate = rate;
        self
    }

    /// Set the dividend yield.
    pub fn div_yield(mut self, div_yield: f64) -> Self {
        self.div_yield = Some(div_yield);
        self.div_yield_id = Some(format!("{}_DIV", self.spot_id));
        self
    }

    /// Set the discount curve ID.
    pub fn discount_curve_id(mut self, id: &str) -> Self {
        self.discount_curve_id = id.to_string();
        self
    }

    /// Set the volatility surface ID.
    pub fn vol_surface_id(mut self, id: &str) -> Self {
        self.vol_surface_id = id.to_string();
        self
    }

    /// Set the spot price scalar ID.
    pub fn spot_id(mut self, id: &str) -> Self {
        self.spot_id = id.to_string();
        if self.div_yield.is_some() {
            self.div_yield_id = Some(format!("{}_DIV", id));
        }
        self
    }

    /// Set the far tenor for the discount curve (default: 2.0 years).
    pub fn tenor_years(mut self, years: f64) -> Self {
        self.tenor_years = years;
        self
    }

    /// Build the market context.
    pub fn build(self) -> MarketContext {
        let disc_curve = DiscountCurve::builder(self.discount_curve_id.as_str())
            .base_date(self.as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0_f64, 1.0_f64),
                (1.0_f64, (-self.rate).exp()),
                (self.tenor_years, (-self.rate * self.tenor_years).exp()),
            ])
            .build()
            .expect("discount curve should build in tests");

        let vol_surface = VolSurface::builder(self.vol_surface_id.as_str())
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[self.vol, self.vol, self.vol])
            .row(&[self.vol, self.vol, self.vol])
            .row(&[self.vol, self.vol, self.vol])
            .build()
            .expect("vol surface should build in tests");

        let mut market = MarketContext::new()
            .insert(disc_curve)
            .insert_surface(vol_surface)
            .insert_price(&self.spot_id, MarketScalar::Unitless(self.spot));

        if let (Some(div), Some(div_id)) = (self.div_yield, &self.div_yield_id) {
            market = market.insert_price(div_id, MarketScalar::Unitless(div));
        }

        market
    }
}

/// Create a simple test market with the given parameters.
///
/// This is a convenience function for simple test cases that don't need
/// the full flexibility of `TestMarketBuilder`.
pub fn simple_option_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    TestMarketBuilder::new(as_of)
        .spot(spot)
        .vol(vol)
        .rate(rate)
        .build()
}

/// Create a test market with dividend yield.
pub fn option_market_with_divs(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
    TestMarketBuilder::new(as_of)
        .spot(spot)
        .vol(vol)
        .rate(rate)
        .div_yield(div_yield)
        .build()
}

// =============================================================================
// Equity Option Builder
// =============================================================================

/// Builder for creating test equity options with common configurations.
pub struct TestOptionBuilder {
    id: String,
    underlying_ticker: String,
    strike: f64,
    currency: Currency,
    option_type: OptionType,
    exercise_style: ExerciseStyle,
    expiry: Date,
    notional_amount: f64,
    day_count: DayCount,
    settlement: SettlementType,
    discount_curve_id: String,
    spot_id: String,
    vol_surface_id: String,
    div_yield_id: Option<CurveId>,
}

impl TestOptionBuilder {
    /// Create a new test option builder.
    pub fn new(expiry: Date) -> Self {
        Self {
            id: "TEST_OPTION".to_string(),
            underlying_ticker: "AAPL".to_string(),
            strike: 100.0,
            currency: Currency::USD,
            option_type: OptionType::Call,
            exercise_style: ExerciseStyle::European,
            expiry,
            notional_amount: 100.0,
            day_count: DayCount::Act365F,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".to_string(),
            spot_id: "SPOT".to_string(),
            vol_surface_id: "SPOT_VOL".to_string(),
            div_yield_id: None,
        }
    }

    /// Set the option ID.
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Set the strike price.
    pub fn strike(mut self, strike: f64) -> Self {
        self.strike = strike;
        self
    }

    /// Set the option type (Call or Put).
    pub fn option_type(mut self, option_type: OptionType) -> Self {
        self.option_type = option_type;
        self
    }

    /// Set the equity option notional amount.
    pub fn contract_size(mut self, size: f64) -> Self {
        self.notional_amount = size;
        self
    }

    /// Set the discount curve ID.
    pub fn discount_curve_id(mut self, id: &str) -> Self {
        self.discount_curve_id = id.to_string();
        self
    }

    /// Set the spot price scalar ID.
    pub fn spot_id(mut self, id: &str) -> Self {
        self.spot_id = id.to_string();
        self
    }

    /// Set the volatility surface ID.
    pub fn vol_surface_id(mut self, id: &str) -> Self {
        self.vol_surface_id = id.to_string();
        self
    }

    /// Set the dividend yield scalar ID.
    pub fn div_yield_id(mut self, id: &str) -> Self {
        self.div_yield_id = Some(CurveId::new(id));
        self
    }

    /// Build the equity option.
    pub fn build(self) -> EquityOption {
        EquityOption {
            id: self.id.into(),
            underlying_ticker: self.underlying_ticker,
            strike: self.strike,
            option_type: self.option_type,
            exercise_style: self.exercise_style,
            expiry: self.expiry,
            notional: Money::new(self.notional_amount, self.currency),
            day_count: self.day_count,
            settlement: self.settlement,
            discount_curve_id: self.discount_curve_id.into(),
            spot_id: self.spot_id.into(),
            vol_surface_id: self.vol_surface_id.into(),
            div_yield_id: self.div_yield_id,
            discrete_dividends: Vec::new(),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            exercise_schedule: None,
            attributes: Default::default(),
        }
    }
}

/// Create a simple test call option.
pub fn simple_call_option(expiry: Date, strike: f64) -> EquityOption {
    TestOptionBuilder::new(expiry).strike(strike).build()
}

/// Create a simple test put option.
pub fn simple_put_option(expiry: Date, strike: f64) -> EquityOption {
    TestOptionBuilder::new(expiry)
        .strike(strike)
        .option_type(OptionType::Put)
        .build()
}

/// Create a test option with full customization via builder.
pub fn test_option(expiry: Date) -> TestOptionBuilder {
    TestOptionBuilder::new(expiry)
}

/// Create a test market context via builder.
pub fn test_market(as_of: Date) -> TestMarketBuilder {
    TestMarketBuilder::new(as_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{assertions::assert_approx_eq, tolerances};
    use time::macros::date;

    // =========================================================================
    // TestMarketBuilder Tests
    // =========================================================================

    #[test]
    fn test_market_builder_defaults() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of).build();

        // Should have discount curve and vol surface
        assert!(market.get_discount("USD-OIS").is_ok());
        assert!(market.get_surface("SPOT_VOL").is_ok());
        assert!(market.get_price("SPOT").is_ok());
    }

    #[test]
    fn test_market_builder_custom_spot() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of).spot(150.0).build();

        let spot = market.get_price("SPOT").expect("spot should exist");
        let spot_value = match spot {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => m.amount(),
        };
        assert_approx_eq(spot_value, 150.0, tolerances::TIGHT);
    }

    #[test]
    fn test_market_builder_custom_vol() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of).vol(0.30).build();

        // Vol surface should exist with custom vol
        let surface = market
            .get_surface("SPOT_VOL")
            .expect("vol surface should exist");
        let vol = surface
            .value_checked(1.0, 100.0)
            .expect("vol lookup should succeed");
        assert_approx_eq(vol, 0.30, tolerances::TIGHT);
    }

    #[test]
    fn test_market_builder_custom_rate() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of).rate(0.03).build();

        let curve = market.get_discount("USD-OIS").expect("curve should exist");
        // At 1Y, DF should be approximately exp(-0.03)
        let expected_df = (-0.03_f64).exp();
        assert_approx_eq(curve.df(1.0), expected_df, tolerances::STANDARD);
    }

    #[test]
    fn test_market_builder_with_dividend_yield() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of).div_yield(0.02).build();

        // Should have dividend yield scalar
        assert!(market.get_price("SPOT_DIV").is_ok());
    }

    #[test]
    fn test_market_builder_custom_ids() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of)
            .discount_curve_id("CUSTOM-DISC")
            .vol_surface_id("CUSTOM-VOL")
            .spot_id("CUSTOM-SPOT")
            .build();

        assert!(market.get_discount("CUSTOM-DISC").is_ok());
        assert!(market.get_surface("CUSTOM-VOL").is_ok());
        assert!(market.get_price("CUSTOM-SPOT").is_ok());
    }

    #[test]
    fn test_market_builder_updates_div_yield_id_when_spot_id_changes() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of)
            .div_yield(0.02)
            .spot_id("CUSTOM-SPOT")
            .build();

        assert!(market.get_price("CUSTOM-SPOT_DIV").is_ok());
        assert!(market.get_price("SPOT_DIV").is_err());
    }

    #[test]
    fn test_market_builder_custom_tenor() {
        let as_of = date!(2024 - 01 - 01);
        let market = TestMarketBuilder::new(as_of)
            .tenor_years(5.0)
            .rate(0.04)
            .build();

        let curve = market.get_discount("USD-OIS").expect("curve should exist");
        // Should be able to get DF at 5Y (within tenor range)
        let df_5y = curve.df(5.0);
        assert!(df_5y > 0.0 && df_5y < 1.0);
    }

    // =========================================================================
    // TestOptionBuilder Tests
    // =========================================================================

    #[test]
    fn test_option_builder_defaults() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry).build();

        assert_eq!(option.strike, 100.0);
        assert!(matches!(option.option_type, OptionType::Call));
        assert_eq!(option.notional.amount(), 100.0);
        assert_eq!(option.expiry, expiry);
    }

    #[test]
    fn test_option_builder_custom_strike() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry).strike(120.0).build();

        assert_eq!(option.strike, 120.0);
    }

    #[test]
    fn test_option_builder_put_option() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry)
            .option_type(OptionType::Put)
            .build();

        assert!(matches!(option.option_type, OptionType::Put));
    }

    #[test]
    fn test_option_builder_custom_contract_size() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry).contract_size(50.0).build();

        assert_eq!(option.notional.amount(), 50.0);
    }

    #[test]
    fn test_option_builder_custom_ids() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry)
            .id("CUSTOM-OPT")
            .discount_curve_id("CUSTOM-DISC")
            .spot_id("CUSTOM-SPOT")
            .vol_surface_id("CUSTOM-VOL")
            .build();

        assert_eq!(option.id.as_str(), "CUSTOM-OPT");
        assert_eq!(option.discount_curve_id.as_str(), "CUSTOM-DISC");
        assert_eq!(option.spot_id.as_str(), "CUSTOM-SPOT");
        assert_eq!(option.vol_surface_id.as_str(), "CUSTOM-VOL");
    }

    #[test]
    fn test_option_builder_with_dividend_yield() {
        let expiry = date!(2025 - 01 - 01);
        let option = TestOptionBuilder::new(expiry)
            .div_yield_id("DIV-YIELD")
            .build();

        assert_eq!(option.div_yield_id, Some(CurveId::new("DIV-YIELD")));
    }

    // =========================================================================
    // Convenience Function Tests
    // =========================================================================

    #[test]
    fn test_simple_option_market() {
        let as_of = date!(2024 - 01 - 01);
        let market = simple_option_market(as_of, 100.0, 0.20, 0.05);

        // Should have all required components
        assert!(market.get_discount("USD-OIS").is_ok());
        assert!(market.get_surface("SPOT_VOL").is_ok());
        assert!(market.get_price("SPOT").is_ok());

        // Verify spot value
        let spot = market.get_price("SPOT").unwrap();
        let spot_value = match spot {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => m.amount(),
        };
        assert_approx_eq(spot_value, 100.0, tolerances::TIGHT);
    }

    #[test]
    fn test_simple_option_market_with_different_params() {
        let as_of = date!(2024 - 06 - 15);
        let market = simple_option_market(as_of, 150.0, 0.35, 0.02);

        let spot = market.get_price("SPOT").unwrap();
        let spot_value = match spot {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => m.amount(),
        };
        assert_approx_eq(spot_value, 150.0, tolerances::TIGHT);

        let curve = market.get_discount("USD-OIS").unwrap();
        let expected_df = (-0.02_f64).exp();
        assert_approx_eq(curve.df(1.0), expected_df, tolerances::STANDARD);
    }

    #[test]
    fn test_option_market_with_divs() {
        let as_of = date!(2024 - 01 - 01);
        let market = option_market_with_divs(as_of, 100.0, 0.25, 0.05, 0.02);

        // Should have dividend yield
        assert!(market.get_price("SPOT_DIV").is_ok());
        let div = market.get_price("SPOT_DIV").unwrap();
        let div_value = match div {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => m.amount(),
        };
        assert_approx_eq(div_value, 0.02, tolerances::TIGHT);
    }

    #[test]
    fn test_simple_call_option() {
        let expiry = date!(2025 - 06 - 30);
        let option = simple_call_option(expiry, 110.0);

        assert!(matches!(option.option_type, OptionType::Call));
        assert_eq!(option.strike, 110.0);
        assert_eq!(option.expiry, expiry);
    }

    #[test]
    fn test_simple_put_option() {
        let expiry = date!(2025 - 06 - 30);
        let option = simple_put_option(expiry, 90.0);

        assert!(matches!(option.option_type, OptionType::Put));
        assert_eq!(option.strike, 90.0);
        assert_eq!(option.expiry, expiry);
    }

    #[test]
    fn test_test_option_returns_builder() {
        let expiry = date!(2025 - 01 - 01);
        let option = test_option(expiry).strike(120.0).build();

        assert_eq!(option.strike, 120.0);
    }

    #[test]
    fn test_test_market_returns_builder() {
        let as_of = date!(2024 - 01 - 01);
        let market = test_market(as_of).spot(200.0).vol(0.40).build();

        let spot = market.get_price("SPOT").unwrap();
        let spot_value = match spot {
            MarketScalar::Unitless(v) => *v,
            MarketScalar::Price(m) => m.amount(),
        };
        assert_approx_eq(spot_value, 200.0, tolerances::TIGHT);
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_market_builder_zero_rate() {
        let as_of = date!(2024 - 01 - 01);
        // Zero rate should work (DF = 1.0 everywhere)
        let market = TestMarketBuilder::new(as_of).rate(0.0).build();
        let curve = market.get_discount("USD-OIS").unwrap();
        assert_approx_eq(curve.df(1.0), 1.0, tolerances::TIGHT);
    }

    #[test]
    fn test_market_builder_low_vol() {
        let as_of = date!(2024 - 01 - 01);
        // Very low vol should work
        let market = TestMarketBuilder::new(as_of).vol(0.01).build();
        assert!(market.get_surface("SPOT_VOL").is_ok());
    }

    #[test]
    fn test_market_builder_high_vol() {
        let as_of = date!(2024 - 01 - 01);
        // High vol (100%) should work
        let market = TestMarketBuilder::new(as_of).vol(1.0).build();
        assert!(market.get_surface("SPOT_VOL").is_ok());
    }

    #[test]
    fn test_option_builder_deep_itm_strike() {
        let expiry = date!(2025 - 01 - 01);
        // Very low strike (deep ITM call)
        let option = TestOptionBuilder::new(expiry).strike(10.0).build();
        assert_eq!(option.strike, 10.0);
    }

    #[test]
    fn test_option_builder_deep_otm_strike() {
        let expiry = date!(2025 - 01 - 01);
        // Very high strike (deep OTM call)
        let option = TestOptionBuilder::new(expiry).strike(500.0).build();
        assert_eq!(option.strike, 500.0);
    }
}
