//! Trait implementations for Bond (Instrument, CurveDependencies, Monte Carlo).

use crate::impl_instrument_base;
use finstack_core::types::CurveId;

use super::definitions::Bond;
use super::CashflowSpec;

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl crate::instruments::common_impl::traits::Instrument for Bond {
    impl_instrument_base!(crate::pricer::InstrumentType::Bond);

    fn base_value(
        &self,
        curves: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> finstack_core::Result<finstack_core::money::Money> {
        use crate::instruments::fixed_income::bond::pricing::quote_conversions;

        // Honor any bond price-from-quote override (clean, dirty, YTM, YTW,
        // Z-spread, OAS, DM, I-spread, ASW). Mutual exclusivity is enforced by
        // `MarketQuoteOverrides::validate`.
        if let Some(dirty_ccy) = quote_conversions::price_from_quote_overrides(self, curves, as_of)?
        {
            return Ok(finstack_core::money::Money::new(
                dirty_ccy,
                self.notional.currency(),
            ));
        }

        // Check if bond has embedded options requiring tree-based pricing
        if let Some(ref cp) = self.call_put {
            if cp.has_options() {
                return self.value_with_tree(curves, as_of);
            }
        }

        // When a credit curve is assigned, use the hazard-rate engine so that PV
        // incorporates survival probabilities. This makes Bond::value consistent
        // with CS01 metrics and enables meaningful credit P&L attribution.
        // Missing hazard market data is an input error; use discount-only pricing
        // explicitly when credit risk should be ignored.
        if self.credit_curve_id.is_some() {
            return crate::instruments::fixed_income::bond::pricing::engine::hazard::HazardBondEngine::price(
                self, curves, as_of,
            );
        }

        // Standard cashflow discounting for straight bonds without credit curves.
        crate::instruments::fixed_income::bond::pricing::engine::discount::BondEngine::price(
            self, curves, as_of,
        )
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
    {
        crate::instruments::common_impl::dependencies::MarketDependencies::from_curve_dependencies(
            self,
        )
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }

    fn expiry(&self) -> Option<finstack_core::dates::Date> {
        Some(self.maturity)
    }

    fn effective_start_date(&self) -> Option<finstack_core::dates::Date> {
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
            clone.pricing_overrides.model_config.merton_mc_config = None;
        }
        Box::new(clone)
    }

    fn has_custom_metrics_equivalent(&self) -> bool {
        true
    }
}

// Implement CurveDependencies for DV01/CS01 calculators
impl crate::instruments::common_impl::traits::CurveDependencies for Bond {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        let mut builder = crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone());

        if let Some(ref forward_curve_id) = self.forward_curve_id {
            builder = builder.forward(forward_curve_id.clone());
        }

        // Add credit curve if present
        if let Some(ref credit_curve_id) = self.credit_curve_id {
            builder = builder.credit(credit_curve_id.clone());
        }

        // For floating rate bonds, add forward curve from the cashflow spec
        match &self.cashflow_spec {
            CashflowSpec::Floating(floating_spec) => {
                builder = builder.forward(floating_spec.rate_spec.index_id.clone());
            }
            CashflowSpec::Amortizing { base, .. } => {
                // Check if the base spec is floating
                if let CashflowSpec::Floating(floating_spec) = base.as_ref() {
                    builder = builder.forward(floating_spec.rate_spec.index_id.clone());
                }
            }
            _ => {}
        }

        builder.build()
    }
}

impl Bond {
    /// Price this bond using the Merton Monte Carlo structural credit model.
    ///
    /// Extracts coupon rate and frequency from the bond's `CashflowSpec`, then
    /// delegates to
    /// [`crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcEngine::price`].
    ///
    /// If the config's `pik_schedule` is `Uniform(Cash)` (the default),
    /// this method overrides it based on the bond's `CouponType`:
    /// - `CouponType::Cash` → `Uniform(Cash)`
    /// - `CouponType::PIK` → `Uniform(Pik)`
    /// - `CouponType::Split{c, p}` → `Uniform(Split{c, p})`
    ///
    /// If the config already has a non-default `pik_schedule`, it is used
    /// as-is (the config schedule takes precedence).
    pub fn price_merton_mc(
        &self,
        config: &crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcConfig,
        discount_rate: f64,
        as_of: time::Date,
    ) -> finstack_core::Result<
        crate::instruments::fixed_income::bond::pricing::engine::merton_mc::MertonMcResult,
    > {
        use crate::cashflow::builder::specs::CouponType;
        use crate::instruments::fixed_income::bond::pricing::engine::merton_mc::{
            MertonMcConfig, MertonMcEngine, PikMode, PikSchedule,
        };
        use rust_decimal::prelude::ToPrimitive;

        let notional = self.notional.amount();

        let (coupon_rate, coupon_type, coupon_frequency) = match &self.cashflow_spec {
            CashflowSpec::Fixed(spec) => {
                let rate = spec.rate.to_f64().unwrap_or(0.0);
                let freq = (1.0 / spec.freq.to_years_simple()).round() as usize;
                (rate, spec.coupon_type, freq)
            }
            CashflowSpec::Floating(_) => {
                return Err(finstack_core::InputError::Invalid.into());
            }
            CashflowSpec::StepUp(spec) => {
                // Use initial_rate for Merton MC calibration
                let rate = spec.initial_rate.to_f64().unwrap_or(0.0);
                let freq = (1.0 / spec.freq.to_years_simple()).round() as usize;
                (rate, spec.coupon_type, freq)
            }
            // The Merton MC engine simulates a constant notional with full
            // bullet redemption at maturity; silently extracting the base
            // coupon (the previous behavior) priced amortizers as bullets.
            CashflowSpec::Amortizing { .. } => {
                return Err(finstack_core::Error::Validation(format!(
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
            finstack_core::dates::DayCountContext::default(),
        )?;

        // If the config uses the default schedule, derive from bond's CouponType
        let effective_config;
        let config_ref = if matches!(config.pik_schedule, PikSchedule::Uniform(PikMode::Cash)) {
            let bond_mode = match coupon_type {
                CouponType::Cash => PikMode::Cash,
                CouponType::PIK => PikMode::Pik,
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
