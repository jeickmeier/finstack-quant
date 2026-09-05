//! Trait implementations for Bond (Instrument and Monte Carlo).

use crate::impl_instrument_base;
use finstack_quant_core::types::CurveId;

use super::definitions::Bond;
use super::CashflowSpec;

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl crate::instruments::common_impl::traits::Instrument for Bond {
    impl_instrument_base!(crate::pricer::InstrumentType::Bond);

    fn default_model(&self) -> crate::pricer::ModelKey {
        self.default_pricing_model()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        finstack_quant_core::money::Money::new(
            self.base_value_raw_impl(curves, as_of)?,
            self.notional.currency(),
        )
    }

    fn base_value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        self.base_value_raw_impl(curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            self.base_value_raw_impl(curves, as_of)?,
            self.notional.currency(),
        ))
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(forward_curve_id) = &self.forward_curve_id {
            deps.add_forward_curve(forward_curve_id.clone());
        }
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        match &self.cashflow_spec {
            CashflowSpec::Floating(spec) => {
                deps.add_forward_curve(spec.rate_spec.index_id.clone());
                deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                    spec.rate_spec.index_id.as_str(),
                ));
            }
            CashflowSpec::Amortizing { base, .. } => {
                if let CashflowSpec::Floating(spec) = base.as_ref() {
                    deps.add_forward_curve(spec.rate_spec.index_id.clone());
                    deps.add_series_id(
                        finstack_quant_core::market_data::fixings::fixing_series_id(
                            spec.rate_spec.index_id.as_str(),
                        ),
                    );
                }
            }
            _ => {}
        }
        if let Some(call_put) = &self.call_put {
            for option in call_put.calls.iter().chain(&call_put.puts) {
                if let Some(make_whole) = &option.make_whole {
                    deps.add_discount_curve(make_whole.reference_curve_id.clone());
                }
            }
        }
        if let Some(curve_id) = &self
            .instrument_pricing_overrides
            .model_config
            .tree_discount_curve_id
        {
            deps.add_discount_curve(curve_id.clone());
        }
        if let Some(curve_id) = &self
            .instrument_pricing_overrides
            .model_config
            .asw_forward_curve_id
        {
            deps.add_forward_curve(curve_id.clone());
        }
        Ok(deps)
    }

    crate::impl_focused_pricing_overrides!();

    fn scenario_spread_shock_supported(&self) -> bool {
        // Mirrors the guards in `base_value`: the shock is exact only for
        // bonds without embedded options, without an assigned credit curve,
        // and without a price-pinning quote other than `quoted_z_spread`.
        self.return_floor.is_none()
            && !self
                .call_put
                .as_ref()
                .is_some_and(super::definitions::CallPutSchedule::has_options)
            && self.credit_curve_id.is_none()
            && !self
                .instrument_pricing_overrides
                .market_quotes
                .has_non_z_price_driver()
    }

    /// Validate the complete bond boundary before every public pricing call.
    ///
    /// Builders and deserializers can construct a `Bond` without using the
    /// convenience constructors, so the pricing lifecycle must enforce the
    /// same economic invariants as [`Bond::validate`].
    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.issue_date)
    }

    fn funding_curve_id(&self) -> Option<CurveId> {
        self.funding_curve_id.clone()
    }

    fn metrics_equivalent(&self) -> Box<dyn crate::instruments::common_impl::traits::Instrument> {
        use crate::cashflow::builder::specs::CouponType;

        let mut clone = self.clone();

        match &mut clone.cashflow_spec {
            CashflowSpec::Fixed(ref mut spec) => {
                spec.coupon_type = CouponType::Cash;
            }
            CashflowSpec::Amortizing { ref mut base, .. } => {
                if let CashflowSpec::Fixed(ref mut spec) = base.as_mut() {
                    spec.coupon_type = CouponType::Cash;
                }
            }
            _ => {}
        }

        {
            clone
                .instrument_pricing_overrides
                .model_config
                .merton_mc_config = None;
        }
        Box::new(clone)
    }

    fn has_custom_metrics_equivalent(&self) -> bool {
        true
    }
}

// Declare canonical market dependencies for DV01/CS01 calculators.
impl Bond {
    /// Reject embedded bondholder or issuer rights that the Merton MC payoff
    /// kernel does not model.
    pub(crate) fn validate_merton_mc_embedded_rights(&self) -> finstack_quant_core::Result<()> {
        let has_call_put = self
            .call_put
            .as_ref()
            .is_some_and(super::definitions::CallPutSchedule::has_options);
        if has_call_put || self.return_floor.is_some() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Merton MC pricing does not support embedded call, put, or return-floor rights for bond '{}'; select model 'tree' for rates-only optionality or 'rates_credit' for joint rates-credit optionality",
                self.id
            )));
        }
        Ok(())
    }

    /// Advanced Merton Monte Carlo structural-credit price for this bond.
    ///
    /// Host and registry callers should use
    /// [`Instrument::price_with_metrics`](crate::instruments::Instrument::price_with_metrics)
    /// with [`ModelKey::MertonMc`](crate::pricer::ModelKey::MertonMc)
    /// (Python: `Bond.price` / `price_instrument(..., model="merton_mc")`).
    /// This method remains for tests and calibration that need the typed
    /// `MertonMcResult` without the valuation envelope.
    ///
    /// Extracts coupon rate and frequency from the bond's `CashflowSpec`, then
    /// delegates to
    /// [`crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcEngine::price`].
    ///
    /// If the config's `pik_schedule` is `Uniform(Cash)` (the default),
    /// this method overrides it based on the bond's `CouponType`:
    /// - `CouponType::Cash` → `Uniform(Cash)`
    /// - `CouponType::Pik` → `Uniform(Pik)`
    /// - `CouponType::Split{c, p}` → `Uniform(Split{c, p})`
    ///
    /// If the config already has a non-default `pik_schedule`, it is used
    /// as-is (the config schedule takes precedence).
    ///
    /// # Arguments
    ///
    /// * `config` - Structural-credit dynamics, recovery, PIK behavior, path
    ///   count, seed, and time discretization for this simulation.
    /// * `discount_rate` - Flat annual continuously compounded risk-free rate
    ///   as a decimal. It drives risk-neutral asset drift and discounts cashflows
    ///   when `config.cashflow_dfs` is absent.
    /// * `as_of` - Valuation date from which remaining maturity and coupon times
    ///   are measured under the bond cashflow day-count convention.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the bond contains call, put, or
    /// return-floor rights because this Merton payoff kernel does not model
    /// issuer or holder exercise. Use the rates-only `tree` model or the joint
    /// `rates_credit` model for embedded optionality. Other errors report
    /// unsupported cashflow shapes, invalid dates, or invalid MC inputs.
    pub fn price_merton_mc(
        &self,
        config: &crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
        discount_rate: f64,
        as_of: time::Date,
    ) -> finstack_quant_core::Result<
        crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcResult,
    > {
        use crate::cashflow::builder::specs::CouponType;
        use crate::instruments::fixed_income::bond::pricing::engine::merton_mc::{
            MertonMcConfig, MertonMcEngine, PikMode, PikSchedule,
        };
        use rust_decimal::prelude::ToPrimitive;

        self.validate_merton_mc_embedded_rights()?;

        let notional = self.notional.amount();

        let (coupon_rate, coupon_type, coupon_frequency) = match &self.cashflow_spec {
            CashflowSpec::Fixed(spec) => {
                let rate = spec.rate.to_f64().unwrap_or(0.0);
                let frequency = (1.0 / spec.schedule.frequency.to_years()).round() as usize;
                (rate, spec.coupon_type, frequency)
            }
            CashflowSpec::Floating(_) => {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            CashflowSpec::StepUp(spec) => {
                // Use initial_rate for Merton MC calibration
                let rate = spec.initial_rate.to_f64().unwrap_or(0.0);
                let frequency = (1.0 / spec.schedule.frequency.to_years()).round() as usize;
                (rate, spec.coupon_type, frequency)
            }
            // The Merton MC engine simulates a constant notional with full
            // bullet redemption at maturity; silently extracting the base
            // coupon (the previous behavior) priced amortizers as bullets.
            CashflowSpec::Amortizing { .. } => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Merton MC pricing does not support amortizing bonds (bond '{}'): the \
                     engine assumes constant notional with bullet redemption at maturity; \
                     the amortization schedule would be ignored",
                    self.id
                )));
            }
        };

        let maturity_years = self.cashflow_spec.day_count().year_fraction(
            as_of,
            self.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;

        // If the config uses the default schedule, derive from bond's CouponType
        let effective_config;
        let config_ref = if matches!(config.pik_schedule, PikSchedule::Uniform(PikMode::Cash)) {
            let bond_mode = match coupon_type {
                CouponType::Cash => PikMode::Cash,
                CouponType::Pik => PikMode::Pik,
                CouponType::Split { cash_pct, pik_pct } => PikMode::Split {
                    cash_fraction: cash_pct.to_f64().unwrap_or(1.0),
                    pik_fraction: pik_pct.to_f64().unwrap_or(0.0),
                },
            };
            if !matches!(bond_mode, PikMode::Cash) {
                effective_config = MertonMcConfig {
                    pik_schedule: PikSchedule::Uniform(bond_mode),
                    ..config.clone()
                };
                &effective_config
            } else {
                config
            }
        } else {
            config
        };

        MertonMcEngine::price(
            notional,
            coupon_rate,
            maturity_years,
            coupon_frequency,
            config_ref,
            discount_rate,
        )
    }
}

#[cfg(test)]
mod dependency_tests {
    use super::*;

    #[test]
    fn floating_bond_uses_the_canonical_fixing_series_id() {
        let bond = Bond::example_floating().expect("floating bond example");
        let CashflowSpec::Floating(spec) = &bond.cashflow_spec else {
            unreachable!("floating example must have a floating coupon");
        };
        let expected = finstack_quant_core::market_data::fixings::fixing_series_id(
            spec.rate_spec.index_id.as_str(),
        );

        let deps =
            crate::instruments::Instrument::market_dependencies(&bond).expect("dependencies");
        assert_eq!(deps.series_ids, vec![expected]);
    }
}
