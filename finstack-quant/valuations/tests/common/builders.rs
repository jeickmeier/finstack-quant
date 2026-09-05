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
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, SettlementType};

// Market Context Builders

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

// Equity Option Builder

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
    div_yield_id: Option<PriceId>,
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
        self.div_yield_id = Some(PriceId::new(id));
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
            notional: Money::new(self.notional_amount, self.currency).expect("valid money fixture"),
            day_count: self.day_count,
            theta_day_basis: Default::default(),
            settlement: self.settlement,
            exercise: None,
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
