//! General-purpose interest-rate futures-option instrument.

use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::FutureOptionTerms;

/// Option on an arbitrary interest-rate futures price.
///
/// One instrument covers exchange-listed contracts and bilateral/OTC trades.
/// All economics are caller-supplied; no exchange symbol or contract definition
/// is embedded in the type.
#[derive(
    Clone,
    Debug,
    PartialEq,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InterestRateFutureOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Complete listed or bilateral option-on-future terms.
    pub terms: FutureOptionTerms,
    /// Instrument-owned pricing inputs: the flat option volatility in
    /// `market_quotes.implied_volatility` (required while live) and optional
    /// tree-step overrides.
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
    /// Attributes for selection and reporting.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
}

impl InterestRateFutureOption {
    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::builder()
                .id(InstrumentId::new("INTEREST-RATE-FUTURE-OPTION-EXAMPLE"))
                .terms(FutureOptionTerms {
                    underlying_price_change_per_bp: Some(-0.01),
                    ..FutureOptionTerms::example()?
                })
                .build()?
        })
    }

    /// PV change for a one-basis-point increase in the mapped underlying rate.
    ///
    /// The calculation combines the option's futures-price delta channel with
    /// the premium discounting channel. The underlying price transform is an
    /// explicit contract input, so STIR and bond-future options share this type
    /// without symbol-specific logic.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context containing the option discount curve.
    /// * `as_of` - Valuation date used for option delta and discount horizon.
    pub fn rate_dv01(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.terms.validate()?;
        let price_change = self.terms.underlying_price_change_per_bp.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "InterestRateFutureOption requires underlying_price_change_per_bp for DV01"
                    .to_string(),
            )
        })?;
        let delta_channel = self.cash_delta(market, as_of)? * price_change;
        let discount_horizon = match self.terms.settlement {
            finstack_quant_valuations_core::instruments::FutureOptionSettlement::Cash {
                payment_date,
            } => Some(payment_date),
            finstack_quant_valuations_core::instruments::FutureOptionSettlement::Future {
                ..
            } => self
                .terms
                .exercise
                .filter(|ex| as_of >= ex.date)
                .map_or(Some(self.terms.expiry), |_| None),
        };
        let discount_channel = if self.terms.premium_style
            == finstack_quant_valuations_core::instruments::FutureOptionPremiumStyle::PremiumPaid
        {
            if let Some(horizon) = discount_horizon.filter(|&h| as_of < h) {
                let t = self.terms.day_count.year_fraction(
                    as_of,
                    horizon,
                    DayCountContext::default(),
                )?;
                -t * 1.0e-4 * self.npv_raw(market, as_of)?
            } else {
                0.0
            }
        } else {
            0.0
        };
        Ok(delta_channel + discount_channel)
    }
}

finstack_quant_valuations_core::impl_future_option_instrument!(
    InterestRateFutureOption,
    finstack_quant_valuations_core::pricer::InstrumentType::InterestRateFutureOption,
    crate::RATES,
    require_rate_risk = true
);

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_valuations_core::instruments::{
        FutureOptionPremiumStyle, FutureOptionSettlement,
    };
    use time::macros::date;

    #[test]
    fn one_type_accepts_bilateral_custom_terms() {
        let mut terms = FutureOptionTerms::example().expect("neutral future-option terms");
        terms.underlying = "OTC-CUSTOM-RATE-FUTURE".to_string();
        terms.contracts = 2.5;
        terms.multiplier = 2_500.0;
        terms.underlying_price_change_per_bp = Some(-0.0125);
        terms.premium_style = FutureOptionPremiumStyle::PremiumPaid;
        terms.settlement = FutureOptionSettlement::Cash {
            payment_date: date!(2026 - 12 - 16),
        };

        let option = InterestRateFutureOption::builder()
            .id(InstrumentId::new("OTC-RATE-FUTURE-OPTION"))
            .terms(terms)
            .build()
            .expect("custom bilateral contract");

        assert_eq!(option.terms.underlying, "OTC-CUSTOM-RATE-FUTURE");
        assert_eq!(option.terms.contracts, 2.5);
        assert_eq!(option.terms.multiplier, 2_500.0);
    }

    #[test]
    fn mapped_rate_dv01_is_available_for_custom_rate_future_options() {
        let as_of = date!(2026 - 01 - 01);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (2.0, 0.90)])
                .build()
                .expect("discount curve"),
        );
        let option = InterestRateFutureOption::example().expect("rate future option");
        let dv01 = option.rate_dv01(&market, as_of).expect("rate DV01");
        assert!(dv01.is_finite());
        assert!(dv01 < 0.0);
    }

    #[test]
    fn rate_future_option_rejects_missing_rate_risk_transform() {
        let terms = FutureOptionTerms::example().expect("terms");
        let error = InterestRateFutureOption::builder()
            .id(InstrumentId::new("MISSING-RISK"))
            .terms(terms)
            .build()
            .expect_err("missing rate-risk transform");
        assert!(error.to_string().contains("underlying_price_change_per_bp"));
    }
}
