//! Exchange-listed options on equity futures.

use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::Attributes;
use finstack_quant_valuations_core::instruments::FutureOptionTerms;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Exchange-listed option on an arbitrary equity or equity-index futures contract.
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
pub struct EquityFutureOption {
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

impl EquityFutureOption {
    /// Create a neutral schema and serialization example.
    pub fn example() -> finstack_quant_core::Result<Self> {
        Ok(Self {
            instrument_pricing_overrides:
                finstack_quant_valuations_core::instruments::InstrumentPricingOverrides::default()
                    .with_implied_volatility(0.20),
            ..Self::builder()
                .id(InstrumentId::new("EQUITY-FUTURE-OPTION-EXAMPLE"))
                .terms(FutureOptionTerms::example()?)
                .build()?
        })
    }
}

finstack_quant_valuations_core::impl_future_option_instrument!(
    EquityFutureOption,
    finstack_quant_valuations_core::pricer::InstrumentType::EquityFutureOption,
    crate::EQUITY
);

/// Register delta, gamma, vega, and theta for equity futures options.
pub(crate) fn register_equity_future_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::EquityFutureOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_future_option::EquityFutureOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_future_option::EquityFutureOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_future_option::EquityFutureOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::equity_future_option::EquityFutureOption>::theta()),
        ]
    }
    Ok(())
}
