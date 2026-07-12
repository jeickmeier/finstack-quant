//! FX Swap types and instrument integration.
//!
//! This file defines the `FxSwap` instrument shape and provides the
//! integration with the shared instrument trait via the `impl_instrument!`
//! macro. Core PV logic is delegated to `pricing::engine` to follow the
//! repository standards. Metrics live under `metrics/` and are registered
//! via the instrument metrics module.

use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

use crate::cashflow::builder::{CashFlowSchedule, Notional};
use crate::cashflow::primitives::CFKind;
use crate::cashflow::CashflowProvider;
use crate::impl_instrument_base;

/// FX Swap instrument definition
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = FxSwap::validate)]
#[serde(deny_unknown_fields, try_from = "FxSwapUnchecked")]
pub struct FxSwap {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign)
    pub base_currency: Currency,
    /// Quote currency (domestic)
    pub quote_currency: Currency,
    /// Near leg settlement date (spot leg)
    #[schemars(with = "String")]
    pub near_date: Date,
    /// Far leg settlement date (forward leg)
    #[schemars(with = "String")]
    pub far_date: Date,
    /// Notional amount in base currency (exchanged on near, reversed on far)
    pub base_notional: Money,
    /// Domestic discount curve id (quote currency)
    pub domestic_discount_curve_id: CurveId,
    /// Foreign discount curve id (base currency)
    pub foreign_discount_curve_id: CurveId,
    /// Optional near leg FX rate (quote per base). If None, source from market.
    #[builder(optional)]
    pub near_rate: Option<f64>,
    /// Optional far leg FX rate (quote per base). If None, source from forwards.
    #[builder(optional)]
    pub far_rate: Option<f64>,
    /// Optional base currency calendar for spot/settlement adjustment metadata.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_calendar_id: Option<String>,
    /// Optional quote currency calendar for spot/settlement adjustment metadata.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_calendar_id: Option<String>,
    /// Attributes for tagging and selection
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct FxSwapUnchecked {
    /// Unique instrument identifier.
    id: InstrumentId,
    /// Base currency (foreign).
    base_currency: Currency,
    /// Quote currency (domestic).
    quote_currency: Currency,
    /// Near leg settlement date (spot leg).
    #[schemars(with = "String")]
    near_date: Date,
    /// Far leg settlement date (forward leg).
    #[schemars(with = "String")]
    far_date: Date,
    /// Notional amount in base currency (exchanged on near, reversed on far).
    base_notional: Money,
    /// Domestic discount curve id (quote currency).
    domestic_discount_curve_id: CurveId,
    /// Foreign discount curve id (base currency).
    foreign_discount_curve_id: CurveId,
    /// Optional near leg FX rate (quote per base). If None, source from market.
    #[serde(default)]
    near_rate: Option<f64>,
    /// Optional far leg FX rate (quote per base). If None, source from forwards.
    #[serde(default)]
    far_rate: Option<f64>,
    /// Optional base currency calendar for spot/settlement adjustment metadata.
    #[serde(default)]
    base_calendar_id: Option<String>,
    /// Optional quote currency calendar for spot/settlement adjustment metadata.
    #[serde(default)]
    quote_calendar_id: Option<String>,
    /// Per-instrument pricing/sensitivity override knobs.
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging.
    attributes: Attributes,
}

impl TryFrom<FxSwapUnchecked> for FxSwap {
    type Error = finstack_quant_core::Error;

    fn try_from(value: FxSwapUnchecked) -> std::result::Result<Self, Self::Error> {
        let swap = Self {
            id: value.id,
            base_currency: value.base_currency,
            quote_currency: value.quote_currency,
            near_date: value.near_date,
            far_date: value.far_date,
            base_notional: value.base_notional,
            domestic_discount_curve_id: value.domestic_discount_curve_id,
            foreign_discount_curve_id: value.foreign_discount_curve_id,
            near_rate: value.near_rate,
            far_rate: value.far_rate,
            base_calendar_id: value.base_calendar_id,
            quote_calendar_id: value.quote_calendar_id,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        swap.validate()?;
        Ok(swap)
    }
}

impl FxSwap {
    /// Validate FX swap economics at construction boundaries.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxSwap",
        )?;
        if self.base_notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSwap base_notional currency ({}) must match base_currency ({})",
                self.base_notional.currency(),
                self.base_currency
            )));
        }
        crate::instruments::common_impl::validation::validate_money_finite(
            self.base_notional,
            "FxSwap base_notional",
        )?;
        crate::instruments::common_impl::validation::validate_money_gt(
            self.base_notional,
            0.0,
            "FxSwap base_notional",
        )?;
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
    #[allow(clippy::expect_used)] // Example uses hardcoded valid values
    pub fn example() -> Self {
        Self::builder()
            .id(InstrumentId::new("FXSWAP-EURUSD-6M"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .near_date(
                Date::from_calendar_date(2024, time::Month::January, 5)
                    .expect("Valid example date"),
            )
            .far_date(
                Date::from_calendar_date(2024, time::Month::July, 5).expect("Valid example date"),
            )
            .base_notional(Money::new(1_000_000.0, Currency::EUR))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .near_rate_opt(Some(1.10))
            .far_rate_opt(Some(1.12))
            .attributes(Attributes::new())
            .build()
            .expect("Example FX swap construction should not fail")
    }

    /// Construct an FX swap from trade date and tenor using joint calendar spot roll.
    ///
    /// `spot_lag_days` defaults to 2 in most markets; supply calendar IDs to enforce
    /// base/quote business-day adjustment.
    #[allow(clippy::too_many_arguments)]
    pub fn from_trade_date(
        id: impl Into<InstrumentId>,
        base_currency: Currency,
        quote_currency: Currency,
        trade_date: Date,
        far_tenor_days: i64,
        base_notional: Money,
        domestic_discount_curve_id: impl Into<CurveId>,
        foreign_discount_curve_id: impl Into<CurveId>,
        base_calendar_id: Option<String>,
        quote_calendar_id: Option<String>,
        spot_lag_days: u32,
        bdc: finstack_quant_core::dates::BusinessDayConvention,
    ) -> finstack_quant_core::Result<Self> {
        use crate::instruments::common_impl::fx_dates::{
            adjust_joint_calendar, fx_spot_date_for_pair,
        };
        // CLS-consistent spot roll: a US holiday on an intermediate day does not
        // delay a USD pair's spot date (FX spot convention
        // finding).
        let near_date = fx_spot_date_for_pair(
            trade_date,
            spot_lag_days,
            base_currency,
            quote_currency,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;
        let far_unadjusted = near_date + time::Duration::days(far_tenor_days);
        let far_date = adjust_joint_calendar(
            far_unadjusted,
            bdc,
            base_calendar_id.as_deref(),
            quote_calendar_id.as_deref(),
        )?;

        Self::builder()
            .id(id.into())
            .base_currency(base_currency)
            .quote_currency(quote_currency)
            .near_date(near_date)
            .far_date(far_date)
            .base_notional(base_notional)
            .domestic_discount_curve_id(domestic_discount_curve_id.into())
            .foreign_discount_curve_id(foreign_discount_curve_id.into())
            .base_calendar_id_opt(base_calendar_id)
            .quote_calendar_id_opt(quote_calendar_id)
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
        let anchor = if as_of < payment_date {
            as_of
        } else {
            payment_date - time::Duration::days(1)
        };
        let mut builder = CashFlowSchedule::builder();
        let _ = builder.principal(Money::new(0.0, amount.currency()), anchor, payment_date);
        let _ = builder.add_principal_event(
            payment_date,
            Money::new(0.0, amount.currency()),
            Some(Money::new(-amount.amount(), amount.currency())),
            CFKind::Notional,
        );
        let mut schedule = builder.build_with_curves(None)?;
        schedule.notional = Notional::par(amount.amount().abs(), amount.currency());
        Ok(schedule)
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxSwap {
    impl_instrument_base!(crate::pricer::InstrumentType::FxSwap);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps =
            crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
                self,
            )?;
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        use super::pricing_helper::FxSwapPricingContext;

        self.validate()?;

        // Validate date ordering
        if self.near_date > self.far_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FxSwap near_date ({}) must be <= far_date ({})",
                self.near_date, self.far_date
            )));
        }

        // If fully settled (on or after far date), return zero.
        // Uses >= for consistency with FxForward (settled when as_of >= maturity_date).
        if as_of >= self.far_date {
            return Ok(finstack_quant_core::money::Money::new(
                0.0,
                self.quote_currency,
            ));
        }

        // Currency safety check before expensive calculations
        if self.base_notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.base_notional.currency(),
            });
        }

        // Build pricing context (handles rate validation and CIP forward calculation)
        let ctx = FxSwapPricingContext::build(self, curves, as_of)?;

        // Calculate total PV using the helper
        let total_pv = ctx.total_pv();
        Ok(finstack_quant_core::money::Money::new(
            total_pv,
            self.quote_currency,
        ))
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.far_date)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.near_date)
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

impl CashflowProvider for FxSwap {
    fn cashflow_schedule(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<CashFlowSchedule> {
        use super::pricing_helper::FxSwapPricingContext;

        if self.base_notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.base_notional.currency(),
            });
        }

        let ctx = FxSwapPricingContext::build(self, curves, as_of)?;
        let base_amount = self.base_notional.amount();
        let near_quote = self.base_notional.amount() * ctx.contract_near_rate;
        let far_quote = self.base_notional.amount() * ctx.contract_far_rate;

        let mut near_base = self.single_cashflow_schedule(
            as_of,
            self.near_date,
            Money::new(base_amount, self.base_currency),
        )?;
        let near_quote_schedule = self.single_cashflow_schedule(
            as_of,
            self.near_date,
            Money::new(-near_quote, self.quote_currency),
        )?;
        let far_base_schedule = self.single_cashflow_schedule(
            as_of,
            self.far_date,
            Money::new(-base_amount, self.base_currency),
        )?;
        let far_quote_schedule = self.single_cashflow_schedule(
            as_of,
            self.far_date,
            Money::new(far_quote, self.quote_currency),
        )?;

        near_base.flows.extend(near_quote_schedule.flows);
        near_base.flows.extend(far_base_schedule.flows);
        near_base.flows.extend(far_quote_schedule.flows);
        near_base.notional = Notional::par(0.0, self.base_currency);
        Ok(near_base.normalize_public(
            as_of,
            crate::cashflow::builder::CashflowRepresentation::Projected,
        ))
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for FxSwap {
    fn curve_dependencies(
        &self,
    ) -> finstack_quant_core::Result<crate::instruments::common_impl::traits::InstrumentCurves>
    {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.domestic_discount_curve_id.clone())
            .discount(self.foreign_discount_curve_id.clone())
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::instruments::common_impl::traits::{CurveDependencies, Instrument};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
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
        let swap = FxSwap::example();
        assert_eq!(swap.id.as_str(), "FXSWAP-EURUSD-6M");
        assert_eq!(swap.base_currency, Currency::EUR);
        assert_eq!(swap.quote_currency, Currency::USD);
    }

    #[test]
    fn test_fx_swap_curve_dependencies() {
        let swap = FxSwap::example();
        let deps = swap.curve_dependencies().expect("curve_dependencies");

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
            .base_notional(Money::new(1_000_000.0, Currency::EUR))
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
        let swap = FxSwap::example();
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
        let swap = FxSwap::example(); // far_date is 2024-07-05

        let pv = swap.value(&market, as_of).expect("should price");
        assert_eq!(pv.amount(), 0.0, "Fully settled swap should have zero PV");
    }

    #[test]
    fn test_fx_swap_near_leg_settled_far_leg_active() {
        // as_of is between near_date and far_date
        let as_of = date(2024, Month::March, 1);
        let market = base_market(as_of);
        let swap = FxSwap::example();
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
        let swap = FxSwap::example();

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
