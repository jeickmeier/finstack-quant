//! Dividend schedules for equity and ETF pricing.
//!
//! Provides types for storing and querying dividend events (cash, stock, yield)
//! used in equity option pricing, dividend swap valuation, and ETF analytics.
//! Schedules are stored in `MarketContext` and keyed by `CurveId` for consistency
//! with other market data.
//!
//! # Dividend Types
//!
//! - **Cash dividends**: Fixed amount in a currency (most common)
//! - **Stock dividends**: Proportional share distribution (e.g., 5% stock dividend)
//! - **Yield**: Continuous dividend yield approximation for modeling
//!
//! # Use Cases
//!
//! - **Equity option pricing**: Discrete dividend adjustments in Black-Scholes
//! - **Dividend futures**: Contract on future dividend payments
//! - **Total return swaps**: Dividend reinvestment calculations
//! - **Ex-dividend adjustments**: Forward price and strike adjustments
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::dividends::{DividendSchedule, DividendScheduleBuilder};
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let d1 = Date::from_calendar_date(2025, Month::March, 15).expect("Valid date");
//! let d2 = Date::from_calendar_date(2025, Month::June, 15).expect("Valid date");
//!
//! let schedule = DividendScheduleBuilder::new("AAPL-DIVS")
//!     .underlying("AAPL")
//!     .currency(Currency::USD)
//!     .cash(d1, Money::new(0.24, Currency::USD).expect("valid money fixture"))
//!     .cash(d2, Money::new(0.25, Currency::USD).expect("valid money fixture"))
//!     .build()
//!     .expect("DividendScheduleBuilder should succeed");
//!
//! assert_eq!(schedule.get_events().len(), 2);
//! ```

use crate::currency::Currency;
use crate::dates::Date;
use crate::money::Money;
use crate::types::CurveId;
use crate::{Error, Result};

use serde::{Deserialize, Serialize};

/// Type of dividend event.
///
/// Distinguishes between cash payments, stock distributions, and continuous
/// yield approximations used in different pricing models.
///
/// # Variants
///
/// - **Cash**: Actual dividend payment (quarterly, semi-annual, etc.)
/// - **Stock**: Share distribution (less common, complicates option pricing)
/// - **Yield**: Continuous approximation for analytical models
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DividendKind {
    /// Cash dividend payment.
    ///
    /// Most common type. Ex-dividend date reduces stock price by dividend amount.
    Cash(Money),

    /// Continuous dividend yield (annualized fraction).
    ///
    /// Used in analytical models (Black-Scholes with dividends) that assume
    /// continuous payout rather than discrete payments.
    Yield(f64),

    /// Stock dividend (share distribution).
    ///
    /// Increases share count proportionally. For example, 5% stock dividend
    /// means each shareholder receives 0.05 additional shares per share held.
    Stock {
        /// Stock distribution ratio; 0.05 corresponds to a 5% stock dividend.
        ratio: f64,
    },
}

/// A dated dividend event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct DividendEvent {
    /// Ex-dividend date.
    #[serde(with = "crate::wire::date")]
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub date: Date,
    /// Event kind.
    pub kind: DividendKind,
}

/// Dividend schedule for an equity or ETF underlying.
///
/// Contains a time-ordered sequence of dividend events used for pricing equity
/// derivatives and calculating total return. The schedule can be referenced by
/// multiple instruments via its [`CurveId`] in the market context.
///
/// # Usage in Pricing
///
/// - **Discrete dividends**: Subtract PV of future dividends from spot for option pricing
/// - **Ex-dividend adjustments**: Reduce forward price by dividend amount
/// - **Dividend futures**: Sum dividends in contract period
/// - **Total return**: Include dividend reinvestment in performance
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::dividends::DividendSchedule;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::dates::Date;
/// use time::Month;
///
/// let schedule = DividendSchedule::builder("AAPL-DIVS")
///     .underlying("AAPL")
///     .currency(Currency::USD)
///     .cash(
///         Date::from_calendar_date(2025, Month::March, 15).expect("Valid date"),
///         Money::new(0.24, Currency::USD).expect("valid money fixture")
///     ).build().expect("Valid dividends");
///
/// assert_eq!(schedule.get_events().len(), 1);
/// ```
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DividendSchedule {
    /// Unique identifier of this schedule in the market context.
    id: CurveId,
    /// Optional display symbol/ticker for convenience.
    underlying: Option<String>,
    /// Sorted events by date (ascending).
    events: Vec<DividendEvent>,
    /// Quote currency for cash dividends (optional metadata).
    currency: Option<Currency>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DividendScheduleWire {
    id: CurveId,
    underlying: Option<String>,
    events: Vec<DividendEvent>,
    currency: Option<Currency>,
}

impl<'de> Deserialize<'de> for DividendSchedule {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        Self::try_from(DividendScheduleWire::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl TryFrom<DividendScheduleWire> for DividendSchedule {
    type Error = Error;
    fn try_from(wire: DividendScheduleWire) -> Result<Self> {
        DividendScheduleBuilder {
            id: wire.id,
            underlying: wire.underlying,
            events: wire.events,
            currency: wire.currency,
        }
        .build()
    }
}

impl DividendSchedule {
    /// Create a [`DividendScheduleBuilder`] for constructing a new schedule.
    ///
    /// This is the preferred entry point, consistent with other curve builders.
    #[must_use]
    pub fn builder(id: impl Into<CurveId>) -> DividendScheduleBuilder {
        DividendScheduleBuilder::new(id)
    }

    /// Identifier used to retrieve this schedule from market data.
    pub fn get_id(&self) -> &CurveId {
        &self.id
    }

    /// Optional human-readable underlying ticker.
    pub fn get_underlying(&self) -> Option<&str> {
        self.underlying.as_deref()
    }

    /// Validated dividend events in ascending date order.
    pub fn get_events(&self) -> &[DividendEvent] {
        &self.events
    }

    /// Optional default cash-dividend currency metadata.
    pub fn get_currency(&self) -> Option<Currency> {
        self.currency
    }

    /// Return events filtered to a date range inclusive.
    pub fn events_between(&self, start: Date, end: Date) -> Vec<&DividendEvent> {
        self.events
            .iter()
            .filter(|e| e.date >= start && e.date <= end)
            .collect()
    }

    /// Convenience: cash dividends only (ignoring yield/stock entries).
    pub fn cash_events(&self) -> impl Iterator<Item = (Date, &Money)> {
        self.events.iter().filter_map(|e| match &e.kind {
            DividendKind::Cash(m) => Some((e.date, m)),
            _ => None,
        })
    }

    /// Validate schedule content (positive cash amounts, non-negative ratios).
    pub fn validate(&self) -> Result<()> {
        for ev in &self.events {
            match &ev.kind {
                DividendKind::Cash(m) => {
                    if m.amount() < 0.0 {
                        return Err(Error::Input(crate::error::InputError::NegativeValue));
                    }
                }
                DividendKind::Yield(y) => {
                    if !y.is_finite() {
                        return Err(Error::Input(crate::error::InputError::Invalid));
                    }
                }
                DividendKind::Stock { ratio } => {
                    if *ratio < 0.0 {
                        return Err(Error::Input(crate::error::InputError::NegativeValue));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Builder for [`DividendSchedule`].
pub struct DividendScheduleBuilder {
    id: CurveId,
    underlying: Option<String>,
    currency: Option<Currency>,
    events: Vec<DividendEvent>,
}

impl DividendScheduleBuilder {
    /// Start a new builder with identifier `id`.
    pub fn new(id: impl Into<CurveId>) -> Self {
        Self {
            id: id.into(),
            underlying: None,
            currency: None,
            events: Vec::new(),
        }
    }

    /// Optional underlying/ticker.
    pub fn underlying(mut self, name: impl Into<String>) -> Self {
        self.underlying = Some(name.into());
        self
    }

    /// Optional default currency for cash dividends.
    pub fn currency(mut self, ccy: Currency) -> Self {
        self.currency = Some(ccy);
        self
    }

    /// Add a cash dividend.
    pub fn cash(mut self, date: Date, amount: Money) -> Self {
        self.events.push(DividendEvent {
            date,
            kind: DividendKind::Cash(amount),
        });
        self
    }

    /// Add a yield dividend.
    pub fn yield_div(mut self, date: Date, y: f64) -> Self {
        self.events.push(DividendEvent {
            date,
            kind: DividendKind::Yield(y),
        });
        self
    }

    /// Add a stock dividend.
    pub fn stock(mut self, date: Date, ratio: f64) -> Self {
        self.events.push(DividendEvent {
            date,
            kind: DividendKind::Stock { ratio },
        });
        self
    }

    /// Build the schedule (events are sorted by date).
    pub fn build(mut self) -> Result<DividendSchedule> {
        self.events.sort_by_key(|e| e.date);
        let schedule = DividendSchedule {
            id: self.id,
            underlying: self.underlying,
            events: self.events,
            currency: self.currency,
        };
        schedule.validate()?;
        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    #[test]
    fn build_and_filter_schedule() {
        let d1 = Date::from_calendar_date(2025, Month::January, 15).expect("Valid test date");
        let d2 = Date::from_calendar_date(2025, Month::March, 15).expect("Valid test date");
        let d3 = Date::from_calendar_date(2025, Month::June, 15).expect("Valid test date");

        let sched = DividendScheduleBuilder::new("AAPL-DIVS")
            .underlying("AAPL")
            .cash(
                d1,
                Money::new(0.24, Currency::USD).expect("valid money fixture"),
            )
            .cash(
                d2,
                Money::new(0.24, Currency::USD).expect("valid money fixture"),
            )
            .stock(d3, 0.02)
            .build()
            .expect("DividendScheduleBuilder should succeed in test");

        assert_eq!(sched.events.len(), 3);
        let between = sched.events_between(d1, d2);
        assert_eq!(between.len(), 2);
    }
}
