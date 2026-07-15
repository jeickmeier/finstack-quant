//! FX digital (binary) option instrument definition.

use super::pricer::{self, FxDigitalOptionGreeks};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::MarketDependencies;
use crate::instruments::OptionType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Payout type for digital (binary) options.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DigitalPayoutType {
    /// Pays a fixed cash amount in the payout currency if ITM at expiry.
    CashOrNothing,
    /// Pays one unit of the foreign (base) currency if ITM at expiry.
    AssetOrNothing,
}

impl std::fmt::Display for DigitalPayoutType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CashOrNothing => write!(f, "cash_or_nothing"),
            Self::AssetOrNothing => write!(f, "asset_or_nothing"),
        }
    }
}

impl std::str::FromStr for DigitalPayoutType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "cash_or_nothing" | "cashornothing" => Ok(Self::CashOrNothing),
            "asset_or_nothing" | "assetornothing" => Ok(Self::AssetOrNothing),
            other => Err(format!(
                "Unknown digital payout type: '{}'. Valid: cash_or_nothing, asset_or_nothing",
                other
            )),
        }
    }
}

/// FX digital (binary) option instrument.
///
/// Pays a fixed cash amount if the option expires in-the-money.
/// Two payout types:
/// - Cash-or-nothing: pays a fixed amount in the payout currency
/// - Asset-or-nothing: pays the spot rate (one unit of foreign currency)
///
/// # Pricing
///
/// Uses Garman-Kohlhagen adapted formulas:
///
/// **Cash-or-nothing call**: `PV = e^{-r_d T} × N(d2) × payout_amount`
/// **Cash-or-nothing put**: `PV = e^{-r_d T} × N(-d2) × payout_amount`
/// **Asset-or-nothing call**: `PV = S × e^{-r_f T} × N(d1) × notional`
/// **Asset-or-nothing put**: `PV = S × e^{-r_f T} × N(-d1) × notional`
///
/// # References
///
/// - Reiner, E., & Rubinstein, M. (1991). "Unscrambling the Binary Code."
///   *Risk Magazine*, 4(9), 75-83.
/// - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[builder(validate = FxDigitalOption::validate)]
#[serde(deny_unknown_fields)]
pub struct FxDigitalOption {
    /// Unique instrument identifier
    pub id: InstrumentId,
    /// Base currency (foreign currency)
    pub base_currency: Currency,
    /// Quote currency (domestic currency)
    pub quote_currency: Currency,
    /// Strike exchange rate (quote per base)
    pub strike: f64,
    /// Option type (call or put on base currency)
    pub option_type: OptionType,
    /// Payout type (cash-or-nothing or asset-or-nothing)
    pub payout_type: DigitalPayoutType,
    /// Fixed payout amount (used for cash-or-nothing; for asset-or-nothing this
    /// is the notional of foreign currency delivered)
    pub payout_amount: Money,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Day count convention
    pub day_count: DayCount,
    /// Notional amount in base currency
    pub notional: Money,
    /// Domestic currency discount curve ID
    pub domestic_discount_curve_id: CurveId,
    /// Foreign currency discount curve ID
    pub foreign_discount_curve_id: CurveId,
    /// FX volatility surface ID
    pub vol_surface_id: CurveId,
    /// Pricing overrides (manual price, yield, spread)
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and grouping
    pub attributes: Attributes,
}

impl FxDigitalOption {
    /// Validate FX digital option input invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        crate::instruments::common_impl::validation::validate_distinct_currencies(
            self.base_currency,
            self.quote_currency,
            "FxDigitalOption",
        )?;
        crate::instruments::common_impl::validation::validate_f64_positive(
            self.strike,
            "FxDigitalOption strike",
        )?;
        crate::instruments::common_impl::validation::validate_f64_finite(
            self.strike,
            "FxDigitalOption strike",
        )?;
        crate::instruments::common_impl::validation::validate_money_gt(
            self.notional,
            0.0,
            "FxDigitalOption notional",
        )?;
        crate::instruments::common_impl::validation::validate_money_finite(
            self.notional,
            "FxDigitalOption notional",
        )?;
        if self.notional.currency() != self.base_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.base_currency,
                actual: self.notional.currency(),
            });
        }
        // The closed form is the domestic (quote-currency) cash digital:
        // e^{−r_d T}·N(±d₂)·Q. A base-currency payout (foreign-cash digital,
        // e^{−r_f T}·N(±d₁)) is a different formula — reject rather than
        // silently mispricing and mislabeling the result currency.
        if self.payout_amount.currency() != self.quote_currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.quote_currency,
                actual: self.payout_amount.currency(),
            });
        }
        Ok(())
    }

    /// Create a canonical example FX digital option for testing and documentation.
    ///
    /// Returns an EUR/USD cash-or-nothing digital call expiring on the
    /// project-wide stable example epoch.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Self::builder()
            .id(InstrumentId::new("FXDIG-EURUSD-CALL"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.12)
            .option_type(OptionType::Call)
            .payout_type(DigitalPayoutType::CashOrNothing)
            .payout_amount(Money::new(1_000_000.0, Currency::USD))
            .expiry(crate::instruments::common_impl::example_constants::FAR_EXPIRY)
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(Attributes::new())
            .build()
    }

    fn price_internal(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }

    fn greeks_internal(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<FxDigitalOptionGreeks> {
        pricer::compute_greeks(self, market, as_of)
    }
}

impl crate::instruments::common_impl::traits::Instrument for FxDigitalOption {
    impl_instrument_base!(crate::pricer::InstrumentType::FxDigitalOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn default_model(&self) -> crate::pricer::ModelKey {
        crate::pricer::ModelKey::Black76
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        self.price_internal(curves, as_of)
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.domestic_discount_curve_id.clone());
        deps.add_discount_curve(self.foreign_discount_curve_id.clone());
        deps.add_volatility_dependency(
            crate::instruments::common_impl::dependencies::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                None,
                Some(self.strike),
            ),
        );
        deps.add_fx_pair(self.base_currency, self.quote_currency);
        Ok(deps)
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for FxDigitalOption {
    fn option_delta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks_internal(market, as_of)?.delta))
    }

    fn option_gamma(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks_internal(market, as_of)?.gamma))
    }

    fn option_vega(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks_internal(market, as_of)?.vega))
    }

    fn option_theta(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.greeks_internal(market, as_of)?.theta))
    }

    fn option_rho_bp(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        // Rho domestic per 1bp
        Ok(Some(
            self.greeks_internal(market, as_of)?.rho_domestic / 100.0,
        ))
    }
}

crate::impl_empty_cashflow_provider!(
    FxDigitalOption,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // -----------------------------------------------------------------------
    // Validation tests
    // -----------------------------------------------------------------------

    fn base_digital_builder(
    ) -> crate::instruments::fx::fx_digital_option::types::FxDigitalOptionBuilder {
        use finstack_quant_core::types::{CurveId, InstrumentId};
        FxDigitalOption::builder()
            .id(InstrumentId::new("FXDIG-VALID"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.12)
            .option_type(OptionType::Call)
            .payout_type(DigitalPayoutType::CashOrNothing)
            .payout_amount(Money::new(1_000_000.0, Currency::USD))
            .expiry(time::macros::date!(2027 - 01 - 15))
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
    }

    #[test]
    fn validation_valid_digital_option_builds_ok() {
        assert!(base_digital_builder().build().is_ok());
    }

    #[test]
    fn validation_rejects_base_currency_payout() {
        // The pricer implements the domestic cash-or-nothing digital only:
        // a base (foreign) currency payout would be silently mispriced.
        let result = base_digital_builder()
            .payout_amount(Money::new(1_000_000.0, Currency::EUR))
            .build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject a payout_amount not in the quote currency"
        );
    }

    #[test]
    fn validation_rejects_same_currencies() {
        let result = base_digital_builder()
            .base_currency(Currency::USD)
            .quote_currency(Currency::USD)
            // notional must match base_currency — set both to USD to get a clean test
            .notional(Money::new(1_000_000.0, Currency::USD))
            .build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject identical base and quote currencies"
        );
    }

    #[test]
    fn validation_rejects_non_positive_strike() {
        let result = base_digital_builder().strike(0.0).build();
        assert!(result.is_err(), "FxDigitalOption must reject strike = 0");
        let result = base_digital_builder().strike(-1.0).build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject negative strike"
        );
    }

    #[test]
    fn validation_rejects_nan_strike() {
        let result = base_digital_builder().strike(f64::NAN).build();
        assert!(result.is_err(), "FxDigitalOption must reject NaN strike");
    }

    #[test]
    fn validation_rejects_inf_strike() {
        let result = base_digital_builder().strike(f64::INFINITY).build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject infinite strike"
        );
    }

    #[test]
    fn validation_rejects_zero_notional() {
        let result = base_digital_builder()
            .notional(Money::new(0.0, Currency::EUR))
            .build();
        assert!(result.is_err(), "FxDigitalOption must reject zero notional");
    }

    #[test]
    fn validation_rejects_negative_notional() {
        let result = base_digital_builder()
            .notional(Money::new(-100.0, Currency::EUR))
            .build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject negative notional"
        );
    }

    #[test]
    fn validation_rejects_notional_currency_mismatch() {
        // Notional in USD but base_currency = EUR
        let result = base_digital_builder()
            .notional(Money::new(1_000_000.0, Currency::USD))
            .build();
        assert!(
            result.is_err(),
            "FxDigitalOption must reject notional currency != base_currency"
        );
    }

    #[test]
    fn digital_payout_type_fromstr_display_roundtrip() {
        fn assert_digital_payout_type(label: &str, expected: DigitalPayoutType) {
            assert!(matches!(DigitalPayoutType::from_str(label), Ok(value) if value == expected));
        }

        let variants = [
            DigitalPayoutType::CashOrNothing,
            DigitalPayoutType::AssetOrNothing,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed = DigitalPayoutType::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_digital_payout_type("cashornothing", DigitalPayoutType::CashOrNothing);
        assert_digital_payout_type("assetornothing", DigitalPayoutType::AssetOrNothing);
        assert!(DigitalPayoutType::from_str("invalid").is_err());
    }
}
