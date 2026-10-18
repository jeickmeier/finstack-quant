//! General-purpose FX futures-option instrument.

use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::common_impl::listed::FutureOptionTerms;
use finstack_quant_valuations_core::instruments::Attributes;

/// Exchange-listed option on an arbitrary FX futures contract.
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
pub struct FxFutureOption {
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
        skip_serializing_if = "finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::ScenarioPricingOverrides,
    /// Attributes for selection and reporting.
    #[builder(default)]
    #[serde(default)]
    pub attributes: Attributes,
}

impl FxFutureOption {
    /// Construct an FX futures option from arbitrary caller-supplied terms.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable instrument identifier.
    /// * `terms` - Complete futures-option economics; no exchange root or contract definition is implied.
    pub fn new(
        id: impl Into<InstrumentId>,
        terms: FutureOptionTerms,
    ) -> finstack_quant_core::Result<Self> {
        Self::builder().id(id.into()).terms(terms).build()
    }

    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::new(
                InstrumentId::new("FX-FUTURE-OPTION-EXAMPLE"),
                FutureOptionTerms::example()?,
            )?
        })
    }
}

finstack_quant_valuations_core::instruments::common_impl::listed::impl_future_option_instrument!(
    FxFutureOption,
    finstack_quant_valuations_core::pricer::InstrumentType::FxFutureOption,
    crate::FX
);
