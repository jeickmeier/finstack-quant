//! FX Swap types and instrument integration.
//!
//! This file defines the `FxSwap` instrument shape and provides the
//! integration with the shared instrument trait via the `impl_instrument!`
//! macro. Core PV logic is delegated to `pricing::engine` to follow the
//! repository standards. Metrics live under `metrics/` and are registered
//! via the instrument metrics module.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId};

use finstack_quant_cashflows::builder::{merge_cashflow_schedules, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_valuations_core::impl_instrument_base;

/// FX Swap instrument definition
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
pub struct FxSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign)
    pub base_currency: Currency,
    /// Quote currency (domestic)
    pub quote_currency: Currency,
    /// Near leg settlement date (spot leg)
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub near_date: Date,
    /// Far leg settlement date (forward leg)
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub far_date: Date,
    /// Notional amount in base currency (exchanged on near, reversed on far)
    pub notional: Money,
    /// Domestic discount curve id (quote currency)
    pub domestic_discount_curve_id: CurveId,
    /// Foreign discount curve id (base currency)
    pub foreign_discount_curve_id: CurveId,
    /// Optional near leg FX rate (quote per base). If absent, use the market
    /// outright forward from the valuation date to near settlement.
    pub near_rate: Option<f64>,
    /// Optional far leg FX rate (quote per base). If absent, use the market
    /// outright forward from the valuation date to far settlement.
    pub far_rate: Option<f64>,
    /// Optional base currency calendar for spot/settlement adjustment metadata.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calendar_id: Option<finstack_quant_core::types::CalendarId>,
    /// Optional quote currency calendar for spot/settlement adjustment metadata.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_calendar_id: Option<finstack_quant_core::types::CalendarId>,
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
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(FxSwap);

impl FxSwap {
    /// Validate FX swap economics at construction boundaries.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        finstack_quant_core::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxSwap",
        )?;
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSwap notional currency ({}) must match base_currency ({})",
                self.notional.currency(),
                self.base_currency
            )));
        }
        finstack_quant_core::validation::validate_money_finite(self.notional, "FxSwap notional")?;
        finstack_quant_core::validation::validate_money_gt(self.notional, 0.0, "FxSwap notional")?;
        if self.near_date > self.far_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSwap near_date ({}) must be <= far_date ({})",
                self.near_date, self.far_date
            )));
        }
        for (name, rate) in [("near_rate", self.near_rate), ("far_rate", self.far_rate)] {
            if let Some(rate) = rate {
                if !rate.is_finite() || rate <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FxSwap {name} must be positive and finite, got {rate}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Create a canonical example FX swap for testing and documentation.
    ///
    /// Returns a 6-month EUR/USD swap with realistic forward points.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FXSWAP-EURUSD-6M"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(time::macros::date!(2024 - 01 - 05))
            .far_date(time::macros::date!(2024 - 07 - 05))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .near_rate_opt(Some(1.10))
            .far_rate_opt(Some(1.12))
            .attributes(Attributes::new())
            .build()
    }

    /// Construct an FX swap from trade date and a standard calendar tenor.
    ///
    /// `settlement_days` (the T+N spot lag) is 2 in most markets; supply calendar IDs to enforce
    /// base/quote business-day adjustment. `end_of_month` explicitly controls
    /// whether a month-end near date produces a month-end far date.
    #[allow(clippy::too_many_arguments)]
    pub fn from_trade_date(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        quote_currency: Currency,
        trade_date: Date,
        far_tenor: Tenor,
        notional: Money,
        domestic_discount_curve_id: impl Into<CurveId>,
        foreign_discount_curve_id: impl Into<CurveId>,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        settlement_days: u32,
        business_day_convention: finstack_quant_core::dates::BusinessDayConvention,
        end_of_month: bool,
    ) -> finstack_quant_core::Result<Self> {
        use finstack_quant_core::dates::fx::{add_fx_standard_tenor, fx_spot_date};
        // CLS-consistent spot roll: a US holiday on an intermediate day does not
        // delay a USD pair's spot date (FX spot convention
        // finding).
        let near_date = fx_spot_date(
            trade_date,
            settlement_days,
            base_currency,
            quote_currency,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let far_date = add_fx_standard_tenor(
            near_date,
            far_tenor,
            business_day_convention,
            end_of_month,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        Self::builder()
            .id(id.into())
            .base_currency(base_currency)
            .quote_currency(quote_currency)
            .near_date(near_date)
            .far_date(far_date)
            .notional(notional)
            .domestic_discount_curve_id(domestic_discount_curve_id.into())
            .foreign_discount_curve_id(foreign_discount_curve_id.into())
            .base_calendar_id_opt(base_calendar_id.map(Into::into))
            .quote_calendar_id_opt(quote_calendar_id.map(Into::into))
            .attributes(Attributes::new())
            .build()
    }

    // Builder entrypoint is provided via derive

    fn single_cashflow_schedule(
        &self,
        as_of: Date,
        payment_date: Date,
        amount: Money,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        let flows = if crate::shared::event_has_occurred(payment_date, as_of) {
            Vec::new()
        } else {
            vec![(payment_date, amount)]
        };
        finstack_quant_cashflows::schedule_from_dated_flows(
            flows,
            CFKind::Notional,
            finstack_quant_core::dates::DayCount::Act365F,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(Money::new(amount.amount().abs(), amount.currency())?),
                ..Default::default()
            },
        )
    }
}

impl finstack_quant_valuations_core::instruments::Instrument for FxSwap {
    fn cashflow_export_terms(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<
        finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms,
    > {
        // Each leg settles in its own currency and discounts on that
        // currency's curve; the envelope reports in the quote currency.
        let mut terms = finstack_quant_valuations_core::instruments::cashflow_export::CashflowExportTerms::from_schedule(
            finstack_quant_cashflows::CashflowProvider::cashflow_schedule(self, market, as_of)?,
        );
        terms
            .currency_discount_curves
            .insert(self.base_currency, self.foreign_discount_curve_id.clone());
        terms
            .currency_discount_curves
            .insert(self.quote_currency, self.domestic_discount_curve_id.clone());
        terms.discount_curve_id = Some(self.domestic_discount_curve_id.clone());
        terms.reporting_currency = Some(self.quote_currency);
        Ok(terms)
    }

    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::FxSwap,
        crate::FX
    );

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use super::pricing_helper::FxSwapPricingContext;

        // End-of-day policy: far-leg settlement remains live on its event date.
        if crate::shared::event_has_occurred(self.far_date, as_of) {
            return Ok(finstack_quant_core::money::Money::from((
                0_i64,
                self.quote_currency,
            )));
        }

        // Build pricing context after validating the contract terms.
        let ctx = FxSwapPricingContext::build(self, curves, as_of)?;

        let total_pv = ctx.total_pv();
        finstack_quant_core::money::Money::new(total_pv, self.quote_currency)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.far_date)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for FxSwap {
    fn raw_cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        use super::pricing_helper::FxSwapPricingContext;

        self.validate()?;
        if crate::shared::event_has_occurred(self.far_date, as_of) {
            return crate::shared::settled_schedule(Some(Money::from((0_i64, self.base_currency))));
        }

        let ctx = FxSwapPricingContext::build(self, curves, as_of)?;
        let base_amount = self.notional.amount();
        let near_quote = self.notional.amount() * ctx.contract_near_rate;
        let far_quote = self.notional.amount() * ctx.contract_far_rate;

        let near_base = self.single_cashflow_schedule(
            as_of,
            self.near_date,
            Money::new(base_amount, self.base_currency)?,
        )?;
        let near_quote_schedule = self.single_cashflow_schedule(
            as_of,
            self.near_date,
            Money::new(-near_quote, self.quote_currency)?,
        )?;
        let far_base_schedule = self.single_cashflow_schedule(
            as_of,
            self.far_date,
            Money::new(-base_amount, self.base_currency)?,
        )?;
        let far_quote_schedule = self.single_cashflow_schedule(
            as_of,
            self.far_date,
            Money::new(far_quote, self.quote_currency)?,
        )?;

        Ok(merge_cashflow_schedules(
            [
                near_base,
                near_quote_schedule,
                far_base_schedule,
                far_quote_schedule,
            ],
            Notional::par(0.0, self.base_currency)?,
            finstack_quant_core::dates::DayCount::Act365F,
        )
        .with_representation(finstack_quant_cashflows::builder::CashflowRepresentation::Projected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_valuations_core::instruments::Instrument;
    use std::sync::Arc;
    use time::Month;

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).expect("valid test date")
    }

    fn base_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("should build");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.97)])
            .build()
            .expect("should build");

        let fx_provider = {
            let provider = Arc::new(SimpleFxProvider::new());
            provider
                .set_quote(Currency::EUR, Currency::USD, 1.10)
                .expect("valid EUR/USD quote");
            provider
        };

        MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(FxMatrix::new(fx_provider))
    }

    #[test]
    fn test_fx_swap_example_creation() {
        let swap = FxSwap::example().expect("example");
        assert_eq!(swap.id.as_str(), "FXSWAP-EURUSD-6M");
        assert_eq!(swap.base_currency, Currency::EUR);
        assert_eq!(swap.quote_currency, Currency::USD);
    }

    #[test]
    fn test_fx_swap_market_dependencies() {
        let swap = FxSwap::example().expect("example");
        let deps = swap
            .market_dependencies()
            .expect("market_dependencies")
            .curves;

        assert_eq!(deps.discount_curves.len(), 2);
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"));
        assert!(deps.discount_curves.iter().any(|c| c.as_str() == "EUR-OIS"));
    }

    #[test]
    fn test_fx_swap_rejects_invalid_date_ordering() {
        let err = FxSwap::builder()
            .id(InstrumentId::new("INVALID-SWAP"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(date(2024, Month::July, 5)) // Far date is actually earlier
            .far_date(date(2024, Month::January, 5))
            .notional(Money::from((1_000_000_i64, Currency::EUR)))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .near_rate_opt(Some(1.10))
            .far_rate_opt(Some(1.12))
            .attributes(Attributes::new())
            .build()
            .expect_err("near_date > far_date should be rejected at construction");

        let err_msg = err.to_string();
        assert!(
            err_msg.contains("near_date") && err_msg.contains("far_date"),
            "Error should mention date ordering: {}",
            err_msg
        );
    }

    #[test]
    fn test_fx_swap_serde_rejects_invalid_date_ordering() {
        let swap = FxSwap::example().expect("example");
        let mut json = serde_json::to_value(&swap).expect("serialize");
        json["near_date"] = serde_json::json!("2024-07-05");
        json["far_date"] = serde_json::json!("2024-01-05");

        let err = serde_json::from_value::<FxSwap>(json)
            .expect_err("near_date > far_date should fail during deserialization");
        assert!(
            err.to_string().contains("near_date") && err.to_string().contains("far_date"),
            "error should mention date ordering: {}",
            err
        );
    }

    #[test]
    fn test_fx_swap_returns_zero_when_fully_settled() {
        let as_of = date(2024, Month::August, 1); // After far_date
        let market = base_market(as_of);
        let swap = FxSwap::example().expect("example"); // far_date is 2024-07-05

        let pv = swap.value(&market, as_of).expect("should price");
        assert_eq!(pv.amount(), 0.0, "Fully settled swap should have zero PV");
    }

    #[test]
    fn test_fx_swap_near_leg_settled_far_leg_active() {
        // as_of is between near_date and far_date
        let as_of = date(2024, Month::March, 1);
        let market = base_market(as_of);
        let swap = FxSwap::example().expect("example");
        // Example has near_date=2024-01-05, far_date=2024-07-05

        // Should only include far leg since near has settled
        let result = swap.value(&market, as_of);
        assert!(
            result.is_ok(),
            "Should price when near settled but far active: {:?}",
            result.as_ref().err()
        );
        let pv = result.expect("should price");
        // PV should not be zero (far leg has value)
        // The sign depends on rates; just verify it's non-zero
        assert!(
            pv.amount().abs() > 1e-6,
            "PV should be non-zero when far leg is active"
        );
    }

    #[test]
    fn test_fx_swap_cashflow_provider_emits_four_settlement_flows() {
        let as_of = date(2024, Month::January, 3);
        let market = base_market(as_of);
        let swap = FxSwap::example().expect("example");

        let flows = swap
            .dated_cashflows(&market, as_of)
            .expect("fx swap contractual schedule should build");

        assert_eq!(
            flows.len(),
            4,
            "fx swap should emit near/far base+quote flows"
        );
        assert!(flows.iter().any(|(date, money)| *date == swap.near_date
            && money.currency() == swap.base_currency
            && money.amount() > 0.0));
        assert!(flows.iter().any(|(date, money)| *date == swap.near_date
            && money.currency() == swap.quote_currency
            && money.amount() < 0.0));
        assert!(flows.iter().any(|(date, money)| *date == swap.far_date
            && money.currency() == swap.base_currency
            && money.amount() < 0.0));
        assert!(flows.iter().any(|(date, money)| *date == swap.far_date
            && money.currency() == swap.quote_currency
            && money.amount() > 0.0));
    }
}
