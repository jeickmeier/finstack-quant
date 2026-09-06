//! Shared trait and pricing delegation for asset-owned futures options.

macro_rules! impl_future_option_instrument {
    ($ty:ty, $key:expr $(, require_rate_risk = $require_rate_risk:expr)?) => {
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
                    market,
                    as_of,
                )
            }

            /// Cash vega for a +0.01 absolute bump in the configured volatility units.
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
                    market,
                    as_of,
                )
            }
        }

        impl crate::instruments::Instrument for $ty {
            crate::impl_instrument_base!($key);

            fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
                self.validate()
            }

            fn market_dependencies(
                &self,
            ) -> finstack_quant_core::Result<crate::instruments::MarketDependencies> {
                let mut dependencies = crate::instruments::MarketDependencies::new();
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

            fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
                None
            }

            crate::impl_focused_pricing_overrides!();
        }

        impl crate::instruments::OptionGreeksProvider for $ty {
            fn option_delta(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<Option<f64>> {
                Ok(Some(self.cash_delta(market, as_of)?))
            }

            fn option_gamma(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<Option<f64>> {
                Ok(Some(self.cash_gamma(market, as_of)?))
            }

            fn option_vega(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<Option<f64>> {
                Ok(Some(self.cash_vega(market, as_of)?))
            }

            fn option_theta(
                &self,
                market: &finstack_quant_core::market_data::context::MarketContext,
                as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<Option<f64>> {
                Ok(Some(self.cash_theta(market, as_of)?))
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
            ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
                Ok(crate::cashflow::traits::schedule_from_classified_flows(
                    Vec::new(),
                    self.terms.day_count,
                    crate::cashflow::traits::ScheduleBuildOpts {
                        notional_hint: Some(finstack_quant_core::money::Money::new(
                            self.terms.contracts * self.terms.multiplier,
                            self.terms.currency,
                        )?),
                        meta: crate::cashflow::builder::CashFlowMeta {
                            representation:
                                crate::cashflow::builder::CashflowRepresentation::Placeholder,
                            ..Default::default()
                        },
                    },
                ))
            }
        }
    };
}

pub(crate) use impl_future_option_instrument;
