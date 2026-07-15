//! FX Spot types and implementations.
//!
//! This file defines the `FxSpot` instrument shape and integrates with the
//! standard instrument macro. Pricing lives on the instrument and the standard
//! registry reaches it through `GenericInstrumentPricer`; metrics live under
//! `metrics/`.
//!
//! # FX Quote Conventions (Market Standards Review - Week 5)
//!
//! ## Base Currency vs Quote Currency
//!
//! FX rates are always quoted as **CCY1/CCY2** where:
//! - **CCY1 (base):** The currency being priced (numerator)
//! - **CCY2 (quote):** The currency used for pricing (denominator)
//!
//! **Example: EUR/USD = 1.10**
//! - Base: EUR (1 Euro)
//! - Quote: USD (costs 1.10 US Dollars)
//! - Interpretation: "1 EUR costs 1.10 USD"
//!
//! ## Common Market Conventions
//!
//! | Pair | Direction | Interpretation |
//! |------|-----------|----------------|
//! | EUR/USD | Euro vs Dollar | "Euro in Dollar" - price of 1 EUR in USD |
//! | GBP/USD | Pound vs Dollar | "Cable" - price of 1 GBP in USD |
//! | USD/JPY | Dollar vs Yen | "Dollar-yen" - price of 1 USD in JPY |
//! | AUD/USD | Aussie vs Dollar | Price of 1 AUD in USD |
//!
//! ## Reciprocal Rates
//!
//! The reciprocal rate swaps base and quote:
//! - EUR/USD = 1.10 → USD/EUR = 1/1.10 = 0.909
//! - GBP/USD = 1.25 → USD/GBP = 1/1.25 = 0.80
//!
//! ## In This Implementation
//!
//! The `FxSpot` instrument stores:
//! ```ignore
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::types::InstrumentId;
//! use finstack_quant_valuations::instruments::FxSpot;
//!
//! // CCY1 (base) is the currency being priced; CCY2 (quote) is the pricing currency.
//! let mut eur_usd = FxSpot::new(InstrumentId::from("EURUSD"), Currency::EUR, Currency::USD);
//! // How many units of `quote` per 1 unit of `base`
//! eur_usd.spot_rate = Some(1.10);
//! ```
//!
//! Example:
//! ```ignore
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::types::InstrumentId;
//! use finstack_quant_valuations::instruments::FxSpot;
//!
//! let eur_usd = FxSpot::new(InstrumentId::from("EURUSD"), Currency::EUR, Currency::USD);
//! // If spot_rate = 1.10, this means: 1 EUR = 1.10 USD
//! ```

use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::Result;

/// FX Spot instrument (1 unit of `base` priced in `quote`).
///
/// Represents the spot exchange rate between two currencies following
/// standard market quoting conventions (base/quote or CCY1/CCY2).
///
/// # Quote Convention
///
/// The rate is interpreted as: **1 unit of base = rate units of quote**
///
/// For example, if `base = EUR`, `quote = USD`, and `spot_rate = 1.10`:
/// - 1 EUR = 1.10 USD
/// - This is the "EUR/USD" rate
///
/// # Settlement
///
/// FX spot typically settles T+2 (two business days after trade date).
/// This can be customized via `settlement_lag_days`.
///
/// See module-level documentation for comprehensive FX quoting conventions.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = FxSpot::validate_economics)]
#[serde(deny_unknown_fields, try_from = "FxSpotUnchecked")]
pub struct FxSpot {
    /// Unique identifier for the FX pair
    pub id: InstrumentId,
    /// Base currency (the currency being priced)
    pub base_currency: Currency,
    /// Quote currency (the currency used for pricing)
    pub quote_currency: Currency,
    /// Optional settlement date (T+2 typically for spot)
    #[builder(optional)]
    #[schemars(with = "Option<String>")]
    pub settlement: Option<Date>,
    /// Optional settlement lag in business days when `settlement` is not provided (default: 2)
    #[builder(optional)]
    pub settlement_lag_days: Option<i32>,
    /// Optional spot rate (if not provided, will look up from market data)
    #[builder(optional)]
    pub spot_rate: Option<f64>,
    /// Optional quote-currency discount curve for PV-ing the settlement
    /// cashflow. When absent the settlement amount is reported undiscounted
    /// (a 1–2 day effect for standard spot lags).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_curve_id: Option<finstack_quant_core::types::CurveId>,
    /// Notional amount in base currency.
    pub notional: Money,
    /// Per-instrument pricing/sensitivity override knobs.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Business day convention to apply when adjusting settlement (default: ModifiedFollowing)
    ///
    /// Note: Default changed from `Following` to `ModifiedFollowing` in v0.8.0 to align
    /// with ISDA standard FX settlement conventions.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    pub bdc: BusinessDayConvention,
    /// Optional base currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    #[builder(optional)]
    pub base_calendar_id: Option<String>,
    /// Optional quote currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    #[builder(optional)]
    pub quote_calendar_id: Option<String>,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct FxSpotUnchecked {
    id: InstrumentId,
    base_currency: Currency,
    quote_currency: Currency,
    #[schemars(with = "Option<String>")]
    settlement: Option<Date>,
    settlement_lag_days: Option<i32>,
    spot_rate: Option<f64>,
    #[serde(default)]
    discount_curve_id: Option<finstack_quant_core::types::CurveId>,
    notional: Money,
    #[serde(default = "crate::serde_defaults::bdc_modified_following")]
    bdc: BusinessDayConvention,
    base_calendar_id: Option<String>,
    quote_calendar_id: Option<String>,
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    attributes: Attributes,
}

impl TryFrom<FxSpotUnchecked> for FxSpot {
    type Error = finstack_quant_core::Error;

    fn try_from(value: FxSpotUnchecked) -> std::result::Result<Self, Self::Error> {
        let spot = Self {
            id: value.id,
            base_currency: value.base_currency,
            quote_currency: value.quote_currency,
            settlement: value.settlement,
            settlement_lag_days: value.settlement_lag_days,
            spot_rate: value.spot_rate,
            discount_curve_id: value.discount_curve_id,
            notional: value.notional,
            pricing_overrides: value.pricing_overrides,
            bdc: value.bdc,
            base_calendar_id: value.base_calendar_id,
            quote_calendar_id: value.quote_calendar_id,
            attributes: value.attributes,
        };
        spot.validate_economics()?;
        Ok(spot)
    }
}

impl FxSpot {
    /// Create a new FX spot instrument.
    ///
    /// Default business day convention is `ModifiedFollowing` per ISDA standard.
    pub fn new(id: InstrumentId, base_currency: Currency, quote_currency: Currency) -> Self {
        Self {
            id,
            base_currency,
            quote_currency,
            settlement: None,
            settlement_lag_days: None,
            spot_rate: None,
            discount_curve_id: None,
            notional: Money::new(1.0, base_currency),
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            bdc: BusinessDayConvention::ModifiedFollowing,
            base_calendar_id: None,
            quote_calendar_id: None,
            attributes: Attributes::new(),
        }
    }

    /// Compute the effective settlement date for this FX spot.
    ///
    /// Returns the settlement date adjusted for business days according to the
    /// configured calendars and business day convention.
    ///
    /// # Joint Calendar Support
    ///
    /// When both `base_calendar_id` and `quote_calendar_id` are provided, settlement
    /// uses joint calendar logic: a date is valid only if it's a business day in both
    /// currencies' calendars. This matches professional FX settlement conventions.
    ///
    pub fn effective_settlement_date(&self, as_of: Date) -> Result<Date> {
        use crate::instruments::common_impl::fx_dates::{
            adjust_joint_calendar, fx_spot_date_for_pair,
        };

        // Check if we should use joint calendar logic
        let use_joint_calendar =
            self.base_calendar_id.is_some() || self.quote_calendar_id.is_some();

        if let Some(date) = self.settlement {
            // Explicit settlement date provided - adjust for business days
            if use_joint_calendar {
                adjust_joint_calendar(
                    date,
                    self.bdc,
                    self.base_calendar_id.as_deref(),
                    self.quote_calendar_id.as_deref(),
                )
            } else {
                Ok(date)
            }
        } else {
            // Compute T+N from as_of date
            let lag_days = self.settlement_lag_days.unwrap_or(2);

            if use_joint_calendar {
                // CLS-consistent spot roll: a US holiday on an intermediate day
                // does not delay a USD pair's spot date (2026-06-09 core quant
                // review, FX spot finding).
                fx_spot_date_for_pair(
                    as_of,
                    lag_days as u32,
                    self.base_currency,
                    self.quote_currency,
                    self.base_calendar_id.as_deref(),
                    self.quote_calendar_id.as_deref(),
                )
            } else {
                Ok(as_of.add_weekdays(lag_days))
            }
        }
    }

    /// Set the settlement date
    pub fn with_settlement(mut self, date: Date) -> Self {
        self.settlement = Some(date);
        self
    }

    /// Set the spot rate with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the rate is negative or zero, as FX rates must be positive.
    /// A zero rate would imply one currency is worthless, and negative rates are
    /// economically meaningless for spot FX.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::{currency::Currency, types::InstrumentId};
    /// use finstack_quant_valuations::instruments::FxSpot;
    ///
    /// let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
    ///     .with_rate(1.10)
    ///     .expect("valid rate");
    /// ```
    pub fn with_rate(mut self, rate: f64) -> finstack_quant_core::Result<Self> {
        if !rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX spot rate must be finite (got {}). NaN and Infinity are not valid rates.",
                rate
            )));
        }
        if rate < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX spot rate cannot be negative (got {}). FX rates must be positive.",
                rate
            )));
        }
        if rate == 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "FX spot rate cannot be zero. A zero rate implies the base currency is worthless."
                    .to_string(),
            ));
        }
        self.spot_rate = Some(rate);
        Ok(self)
    }

    /// Set the notional amount with currency validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the notional currency doesn't match the base currency.
    pub fn with_notional(mut self, notional: Money) -> finstack_quant_core::Result<Self> {
        if notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: notional.currency(),
            });
        }
        self.notional = notional;
        Ok(self)
    }

    /// Set the business day convention
    pub fn with_bdc(mut self, bdc: BusinessDayConvention) -> Self {
        self.bdc = bdc;
        self
    }

    /// Set the holiday calendar identifier used for settlement adjustment.
    ///
    /// Set the base currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::{currency::Currency, types::InstrumentId};
    /// use finstack_quant_valuations::instruments::FxSpot;
    ///
    /// let eur_usd = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
    ///     .with_base_calendar_id("TARGET")    // ECB TARGET calendar for EUR
    ///     .with_quote_calendar_id("USNY");    // US New York calendar for USD
    /// ```
    pub fn with_base_calendar_id(mut self, id: impl Into<String>) -> Self {
        self.base_calendar_id = Some(id.into());
        self
    }

    /// Set the quote currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    pub fn with_quote_calendar_id(mut self, id: impl Into<String>) -> Self {
        self.quote_calendar_id = Some(id.into());
        self
    }

    /// Set the settlement lag in business days (positive for T+N, negative for T-N).
    pub fn with_settlement_lag_days(mut self, lag_days: i32) -> Self {
        self.settlement_lag_days = Some(lag_days);
        self
    }

    /// Get the configured notional amount.
    pub fn effective_notional(&self) -> Money {
        self.notional
    }

    fn validate_economics(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxSpot",
        )?;
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        // The builder and serde paths must enforce the same `spot_rate`
        // invariants as `with_rate`: finite and strictly positive.
        if let Some(rate) = self.spot_rate {
            if !rate.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FX spot rate must be finite (got {rate}). \
                     NaN and Infinity are not valid rates."
                )));
            }
            if rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FX spot rate must be positive (got {rate})."
                )));
            }
        }
        Ok(())
    }

    /// Create a representative EUR/USD spot example with 1M EUR notional.
    ///
    /// Useful for tests, demos, and documentation examples.
    pub fn example() -> Result<Self> {
        Ok(
            Self::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
                .with_notional(Money::new(1_000_000.0, Currency::EUR))?
                .with_rate(1.10)?
                .with_settlement_lag_days(2),
        )
    }

    /// Standard FX pair name (e.g., "EURUSD")
    pub fn pair_name(&self) -> String {
        format!("{}{}", self.base_currency, self.quote_currency)
    }

    /// Create an FX spot with T+1 settlement (used for USD/CAD and other same-day pairs).
    ///
    /// Per market convention, USD/CAD settles T+1 rather than the standard T+2.
    /// This convenience method creates an FX spot with the appropriate settlement lag.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::{currency::Currency, types::InstrumentId};
    /// use finstack_quant_valuations::instruments::FxSpot;
    ///
    /// // USD/CAD with standard T+1 settlement
    /// let usd_cad = FxSpot::new_t1(
    ///     InstrumentId::new("USDCAD"),
    ///     Currency::USD,
    ///     Currency::CAD,
    /// );
    /// ```
    pub fn new_t1(id: InstrumentId, base_currency: Currency, quote_currency: Currency) -> Self {
        Self::new(id, base_currency, quote_currency).with_settlement_lag_days(1)
    }

    /// Check if this is a same-region pair that typically settles T+1.
    ///
    /// Returns `true` for pairs like USD/CAD and USD/TRY that conventionally
    /// settle in one business day rather than the standard T+2.
    ///
    /// # Market Convention Reference
    ///
    /// Per Bloomberg/Reuters FX settlement conventions:
    /// - **USD/CAD**: North American same-day zone (T+1)
    /// - **USD/TRY**: Turkish Lira settles T+1 per Istanbul market convention
    ///
    /// Note: This is informational only and does not affect settlement calculation.
    /// Use `new_t1` or `with_settlement_lag_days(1)` to set T+1 settlement.
    pub fn is_t1_pair(&self) -> bool {
        // USD/CAD and USD/TRY are the most common T+1 pairs
        let pair = (self.base_currency, self.quote_currency);
        matches!(
            pair,
            (Currency::USD, Currency::CAD)
                | (Currency::CAD, Currency::USD)
                | (Currency::USD, Currency::TRY)
                | (Currency::TRY, Currency::USD)
        )
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxSpot {
    impl_instrument_base!(crate::pricer::InstrumentType::FxSpot);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        if let Some(discount_curve) = &self.discount_curve_id {
            deps.add_discount_curve(discount_curve.clone());
        }
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        // PV the settlement cashflow on the configured quote-currency
        // discount curve; without one the flow is reported undiscounted
        // (1–2 days of carry for standard spot lags).
        let disc = self
            .discount_curve_id
            .as_ref()
            .map(|id| market.get_discount(id))
            .transpose()?;
        self.dated_cashflows(market, as_of)?.into_iter().try_fold(
            Money::new(0.0, self.quote_currency),
            |acc, (date, amount)| {
                let df = match disc.as_ref() {
                    Some(curve) => curve.df_between_dates(as_of, date)?,
                    None => 1.0,
                };
                acc.checked_add(Money::new(amount.amount() * df, amount.currency()))
            },
        )
    }

    fn value_raw(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.value(market, as_of)?.amount())
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

// Implement CurveDependencies for DV01 calculator
impl crate::instruments::common_impl::traits::CurveDependencies for FxSpot {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        let mut builder = crate::instruments::common_impl::traits::InstrumentCurves::builder();
        if let Some(ref id) = self.discount_curve_id {
            builder = builder.discount(id.clone());
        }
        builder.build()
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for FxSpot {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        self.validate_economics()?;
        let settle_date = self.effective_settlement_date(as_of)?;

        if settle_date <= as_of {
            return Ok(crate::cashflow::traits::schedule_from_classified_flows(
                Vec::new(),
                finstack_quant_core::dates::DayCount::Act365F,
                crate::cashflow::traits::ScheduleBuildOpts {
                    notional_hint: self.notional(),
                    meta: crate::cashflow::builder::CashFlowMeta {
                        representation:
                            crate::cashflow::builder::CashflowRepresentation::NoResidual,
                        ..Default::default()
                    },
                },
            ));
        }

        let flows = {
            // Future settlement - use explicit spot_rate if provided, otherwise query FX matrix
            let rate = if let Some(rate) = self.spot_rate {
                rate
            } else {
                // Try market context FX matrix
                let matrix = curves.fx().ok_or_else(|| {
                    finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                        id: "fx_matrix".to_string(),
                    })
                })?;
                let q = finstack_quant_core::money::fx::FxQuery::new(
                    self.base_currency,
                    self.quote_currency,
                    settle_date,
                );
                matrix.as_ref().rate(q)?.rate
            };
            let value = Money::new(
                self.effective_notional().amount() * rate,
                self.quote_currency,
            );
            vec![(settle_date, value)]
        };

        let schedule = crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            crate::cashflow::primitives::CFKind::Notional,
            finstack_quant_core::dates::DayCount::Act365F, // Standard for FX spot
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Contractual,
                    ..Default::default()
                },
            },
        );
        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    #[test]
    fn test_fx_spot_creation() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD);
        assert_eq!(spot.base_currency, Currency::EUR);
        assert_eq!(spot.quote_currency, Currency::USD);
        assert_eq!(spot.pair_name(), "EURUSD");
    }

    #[test]
    fn test_fx_spot_with_explicit_rate() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_rate(1.10)
            .expect("valid rate");

        let market = MarketContext::new();
        let as_of = date(2025, Month::January, 15);
        let pv = spot
            .value(&market, as_of)
            .expect("should price with explicit rate");

        // 1 EUR * 1.10 = 1.10 USD
        assert!((pv.amount() - 1.10).abs() < 1e-10);
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn test_fx_spot_effective_settlement_date_default() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD);

        // Wednesday -> should settle Friday (T+2 weekdays)
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 17)); // Friday
    }

    #[test]
    fn test_fx_spot_effective_settlement_date_explicit() {
        let settle_date = date(2025, Month::January, 20);
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_settlement(settle_date);

        let as_of = date(2025, Month::January, 15);
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, settle_date);
    }

    #[test]
    fn test_fx_spot_effective_settlement_date_custom_lag() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_settlement_lag_days(1);

        // Wednesday -> should settle Thursday (T+1 weekdays)
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 16)); // Thursday
    }

    #[test]
    fn test_fx_spot_returns_zero_when_settled() {
        let settle_date = date(2025, Month::January, 10);
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_settlement(settle_date)
            .with_rate(1.10)
            .expect("valid rate");

        let market = MarketContext::new();
        let as_of = date(2025, Month::January, 15); // After settlement
        let pv = spot.value(&market, as_of).expect("should price");

        assert_eq!(pv.amount(), 0.0, "Should return zero when settled");
    }

    #[test]
    fn test_fx_spot_notional_currency_validation() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD);
        let wrong_notional = Money::new(1000.0, Currency::GBP);

        let result = spot.with_notional(wrong_notional);
        assert!(result.is_err(), "Should reject notional in wrong currency");
    }

    #[test]
    fn test_fx_spot_with_notional() {
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_notional(Money::new(1_000_000.0, Currency::EUR))
            .expect("valid notional")
            .with_rate(1.10)
            .expect("valid rate");

        let market = MarketContext::new();
        let as_of = date(2025, Month::January, 15);
        let pv = spot.value(&market, as_of).expect("should price");

        // 1,000,000 EUR * 1.10 = 1,100,000 USD
        assert!((pv.amount() - 1_100_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_fx_spot_rejects_negative_rate() {
        let result =
            FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD).with_rate(-1.10);
        assert!(result.is_err(), "Should reject negative rate");

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("negative") || err_msg.contains("spot_rate"),
            "Error should mention negative rate"
        );
    }

    #[test]
    fn test_fx_spot_rejects_nan_rate() {
        let result = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_rate(f64::NAN);
        assert!(result.is_err(), "Should reject NaN rate");

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("finite") || err_msg.contains("NaN"),
            "Error should mention finite: {}",
            err_msg
        );
    }

    #[test]
    fn test_fx_spot_rejects_infinity_rate() {
        let result = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_rate(f64::INFINITY);
        assert!(result.is_err(), "Should reject Infinity rate");
    }

    #[test]
    fn test_fx_spot_rejects_zero_rate() {
        let result =
            FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD).with_rate(0.0);
        assert!(result.is_err(), "Should reject zero rate");

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("zero") || err_msg.contains("spot_rate"),
            "Error should mention zero rate"
        );
    }

    #[test]
    fn builder_rejects_same_base_and_quote_currency() {
        let result = FxSpot::builder()
            .id(InstrumentId::new("USDUSD"))
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .attributes(Attributes::new())
            .build();

        assert!(
            result.is_err(),
            "FX spot builder must reject identical base and quote currencies"
        );
    }

    #[test]
    fn test_fx_spot_new_t1() {
        let spot = FxSpot::new_t1(InstrumentId::new("USDCAD"), Currency::USD, Currency::CAD);

        // Wednesday -> should settle Thursday (T+1 weekdays)
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 16)); // Thursday (T+1)
    }

    #[test]
    fn test_fx_spot_is_t1_pair() {
        let usd_cad = FxSpot::new(InstrumentId::new("USDCAD"), Currency::USD, Currency::CAD);
        assert!(usd_cad.is_t1_pair(), "USD/CAD should be T+1 pair");

        let cad_usd = FxSpot::new(InstrumentId::new("CADUSD"), Currency::CAD, Currency::USD);
        assert!(cad_usd.is_t1_pair(), "CAD/USD should be T+1 pair");

        let usd_try = FxSpot::new(InstrumentId::new("USDTRY"), Currency::USD, Currency::TRY);
        assert!(usd_try.is_t1_pair(), "USD/TRY should be T+1 pair");

        let try_usd = FxSpot::new(InstrumentId::new("TRYUSD"), Currency::TRY, Currency::USD);
        assert!(try_usd.is_t1_pair(), "TRY/USD should be T+1 pair");

        let usd_mxn = FxSpot::new(InstrumentId::new("USDMXN"), Currency::USD, Currency::MXN);
        assert!(!usd_mxn.is_t1_pair(), "USD/MXN should NOT be T+1 pair");

        let eur_usd = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD);
        assert!(!eur_usd.is_t1_pair(), "EUR/USD should NOT be T+1 pair");
    }

    #[test]
    fn test_fx_spot_negative_settlement_lag() {
        // Negative lag for historical valuations (T-1)
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_settlement_lag_days(-1);

        // Wednesday with T-1 -> should settle Tuesday
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 14)); // Tuesday (T-1)
    }

    #[test]
    fn test_fx_spot_zero_settlement_lag() {
        // T+0 same-day settlement
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_settlement_lag_days(0);

        let as_of = date(2025, Month::January, 15);
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, as_of, "T+0 should settle same day");
    }

    #[test]
    fn builder_and_serde_reject_invalid_spot_rate() {
        // Builder path: validation runs through validate_economics.
        let result = FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_rate_opt(Some(-1.10))
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "builder must reject a negative spot rate");

        let result = FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_rate_opt(Some(0.0))
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "builder must reject a zero spot rate");

        // Serde path: try_from validation.
        let json = serde_json::json!({
            "id": "EURUSD",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "settlement": null,
            "settlement_lag_days": null,
            "spot_rate": -1.10,
            "notional": {"amount": "1000000", "currency": "EUR"},
            "base_calendar_id": null,
            "quote_calendar_id": null,
            "attributes": {}
        });
        let parsed: std::result::Result<FxSpot, _> = serde_json::from_value(json);
        assert!(parsed.is_err(), "serde must reject a negative spot rate");
    }

    #[test]
    fn settlement_cashflow_is_discounted_when_curve_configured() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let as_of = date(2025, Month::January, 15);
        let settle = date(2026, Month::January, 15); // 1y out: visible discount
        let rate = 0.05_f64;
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (2.0, (-rate * 2.0_f64).exp())])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let undiscounted = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_rate(1.10)
            .expect("valid rate")
            .with_settlement(settle);
        let mut discounted = undiscounted.clone();
        discounted.discount_curve_id = Some(finstack_quant_core::types::CurveId::new("USD-OIS"));

        let pv_undisc = undiscounted.value(&market, as_of).expect("pv");
        let pv_disc = discounted.value(&market, as_of).expect("pv");

        let df = market
            .get_discount("USD-OIS")
            .expect("curve")
            .df_between_dates(as_of, settle)
            .expect("df");
        assert!(
            df < 0.96,
            "1y DF at 5% must be materially below 1, got {df}"
        );
        assert!((pv_undisc.amount() - 1.10).abs() < 1e-10);
        assert!(
            (pv_disc.amount() - 1.10 * df).abs() < 1e-10,
            "settlement cashflow must be discounted: got {}, expected {}",
            pv_disc.amount(),
            1.10 * df
        );
    }

    #[test]
    fn cashflow_schedule_marks_future_spot_settlement_as_contractual() {
        let as_of = date(2025, Month::January, 15);
        let market = MarketContext::new();
        let spot = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
            .with_rate(1.10)
            .expect("valid rate");

        let schedule = spot
            .cashflow_schedule(&market, as_of)
            .expect("future-dated spot schedule");

        assert_eq!(
            schedule.get_meta().representation,
            crate::cashflow::builder::CashflowRepresentation::Contractual
        );
        assert_eq!(schedule.get_flows().len(), 1);
    }

    #[test]
    fn cashflow_schedule_marks_settled_spot_as_no_residual() {
        let as_of = date(2025, Month::January, 15);
        let market = MarketContext::new();
        let spot = FxSpot::new(InstrumentId::new("EURUSD-T0"), Currency::EUR, Currency::USD)
            .with_rate(1.10)
            .expect("valid rate")
            .with_settlement_lag_days(0);

        let schedule = spot
            .cashflow_schedule(&market, as_of)
            .expect("same-day settled spot schedule");

        assert_eq!(
            schedule.get_meta().representation,
            crate::cashflow::builder::CashflowRepresentation::NoResidual
        );
        assert!(schedule.get_flows().is_empty());
    }
}
