//! General-purpose interest-rate futures-option instrument.

use crate::instruments::common_impl::listed::FutureOptionTerms;
use crate::instruments::Attributes;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::InstrumentId;

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
        skip_serializing_if = "crate::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-only pricing controls.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "crate::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for selection and reporting.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
}

impl InterestRateFutureOption {
    /// Construct an interest-rate futures option from arbitrary listed or OTC terms.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable instrument identifier.
    /// * `terms` - Complete caller-supplied futures-option economics; no venue, root, or contract definition is implied.
    pub fn new(id: InstrumentId, terms: FutureOptionTerms) -> finstack_quant_core::Result<Self> {
        Self::builder().id(id).terms(terms).build()
    }

    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides::default()
                .with_implied_vol(0.20),
            ..Self::new(
                InstrumentId::new("INTEREST-RATE-FUTURE-OPTION-EXAMPLE"),
                FutureOptionTerms {
                    underlying_price_change_per_bp: Some(-0.01),
                    ..FutureOptionTerms::example()?
                },
            )?
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
        let discount_channel = if self.terms.premium_style
            == crate::instruments::FutureOptionPremiumStyle::PremiumPaid
            && as_of < self.terms.expiry
        {
            let t = self.terms.day_count.year_fraction(
                as_of,
                self.terms.expiry,
                DayCountContext::default(),
            )?;
            -t * 1.0e-4 * self.npv_raw(market, as_of)?
        } else {
            0.0
        };
        Ok(delta_channel + discount_channel)
    }
}

crate::instruments::common_impl::listed::impl_future_option_instrument!(
    InterestRateFutureOption,
    crate::pricer::InstrumentType::InterestRateFutureOption,
    require_rate_risk = true
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::listed::{
        FutureOptionPremiumStyle, FutureOptionSettlement,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
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

        let option =
            InterestRateFutureOption::new(InstrumentId::new("OTC-RATE-FUTURE-OPTION"), terms)
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
        let error = InterestRateFutureOption::new(InstrumentId::new("MISSING-RISK"), terms)
            .expect_err("missing rate-risk transform");
        assert!(error.to_string().contains("underlying_price_change_per_bp"));
    }
}
