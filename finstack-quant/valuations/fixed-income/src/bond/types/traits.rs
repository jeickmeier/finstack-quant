//! Trait implementations for Bond (Instrument, Monte Carlo and cashflow source).
//!
//! The cashflow schedule source produces a signed canonical schedule that
//! preserves fees, signed notionals, and all valid cash events. PIK accretion
//! retains its classification and amount for accrual and balance replay; dated
//! settlement views omit the non-cash rows.

use finstack_quant_cashflows::builder::CashflowRepresentation;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::impl_instrument_base;

use super::definitions::Bond;
use super::CashflowSpec;

// Explicit Instrument trait implementation (replaces macro for better IDE visibility)
impl finstack_quant_valuations_core::instruments::Instrument for Bond {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::Bond,
        crate::FIXED_INCOME
    );

    fn notional_draw_date(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.issue_date)
    }

    fn validate_cashflow_export(
        &self,
        model: finstack_quant_valuations_core::pricer::ModelKey,
    ) -> finstack_quant_core::Result<()> {
        if self.has_exercise_rights() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "instrument_cashflows: static cashflow rows cannot decompose the \
                 exercise-contingent value of bond '{}' under model '{}'; request the model \
                 price directly",
                self.id.as_str(),
                model.as_str()
            )));
        }
        Ok(())
    }

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        self.default_pricing_model()
    }

    fn base_value(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        finstack_quant_core::money::Money::new(
            finstack_quant_valuations_core::instruments::Instrument::base_value_raw(
                self, curves, as_of,
            )?,
            self.notional.currency(),
        )
    }

    fn base_value_raw(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<f64> {
        // Instrument-native default path: the unshocked pricing-policy value
        // without entering the public instrument lifecycle. Explicit registry
        // pricing bypasses inference and calls `price_for_model_raw` with the
        // caller-selected model.
        self.price_for_model_raw(self.default_pricing_model(), curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((
            finstack_quant_valuations_core::instruments::Instrument::base_value_raw(
                self, curves, as_of,
            )?,
            self.notional.currency(),
        ))
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        if let Some(credit_curve_id) = &self.credit_curve_id {
            deps.add_credit_curve(credit_curve_id.clone());
        }
        if let Some(rate_spec) = self.cashflow_spec.floating_rate_spec() {
            deps.add_forward_curve(rate_spec.forward_curve_id.clone());
            deps.add_series_id(finstack_quant_core::market_data::fixings::fixing_series_id(
                rate_spec.forward_curve_id.as_str(),
            ));
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

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();

    fn scenario_spread_shock_supported(&self) -> bool {
        // Mirrors the guards in `base_value`: the shock is exact only for
        // bonds without embedded options, without an assigned credit curve,
        // and without a price-pinning quote other than `quoted_z_spread`.
        !self.has_exercise_rights()
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

    fn last_payment_date(
        &self,
        curves: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Option<finstack_quant_core::dates::Date>> {
        let schedule =
            finstack_quant_cashflows::CashflowProvider::cashflow_schedule(self, curves, as_of)?;
        Ok(schedule
            .get_flows()
            .iter()
            .map(|flow| flow.date)
            .chain(self.expiry())
            .max())
    }

    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }

    fn expiry(&self) -> Option<finstack_quant_core::dates::Date> {
        Some(self.maturity)
    }

    fn repo_curve_id(&self) -> Option<CurveId> {
        self.repo_curve_id.clone()
    }

    fn metrics_equivalent(
        &self,
    ) -> Option<Box<dyn finstack_quant_valuations_core::instruments::Instrument>> {
        let mut clone = self.clone();
        clone.cashflow_spec.make_cash_pay();
        clone
            .instrument_pricing_overrides
            .model_config
            .merton_mc_config = None;
        Some(Box::new(clone))
    }
}

impl Bond {
    /// Reject embedded bondholder or issuer rights that the Merton MC payoff
    /// kernel does not model.
    pub(crate) fn validate_merton_mc_embedded_rights(&self) -> finstack_quant_core::Result<()> {
        if self.has_exercise_rights() {
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
    /// [`Instrument::price_with_metrics`](finstack_quant_valuations_core::instruments::Instrument::price_with_metrics)
    /// with [`ModelKey::MertonMc`](finstack_quant_valuations_core::pricer::ModelKey::MertonMc)
    /// (Python: `Bond.price` / `price_instrument(..., model="merton_mc")`).
    /// This method remains for tests and calibration that need the typed
    /// `MertonMcResult` without the valuation envelope.
    ///
    /// Extracts the coupon rate from the bond's `CashflowSpec` and drives the
    /// simulation from the bond's own remaining coupon dates and accrual
    /// fractions (on the coupon day-count clock, with the ACT/ACT (ICMA)
    /// quasi-coupon context where needed). The full next coupon is paid, so
    /// `dirty_price_pct` is the simulated PV and `clean_price_pct` subtracts
    /// the accrued interest at `as_of`.
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
    /// * `config` - Structural-credit dynamics, recovery, PIK behavior and
    ///   time discretization for this simulation. The independent-estimator
    ///   count, antithetic flag and seed label come from this bond's
    ///   `instrument_pricing_overrides.model_config` (`mc_paths`,
    ///   `mc_antithetic`, `mc_seed_scenario`), with the registry
    ///   `merton_pik_bond` defaults.
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
        config: &finstack_quant_valuations_core::instruments::MertonMcConfig,
        discount_rate: f64,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::bond::pricing::merton_mc::MertonMcResult> {
        let run = crate::bond::pricing::merton_mc::MertonMcRun::resolve(
            &self.id,
            &self.instrument_pricing_overrides.model_config,
        )?;
        self.price_merton_mc_with_run(config, &run, discount_rate, as_of)
    }

    /// Merton Monte Carlo price with explicit sampling settings.
    ///
    /// Shared by [`Bond::price_merton_mc`] and the low-path market calibration.
    pub(crate) fn price_merton_mc_with_run(
        &self,
        config: &finstack_quant_valuations_core::instruments::MertonMcConfig,
        run: &crate::bond::pricing::merton_mc::MertonMcRun,
        discount_rate: f64,
        as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<crate::bond::pricing::merton_mc::MertonMcResult> {
        use crate::bond::pricing::merton_mc::{MertonBondTerms, MertonMcEngine};
        use finstack_quant_cashflows::builder::CouponType;
        use finstack_quant_valuations_core::instruments::{MertonMcConfig, PikMode, PikSchedule};
        use rust_decimal::prelude::ToPrimitive;

        self.validate_merton_mc_embedded_rights()?;

        let notional = self.notional.amount();

        let (coupon_rate, coupon_type) = match &self.cashflow_spec {
            CashflowSpec::Fixed(spec) => (spec.rate.to_f64().unwrap_or(0.0), spec.coupon_type),
            CashflowSpec::Floating(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Merton MC pricing requires a fixed-coupon bond".to_string(),
                ));
            }
            // Step-up bonds are simulated at their initial rate.
            CashflowSpec::StepUp(spec) => {
                (spec.initial_rate.to_f64().unwrap_or(0.0), spec.coupon_type)
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

        let schedule = crate::bond::pricing::time_basis::bond_model_schedule(self, as_of)?;
        let terms = MertonBondTerms {
            notional,
            coupon_rate,
            maturity_years: schedule.maturity_years,
            coupons: schedule
                .coupons
                .iter()
                .map(|&(t, accrual, _)| (t, accrual))
                .collect(),
            accrued: schedule.accrued,
        };

        // If the config uses the default schedule, derive from bond's CouponType
        let effective_config;
        let config_ref = if matches!(config.pik_schedule, PikSchedule::Uniform(PikMode::Cash)) {
            let bond_mode = match coupon_type {
                CouponType::Cash => PikMode::Cash,
                CouponType::Pik => PikMode::Pik,
                CouponType::Split {
                    cash_fraction,
                    pik_fraction,
                } => PikMode::Split {
                    cash_fraction: cash_fraction.to_f64().unwrap_or(1.0),
                    pik_fraction: pik_fraction.to_f64().unwrap_or(0.0),
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

        MertonMcEngine::price_terms(&terms, config_ref, run, discount_rate)
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for Bond {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(Some(self.notional))
    }

    fn raw_cashflow_schedule(
        &self,
        curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let schedule = if let Some(ref custom) = self.custom_cashflows {
            custom.clone()
        } else {
            self.full_cashflow_schedule(curves)?
        };

        let representation = if self.has_floating_coupons() {
            CashflowRepresentation::Projected
        } else {
            CashflowRepresentation::Contractual
        };

        Ok(schedule.with_representation(representation))
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
            spec.rate_spec.forward_curve_id.as_str(),
        );

        let deps =
            finstack_quant_valuations_core::instruments::Instrument::market_dependencies(&bond)
                .expect("dependencies");
        assert_eq!(deps.series_ids, vec![expected]);
    }
}

#[cfg(test)]
mod metrics_equivalent_tests {
    use super::*;
    use finstack_quant_cashflows::builder::{AmortizationSpec, CouponType};
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_valuations_core::instruments::Instrument;

    fn pik_step_up_spec() -> CashflowSpec {
        let mut spec =
            CashflowSpec::step_up(0.05, vec![], Tenor::semi_annual(), DayCount::Thirty360)
                .expect("finite step-up coupon");
        if let CashflowSpec::StepUp(ref mut s) = spec {
            s.coupon_type = CouponType::Pik;
        }
        spec
    }

    fn pik_floating_spec() -> CashflowSpec {
        let bond = Bond::example_floating().expect("floating bond example");
        let mut spec = bond.cashflow_spec;
        if let CashflowSpec::Floating(ref mut s) = spec {
            s.coupon_type = CouponType::Pik;
        }
        spec
    }

    fn coupon_type_of(spec: &CashflowSpec) -> CouponType {
        match spec {
            CashflowSpec::Fixed(s) => s.coupon_type,
            CashflowSpec::Floating(s) => s.coupon_type,
            CashflowSpec::StepUp(s) => s.coupon_type,
            CashflowSpec::Amortizing { base, .. } => coupon_type_of(base),
        }
    }

    fn metrics_equivalent_coupon_type(spec: CashflowSpec) -> CouponType {
        let mut bond = Bond::example().expect("bond example");
        bond.cashflow_spec = spec;
        assert_eq!(coupon_type_of(&bond.cashflow_spec), CouponType::Pik);
        let equivalent = bond.metrics_equivalent().expect("bond equivalent");
        let equivalent = equivalent
            .as_any()
            .downcast_ref::<Bond>()
            .expect("metrics equivalent of a bond is a bond");
        coupon_type_of(&equivalent.cashflow_spec)
    }

    #[test]
    fn metrics_equivalent_preserves_all_other_bond_state() {
        use finstack_quant_models::credit::{AssetDynamics, MertonBarrierType, MertonModel};
        use finstack_quant_valuations_core::instruments::MertonMcConfig;

        let mut bond = Bond::example().expect("bond example");
        bond.cashflow_spec = pik_step_up_spec();
        let merton = MertonModel::new_with_dynamics(
            200.0,
            0.25,
            100.0,
            0.04,
            0.0,
            MertonBarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
        .expect("Merton model");
        bond.instrument_pricing_overrides = bond
            .instrument_pricing_overrides
            .with_merton_mc_config(MertonMcConfig::new(merton, 0.4).expect("Merton config"))
            .with_implied_volatility(0.3);
        bond.instrument_pricing_overrides.model_config.tree_steps = Some(71);
        bond.metric_pricing_overrides.theta_period = Some(Tenor::semi_annual());
        bond.scenario_pricing_overrides.scenario_price_shock_decimal = Some(0.1);
        let original = serde_json::to_value(&bond).expect("original bond");
        let equivalent = bond.metrics_equivalent().expect("bond equivalent");
        let equivalent = equivalent.as_any().downcast_ref::<Bond>().expect("bond");
        assert_eq!(coupon_type_of(&equivalent.cashflow_spec), CouponType::Cash);
        assert!(equivalent
            .instrument_pricing_overrides
            .model_config
            .merton_mc_config
            .is_none());
        assert_eq!(
            serde_json::to_value(&bond).expect("original unchanged"),
            original
        );
        let mut expected = original;
        let actual = serde_json::to_value(equivalent).expect("equivalent bond");
        expected["cashflow_spec"] = actual["cashflow_spec"].clone();
        expected["instrument_pricing_overrides"]["model_config"]
            .as_object_mut()
            .expect("model config")
            .remove("merton_mc_config");
        assert_eq!(actual, expected);
    }

    #[test]
    fn metrics_equivalent_cashifies_pik_step_up() {
        assert_eq!(
            metrics_equivalent_coupon_type(pik_step_up_spec()),
            CouponType::Cash
        );
    }

    #[test]
    fn metrics_equivalent_cashifies_pik_floating() {
        assert_eq!(
            metrics_equivalent_coupon_type(pik_floating_spec()),
            CouponType::Cash
        );
    }

    #[test]
    fn metrics_equivalent_cashifies_amortizing_pik_step_up_and_floating() {
        for base in [pik_step_up_spec(), pik_floating_spec()] {
            let spec = CashflowSpec::amortizing(base, AmortizationSpec::None);
            assert_eq!(metrics_equivalent_coupon_type(spec), CouponType::Cash);
        }
    }
}
