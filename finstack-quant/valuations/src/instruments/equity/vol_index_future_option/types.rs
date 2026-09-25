//! General-purpose volatility-index futures-option instrument.

use crate::instruments::common_impl::listed::{
    FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement, FutureOptionTerms,
};
use crate::instruments::common_impl::parameters::{ExerciseStyle, OptionType};
use crate::instruments::{Attributes, Position};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Exchange-listed option on a volatility-index futures contract.
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
pub struct VolatilityIndexFutureOption {
    /// Unique instrument identifier.
    pub id: InstrumentId,
    /// Caller-supplied option-on-future pricing and settlement terms.
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
    /// Metric-time pricing configuration.
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

impl VolatilityIndexFutureOption {
    /// Construct a volatility-index futures option from arbitrary caller-supplied terms.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable instrument identifier.
    /// * `terms` - Complete futures-option economics; no exchange root or contract definition is implied.
    pub fn new(id: InstrumentId, terms: FutureOptionTerms) -> finstack_quant_core::Result<Self> {
        Self::builder().id(id).terms(terms).build()
    }

    /// Create a VSTOXX futures-option example for schema and serialization output.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;

        let expiry = date!(2026 - 12 - 16);
        let terms = FutureOptionTerms::builder()
            .underlying("FVS".to_string())
            .futures_price(20.0)
            .option_reference_price(2.0)
            .strike(20.0)
            .contracts(1.0)
            .multiplier(100.0)
            .currency(Currency::EUR)
            .option_type(OptionType::Call)
            .position(Position::Long)
            .exercise_style(ExerciseStyle::American)
            .expiry(expiry)
            .settlement(FutureOptionSettlement::Future {
                underlying_last_trading_date: expiry,
                underlying_settlement_date: expiry,
                underlying_settlement_price: None,
            })
            .model(FutureOptionModel::Black76)
            .premium_style(FutureOptionPremiumStyle::FuturesStyle)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("EUR-OIS"))
            .build()?;
        Ok(Self {
            instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides::default()
                .with_implied_vol(0.50),
            ..Self::new(InstrumentId::new("VSTOXX-FUTURE-OPTION-EXAMPLE"), terms)?
        })
    }
}

crate::instruments::common_impl::listed::impl_future_option_instrument!(
    VolatilityIndexFutureOption,
    crate::pricer::InstrumentType::VolatilityIndexFutureOption
);
