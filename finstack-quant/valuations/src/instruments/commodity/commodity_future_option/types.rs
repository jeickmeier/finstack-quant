//! General-purpose commodity futures-option instrument.

use crate::instruments::common_impl::listed::FutureOptionTerms;
use crate::instruments::Attributes;
use finstack_quant_core::types::InstrumentId;

/// Exchange-listed option on an arbitrary commodity futures contract.
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
pub struct CommodityFutureOption {
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

impl CommodityFutureOption {
    /// Construct a commodity futures option from arbitrary caller-supplied terms.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable instrument identifier.
    /// * `terms` - Complete futures-option economics; no exchange root or contract definition is implied.
    pub fn new(id: InstrumentId, terms: FutureOptionTerms) -> finstack_quant_core::Result<Self> {
        Self::builder().id(id).terms(terms).build()
    }

    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides::default()
                .with_implied_vol(0.20),
            ..Self::new(
                InstrumentId::new("COMMODITY-FUTURE-OPTION-EXAMPLE"),
                FutureOptionTerms::example()?,
            )?
        })
    }
}

crate::instruments::common_impl::listed::impl_future_option_instrument!(
    CommodityFutureOption,
    crate::pricer::InstrumentType::CommodityFutureOption
);
