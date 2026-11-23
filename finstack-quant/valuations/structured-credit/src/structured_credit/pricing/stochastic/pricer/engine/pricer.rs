//! Core [`StochasticPricer`] and [`PreparedRun`] engine.

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec};

use super::super::config::StochasticPricerConfig;
use crate::structured_credit::pricing::simulation_engine::{
    PreparedDealSimulation, PreparedInstrumentSchedules, prepare_deal_simulation,
};
use crate::structured_credit::types::StructuredCredit;
use finstack_quant_core::Result;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::correlation::{RecoveryModel, RecoverySpec};
use finstack_quant_models::credit::pool::{
    StochasticDefault, StochasticDefaultSpec, StochasticPrepaySpec, StochasticPrepayment,
};
use finstack_quant_valuations_core::results::structured_credit::{
    StochasticPricingResult, StructuredCreditPricingMode,
};
use std::sync::Arc;

use super::collector::{PathScenarioOutput, ScenarioCollector};

/// Stochastic pricing engine for structured credit.
///
/// Each scenario path feeds period SMM/MDR/recovery assumptions into the same
/// waterfall simulation used by deterministic tranche valuation. PV is computed
/// from actual dated tranche payments, not from terminal expected loss shortcuts.
pub(crate) struct StochasticPricer {
    pub(super) config: StochasticPricerConfig,
}

/// Selected default branch, prepared once from the immutable run config.
pub(super) enum PreparedDefault {
    Deterministic(DefaultModelSpec),
    Model(Box<dyn StochasticDefault>),
}

impl PreparedDefault {
    pub(super) fn new(spec: &StochasticDefaultSpec) -> Result<Self> {
        if let StochasticDefaultSpec::Deterministic(spec) = spec {
            spec.validate()?;
            return Ok(Self::Deterministic(spec.clone()));
        }
        spec.build()?.map(Self::Model).ok_or_else(|| {
            finstack_quant_core::Error::Internal(
                "stochastic default specification built no model".into(),
            )
        })
    }
}

/// Selected prepayment branch, with no spec/model mismatch in path loops.
pub(super) enum PreparedPrepayment {
    Deterministic(PrepaymentModelSpec),
    Model(Box<dyn StochasticPrepayment>),
}

impl PreparedPrepayment {
    pub(super) fn new(spec: &StochasticPrepaySpec) -> Result<Self> {
        if let StochasticPrepaySpec::Deterministic(spec) = spec {
            spec.validate()?;
            return Ok(Self::Deterministic(spec.clone()));
        }
        spec.build().map(Self::Model).ok_or_else(|| {
            finstack_quant_core::Error::Internal(
                "stochastic prepayment specification built no model".into(),
            )
        })
    }
}

/// Loop-invariant state built once per pricing run and shared read-only
/// across all paths.
///
/// The default/prepay model builds are pure functions of immutable config
/// fields, so building them once here — instead of inside every path's month
/// loop, where 10k paths × 360 months would mean millions of identical
/// `Box<dyn>` allocations plus deep hazard-curve clones — is bit-identical:
/// every trait method takes `&self` and no model carries cross-call state
/// (burnout and latent regimes live in the caller's [`PrepaymentPath`]).
pub(crate) struct PreparedRun {
    /// Checked deterministic specification or built stochastic default model.
    /// Selects the monthly MDR calculation and sources copula marginal PDs.
    pub(super) default_model: PreparedDefault,
    /// Canonical recovery distribution shared across path engines.
    pub(super) recovery_model: Arc<dyn RecoveryModel>,
    /// Checked deterministic specification or built stochastic prepayment model.
    /// Selects the SMM calculation and whether latent path state is advanced.
    pub(super) prepay_model: PreparedPrepayment,
    /// `Some(rho)` iff the default spec is a copula variant, with the deal
    /// correlation override applied. Gates the per-name overlay channel;
    /// other stochastic default variants keep the pool-wide channel.
    pub(crate) copula_rho: Option<f64>,
    /// Configured systematic-factor mean-reversion speed κ (per year).
    pub(crate) factor_kappa: f64,
    /// Monthly autocorrelation φ = e^{−κ/12}.
    pub(crate) factor_phi: f64,
    /// Validated prepay/default factor correlation.
    pub(crate) factor_correlation: Option<f64>,
    /// Loop-invariant deal simulation (validation, calendar, schedule,
    /// waterfall) prepared once for this run's valuation date.
    pub(crate) sim: PreparedDealSimulation,
    /// Bucketed instrument schedules when the pool holds instrument
    /// collateral; every path drives the instrument engine from them.
    pub(crate) instrument_schedules: Option<PreparedInstrumentSchedules>,
    /// Deal asset correlation loading instrument-collateral spread shocks on
    /// the systematic factor.
    pub(crate) asset_correlation: f64,
}

impl StochasticPricer {
    /// Create a new stochastic pricer.
    pub(crate) fn new(config: StochasticPricerConfig) -> Self {
        Self { config }
    }

    /// Build the loop-invariant [`PreparedRun`] for one pricing invocation.
    ///
    /// # Errors
    ///
    /// Propagates invalid default-spec construction (e.g. Student-t dof ≤ 2)
    /// and unsupported latent-factor specs, once and before any path runs.
    pub(super) fn prepare_run(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<PreparedRun> {
        // Fail fast on invalid default specs and keep the built model so the
        // per-path hot loops can assume a validated, already-built spec.
        let default_model = PreparedDefault::new(&self.config.tree_config.default_spec)?;
        // Prepare the loop-invariant deal simulation once. A `None` here
        // (exhausted pool) yields no tranche results, which the collector
        // reports as a missing-tranche validation error; raise it here with
        // the same message.
        let sim =
            prepare_deal_simulation(instrument, self.config.valuation_date)?.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "stochastic waterfall omitted tranche result '{}'",
                    instrument
                        .tranches
                        .tranches
                        .first()
                        .map(|t| t.id.as_str())
                        .unwrap_or_default()
                ))
            })?;
        let prepay_model = PreparedPrepayment::new(&self.config.tree_config.prepay_spec)?;
        let copula_rho = self.copula_rho();
        let factor_kappa = self.factor_mean_reversion();
        let factor_correlation = self.factor_correlation()?;
        let instrument_schedules = if instrument.pool.instruments.is_some() {
            Some(PreparedInstrumentSchedules::prepare(
                instrument, context, &sim,
            )?)
        } else {
            None
        };
        let asset_correlation = match copula_rho {
            Some(rho) => rho,
            None => instrument.effective_asset_correlation()?,
        };
        Ok(PreparedRun {
            default_model,
            recovery_model: Arc::from(self.config.tree_config.recovery_spec.build()?),
            prepay_model,
            copula_rho,
            factor_kappa,
            factor_phi: (-factor_kappa / 12.0).exp(),
            factor_correlation,
            sim,
            instrument_schedules,
            asset_correlation,
        })
    }

    /// Price the full deal and all tranches through scenario-level waterfalls.
    #[tracing::instrument(
        name = "structured_credit.stochastic.price",
        level = "debug",
        skip(self, instrument, context),
        fields(
            inst_id = %instrument.id.as_str(),
            deal_type = ?instrument.deal_type,
            as_of = %self.config.valuation_date,
            tranche_count = instrument.tranches.tranches.len(),
        ),
        err,
    )]
    pub(crate) fn price(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
    ) -> Result<StochasticPricingResult> {
        let prepared = self.prepare_run(instrument, context)?;
        let StructuredCreditPricingMode::MonteCarlo {
            num_paths,
            antithetic,
        } = &self.config.pricing_mode;
        self.price_monte_carlo(instrument, context, *num_paths, *antithetic, &prepared)
    }

    /// Price `num_paths` independent estimators, each path drawing its monthly
    /// factors from its own Philox substream inside the (parallel) path loop
    /// so no path's factors outlive its pricing.
    #[tracing::instrument(
        name = "structured_credit.stochastic.price_monte_carlo",
        level = "debug",
        skip(self, instrument, context, prepared),
        fields(
            inst_id = %instrument.id.as_str(),
            as_of = %self.config.valuation_date,
            num_paths = num_paths,
            antithetic = antithetic,
            periods = prepared.sim.periods.len(),
        ),
        err,
    )]
    pub(super) fn price_monte_carlo(
        &self,
        instrument: &StructuredCredit,
        context: &MarketContext,
        num_paths: usize,
        antithetic: bool,
        prepared: &PreparedRun,
    ) -> Result<StochasticPricingResult> {
        if num_paths < 2 {
            return Err(finstack_quant_core::Error::Validation(
                "Monte Carlo pricing requires at least two independent estimators \
                 (pricing_mode.monte_carlo.num_paths); each antithetic pair counts once"
                    .to_string(),
            ));
        }

        // `num_paths` counts independent estimators; antithetic pairing
        // simulates each estimator as a `(Z, -Z)` pair of adjacent
        // `(2k, 2k+1)` paths.
        let total_paths = num_paths
            .checked_mul(if antithetic { 2 } else { 1 })
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Monte Carlo pricing path count overflow".to_string(),
                )
            })?;
        let per_name_simulator = self.per_name_simulator()?;
        // `try_map_ordered` over `0..n` returns outputs in path order
        // regardless of rayon scheduling, and each path keeps a stable
        // index for its factor and idiosyncratic-draw substreams. Both
        // properties are required for bit-identical serial/parallel results
        // (the downstream Welford accumulation is order-sensitive).
        let price_factors = |path_index: usize| {
            let factors = self.monte_carlo_path_factors(instrument, path_index, antithetic)?;
            let shocks = self.path_shocks_from_factors(
                instrument,
                &factors,
                (path_index, antithetic),
                prepared,
            )?;
            let per_name_engine = per_name_simulator
                .as_ref()
                .map(|sim| self.per_name_engine(sim, path_index, antithetic, prepared));
            self.price_path(
                instrument,
                context,
                prepared,
                shocks,
                per_name_engine,
                (path_index, antithetic),
            )
        };

        // A failure reports the first failing path in path order.
        let outputs: Vec<PathScenarioOutput> =
            finstack_quant_core::parallel::try_map_ordered(0..total_paths, price_factors)?;

        let mut collector = ScenarioCollector::new(
            instrument,
            total_paths,
            antithetic,
            self.tracks_option_cost(prepared),
        )?;
        for output in outputs {
            collector.record_output(output)?;
        }

        collector.finalize(StructuredCreditPricingMode::MonteCarlo {
            num_paths,
            antithetic,
        })
    }

    /// Whether the run values revolver draws against their fair spread.
    pub(super) fn tracks_option_cost(&self, prepared: &PreparedRun) -> bool {
        prepared
            .instrument_schedules
            .as_ref()
            .is_some_and(PreparedInstrumentSchedules::has_draw_option_cost)
    }

    pub(super) fn has_stochastic_rates(&self) -> bool {
        self.config.tree_config.prepay_spec.is_stochastic()
            || self.config.tree_config.default_spec.is_stochastic()
            || matches!(
                self.config.tree_config.recovery_spec,
                RecoverySpec::MarketCorrelated { .. }
            )
    }

    pub(super) fn month_count(&self, instrument: &StructuredCredit) -> usize {
        let periods = self.payment_period_count(instrument);
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        periods.saturating_mul(months_per_period).max(1)
    }

    pub(super) fn payment_period_count(&self, instrument: &StructuredCredit) -> usize {
        let months_per_period = instrument.frequency.months().unwrap_or(1).max(1) as usize;
        let base_periods = self
            .config
            .tree_config
            .num_periods
            .saturating_add(months_per_period - 1)
            / months_per_period;
        base_periods.saturating_add(2).max(1)
    }
}
