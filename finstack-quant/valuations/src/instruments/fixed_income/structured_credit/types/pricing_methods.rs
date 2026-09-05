use super::{DealType, StructuredCredit, TrancheCashflows, TrancheValuation};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use crate::instruments::fixed_income::structured_credit::metrics::{
    calculate_tranche_cs01, calculate_tranche_duration, calculate_tranche_wal,
    calculate_tranche_z_spread,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::calibrations::{
    abs_auto_correlation_structure, clo_correlation_structure, cmbs_correlation_structure,
    rmbs_correlation_structure,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::{
    PricingMode, StochasticPricer, StochasticPricerConfig, StochasticPricingResult,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;
use crate::instruments::fixed_income::structured_credit::utils::rates::{
    clamped_cdr_to_mdr, clamped_cpr_to_smm,
};
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
    /// Calculate prepayment rate (SMM) for a given period.
    pub fn calculate_prepayment_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(override_rate) = self.prepayment_rate_override(pay_date, seasoning_months) {
            return Ok(override_rate);
        }
        Ok(self
            .credit_model
            .prepayment_spec
            .smm(seasoning_months)?
            .max(0.0))
    }

    /// Calculate default rate (MDR) for a given period.
    pub fn calculate_default_rate(
        &self,
        pay_date: Date,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        if let Some(override_rate) = self.default_rate_override(pay_date, seasoning_months) {
            return Ok(override_rate);
        }
        Ok(self
            .credit_model
            .default_spec
            .mdr(seasoning_months)?
            .max(0.0))
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
        let mut tree_config = self.build_scenario_tree_config(effective_as_of)?;
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
        self.run_stochastic_pricer(config, context)
    }

    fn apply_stochastic_price_scenario(
        &self,
        mut result: StochasticPricingResult,
    ) -> finstack_quant_core::Result<StochasticPricingResult> {
        Ok({
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
            result
        })
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
        tree_config.recovery_spec =
            StochasticRecoverySpec::constant(self.credit_model.recovery_spec.rate)
                .map_err(|err| finstack_quant_core::Error::Validation(err.to_string()))?;
        // Explicit deal correlation overrides the copula spec's scalar.
        // The engine consumes only this scalar override; per-pair
        // Matrix/Sectored correlation in the copula is a deferred feature.
        tree_config.asset_correlation_override = self
            .credit_model
            .correlation_structure
            .as_ref()
            .map(CorrelationStructure::asset_correlation);
        // Market refi rate for Richard-Roll; 4.5% fallback matches RMBS defaults.
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

        tree_config.market_refi_rate = if self.market_conditions.refi_rate > 0.0 {
            self.market_conditions.refi_rate
        } else {
            0.045
        };
        tree_config.initial_balance = self.pool.total_balance()?.amount().max(1.0);
        let seasoning = if as_of > self.closing_date {
            self.closing_date.months_until(as_of)
        } else {
            0
        };
        tree_config.initial_seasoning = seasoning;
        tree_config.seed = self.derive_seed(as_of);
        Ok(tree_config)
    }

    fn validate_stochastic_tranches(&self) -> finstack_quant_core::Result<()> {
        let mut previous_detachment = 0.0;
        const EPS: f64 = 1e-9;

        for (idx, tranche) in self.tranches.tranches.iter().enumerate() {
            let attachment = tranche.attachment_point;
            let detachment = tranche.detachment_point;
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
        let prepay = self
            .credit_model
            .stochastic_prepay_spec
            .clone()
            .unwrap_or_else(|| {
                StochasticPrepaySpec::deterministic(self.credit_model.prepayment_spec.clone())
            });

        let default = self
            .credit_model
            .stochastic_default_spec
            .clone()
            .unwrap_or_else(|| {
                StochasticDefaultSpec::deterministic(self.credit_model.default_spec.clone())
            });

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

    fn prepayment_rate_override(&self, _pay_date: Date, seasoning: u32) -> Option<f64> {
        if let Some(abs_speed) = self.behavior_overrides.abs_speed {
            return Some(abs_speed);
        }

        if let Some(cpr) = self.behavior_overrides.cpr_annual {
            return Some(clamped_cpr_to_smm(cpr));
        }

        if let Some(psa_mult) = self.behavior_overrides.psa_speed_multiplier {
            let psa_curve = embedded_registry_or_panic().psa_curve();
            let base_cpr = if seasoning <= psa_curve.ramp_months {
                (seasoning as f64 / psa_curve.ramp_months as f64) * psa_curve.terminal_cpr
            } else {
                psa_curve.terminal_cpr
            };
            let cpr = base_cpr * psa_mult;
            return Some(clamped_cpr_to_smm(cpr));
        }

        None
    }

    fn default_rate_override(&self, _pay_date: Date, seasoning: u32) -> Option<f64> {
        if let Some(cdr) = self.behavior_overrides.cdr_annual {
            return Some(clamped_cdr_to_mdr(cdr));
        }

        if let Some(sda_mult) = self.behavior_overrides.sda_speed_multiplier {
            // Canonical PSA SDA shape (ramp / plateau / decline / terminal)
            // lives on `SdaCurveDefaults::cdr_at`.
            let cdr = embedded_registry_or_panic().sda_curve().cdr_at(seasoning) * sda_mult;
            return Some(clamped_cdr_to_mdr(cdr));
        }

        None
    }

    /// Generate cashflows for a specific tranche after waterfall allocation.
    pub fn get_tranche_cashflows(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<TrancheCashflows> {
        crate::instruments::fixed_income::structured_credit::pricing::generate_tranche_cashflows(
            self, tranche_id, context, as_of,
        )
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
        let cashflows = self.get_tranche_cashflows(tranche_id, context, as_of)?;
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

    /// Get full valuation with metrics for a specific tranche.
    pub fn value_tranche_with_metrics(
        &self,
        tranche_id: &str,
        context: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> finstack_quant_core::Result<TrancheValuation> {
        let cashflow_result = self.get_tranche_cashflows(tranche_id, context, as_of)?;
        let effective_as_of = self.resolve_pricing_as_of(context, as_of);
        let pv = self.value_tranche_cashflows(&cashflow_result, context, effective_as_of)?;

        let mut metric_context = crate::metrics::MetricContext::new(
            std::sync::Arc::new(self.clone())
                as std::sync::Arc<dyn crate::instruments::common_impl::traits::Instrument>,
            std::sync::Arc::new(context.clone()),
            effective_as_of,
            pv,
            MetricContext::default_config(),
        );
        metric_context.cashflows = Some(cashflow_result.cashflows.clone());
        metric_context.tagged_cashflows = Some(cashflow_result.detailed_flows.clone());
        metric_context.detailed_tranche_cashflows = Some(cashflow_result.clone());
        metric_context.discount_curve_id = Some(self.discount_curve_id.to_owned());

        let registry = crate::metrics::standard_registry();
        let computed_metrics = registry.compute(metrics, &mut metric_context)?;

        let tranche = self
            .tranches
            .tranches
            .iter()
            .find(|t| t.id.as_str() == tranche_id)
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;

        let notional = tranche.original_balance.amount();

        let dirty_price = if notional > 0.0 {
            (pv.amount() / notional) * 100.0
        } else {
            0.0
        };

        let accrued_value = computed_metrics
            .get(&MetricId::Accrued)
            .copied()
            .unwrap_or(0.0);
        let accrued = Money::new(accrued_value, pv.currency())?;

        let clean_price = if notional > 0.0 {
            dirty_price - (accrued.amount() / notional) * 100.0
        } else {
            dirty_price
        };

        let wal = match computed_metrics.get(&MetricId::WAL) {
            Some(v) => *v,
            None => calculate_tranche_wal(&cashflow_result, effective_as_of)?,
        };

        let disc = context.get_discount(&self.discount_curve_id)?;
        let modified_duration = computed_metrics
            .get(&MetricId::DurationMod)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_duration(&cashflow_result.cashflows, &disc, effective_as_of, pv)
                    .unwrap_or(0.0)
            });

        let z_spread = computed_metrics
            .get(&MetricId::ZSpread)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_z_spread(&cashflow_result.cashflows, &disc, pv, effective_as_of)
                    .unwrap_or(0.0)
            });

        let z_spread_decimal = z_spread / 10_000.0;
        let cs01 = computed_metrics
            .get(&MetricId::Cs01)
            .copied()
            .unwrap_or_else(|| {
                calculate_tranche_cs01(
                    &cashflow_result.cashflows,
                    &disc,
                    z_spread_decimal,
                    effective_as_of,
                )
                .unwrap_or(0.0)
            });

        let ytm = computed_metrics
            .get(&MetricId::Ytm)
            .copied()
            .unwrap_or(0.05);

        let final_metrics: std::collections::BTreeMap<MetricId, f64> =
            computed_metrics.into_iter().collect();

        Ok(TrancheValuation {
            tranche_id: tranche_id.to_string(),
            pv,
            clean_price,
            dirty_price,
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
