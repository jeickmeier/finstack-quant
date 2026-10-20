//! Shared trait and pricing delegation for asset-owned futures options.

#[macro_export]
macro_rules! impl_future_option_instrument {
    ($ty:ty, $key:expr, $family:expr $(, require_rate_risk = $require_rate_risk:expr)?) => {
        impl $ty {
            /// Validate the shared contract economics and lifecycle state.
            pub fn validate(&self) -> finstack_quant_core::Result<()> {
                self.terms.validate()?;
                $(
                    if $require_rate_risk && self.terms.underlying_price_change_per_bp.is_none() {
                        return Err(finstack_quant_core::Error::Validation(
                            "interest-rate futures option requires underlying_price_change_per_bp for DV01"
                                .to_string(),
                        ));
                    }
                )?
                Ok(())
            }

            /// Return the contract's signed fair value in settlement currency.
            ///
            /// A live option is priced with the flat volatility in
            /// `instrument_pricing_overrides.market_quotes.implied_volatility`.
            ///
            /// # Arguments
            ///
            /// * `market` - Market context containing the settlement discount curve.
            /// * `as_of` - Valuation date controlling live versus exercised lifecycle state.
            pub fn npv_raw(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.terms.npv_raw(
                    &self.id,
                    self.instrument_pricing_overrides.model_config.tree_steps,
                    self.instrument_pricing_overrides.market_quotes.implied_volatility,
                    market,
                    as_of,
                )
            }

            /// Cash delta for a one-point move in the underlying futures price.
            ///
            /// # Arguments
            ///
            /// * `market` - Market context containing the settlement discount curve.
            /// * `as_of` - Valuation date.
            pub fn cash_delta(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.terms.cash_delta(
                    self.instrument_pricing_overrides.model_config.tree_steps,
                    self.instrument_pricing_overrides.market_quotes.implied_volatility,
                    market,
                    as_of,
                )
            }

            /// Cash gamma for a one-point squared move in the underlying futures price.
            ///
            /// # Arguments
            ///
            /// * `market` - Market context containing the settlement discount curve.
            /// * `as_of` - Valuation date.
            pub fn cash_gamma(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.terms.cash_gamma(
                    self.instrument_pricing_overrides.model_config.tree_steps,
                    self.instrument_pricing_overrides.market_quotes.implied_volatility,
                    market,
                    as_of,
                )
            }

            /// Cash vega for a +0.01 absolute bump in `market_quotes.implied_volatility` units.
            ///
            /// # Arguments
            ///
            /// * `market` - Market context containing the settlement discount curve.
            /// * `as_of` - Valuation date.
            pub fn cash_vega(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.terms.cash_vega(
                    self.instrument_pricing_overrides.model_config.tree_steps,
                    self.instrument_pricing_overrides.market_quotes.implied_volatility,
                    market,
                    as_of,
                )
            }

            /// One-calendar-day theta with market quotes held fixed.
            ///
            /// # Arguments
            ///
            /// * `market` - Market context containing the settlement discount curve.
            /// * `as_of` - Valuation date.
            pub fn cash_theta(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.terms.cash_theta(
                    self.instrument_pricing_overrides.model_config.tree_steps,
                    self.instrument_pricing_overrides.market_quotes.implied_volatility,
                    market,
                    as_of,
                )
            }
        }

        impl $crate::instruments::Instrument for $ty {
            $crate::impl_instrument_base!($key, $family);

            fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
                self.validate()
            }

            fn market_dependencies(
                &self,
            ) -> finstack_quant_core::Result<$crate::instruments::MarketDependencies> {
                let mut dependencies = $crate::instruments::MarketDependencies::new();
                dependencies.add_discount_curve(self.terms.discount_curve_id.clone());
                Ok(dependencies)
            }

            fn base_value(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
                finstack_quant_core::money::Money::new(
                    self.npv_raw(market, as_of)?,
                    self.terms.currency,
                )
            }

            fn base_value_raw(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<f64> {
                self.npv_raw(market, as_of)
            }

            fn base_value_raw_with_currency(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
                Ok((self.npv_raw(market, as_of)?, self.terms.currency))
            }

            fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
                Some(self.terms.settlement.terminal_date())
            }


            $crate::impl_focused_pricing_overrides!();
        }

        impl $crate::instruments::OptionGreeksProvider for $ty {
            fn option_greeks(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
                request: &$crate::instruments::OptionGreeksRequest,
            ) -> finstack_quant_core::Result<$crate::instruments::OptionGreeks> {
                use $crate::instruments::{OptionGreekKind, OptionGreeks};

                let mut greeks = OptionGreeks::default();
                match request.greek {
                    OptionGreekKind::Delta => greeks.delta = Some(self.cash_delta(market, as_of)?),
                    OptionGreekKind::Gamma => greeks.gamma = Some(self.cash_gamma(market, as_of)?),
                    OptionGreekKind::Vega => greeks.vega = Some(self.cash_vega(market, as_of)?),
                    OptionGreekKind::Theta => greeks.theta = Some(self.cash_theta(market, as_of)?),
                    _ => {}
                }
                Ok(greeks)
            }
        }

        impl finstack_quant_cashflows::CashflowScheduleSource for $ty {
            fn notional(&self) -> finstack_quant_core::Result<Option<finstack_quant_core::money::Money>> {
                Ok(Some(finstack_quant_core::money::Money::new(
                    self.terms.contracts * self.terms.multiplier,
                    self.terms.currency,
                )?))
            }

            fn raw_cashflow_schedule(
                &self,
                _market: &finstack_quant_core::market_data::context::MarketContext,
                _as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
                Ok(finstack_quant_cashflows::schedule_from_classified_flows(
                    Vec::new(),
                    self.terms.day_count,
                    finstack_quant_cashflows::ScheduleBuildOpts {
                        notional_hint: Some(finstack_quant_core::money::Money::new(
                            self.terms.contracts * self.terms.multiplier,
                            self.terms.currency,
                        )?),
                        meta: finstack_quant_cashflows::builder::CashFlowMeta {
                            representation:
                                finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder,
                            ..Default::default()
                        },
                    },
                ))
            }
        }
    };
}

/// Define an asset-owned futures-option instrument.
///
/// Emits the instrument struct (an `id`, the shared
/// [`FutureOptionTerms`](crate::instruments::FutureOptionTerms) and the
/// instrument/metric/scenario pricing overrides), its
/// [`Instrument`](crate::instruments::Instrument) implementation through
/// [`impl_future_option_instrument!`], and a `pub(crate)` registration
/// function named by `register_metrics` that registers delta, gamma, vega and
/// theta for `instrument_type`. Doc comments and attributes written before
/// `pub struct` are applied to the struct. The expanding crate must depend on
/// `finstack-quant-valuations-macros`, `serde` and (behind its `json-schema`
/// feature) `schemars`.
///
/// # Arguments
///
/// * `$name` - Struct name; also the JSON-schema type name.
/// * `instrument_type` - The `InstrumentType` variant the instrument and its
///   metrics are registered under.
/// * `family` - The family static owning the instrument.
/// * `register_metrics` - Name of the generated metric-registration function.
#[macro_export]
macro_rules! define_future_option {
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        instrument_type: $key:expr,
        family: $family:expr,
        register_metrics: $register:ident $(,)?
    ) => {
        $(#[$meta])*
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
        pub struct $name {
            /// Unique instrument identifier.
            pub id: finstack_quant_core::types::InstrumentId,
            /// Caller-supplied option-on-future pricing and settlement terms.
            pub terms: finstack_quant_valuations_core::instruments::FutureOptionTerms,
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
            pub attributes: finstack_quant_valuations_core::instruments::Attributes,
        }

        $crate::impl_future_option_instrument!($name, $key, $family);

        /// Register delta, gamma, vega, and theta for this futures option.
        pub(crate) fn $register(
            registry: &mut $crate::metrics::MetricRegistry,
        ) -> std::result::Result<(), $crate::metrics::MetricRegistryError> {
            $crate::register_metrics! {
                registry: registry,
                instrument: $key,
                metrics: [
                    (Delta, $crate::metrics::sensitivities::option_greeks::OptionGreekCalculator::<$name>::delta()),
                    (Gamma, $crate::metrics::sensitivities::option_greeks::OptionGreekCalculator::<$name>::gamma()),
                    (Vega, $crate::metrics::sensitivities::option_greeks::OptionGreekCalculator::<$name>::vega()),
                    (Theta, $crate::metrics::sensitivities::option_greeks::OptionGreekCalculator::<$name>::theta()),
                ]
            }
            Ok(())
        }
    };
}
