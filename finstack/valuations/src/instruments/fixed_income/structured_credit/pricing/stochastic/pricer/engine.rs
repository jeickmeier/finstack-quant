//! Stochastic structured-credit scenario waterfall pricing engine.

use super::config::{PricingMode, StochasticPricerConfig};
use super::result::{StochasticPricingResult, TranchePricingResult};
use crate::correlation::{FactorSpec, RecoverySpec};
use crate::instruments::fixed_income::structured_credit::pricing::simulation_engine::{
    run_simulation_with_source, PeriodPoolShock, StochasticPathFlowSource,
};
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::default::MacroCreditFactors;
use crate::instruments::fixed_income::structured_credit::pricing::{
    StochasticDefaultSpec, StochasticPrepaySpec,
};
use crate::instruments::fixed_income::structured_credit::types::{StructuredCredit, Tranche};
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::traits::RandomStream;
use finstack_core::math::stats::OnlineStats;
use finstack_core::money::Money;
use finstack_core::Result;
use rayon::prelude::*;
use std::cmp::Ordering;

/// Stochastic pricing engine for structured credit.
///
/// Each scenario path feeds period SMM/MDR/recovery assumptions into the same
/// waterfall simulation used by deterministic tranche valuation. PV is computed
/// from actual dated tranche payments, not from terminal expected loss shortcuts.
pub(crate) struct StochasticPricer {
    config: StochasticPricerConfig,
}

impl StochasticPricer {
    /// Create a new stochastic pricer.
    pub(crate) fn new(config: StochasticPricerConfig) -> Self {
        Self { config }
    }

    /// Price the full deal and all tranches through scenario-level waterfalls.
    pub(crate) fn price(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        match &self.config.pricing_mode {
            PricingMode::Tree => self.price_tree(instrument, context),
            PricingMode::MonteCarlo {
                num_paths,
                antithetic,
            } => self.price_monte_carlo(instrument, context, *num_paths, *antithetic),
            PricingMode::Hybrid {
                tree_periods,
                mc_paths,
            } => self.price_hybrid(instrument, context, *tree_periods, *mc_paths),
        }
    }

    fn price_tree(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        let terminal_paths = self
            .config
            .tree_config
            .branching
            .estimate_terminal_nodes(self.config.tree_config.num_periods);
        if terminal_paths > self.config.max_tree_paths {
            return Err(finstack_core::Error::Validation(format!(
                "structured_credit_stochastic tree requires {terminal_paths} terminal paths, \
                 above max_tree_paths={}",
                self.config.max_tree_paths
            )));
        }

        let branch_count = self
            .config
            .tree_config
            .branching
            .branches_at_node(self.branching_variance_proxy())
            .max(1);
        let path_count = terminal_paths.max(1);
        let mut collector = ScenarioCollector::new(instrument, path_count)?;
        for path_index in 0..path_count {
            let shocks = self.tree_path_shocks(instrument, path_index, path_count, branch_count)?;
            let output = self.price_path(instrument, context, shocks)?;
            collector.record_output(output);
        }
        Ok(collector.finalize(self, "Tree"))
    }

    fn price_monte_carlo(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        num_paths: usize,
        antithetic: bool,
    ) -> Result<StochasticPricingResult> {
        if num_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "Monte Carlo pricing requires at least one simulation path".to_string(),
            ));
        }

        let factor_sets = self.monte_carlo_factor_sets(instrument, num_paths, antithetic);
        let mode = if antithetic {
            format!("MonteCarlo({}, antithetic)", num_paths)
        } else {
            format!("MonteCarlo({num_paths})")
        };
        self.price_factor_sets(instrument, context, factor_sets, num_paths, &mode)
    }

    fn price_hybrid(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        tree_periods: usize,
        mc_paths: usize,
    ) -> Result<StochasticPricingResult> {
        if tree_periods == 0 {
            return Err(finstack_core::Error::Validation(
                "Hybrid pricing requires at least one tree prefix period".to_string(),
            ));
        }
        if mc_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "Hybrid pricing requires at least one Monte Carlo suffix path".to_string(),
            ));
        }

        let branch_count = self
            .config
            .tree_config
            .branching
            .branches_at_node(self.branching_variance_proxy())
            .max(1);
        let prefix_count = self
            .config
            .tree_config
            .branching
            .estimate_terminal_nodes(tree_periods)
            .max(1);
        let total_paths = prefix_count.checked_mul(mc_paths).ok_or_else(|| {
            finstack_core::Error::Validation("Hybrid pricing path count overflow".to_string())
        })?;
        if total_paths > self.config.max_tree_paths {
            return Err(finstack_core::Error::Validation(format!(
                "Hybrid pricing requires {total_paths} paths, above max_tree_paths={}",
                self.config.max_tree_paths
            )));
        }

        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        let month_count = self.month_count(instrument);
        let prefix_months = tree_periods
            .saturating_mul(months_per_period)
            .min(month_count);
        let suffix_months = month_count.saturating_sub(prefix_months);
        let has_stochastic_rates = self.has_stochastic_rates();

        let mut factor_sets = Vec::with_capacity(total_paths);
        for prefix_index in 0..prefix_count {
            let prefix =
                self.tree_path_factors(prefix_index, prefix_count, branch_count, prefix_months);
            for suffix_index in 0..mc_paths {
                let path_index = prefix_index * mc_paths + suffix_index;
                // Per-path counter-based substream: Philox(seed).substream(path_id)
                // is statistically independent for any pair of distinct path_ids,
                // so the hybrid suffix factors carry no inter-path correlation.
                let mut rng = PhiloxRng::new(self.config.seed).substream(path_index as u64);
                // Pre-size to exact total length so neither the prefix copy nor
                // the suffix push triggers a Vec re-grow. Each path needs its
                // own owned Vec because `factor_sets` is consumed by a parallel
                // iterator below.
                let mut factors = Vec::with_capacity(prefix.len() + suffix_months);
                factors.extend_from_slice(&prefix);
                for _ in 0..suffix_months {
                    factors.push(if has_stochastic_rates {
                        rng.next_std_normal()
                    } else {
                        0.0
                    });
                }
                factor_sets.push(factors);
            }
        }

        self.price_factor_sets(
            instrument,
            context,
            factor_sets,
            total_paths,
            &format!("Hybrid(tree_periods={tree_periods}, mc_paths={mc_paths})"),
        )
    }

    fn price_factor_sets(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        factor_sets: Vec<Vec<f64>>,
        num_paths: usize,
        pricing_mode: &str,
    ) -> Result<StochasticPricingResult> {
        let outputs: Vec<PathScenarioOutput> = factor_sets
            .into_par_iter()
            .map(|factors| {
                let shocks = self.path_shocks_from_factors(instrument, &factors)?;
                self.price_path(instrument, context, shocks)
            })
            .collect::<Result<Vec<_>>>()?;

        let mut collector = ScenarioCollector::new(instrument, num_paths)?;
        for output in outputs {
            collector.record_output(output);
        }

        Ok(collector.finalize(self, pricing_mode))
    }

    fn monte_carlo_factor_sets(
        &self,
        instrument: &StructuredCredit,
        num_paths: usize,
        antithetic: bool,
    ) -> Vec<Vec<f64>> {
        // One base Philox RNG seeded from config.seed.  Each (logical) path
        // index gets its own counter-based substream via `substream(path_id)`.
        //
        // For antithetic pairs path 2k+1 is the negation of path 2k.  Both
        // members of a pair share the same Philox stream (stream_id = k) so
        // the antithetic pair is perfectly correlated by construction; all
        // pairs are independent of one another because they have distinct
        // stream IDs.
        let base_rng = PhiloxRng::new(self.config.seed);
        let mut factor_sets = Vec::with_capacity(num_paths);

        let mut path_index = 0usize;
        while path_index < num_paths {
            if antithetic && path_index + 1 < num_paths {
                // Stream ID is the pair index (path_index / 2) so each pair
                // draws from a stream that is independent of all other pairs.
                let stream_id = (path_index / 2) as u64;
                let mut rng = base_rng.substream(stream_id);
                let factors = self.random_factors(instrument, &mut rng);
                let antithetic_factors = factors.iter().map(|z| -z).collect();
                factor_sets.push(factors);
                factor_sets.push(antithetic_factors);
                path_index += 2;
            } else {
                // Non-antithetic path: stream_id equals the path index so
                // every path is independent regardless of execution order.
                let mut rng = base_rng.substream(path_index as u64);
                let factors = self.random_factors(instrument, &mut rng);
                factor_sets.push(factors);
                path_index += 1;
            }
        }

        factor_sets
    }

    fn price_path(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        shocks: Vec<PeriodPoolShock>,
    ) -> Result<PathScenarioOutput> {
        let mut source = StochasticPathFlowSource::new(shocks);
        let path_results = run_simulation_with_source(
            instrument,
            context,
            self.config.valuation_date,
            &mut source,
        )?;

        let mut deal_pv = 0.0;
        let mut deal_loss = 0.0;
        let mut tranches = Vec::with_capacity(instrument.tranches.tranches.len());
        for (idx, tranche) in instrument.tranches.tranches.iter().enumerate() {
            let tranche_result = path_results.get(tranche.id.as_str()).ok_or_else(|| {
                finstack_core::Error::Validation(format!(
                    "stochastic waterfall omitted tranche result '{}'",
                    tranche.id
                ))
            })?;
            let metrics = PathTrancheMetrics::from_cashflows(
                tranche_result,
                self.config.valuation_date,
                &self.config.discount_curve,
            )?;
            deal_pv += metrics.pv;
            deal_loss += metrics.loss;
            tranches.push((idx, metrics));
        }
        Ok(PathScenarioOutput {
            deal_pv,
            deal_loss,
            tranches,
        })
    }

    fn random_factors(&self, instrument: &StructuredCredit, rng: &mut PhiloxRng) -> Vec<f64> {
        let month_count = self.month_count(instrument);
        if !self.has_stochastic_rates() {
            return vec![0.0; month_count];
        }
        (0..month_count).map(|_| rng.next_std_normal()).collect()
    }

    fn tree_path_shocks(
        &self,
        instrument: &StructuredCredit,
        path_index: usize,
        path_count: usize,
        branch_count: usize,
    ) -> Result<Vec<PeriodPoolShock>> {
        let month_count = self.month_count(instrument);
        let factors = self.tree_path_factors(path_index, path_count, branch_count, month_count);
        self.path_shocks_from_factors(instrument, &factors)
    }

    fn tree_path_factors(
        &self,
        mut path_index: usize,
        path_count: usize,
        branch_count: usize,
        month_count: usize,
    ) -> Vec<f64> {
        let path_count = path_count.max(1);
        let branch_count = branch_count.max(1);
        let mut factors = Vec::with_capacity(month_count);
        let stratified = matches!(
            self.config.tree_config.branching,
            super::super::tree::BranchingSpec::Stratified { .. }
        );

        for month in 0..month_count {
            let z = if !self.has_stochastic_rates() {
                0.0
            } else if stratified {
                let p = (((path_index + month) % path_count) as f64 + 0.5) / path_count as f64;
                finstack_core::math::standard_normal_inv_cdf(p)
            } else {
                let branch = path_index % branch_count;
                path_index /= branch_count;
                let p = (branch as f64 + 0.5) / branch_count as f64;
                finstack_core::math::standard_normal_inv_cdf(p)
            };
            factors.push(z);
        }
        factors
    }

    fn path_shocks_from_factors(
        &self,
        instrument: &StructuredCredit,
        factors: &[f64],
    ) -> Result<Vec<PeriodPoolShock>> {
        let months_per_period = instrument.frequency.months().ok_or_else(|| {
            finstack_core::Error::Validation(
                "Structured credit stochastic pricing requires month-based payment frequencies"
                    .to_string(),
            )
        })? as usize;
        let months_per_period = months_per_period.max(1);
        let payment_periods = self.payment_period_count(instrument);
        let mut shocks = Vec::with_capacity(payment_periods);

        for period in 0..payment_periods {
            let start = period * months_per_period;
            let end = (start + months_per_period).min(factors.len());
            let month_slice = if start < end {
                &factors[start..end]
            } else {
                &[][..]
            };
            shocks.push(self.aggregate_monthly_shocks(start as u32, month_slice));
        }

        Ok(shocks)
    }

    fn aggregate_monthly_shocks(&self, start_month: u32, factors: &[f64]) -> PeriodPoolShock {
        if factors.is_empty() {
            return self.monthly_shock(start_month.saturating_add(1), 0.0);
        }

        let mut prepay_survival = 1.0;
        let mut default_survival = 1.0;
        let mut recovery_sum = 0.0;
        for (offset, factor) in factors.iter().enumerate() {
            let shock = self.monthly_shock(start_month.saturating_add(offset as u32 + 1), *factor);
            prepay_survival *= 1.0 - shock.smm;
            default_survival *= 1.0 - shock.mdr;
            recovery_sum += shock.recovery_rate;
        }

        let months = factors.len() as f64;
        PeriodPoolShock {
            smm: 1.0 - prepay_survival.powf(1.0 / months),
            mdr: 1.0 - default_survival.powf(1.0 / months),
            recovery_rate: recovery_sum / months,
        }
    }

    fn monthly_shock(&self, month_offset: u32, z: f64) -> PeriodPoolShock {
        let factor = if self.has_stochastic_rates() { z } else { 0.0 };
        let seasoning = self
            .config
            .tree_config
            .initial_seasoning
            .saturating_add(month_offset);
        let factors = [factor];

        PeriodPoolShock {
            smm: self.conditional_smm(seasoning, &factors),
            mdr: self.conditional_mdr(seasoning, &factors),
            recovery_rate: self.recovery_rate(factor),
        }
    }

    fn conditional_smm(&self, seasoning: u32, factors: &[f64]) -> f64 {
        if let Some(model) = self.config.tree_config.prepay_spec.build() {
            return model
                .conditional_smm(seasoning, factors, self.config.tree_config.pool_coupon, 1.0)
                .clamp(0.0, 0.50);
        }
        match &self.config.tree_config.prepay_spec {
            StochasticPrepaySpec::Deterministic(spec) => {
                spec.smm(seasoning).unwrap_or(0.0).clamp(0.0, 0.50)
            }
            _ => self
                .config
                .tree_config
                .prepay_spec
                .base_smm()
                .clamp(0.0, 0.50),
        }
    }

    fn conditional_mdr(&self, seasoning: u32, factors: &[f64]) -> f64 {
        if let Some(model) = self.config.tree_config.default_spec.build() {
            return model
                .conditional_mdr(seasoning, factors, &MacroCreditFactors::default())
                .clamp(0.0, 0.50);
        }
        match &self.config.tree_config.default_spec {
            StochasticDefaultSpec::Deterministic(spec) => {
                spec.mdr(seasoning).unwrap_or(0.0).clamp(0.0, 0.50)
            }
            _ => self
                .config
                .tree_config
                .default_spec
                .base_mdr()
                .clamp(0.0, 0.50),
        }
    }

    fn recovery_rate(&self, factor: f64) -> f64 {
        match &self.config.tree_config.recovery_spec {
            RecoverySpec::Constant { rate } => *rate,
            RecoverySpec::MarketCorrelated {
                mean_recovery,
                recovery_volatility,
                factor_correlation,
            } => {
                (mean_recovery + factor_correlation * recovery_volatility * factor).clamp(0.0, 1.0)
            }
        }
    }

    fn has_stochastic_rates(&self) -> bool {
        self.config.tree_config.prepay_spec.is_stochastic()
            || self.config.tree_config.default_spec.is_stochastic()
            || matches!(
                self.config.tree_config.recovery_spec,
                RecoverySpec::MarketCorrelated { .. }
            )
    }

    fn month_count(&self, instrument: &StructuredCredit) -> usize {
        let periods = self.payment_period_count(instrument);
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        periods.saturating_mul(months_per_period).max(1)
    }

    fn payment_period_count(&self, instrument: &StructuredCredit) -> usize {
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        let base_periods = self
            .config
            .tree_config
            .num_periods
            .saturating_add(months_per_period - 1)
            / months_per_period;
        base_periods.saturating_add(2).max(1)
    }

    fn branching_variance_proxy(&self) -> f64 {
        let factor_var = match &self.config.tree_config.factor_spec {
            FactorSpec::SingleFactor { volatility, .. } => volatility * volatility,
            FactorSpec::TwoFactor {
                credit_vol,
                prepay_vol,
                ..
            } => credit_vol * credit_vol + prepay_vol * prepay_vol,
            FactorSpec::MultiFactor { volatilities, .. } => {
                volatilities.iter().map(|v| v * v).sum::<f64>()
            }
        };
        let prepay_loading = self
            .config
            .tree_config
            .prepay_spec
            .factor_loading()
            .unwrap_or(0.0);
        let default_loading = self
            .config
            .tree_config
            .default_spec
            .correlation()
            .unwrap_or(0.0);
        let recovery_loading = match &self.config.tree_config.recovery_spec {
            RecoverySpec::Constant { .. } => 0.0,
            RecoverySpec::MarketCorrelated {
                factor_correlation,
                recovery_volatility,
                ..
            } => factor_correlation * recovery_volatility,
        };

        (factor_var * (prepay_loading.abs() + default_loading.abs() + recovery_loading.abs()))
            .clamp(0.0, 1.0)
    }

}

struct PathScenarioOutput {
    deal_pv: f64,
    deal_loss: f64,
    tranches: Vec<(usize, PathTrancheMetrics)>,
}

#[derive(Clone, Copy, Default)]
struct PathTrancheMetrics {
    pv: f64,
    loss: f64,
    wal: f64,
    duration: f64,
}

impl PathTrancheMetrics {
    fn from_cashflows(
        cashflows: &crate::instruments::fixed_income::structured_credit::TrancheCashflows,
        as_of: Date,
        discount_curve: &finstack_core::market_data::term_structures::DiscountCurve,
    ) -> Result<Self> {
        let mut pv = 0.0;
        let mut positive_pv = 0.0;
        let mut weighted_duration = 0.0;
        for (date, amount) in &cashflows.cashflows {
            if *date <= as_of {
                continue;
            }
            let df = discount_curve.df_between_dates(as_of, *date)?;
            let flow_pv = amount.amount() * df;
            pv += flow_pv;
            if flow_pv > 0.0 {
                let t =
                    DayCount::Act365F.year_fraction(as_of, *date, DayCountContext::default())?;
                positive_pv += flow_pv;
                weighted_duration += flow_pv * t;
            }
        }

        let mut principal = 0.0;
        let mut weighted_principal_time = 0.0;
        for (date, amount) in &cashflows.principal_flows {
            if *date <= as_of || amount.amount() <= 0.0 {
                continue;
            }
            let t = DayCount::Act365F.year_fraction(as_of, *date, DayCountContext::default())?;
            principal += amount.amount();
            weighted_principal_time += amount.amount() * t;
        }

        Ok(Self {
            pv,
            loss: cashflows.total_writedown.amount(),
            wal: if principal > f64::EPSILON {
                weighted_principal_time / principal
            } else {
                0.0
            },
            duration: if positive_pv > f64::EPSILON {
                weighted_duration / positive_pv
            } else {
                0.0
            },
        })
    }
}

struct TrancheScenarioStats {
    tranche_id: String,
    seniority: String,
    attachment: f64,
    detachment: f64,
    pv_stats: OnlineStats,
    loss_stats: OnlineStats,
    losses: Vec<f64>,
    wal_sum: f64,
    duration_sum: f64,
}

impl TrancheScenarioStats {
    fn new(tranche: &Tranche, num_paths: usize) -> Self {
        Self {
            tranche_id: tranche.id.to_string(),
            seniority: format!("{:?}", tranche.seniority),
            attachment: tranche.attachment_point / 100.0,
            detachment: tranche.detachment_point / 100.0,
            pv_stats: OnlineStats::new(),
            loss_stats: OnlineStats::new(),
            losses: Vec::with_capacity(num_paths),
            wal_sum: 0.0,
            duration_sum: 0.0,
        }
    }

    fn record(&mut self, metrics: PathTrancheMetrics) {
        self.pv_stats.update(metrics.pv);
        self.loss_stats.update(metrics.loss);
        self.losses.push(metrics.loss);
        self.wal_sum += metrics.wal;
        self.duration_sum += metrics.duration;
    }

    fn finalize(
        mut self,
        currency: finstack_core::currency::Currency,
        num_paths: usize,
        es_confidence: f64,
    ) -> TranchePricingResult {
        let paths = num_paths.max(1) as f64;
        let mean_pv = self.pv_stats.mean();
        let mean_loss = self.loss_stats.mean();
        // Use population variance (n denominator) to match the sibling
        // `StochasticMetricsCalculator::weighted_variance` convention and
        // the INVARIANTS §3 requirement for MC estimators.
        let loss_std = self.loss_stats.population_variance().sqrt();
        let es = expected_shortfall(&mut self.losses, es_confidence);

        TranchePricingResult::new(
            self.tranche_id,
            self.seniority,
            Money::new(mean_pv, currency),
        )
        .with_subordination(self.attachment, self.detachment)
        .with_risk_metrics(
            Money::new(mean_loss, currency),
            Money::new(loss_std, currency),
            Money::new(es, currency),
        )
        .with_average_life(self.wal_sum / paths)
        .with_credit_duration(self.duration_sum / paths)
    }
}

struct ScenarioCollector {
    currency: finstack_core::currency::Currency,
    num_paths: usize,
    deal_pv_stats: OnlineStats,
    deal_loss_stats: OnlineStats,
    deal_losses: Vec<f64>,
    tranche_stats: Vec<TrancheScenarioStats>,
}

impl ScenarioCollector {
    fn new(instrument: &StructuredCredit, num_paths: usize) -> Result<Self> {
        if num_paths == 0 {
            return Err(finstack_core::Error::Validation(
                "stochastic scenario collector requires at least one path".to_string(),
            ));
        }
        Ok(Self {
            currency: instrument.pool.base_currency(),
            num_paths,
            deal_pv_stats: OnlineStats::new(),
            deal_loss_stats: OnlineStats::new(),
            deal_losses: Vec::with_capacity(num_paths),
            tranche_stats: instrument
                .tranches
                .tranches
                .iter()
                .map(|tranche| TrancheScenarioStats::new(tranche, num_paths))
                .collect(),
        })
    }

    fn record_tranche(&mut self, idx: usize, metrics: PathTrancheMetrics) {
        if let Some(stats) = self.tranche_stats.get_mut(idx) {
            stats.record(metrics);
        }
    }

    fn record_deal(&mut self, pv: f64, loss: f64) {
        self.deal_pv_stats.update(pv);
        self.deal_loss_stats.update(loss);
        self.deal_losses.push(loss);
    }

    fn record_output(&mut self, output: PathScenarioOutput) {
        for (idx, metrics) in output.tranches {
            self.record_tranche(idx, metrics);
        }
        self.record_deal(output.deal_pv, output.deal_loss);
    }

    fn finalize(
        mut self,
        pricer: &StochasticPricer,
        pricing_mode: &str,
    ) -> StochasticPricingResult {
        let paths = self.num_paths as f64;
        let mean_pv = self.deal_pv_stats.mean();
        let mean_loss = self.deal_loss_stats.mean();
        // Welford population variance avoids catastrophic cancellation when
        // tranche PVs are large (≥ 1e7) and relative dispersion is small.
        // The sibling `StochasticMetricsCalculator::weighted_variance` uses
        // the same population-variance convention.
        let pv_pop_var = self.deal_pv_stats.population_variance();
        let loss_pop_var = self.deal_loss_stats.population_variance();
        let std_error = pv_pop_var.sqrt() / paths.sqrt();
        let es = expected_shortfall(&mut self.deal_losses, pricer.config.es_confidence);

        let mut result = StochasticPricingResult::new(
            Money::new(mean_pv, self.currency),
            Money::new(mean_loss, self.currency),
            self.num_paths,
        )
        .with_unexpected_loss(Money::new(loss_pop_var.sqrt(), self.currency))
        .with_expected_shortfall(Money::new(es, self.currency), pricer.config.es_confidence);

        let notional = pricer.config.tree_config.initial_balance;
        if notional > f64::EPSILON {
            result.clean_price = mean_pv / notional * 100.0;
            result.dirty_price = result.clean_price;
        }
        result.pv_std_error = std_error;
        result.pv_confidence_interval = (mean_pv - 1.96 * std_error, mean_pv + 1.96 * std_error);
        result.pricing_mode = pricing_mode.to_string();
        result.tranche_results = self
            .tranche_stats
            .into_iter()
            .map(|stats| stats.finalize(self.currency, self.num_paths, pricer.config.es_confidence))
            .collect();

        result
    }
}

fn expected_shortfall(losses: &mut [f64], confidence: f64) -> f64 {
    if losses.is_empty() {
        return 0.0;
    }
    losses.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    let tail = (1.0 - confidence).clamp(0.0, 1.0);
    let tail_count = (tail * losses.len() as f64).ceil().max(1.0) as usize;
    let tail_count = tail_count.min(losses.len());
    losses.iter().take(tail_count).sum::<f64>() / tail_count as f64
}

trait TrancheDurationSetter {
    fn with_credit_duration(self, duration: f64) -> Self;
}

impl TrancheDurationSetter for TranchePricingResult {
    fn with_credit_duration(mut self, duration: f64) -> Self {
        self.credit_duration = duration;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
        BranchingSpec, ScenarioTreeConfig,
    };
    use crate::instruments::fixed_income::structured_credit::{
        DealType, DefaultModelSpec, Pool, PoolAsset, RecoveryModelSpec, Seniority, Tranche,
        TrancheCoupon, TrancheStructure,
    };
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    fn test_discount_curve() -> std::sync::Arc<DiscountCurve> {
        std::sync::Arc::new(
            DiscountCurve::builder("USD-OIS")
                .base_date(test_date())
                .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.90)])
                .build()
                .expect("curve"),
        )
    }

    fn test_instrument() -> StructuredCredit {
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let mut pool = Pool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity,
            DayCount::Thirty360,
        ));
        let tranche = Tranche::new(
            "A",
            0.0,
            100.0,
            Seniority::Senior,
            Money::new(1_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity,
        )
        .expect("tranche");
        let mut instrument = StructuredCredit::new_abs(
            "ABS",
            pool,
            TrancheStructure::new(vec![tranche]).expect("structure"),
            test_date(),
            maturity,
            "USD-OIS",
        )
        .with_payment_calendar("nyse");
        instrument.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        instrument.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        instrument
    }

    #[test]
    fn monte_carlo_one_path_prices_waterfall_cashflows() {
        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
        )
        .with_pricing_mode(PricingMode::MonteCarlo {
            num_paths: 1,
            antithetic: false,
        });
        let pricer = StochasticPricer::new(config);

        let result = pricer.price(&instrument, &market).expect("price");

        assert_eq!(result.num_paths, 1);
        assert_eq!(result.tranche_results.len(), 1);
        assert!(result.npv.amount().is_finite());
    }

    #[test]
    fn hybrid_mode_prices_tree_prefix_and_mc_suffix_paths() {
        let instrument = test_instrument();
        let market = MarketContext::new().insert((*test_discount_curve()).clone());
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
        )
        .with_pricing_mode(PricingMode::Hybrid {
            tree_periods: 3,
            mc_paths: 100,
        });
        let pricer = StochasticPricer::new(config);

        let result = pricer.price(&instrument, &market).expect("hybrid price");

        assert_eq!(result.num_paths, 800);
        assert_eq!(result.tranche_results.len(), 1);
        assert!(result.npv.amount().is_finite());
        assert!(result.pricing_mode.contains("Hybrid"));
    }

    /// Regression test: catastrophic cancellation in MC variance accumulation.
    ///
    /// The `E[X²] - E[X]²` form (`sq_sum / paths - mean * mean`) suffers
    /// catastrophic cancellation when `delta² ≪ ULP(mean²)`.  For `mean = 5e7`
    /// the ULP of `mean²` is `≈ 0.555` (since `2^−52 · (5e7)² ≈ 0.555`).
    /// When `delta = 0.05` (`delta² = 0.0025 ≪ 0.555`) the two terms in the
    /// subtraction are identical in f64, so the naive form returns **exactly
    /// zero**, collapsing `pv_std_error` to zero even though the true value is
    /// `0.05 / √1000 ≈ 0.00158`.
    ///
    /// This test drives `ScenarioCollector` directly with synthetic path outputs
    /// whose population variance is known exactly, then verifies that the
    /// computed `pv_std_error` is accurate.
    ///
    /// For the buggy `E[X²] - E[X]²` form the computed variance is exactly
    /// **0.0** (collapsed), making `pv_std_error = 0.0` even though the true
    /// variance is `0.0025`.  The Welford fix recovers the correct value.
    #[test]
    fn scenario_collector_variance_no_catastrophic_cancellation() {
        let instrument = test_instrument();
        let n = 1000usize;
        let mut collector = ScenarioCollector::new(&instrument, n).expect("collector");

        // Synthetic PVs: alternating mean ± delta where delta is tiny relative to mean.
        // True population variance = delta² = 0.0025.
        // True population std      = 0.05.
        //
        // At mean = 5e7, ULP(mean²) ≈ 0.555.  delta² = 0.0025 ≪ 0.555, so the
        // naive sq_sum/n − mean² subtraction cancels completely to 0.0 in f64.
        let mean_pv: f64 = 50_000_000.0; // $50 M — large enough for cancellation
        let delta: f64 = 0.05; // $0.05 spread → sigma/mean = 1e-9, delta² ≪ ULP(mean²)

        for i in 0..n {
            let pv = if i % 2 == 0 {
                mean_pv + delta
            } else {
                mean_pv - delta
            };
            // Feed as deal-level output (no tranche sub-paths needed here).
            collector.record_deal(pv, 0.0);
        }

        // Extract the deal-level variance directly via the finalize path.
        // We use a minimal StochasticPricer config just to call finalize.
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::{
            BranchingSpec, ScenarioTreeConfig,
        };
        use crate::instruments::fixed_income::structured_credit::pricing::stochastic::pricer::config::StochasticPricerConfig;
        let config = StochasticPricerConfig::new(
            test_date(),
            test_discount_curve(),
            ScenarioTreeConfig::new(12, 1.0, BranchingSpec::fixed(2)),
        );
        let pricer = StochasticPricer::new(config);
        let result = collector.finalize(&pricer, "Test");

        // True population variance = delta² = 0.0025
        // True std_error of the mean = 0.05 / sqrt(1000) ≈ 0.001581
        let true_pop_var: f64 = delta * delta; // = 0.0025
        let true_std_error = true_pop_var.sqrt() / (n as f64).sqrt();

        // The E[X²]-E[X]² form collapses sq_sum/n − mean² to exactly 0.0 here:
        // delta² = 0.0025 is below the ~0.555 ULP of mean², so the subtraction
        // rounds to zero, making pv_std_error = 0. The Welford form is immune.
        assert!(
            result.pv_std_error > 0.0,
            "pv_std_error must be strictly positive (true value ≈ {true_std_error:.8}); \
             got {}. Catastrophic cancellation in sq_sum/n - mean² collapses to 0 \
             when delta²={:.6} ≪ ULP(mean²)≈0.555.",
            result.pv_std_error,
            delta * delta,
        );

        // Relative error must be small (< 0.5%). The E[X²]-E[X]² form
        // produces 100% relative error here; Welford is accurate to rounding.
        let rel_err = (result.pv_std_error - true_std_error).abs() / true_std_error;
        assert!(
            rel_err < 0.005,
            "pv_std_error relative error {rel_err:.4} exceeds 0.5%: \
             computed={}, true={true_std_error:.8}. \
             This indicates the E[X²]-E[X]² form is being used instead of Welford.",
            result.pv_std_error
        );
    }
}
