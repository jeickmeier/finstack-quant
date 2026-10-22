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
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::InstrumentId;
//! use finstack_quant_valuations_fx::fx_spot::FxSpot;
//!
//! // CCY1 (base) is the currency being priced; CCY2 (quote) is the pricing currency.
//! let eur_usd = FxSpot::builder()
//!     .id(InstrumentId::new("EURUSD"))
//!     .base_currency(Currency::EUR)
//!     .quote_currency(Currency::USD)
//!     .notional(Money::from((1_000_000_i64, Currency::EUR)))
//!     // How many units of `quote` per 1 unit of `base`
//!     .quoted_spot(1.10)
//!     .build()?;
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! With `quoted_spot = Some(1.10)`, 1 EUR = 1.10 USD.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_core::Result;
use finstack_quant_valuations_core::impl_instrument_base;

/// FX Spot instrument (1 unit of `base` priced in `quote`).
///
/// Represents the spot exchange rate between two currencies following
/// standard market quoting conventions (base/quote or CCY1/CCY2).
///
/// # Quote Convention
///
/// The rate is interpreted as: **1 unit of base = rate units of quote**
///
/// For example, if `base = EUR`, `quote = USD`, and `quoted_spot = 1.10`:
/// - 1 EUR = 1.10 USD
/// - This is the "EUR/USD" rate
///
/// # Settlement
///
/// When `settlement_days` is `None`, the pair-aware default is T+1 for
/// USD↔CAD, USD↔TRY, USD↔PHP and USD↔RUB and T+2 otherwise (including
/// EUR/USD). An explicit `settlement_days` is the contractual lag and overrides
/// that default in either direction (for example tom-next T+1 on EUR/USD);
/// `settlement_date` overrides both. Without calendars the USD leg uses the
/// `usny` calendar and other currencies count weekdays only.
///
/// See module-level documentation for comprehensive FX quoting conventions.
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct FxSpot {
    /// Unique identifier for the FX pair
    pub id: InstrumentId,
    /// Base currency (the currency being priced)
    pub base_currency: Currency,
    /// Quote currency (the currency used for pricing)
    pub quote_currency: Currency,
    /// Optional explicit settlement (value) date. When set it is only
    /// business-day adjusted and `settlement_days` is ignored.
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub settlement_date: Option<Date>,
    /// Optional contractual T+N settlement lag in business days when
    /// `settlement_date` is not provided.
    ///
    /// `None` uses the pair-aware default from
    /// [`finstack_quant_core::dates::fx::fx_standard_settlement_days`]: T+1 for
    /// USD↔CAD, USD↔TRY, USD↔PHP and USD↔RUB, T+2 otherwise. An explicit value
    /// overrides that default.
    pub settlement_days: Option<u32>,
    /// Optional quoted FX spot rate (quote per base); `None` reads the FxMatrix.
    pub quoted_spot: Option<f64>,
    /// Optional quote-currency discount curve for PV-ing the settlement
    /// cashflow. When absent the settlement amount is reported undiscounted
    /// (a 1–2 day effect for standard spot lags).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domestic_discount_curve_id: Option<CurveId>,
    /// Notional amount in base currency.
    pub notional: Money,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Business day convention to apply when adjusting settlement (default: ModifiedFollowing)
    ///
    /// Note: Default changed from `Following` to `ModifiedFollowing` in v0.8.0 to align
    /// with ISDA standard FX settlement conventions.
    #[builder(default = BusinessDayConvention::ModifiedFollowing)]
    #[serde(default = "finstack_quant_valuations_core::serde_defaults::bdc_modified_following")]
    pub business_day_convention: BusinessDayConvention,
    /// Optional base currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    pub base_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Optional quote currency calendar for joint calendar settlement adjustment.
    ///
    /// Per market convention, FX settlement uses the joint calendar of both currencies.
    /// A date is a good business day only if it's valid in both calendars.
    pub quote_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(FxSpot);

impl FxSpot {
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
    /// # Arguments
    ///
    /// * `as_of` - Trade/valuation date used as the T+0 origin when
    ///   `settlement_date` is unset. The spot lag and joint-calendar roll are
    ///   applied from this date; an explicit `settlement_date` is only
    ///   business-day-adjusted and ignores `as_of`.
    pub fn effective_settlement_date(&self, as_of: Date) -> Result<Date> {
        use finstack_quant_core::dates::fx::fx_spot_date;
        use finstack_quant_core::dates::{adjust, resolve_joint_calendar};

        // Check if we should use joint calendar logic
        let use_joint_calendar =
            self.base_calendar_id.is_some() || self.quote_calendar_id.is_some();

        if let Some(date) = self.settlement_date {
            // Explicit settlement date provided - adjust for business days
            if use_joint_calendar {
                let calendar = resolve_joint_calendar(
                    self.base_calendar_id.as_deref(),
                    self.quote_calendar_id.as_deref(),
                )?;
                adjust(date, self.business_day_convention, calendar)
            } else {
                Ok(date)
            }
        } else {
            // Compute T+N from as_of date
            let settlement_days = self.settlement_days.unwrap_or_else(|| {
                finstack_quant_core::dates::fx::fx_standard_settlement_days(
                    self.base_currency,
                    self.quote_currency,
                )
            });

            fx_spot_date(
                as_of,
                settlement_days,
                self.base_currency,
                self.quote_currency,
                self.base_calendar_id.as_deref(),
                self.quote_calendar_id.as_deref(),
            )
        }
    }

    fn validate_economics(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_distinct_currencies(
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
        // The builder and serde paths share the `quoted_spot` invariants:
        // finite and strictly positive.
        if let Some(rate) = self.quoted_spot {
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
        Self::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .quoted_spot(1.10)
            .settlement_days(2)
            .build()
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for FxSpot {
    fn with_bumped_fx_quote(
        &self,
        relative: f64,
    ) -> Option<Box<dyn finstack_quant_valuations_core::instruments::Instrument>> {
        let mut bumped = self.clone();
        *bumped.quoted_spot.as_mut()? *= 1.0 + relative;
        Some(Box::new(bumped))
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::FxSpot,
        crate::FX
    );

    fn validate_invariants(&self) -> Result<()> {
        self.validate_economics()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        if let Some(discount_curve) = &self.domestic_discount_curve_id {
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
            .domestic_discount_curve_id
            .as_ref()
            .map(|id| market.get_discount(id))
            .transpose()?;
        self.dated_cashflows(market, as_of)?.into_iter().try_fold(
            Money::from((0_i64, self.quote_currency)),
            |acc, (date, amount)| {
                let df = match disc.as_ref() {
                    Some(curve) => curve.df_between_dates(as_of, date)?,
                    None => 1.0,
                };
                acc.checked_add(Money::new(amount.amount() * df, amount.currency())?)
            },
        )
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FxSpot {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        self.validate_economics()?;
        let settle_date = self.effective_settlement_date(as_of)?;

        if crate::shared::event_has_occurred(settle_date, as_of) {
            return crate::shared::settled_schedule(self.notional()?);
        }

        let flows = {
            // Future settlement - use explicit quoted_spot if provided, otherwise query FX matrix
            let rate = if let Some(rate) = self.quoted_spot {
                rate
            } else {
                // Try market context FX matrix
                let matrix = curves.fx_required()?;
                let q = finstack_quant_core::money::fx::FxQuery::new(
                    self.base_currency,
                    self.quote_currency,
                    as_of,
                    Default::default(),
                );
                matrix.as_ref().rate(q)?.rate
            };
            let value = Money::new(self.notional.amount() * rate, self.quote_currency)?;
            vec![(settle_date, value)]
        };

        let schedule = finstack_quant_cashflows::schedule_from_dated_flows(
            flows,
            finstack_quant_core::cashflow::CFKind::Notional,
            finstack_quant_core::dates::DayCount::Act365F, // Standard for FX spot
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Contractual,
                    ..Default::default()
                },
            },
        )?;
        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::instruments::Instrument;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    /// EUR/USD builder with a one-euro notional and no quoted spot.
    fn eurusd() -> FxSpotBuilder {
        FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_i64, Currency::EUR)))
    }

    #[test]
    fn test_fx_spot_creation() {
        let spot = eurusd().build().expect("valid FX spot");
        assert_eq!(spot.base_currency, Currency::EUR);
        assert_eq!(spot.quote_currency, Currency::USD);
    }

    #[test]
    fn test_fx_spot_with_explicit_rate() {
        let spot = eurusd().quoted_spot(1.10).build().expect("valid FX spot");

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
        let spot = eurusd().build().expect("valid FX spot");

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
        let spot = eurusd()
            .settlement_date(settle_date)
            .build()
            .expect("valid FX spot");

        let as_of = date(2025, Month::January, 15);
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, settle_date);
    }

    #[test]
    fn test_fx_spot_effective_settlement_date_custom_lag() {
        let spot = eurusd().settlement_days(1).build().expect("valid FX spot");

        // A contractual T+1 (tom) lag is honored: Wednesday settles Thursday.
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 16)); // Thursday
    }

    #[test]
    fn test_fx_spot_returns_zero_when_settled() {
        let settle_date = date(2025, Month::January, 10);
        let spot = eurusd()
            .settlement_date(settle_date)
            .quoted_spot(1.10)
            .build()
            .expect("valid FX spot");

        let market = MarketContext::new();
        let as_of = date(2025, Month::January, 15); // After settlement
        let pv = spot.value(&market, as_of).expect("should price");

        assert_eq!(pv.amount(), 0.0, "Should return zero when settled");
    }

    #[test]
    fn test_fx_spot_notional_currency_validation() {
        let wrong_notional = Money::from((1000_i64, Currency::GBP));

        let result = eurusd().notional(wrong_notional).build();
        assert!(result.is_err(), "Should reject notional in wrong currency");
    }

    #[test]
    fn test_fx_spot_with_notional() {
        let spot = eurusd()
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .quoted_spot(1.10)
            .build()
            .expect("valid FX spot");

        let market = MarketContext::new();
        let as_of = date(2025, Month::January, 15);
        let pv = spot.value(&market, as_of).expect("should price");

        // 1,000,000 EUR * 1.10 = 1,100,000 USD
        assert!((pv.amount() - 1_100_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_fx_spot_rejects_negative_rate() {
        let result = eurusd().quoted_spot(-1.10).build();
        assert!(result.is_err(), "Should reject negative rate");

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("must be positive"),
            "Error should require a positive rate: {err_msg}"
        );
    }

    #[test]
    fn test_fx_spot_rejects_nan_rate() {
        let result = eurusd().quoted_spot(f64::NAN).build();
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
        let result = eurusd().quoted_spot(f64::INFINITY).build();
        assert!(result.is_err(), "Should reject Infinity rate");
    }

    #[test]
    fn test_fx_spot_rejects_zero_rate() {
        let result = eurusd().quoted_spot(0.0).build();
        assert!(result.is_err(), "Should reject zero rate");

        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("must be positive"),
            "Error should require a positive rate: {err_msg}"
        );
    }

    #[test]
    fn builder_rejects_same_base_and_quote_currency() {
        let result = FxSpot::builder()
            .id(InstrumentId::new("USDUSD"))
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .attributes(Attributes::new())
            .build();

        assert!(
            result.is_err(),
            "FX spot builder must reject identical base and quote currencies"
        );
    }

    #[test]
    fn test_fx_spot_new_defaults_t1_pairs_to_one_day() {
        let spot = FxSpot::builder()
            .id(InstrumentId::new("USDCAD"))
            .base_currency(Currency::USD)
            .quote_currency(Currency::CAD)
            .notional(Money::from((1_i64, Currency::USD)))
            .build()
            .expect("valid FX spot");
        let as_of = date(2025, Month::January, 15); // Wednesday
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, date(2025, Month::January, 16)); // Thursday (T+1)
    }

    #[test]
    fn negative_settlement_days_are_rejected_on_the_wire() {
        // `settlement_days` is a non-negative T+N lag (u32); T-N lags are not supported.
        let mut value = serde_json::to_value(FxSpot::example().expect("example")).expect("ser");
        value["settlement_days"] = serde_json::json!(-1);
        let err = serde_json::from_value::<FxSpot>(value).expect_err("negative lag must fail");
        assert!(err.to_string().contains("invalid value"), "{err}");
    }

    #[test]
    // schema-rejection-test: retired `settlement` / `settlement_lag_days` keys
    fn retired_settlement_keys_are_rejected() {
        for (canonical, retired) in [
            ("settlement_date", "settlement"),
            ("settlement_days", "settlement_lag_days"),
        ] {
            let mut value = serde_json::to_value(FxSpot::example().expect("example")).expect("ser");
            let map = value.as_object_mut().expect("object");
            let inner = map.remove(canonical).expect("canonical key serialized");
            map.insert(retired.to_string(), inner);
            let err = serde_json::from_value::<FxSpot>(value).expect_err("retired key");
            assert!(err.to_string().contains(retired), "{retired}: {err}");
        }
    }

    #[test]
    fn test_fx_spot_zero_settlement_lag() {
        // T+0 same-day settlement
        let spot = eurusd().settlement_days(0).build().expect("valid FX spot");

        let as_of = date(2025, Month::January, 15);
        let settle = spot
            .effective_settlement_date(as_of)
            .expect("should compute");
        assert_eq!(settle, as_of, "T+0 should settle same day");
    }

    #[test]
    fn builder_and_serde_reject_invalid_quoted_spot() {
        // Builder path: validation runs through validate_economics.
        let result = FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .quoted_spot_opt(Some(-1.10))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "builder must reject a negative spot rate");

        let result = FxSpot::builder()
            .id(InstrumentId::new("EURUSD"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .quoted_spot_opt(Some(0.0))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .attributes(Attributes::new())
            .build();
        assert!(result.is_err(), "builder must reject a zero spot rate");

        // Serde path: try_from validation.
        let json = serde_json::json!({
            "id": "EURUSD",
            "base_currency": "EUR",
            "quote_currency": "USD",
            "settlement_date": null,
            "settlement_days": null,
            "quoted_spot": -1.10,
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

        let undiscounted = eurusd()
            .quoted_spot(1.10)
            .settlement_date(settle)
            .build()
            .expect("valid FX spot");
        let mut discounted = undiscounted.clone();
        discounted.domestic_discount_curve_id =
            Some(finstack_quant_core::types::CurveId::new("USD-OIS"));

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
        let spot = eurusd().quoted_spot(1.10).build().expect("valid FX spot");

        let schedule = spot
            .cashflow_schedule(&market, as_of)
            .expect("future-dated spot schedule");

        assert_eq!(
            schedule.get_meta().representation,
            finstack_quant_cashflows::builder::CashflowRepresentation::Contractual
        );
        assert_eq!(schedule.get_flows().len(), 1);
    }

    #[test]
    fn cashflow_schedule_marks_settled_spot_as_no_residual() {
        let settlement = date(2025, Month::January, 15);
        let as_of = date(2025, Month::January, 16);
        let market = MarketContext::new();
        let spot = FxSpot::builder()
            .id(InstrumentId::new("EURUSD-SETTLED"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::from((1_i64, Currency::EUR)))
            .quoted_spot(1.10)
            .settlement_date(settlement)
            .build()
            .expect("valid FX spot");

        let schedule = spot
            .cashflow_schedule(&market, as_of)
            .expect("post-settlement spot schedule");

        assert_eq!(
            schedule.get_meta().representation,
            finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
        );
        assert!(schedule.get_flows().is_empty());
    }
}
