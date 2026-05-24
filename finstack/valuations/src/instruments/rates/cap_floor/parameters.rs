//! Interest rate option specific parameters.
//!
//! This module groups parameters used to construct cap/floor instruments.
//! It mirrors the structure used across other instruments (e.g., `cds`).

use super::types::RateOptionType;
use finstack_core::{
    dates::{BusinessDayConvention, DayCount, StubKind, Tenor},
    money::Money,
    types::Rate,
};
use rust_decimal::Decimal;

/// Interest rate option specific parameters.
///
/// Groups parameters specific to interest rate options (caps/floors).
#[derive(Debug, Clone)]
pub struct CapFloorParams {
    /// Type of rate option (Cap/Floor)
    pub rate_option_type: RateOptionType,
    /// Notional amount
    pub notional: Money,
    /// Strike rate
    pub strike: Decimal,
    /// Payment frequency
    pub frequency: Tenor,
    /// Day count convention
    pub day_count: DayCount,
    /// Stub convention for schedule generation
    pub stub: StubKind,
    /// Business day convention for schedule generation
    pub bdc: BusinessDayConvention,
    /// Optional holiday calendar identifier for schedule generation
    pub calendar_id: Option<&'static str>,
}

impl CapFloorParams {
    fn to_decimal(value: f64) -> finstack_core::Result<Decimal> {
        Decimal::try_from(value).map_err(|_| {
            finstack_core::Error::Validation(format!(
                "Strike {value} is not representable as Decimal (must be finite)"
            ))
        })
    }

    /// Create cap parameters.
    pub fn cap(
        notional: Money,
        strike: f64,
        frequency: Tenor,
        day_count: DayCount,
    ) -> finstack_core::Result<Self> {
        Ok(Self {
            rate_option_type: RateOptionType::Cap,
            notional,
            strike: Self::to_decimal(strike)?,
            frequency,
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        })
    }

    /// Create cap parameters using a typed strike rate.
    pub fn cap_rate(
        notional: Money,
        strike: Rate,
        frequency: Tenor,
        day_count: DayCount,
    ) -> finstack_core::Result<Self> {
        Ok(Self {
            rate_option_type: RateOptionType::Cap,
            notional,
            strike: Self::to_decimal(strike.as_decimal())?,
            frequency,
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        })
    }

    /// Create floor parameters.
    pub fn floor(
        notional: Money,
        strike: f64,
        frequency: Tenor,
        day_count: DayCount,
    ) -> finstack_core::Result<Self> {
        Ok(Self {
            rate_option_type: RateOptionType::Floor,
            notional,
            strike: Self::to_decimal(strike)?,
            frequency,
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        })
    }

    /// Create floor parameters using a typed strike rate.
    pub fn floor_rate(
        notional: Money,
        strike: Rate,
        frequency: Tenor,
        day_count: DayCount,
    ) -> finstack_core::Result<Self> {
        Ok(Self {
            rate_option_type: RateOptionType::Floor,
            notional,
            strike: Self::to_decimal(strike.as_decimal())?,
            frequency,
            day_count,
            stub: StubKind::ShortFront,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
        })
    }
}
