//! Equity-option builders for the umbrella test suite.
//!
//! Market-context builders live in `finstack_quant_valuations_test_support::builders`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, SettlementType};

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
            quantity: self.notional_amount,
            currency: self.currency,
            day_count: self.day_count,
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
            exercise_dates: None,
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
