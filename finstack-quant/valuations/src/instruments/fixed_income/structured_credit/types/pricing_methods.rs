use super::{DealType, StructuredCredit, TrancheCashflows, TrancheValuation};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::structured_credit::metrics::{
    calculate_tranche_cs01, calculate_tranche_duration, calculate_tranche_z_spread,
};
use crate::instruments::fixed_income::structured_credit::pricing::generate_tranche_cashflows;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    abs_auto_correlation_structure, clo_correlation_structure, cmbs_correlation_structure,
    rmbs_correlation_structure,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::{
    PricingMode, StochasticPricer, StochasticPricerConfig, StochasticPricingResult,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
use crate::metrics::{MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::correlation::RecoverySpec as StochasticRecoverySpec;
use finstack_quant_models::credit::pool::{
    CorrelationStructure, StochasticDefaultSpec, StochasticPrepaySpec,
};

impl StructuredCredit {
    /// Calculate monthly prepayment probability at the supplied collateral age.
    ///
    /// # Arguments
    ///
    /// * `pay_date` - Contractual payment date; rate curves are currently indexed
    ///   by collateral age rather than calendar date.
    /// * `seasoning_months` - Collateral age in months, including age at closing.
    pub fn calculate_prepayment_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        let _ = pay_date;
        self.resolved_credit_model()?
            .prepayment_spec
            .smm(seasoning_months)
    }

    /// Calculate monthly default probability at the supplied collateral age.
    ///
    /// # Arguments
    ///
    /// * `pay_date` - Contractual payment date; rate curves are currently indexed
    ///   by collateral age rather than calendar date.
    /// * `seasoning_months` - Collateral age in months, including age at closing.
    pub fn calculate_default_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        let _ = pay_date;
        self.resolved_credit_model()?
            .default_spec
            .mdr(seasoning_months)
    }

    /// Advanced stochastic pricing that defaults to Monte Carlo.
    ///
    /// Host and registry callers should use
    /// [`Instrument::price_with_metrics`](crate::instruments::Instrument::price_with_metrics)
    /// with [`ModelKey::StructuredCreditStochastic`](crate::pricer::ModelKey::StructuredCreditStochastic)
    /// (or `price_instrument` on the same model key). This method remains for
    /// tests and direct Rust callers that want a `StochasticPricingResult`
    /// without going through the registry envelope.
    pub fn price_stochastic(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let lifecycle =
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(self)?;
        let effective_as_of = lifecycle.effective_as_of(context, as_of);
        let result = self.price_stochastic_base(context, effective_as_of)?;
        self.apply_stochastic_price_scenario(result)
    }

    fn default_stochastic_pricing_mode(&self) -> PricingMode {
        let num_paths = self
            .instrument_pricing_overrides
            .model_config
            .mc_paths
            .unwrap_or(10_000);
        PricingMode::MonteCarlo {
            num_paths,
            antithetic: num_paths > 1,
        }
    }

    pub(crate) fn price_stochastic_base(
        &self,
        context: &MarketContext,
        effective_as_of: Date,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        self.price_stochastic_base_with_mode(
            context,
            effective_as_of,
            self.default_stochastic_pricing_mode(),
        )
    }

    /// Advanced stochastic pricing with an explicit mode (tree, Monte Carlo, or hybrid).
    ///
    /// Prefer the registry `StructuredCreditStochastic` model key for host
    /// pricing. Use this method only when a caller needs to force a
    /// [`PricingMode`] without going through `price_with_metrics`.
    pub fn price_stochastic_with_mode(
        &self,
        context: &MarketContext,
        as_of: Date,
        pricing_mode: PricingMode,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let lifecycle =
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(self)?;
        let effective_as_of = lifecycle.effective_as_of(context, as_of);
        let result =
            self.price_stochastic_base_with_mode(context, effective_as_of, pricing_mode)?;
        self.apply_stochastic_price_scenario(result)
    }

    fn price_stochastic_base_with_mode(
        &self,
        context: &MarketContext,
        effective_as_of: Date,
        pricing_mode: PricingMode,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let resolved = self.resolved_for_pricing()?;
        let mut tree_config = resolved.build_scenario_tree_config(effective_as_of)?;
        if let Some(tree_steps) = self.instrument_pricing_overrides.model_config.tree_steps {
            tree_config.num_periods = tree_steps.max(1);
        }
        let discount_curve = context.get_discount(self.discount_curve_id.as_str())?;
        let mut config = StochasticPricerConfig::new(effective_as_of, discount_curve, tree_config)
            .with_pricing_mode(pricing_mode);
        if let Some(granularity) = self
            .instrument_pricing_overrides
            .model_config
            .structured_credit_pool_granularity
        {
            config = config.with_pool_granularity(granularity);
        }
        resolved.run_stochastic_pricer(config, context)
    }

    fn apply_stochastic_price_scenario(
        &self,
        mut result: StochasticPricingResult,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let Some(shock) = self.scenario_pricing_overrides.scenario_price_shock_pct else {
            return Ok(result);
        };
        let factor = 1.0 + shock;
        result.npv = Money::new(result.npv.amount() * factor, result.npv.currency())?;
        result.clean_price *= factor;
        result.dirty_price *= factor;
        result.pv_std_error *= factor.abs();
        let lo = result.pv_confidence_interval.0 * factor;
        let hi = result.pv_confidence_interval.1 * factor;
        result.pv_confidence_interval = (lo.min(hi), lo.max(hi));
        for tranche in &mut result.tranche_results {
            tranche.npv = Money::new(tranche.npv.amount() * factor, tranche.npv.currency())?;
        }
        Ok(result)
    }

    fn run_stochastic_pricer(
        &self,
        config: StochasticPricerConfig,
        context: &MarketContext,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        let notional = self.pool.total_balance()?.amount();

        if notional.abs() <= f64::EPSILON {
            return Err(finstack_quant_core::Error::Validation(
                "structured-credit stochastic pricing requires positive pool notional".to_string(),
            ));
        }

        self.validate_stochastic_tranches()?;

        let pricer = StochasticPricer::new(config);
        let result = pricer.price(self, context)?;

        if result.tranche_results.len() != self.tranches.tranches.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "stochastic pricing produced {} tranche results for {} input tranches",
                result.tranche_results.len(),
                self.tranches.tranches.len()
            )));
        }

        Ok(result)
    }

    pub(crate) fn build_scenario_tree_config(
        &self,
        as_of: Date,
    ) -> finstack_quant_core::Result<ScenarioTreeConfig> {
        let months_to_maturity = as_of.months_until(self.maturity).max(1) as usize;
        let mut tree_config = ScenarioTreeConfig::new(months_to_maturity, 3);

        let (prepay, default, _correlation) = self.effective_stochastic_specs()?;
        tree_config.prepay_spec = prepay;
        tree_config.default_spec = default;
        tree_config.recovery_spec = match &self.credit_model.stochastic_recovery_spec {
            Some(spec) => spec.clone(),
            None => {
                StochasticRecoverySpec::constant(self.resolved_credit_model()?.recovery_spec.rate)
                    .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))?
            }
        };
        // Explicit deal correlation overrides the copula spec's scalar.
        // The engine consumes only this scalar override; per-pair
        // Matrix/Sectored correlation in the copula is a deferred feature.
        tree_config.asset_correlation_override = self
            .credit_model
            .correlation_structure
            .as_ref()
            .map(CorrelationStructure::asset_correlation);
        // The intensity model's κ drives the systematic OU factor in
        // `dX = κ(θ − X)dt + σdW`, making
        // `λ = λ₀ exp(-βσX - 0.5β²σ²)` an exponential-OU intensity
        // (Duffie-Singleton 1999; Lando 1998). κ = 0 intentionally retains the
        // horizon-persistent factor configured by the base tree.
        if let StochasticDefaultSpec::IntensityProcess { mean_reversion, .. } =
            &tree_config.default_spec
        {
            if *mean_reversion > 0.0 {
                tree_config.factor_spec =
                    finstack_quant_models::correlation::LatentFactorSpec::SingleFactor {
                        volatility: 1.0,
                        mean_reversion: *mean_reversion,
                    };
            }
        }

        if !self.market_conditions.refi_rate.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "refinancing rate must be finite".into(),
            ));
        }
        tree_config.market_refi_rate = self.market_conditions.refi_rate;
        tree_config.initial_balance = self.pool.total_balance()?.amount().max(1.0);
        tree_config.initial_seasoning = self
            .pool
            .weighted_average_seasoning(as_of, self.closing_date);
        tree_config.seed = self.derive_seed(as_of);
        Ok(tree_config)
    }

    fn validate_stochastic_tranches(&self) -> finstack_quant_core::Result<()> {
        let mut previous_detachment = 0.0;
        const EPS: f64 = 1e-9;

        for (idx, tranche) in self.tranches.tranches.iter().enumerate() {
            let attachment = tranche.attachment_pct();
            let detachment = tranche.detachment_pct();
            if !attachment.is_finite() || !detachment.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' has non-finite attachment/detachment",
                    tranche.id
                )));
            }
            if !(0.0..=100.0).contains(&attachment)
                || !(0.0..=100.0).contains(&detachment)
                || attachment >= detachment
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' has invalid attachment/detachment [{attachment}, {detachment}]",
                    tranche.id
                )));
            }
            if idx == 0 && attachment.abs() > EPS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' starts at {attachment}; first attachment must be 0",
                    tranche.id
                )));
            }
            if idx > 0 && (attachment - previous_detachment).abs() > EPS {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "structured-credit tranche '{}' creates a gap/overlap: attachment {attachment} after previous detachment {previous_detachment}",
                    tranche.id
                )));
            }
            previous_detachment = detachment;
        }

        if !self.tranches.tranches.is_empty() && (previous_detachment - 100.0).abs() > EPS {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit final tranche detachment must be exactly 100, got {previous_detachment}"
            )));
        }

        Ok(())
    }

    fn derive_seed(&self, as_of: Date) -> u64 {
        // Use a simple deterministic mixing of the ID and date bytes to ensure reproducibility
        // across different Rust versions/platforms (unlike DefaultHasher).
        let mut seed: u64 = 0xcbf29ce484222325; // FNV offset basis

        for byte in self.id.as_bytes() {
            seed ^= *byte as u64;
            seed = seed.wrapping_mul(0x100000001b3); // FNV prime
        }

        // Mix in date
        let date_val = as_of.to_julian_day() as u64;
        seed ^= date_val;
        seed = seed.wrapping_mul(0x100000001b3);

        seed
    }

    fn effective_stochastic_specs(
        &self,
    ) -> Result<(
        StochasticPrepaySpec,
        StochasticDefaultSpec,
        CorrelationStructure,
    )> {
        let model = self.resolved_credit_model()?;
        // The stochastic engines size defaults from a per-month rate on the
        // surviving balance; curves stated against the original balance and
        // severities by month of default are deterministic-only.
        if matches!(
            model.default_spec.curve,
            Some(
                crate::cashflow::builder::DefaultCurve::CumulativeLoss { .. }
                    | crate::cashflow::builder::DefaultCurve::Timing { .. }
            )
        ) {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic pricing supports constant, SDA and vector default curves; \
                 cumulative-loss and timing curves are deterministic-only"
                    .to_string(),
            ));
        }
        if model.recovery_spec.severity_vector.is_some() {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic pricing uses a flat recovery rate; \
                 recovery_spec.severity_vector is deterministic-only"
                    .to_string(),
            ));
        }
        if self
            .pool
            .assets
            .iter()
            .any(|asset| asset.liquidation.is_some())
        {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic pricing does not model NPL resolution timelines; \
                 PoolAsset.liquidation is deterministic-only"
                    .to_string(),
            ));
        }
        let prepay = model
            .stochastic_prepay_spec
            .unwrap_or_else(|| StochasticPrepaySpec::deterministic(model.prepayment_spec));
        let default = model
            .stochastic_default_spec
            .unwrap_or_else(|| StochasticDefaultSpec::deterministic(model.default_spec));

        let correlation = match &self.credit_model.correlation_structure {
            Some(correlation) => correlation.clone(),
            None => match self.deal_type {
                DealType::Rmbs => rmbs_correlation_structure()?,
                DealType::Clo | DealType::Cbo => clo_correlation_structure()?,
                DealType::Cmbs => cmbs_correlation_structure()?,
                _ => abs_auto_correlation_structure()?,
            },
        };

        Ok((prepay, default, correlation))
    }

    /// Effective asset correlation of the deal's correlation structure (the
    /// explicit structure, else the deal-type default), as a decimal.
    pub(crate) fn effective_asset_correlation(&self) -> Result<f64> {
        let (_, _, correlation) = self.effective_stochastic_specs()?;
        Ok(correlation.asset_correlation())
    }

    /// Present value of one named tranche.
    ///
    /// Deal-level host pricing uses
    /// [`Instrument::price_with_metrics`](crate::instruments::Instrument::price_with_metrics)
    /// (deterministic) or `ModelKey::StructuredCreditStochastic` (stochastic).
    /// This method is the advanced per-tranche convenience used by tests and
    /// scenario metrics; it does not run the registry pipeline.
    pub fn value_tranche(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let cashflows = generate_tranche_cashflows(self, tranche_id, context, as_of)?;
        let effective_as_of = self.resolve_pricing_as_of(context, as_of);
        self.value_tranche_cashflows(&cashflows, context, effective_as_of)
    }

    fn value_tranche_cashflows(
        &self,
        cashflows: &TrancheCashflows,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = context.get_discount(&self.discount_curve_id)?;

        let mut pv = Money::from((0_i64, self.pool.get_base_currency()));
        for (date, amount) in &cashflows.cashflows {
            if *date > as_of {
                let df = disc.df_between_dates(as_of, *date)?;
                let flow_pv = Money::new(amount.amount() * df, amount.currency())?;
                pv = pv.checked_add(flow_pv)?;
            }
        }

        crate::instruments::common_impl::helpers::apply_scenario_value(self, pv)
    }

    /// Value a note and calculate its mandatory price, yield, life and spread metrics.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Exact note identifier within this deal's capital structure.
    /// * `context` - Discount curves, forward curves and fixings used by projection.
    /// * `as_of` - Valuation date of the current collateral and note balances.
    /// * `metrics` - Additional registered metrics, computed for THIS note:
    ///   every registry reprice (DV01, theta, the Default01 / Prepayment01 /
    ///   Recovery01 / Severity01 bumps) values the tranche through
    ///   [`super::StructuredCreditTranche`], so the senior and the equity carry
    ///   their own sensitivities that sum to the deal's. Mandatory result
    ///   fields are calculated even when this list is empty; calculation
    ///   failures propagate.
    ///
    /// Prices and yields use buyer settlement entitlement and are quoted per
    /// the tranche's CURRENT balance (the factor-adjusted secondary-market
    /// basis; `factor` reports current over original face). PV remains at
    /// valuation; spread results use an external clean/dirty quote when
    /// supplied, otherwise the model's dirty settlement value. Z-spread is
    /// returned in basis points.
    pub fn value_tranche_with_metrics(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> finstack_quant_core::Result<TrancheValuation> {
        let lifecycle =
            crate::instruments::common_impl::helpers::ValidatedPricingLifecycle::new(self)?;
        let effective_as_of = lifecycle.effective_as_of(context, as_of);
        let tranche = self
            .tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == tranche_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{tranche_id}"),
                })
            })?;
        let cashflow_result = generate_tranche_cashflows(self, tranche_id, context, as_of)?;
        let pv = self.value_tranche_cashflows(&cashflow_result, context, effective_as_of)?;
        // Prices are per CURRENT face (the factor-adjusted quote basis).
        let quote = super::super::metrics::quote::SettlementQuote::for_tranche(
            self,
            effective_as_of,
            tranche.current_balance.amount(),
            &cashflow_result,
        )?;
        let factor = if tranche.original_balance.amount() > 0.0 {
            tranche.current_balance.amount() / tranche.original_balance.amount()
        } else {
            0.0
        };
        let disc = context.get_discount(&self.discount_curve_id)?;
        let model_dirty = quote.model_dirty(&cashflow_result.cashflows, &disc)?;
        // An impaired note — no current face or no positive settlement value
        // (fully written down in the projection) — prices at zero and carries
        // no yield or spread instead of failing the solves.
        let impaired = tranche.current_balance.amount() <= 0.0 || model_dirty <= 0.0;
        let target = if impaired {
            0.0
        } else {
            let target = quote.external_target(self)?.unwrap_or(model_dirty);
            quote.dirty_target(target)?
        };

        // The priced instrument is the NOTE: every registry reprice (DV01,
        // theta, the *01 bumps) then values this tranche, not the deal.
        let mut metric_context = MetricContext::new(
            std::sync::Arc::new(super::StructuredCreditTranche::new(
                self.clone(),
                tranche_id,
            )),
            std::sync::Arc::new(context.clone()),
            effective_as_of,
            pv,
            MetricContext::default_config(),
        );
        metric_context.cashflows = Some(cashflow_result.cashflows.clone());
        metric_context.tagged_cashflows = Some(cashflow_result.detailed_flows.clone());
        metric_context.detailed_tranche_cashflows = Some(cashflow_result.clone());
        metric_context.discount_curve_id = Some(self.discount_curve_id.to_owned());
        metric_context.notional = Some(tranche.current_balance);
        let mut requested = metrics.to_vec();
        for required in [
            MetricId::Accrued,
            MetricId::CleanPrice,
            MetricId::DirtyPrice,
            MetricId::Ytm,
            MetricId::WAL,
        ] {
            if !requested.contains(&required) {
                requested.push(required);
            }
        }
        if impaired {
            // No yield on a worthless note: the solver has no target.
            requested.retain(|id| *id != MetricId::Ytm);
        }
        let computed_metrics =
            crate::metrics::standard_registry().compute(&requested, &mut metric_context)?;
        let accrued = Money::new(computed_metrics[&MetricId::Accrued], pv.currency())?;
        let dirty_price = computed_metrics[&MetricId::DirtyPrice];
        let clean_price = computed_metrics[&MetricId::CleanPrice];
        let wal = computed_metrics[&MetricId::WAL];
        let ytm = computed_metrics.get(&MetricId::Ytm).copied();
        let modified_duration = match computed_metrics.get(&MetricId::DurationMod) {
            Some(value) => *value,
            None if impaired => 0.0,
            None => calculate_tranche_duration(
                &cashflow_result.cashflows,
                &disc,
                quote.settlement,
                Money::new(model_dirty, pv.currency())?,
            )?,
        };
        let z_spread = match computed_metrics.get(&MetricId::ZSpread) {
            Some(decimal) => decimal * 10_000.0,
            None if impaired => 0.0,
            None => calculate_tranche_z_spread(
                &cashflow_result.cashflows,
                &disc,
                Money::new(target, pv.currency())?,
                quote.settlement,
            )?,
        };
        let cs01 = match computed_metrics.get(&MetricId::Cs01) {
            Some(value) => *value,
            None if impaired => 0.0,
            None => calculate_tranche_cs01(
                &cashflow_result.cashflows,
                &disc,
                z_spread * 1e-4,
                quote.settlement,
            )?,
        };

        let final_metrics: std::collections::BTreeMap<MetricId, f64> =
            computed_metrics.into_iter().collect();

        Ok(TrancheValuation {
            tranche_id: tranche_id.to_string(),
            pv,
            clean_price,
            dirty_price,
            factor,
            accrued,
            wal,
            modified_duration,
            z_spread_bp: z_spread,
            cs01,
            ytm,
            metrics: final_metrics,
        })
    }
}

#[cfg(test)]
mod production_structured_assumptions {
    use super::*;
    use time::macros::date;

    #[test]
    fn production_structured_stochastic_uses_behavior_overrides() {
        let mut deal = StructuredCredit::example();
        deal.behavior_overrides.cpr_annual = Some(0.23);
        deal.behavior_overrides.cdr_annual = Some(0.12);
        deal.behavior_overrides.recovery_rate = Some(0.71);
        let config = deal
            .build_scenario_tree_config(deal.closing_date)
            .expect("config");
        let StochasticPrepaySpec::Deterministic(prepay) = config.prepay_spec else {
            panic!("deterministic default prepayment");
        };
        let StochasticDefaultSpec::Deterministic(default) = config.default_spec else {
            panic!("deterministic default model");
        };
        assert!(
            (prepay.smm(12).expect("SMM")
                - deal
                    .calculate_prepayment_rate(deal.closing_date, 12)
                    .expect("resolved SMM"))
            .abs()
                < 1e-14
        );
        assert!(
            (default.mdr(12).expect("MDR")
                - deal
                    .calculate_default_rate(deal.closing_date, 12)
                    .expect("resolved MDR"))
            .abs()
                < 1e-14
        );
        assert!(
            matches!(config.recovery_spec, StochasticRecoverySpec::Constant { rate } if (rate - 0.71).abs() < 1e-14)
        );
    }

    #[test]
    fn production_structured_stochastic_includes_collateral_seasoning() {
        let mut deal = StructuredCredit::example();
        deal.pool.assets[0].acquisition_date = Some(date!(2022 - 01 - 01));
        let config = deal
            .build_scenario_tree_config(date!(2024 - 07 - 01))
            .expect("config");
        assert_eq!(config.initial_seasoning, 30);
    }
}
