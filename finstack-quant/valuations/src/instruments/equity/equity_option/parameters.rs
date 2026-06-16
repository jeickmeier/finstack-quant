//! Equity option specific parameters.

use crate::instruments::{ExerciseStyle, OptionType, SettlementType};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, PriceId};

/// Equity option specific parameters.
///
/// Groups parameters specific to equity options.
#[derive(Debug, Clone)]
pub struct EquityOptionParams {
    /// Strike price in underlying price units
    pub strike: f64,
    /// Option expiry date
    pub expiry: Date,
    /// Option type (Call/Put)
    pub option_type: OptionType,
    /// Exercise style (European/American/Bermudan)
    pub exercise_style: ExerciseStyle,
    /// Settlement type (Cash/Physical)
    pub settlement: SettlementType,
    /// Notional amount for valuation scaling.
    pub notional: Money,
}

/// Explicit market data identifiers for pricing an equity option.
#[derive(Debug, Clone)]
pub struct EquityOptionMarketData {
    /// Discount curve used for present value calculations.
    pub discount_curve_id: CurveId,
    /// Spot identifier for the underlying equity.
    pub spot_id: PriceId,
    /// Volatility surface used for option pricing.
    pub vol_surface_id: CurveId,
    /// Optional continuous dividend-yield curve identifier.
    pub div_yield_id: Option<CurveId>,
}

impl EquityOptionParams {
    /// Create new equity option parameters
    pub fn new(strike: f64, expiry: Date, option_type: OptionType, notional: Money) -> Self {
        Self {
            strike,
            expiry,
            option_type,
            exercise_style: ExerciseStyle::European,
            settlement: SettlementType::Physical,
            notional,
        }
    }

    /// Create European call option parameters
    pub fn european_call(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Call, notional)
    }

    /// Create European put option parameters
    pub fn european_put(strike: f64, expiry: Date, notional: Money) -> Self {
        Self::new(strike, expiry, OptionType::Put, notional)
    }

    /// Set exercise style
    pub fn with_exercise_style(mut self, style: ExerciseStyle) -> Self {
        self.exercise_style = style;
        self
    }

    /// Set settlement type
    pub fn with_settlement(mut self, settlement: SettlementType) -> Self {
        self.settlement = settlement;
        self
    }
}

impl EquityOptionMarketData {
    /// Create explicit market-data identifiers for an equity option.
    pub fn new(
        discount_curve_id: impl Into<CurveId>,
        spot_id: impl Into<PriceId>,
        vol_surface_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            discount_curve_id: discount_curve_id.into(),
            spot_id: spot_id.into(),
            vol_surface_id: vol_surface_id.into(),
            div_yield_id: None,
        }
    }

    /// Attach a continuous dividend-yield curve identifier.
    pub fn with_dividend_yield(mut self, div_yield_id: impl Into<CurveId>) -> Self {
        self.div_yield_id = Some(div_yield_id.into());
        self
    }
}
