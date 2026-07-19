//! Exact-profile request planning and bounded batch execution.

use super::executor::{evaluate_portfolio, EvaluationInput, PositionExecution, SelectiveSeed};
use super::state::{PreparedMarketState, PreparedPortfolioState};
use crate::error::{Error, Result};
use crate::valuation::{PortfolioValuation, PortfolioValuationOptions, RequestedMetrics};
use crate::Portfolio;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;
use std::sync::Arc;

const STATE_PARALLEL_MIN_JOBS: usize = 8;
const STATE_PARALLEL_MAX_POSITIONS: usize = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MarketStateId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PortfolioStateId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct EvaluationId(u32);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EvaluationMetricProfile {
    PvOnly,
    Metrics(Box<[MetricId]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RiskFailurePolicy {
    Strict,
    BestEffort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BaseCurrencyPolicy {
    Convert,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct EvaluationProfile {
    pub(crate) metrics: EvaluationMetricProfile,
    pub(crate) risk_policy: RiskFailurePolicy,
    pub(crate) base_currency_policy: BaseCurrencyPolicy,
}

#[derive(Clone, Debug)]
pub(crate) struct EvaluationProvenance {
    pub(crate) profile: EvaluationProfile,
    pub(crate) portfolio_state_id: u64,
    pub(crate) base_ccy: Currency,
}

impl EvaluationProfile {
    pub(crate) fn from_options(options: &PortfolioValuationOptions) -> Self {
        let metrics = match &options.metrics {
            RequestedMetrics::Standard => standard_metrics(),
            RequestedMetrics::StandardPlus(extra) => {
                let mut metrics = standard_metrics();
                extend_unique(&mut metrics, extra);
                metrics
            }
            RequestedMetrics::Only(metrics) => stable_unique(metrics),
        };

        Self {
            metrics: if metrics.is_empty() {
                EvaluationMetricProfile::PvOnly
            } else {
                EvaluationMetricProfile::Metrics(metrics.into_boxed_slice())
            },
            risk_policy: if options.strict_risk {
                RiskFailurePolicy::Strict
            } else {
                RiskFailurePolicy::BestEffort
            },
            base_currency_policy: BaseCurrencyPolicy::Convert,
        }
    }

    pub(crate) fn strict_metrics(metrics: &[MetricId]) -> Self {
        let metrics = stable_unique(metrics);
        Self {
            metrics: if metrics.is_empty() {
                EvaluationMetricProfile::PvOnly
            } else {
                EvaluationMetricProfile::Metrics(metrics.into_boxed_slice())
            },
            risk_policy: RiskFailurePolicy::Strict,
            base_currency_policy: BaseCurrencyPolicy::Convert,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct EvaluationKey {
    market_state: MarketStateId,
    portfolio_state: PortfolioStateId,
    profile: EvaluationProfile,
}

pub(crate) enum ParentResult<'a> {
    External(&'a PortfolioValuation),
}

pub(crate) struct PositionInvalidation {
    reprice_indices: Vec<usize>,
    refresh_base_currency: bool,
    authoritative_portfolio_change: bool,
}

impl PositionInvalidation {
    pub(crate) fn new(mut reprice_indices: Vec<usize>, refresh_base_currency: bool) -> Self {
        reprice_indices.sort_unstable();
        reprice_indices.dedup();
        Self {
            reprice_indices,
            refresh_base_currency,
            authoritative_portfolio_change: false,
        }
    }

    pub(crate) fn with_authoritative_portfolio_change(mut self) -> Self {
        self.authoritative_portfolio_change = true;
        self
    }
}

struct SelectiveJob<'a> {
    prior: &'a PortfolioValuation,
    reprice_indices: Vec<usize>,
    refresh_base_currency: bool,
}

struct EvaluationJob<'a> {
    id: EvaluationId,
    key: EvaluationKey,
    execution: PositionExecution,
    selective: Option<SelectiveJob<'a>>,
}

pub(crate) struct PortfolioEvaluationPlan<'a> {
    config: &'a FinstackConfig,
    markets: Vec<PreparedMarketState<'a>>,
    portfolios: Vec<PreparedPortfolioState<'a>>,
    jobs: Vec<EvaluationJob<'a>>,
    jobs_by_key: HashMap<EvaluationKey, EvaluationId>,
}

pub(crate) struct PortfolioEvaluationBatch {
    results: IndexMap<EvaluationId, Arc<PortfolioValuation>>,
}

pub(crate) struct PortfolioEvaluationOutcome {
    batch: PortfolioEvaluationBatch,
    failures: IndexMap<EvaluationId, Error>,
}

impl<'a> PortfolioEvaluationPlan<'a> {
    pub(crate) fn new(config: &'a FinstackConfig) -> Self {
        Self {
            config,
            markets: Vec::new(),
            portfolios: Vec::new(),
            jobs: Vec::new(),
            jobs_by_key: HashMap::default(),
        }
    }

    pub(crate) fn register_market(
        &mut self,
        market: &'a MarketContext,
        as_of: Date,
    ) -> MarketStateId {
        let id = MarketStateId(next_id(self.markets.len()));
        self.markets.push(PreparedMarketState::borrowed(
            id,
            market,
            as_of,
            self.config,
        ));
        id
    }

    pub(crate) fn register_owned_market(
        &mut self,
        market: MarketContext,
        as_of: Date,
    ) -> MarketStateId {
        let id = MarketStateId(next_id(self.markets.len()));
        self.markets
            .push(PreparedMarketState::owned(id, market, as_of, self.config));
        id
    }

    pub(crate) fn register_portfolio(&mut self, portfolio: &'a Portfolio) -> PortfolioStateId {
        let id = PortfolioStateId(next_id(self.portfolios.len()));
        self.portfolios
            .push(PreparedPortfolioState::borrowed(id, portfolio));
        id
    }

    pub(crate) fn register_owned_portfolio(&mut self, portfolio: Portfolio) -> PortfolioStateId {
        let id = PortfolioStateId(next_id(self.portfolios.len()));
        self.portfolios
            .push(PreparedPortfolioState::owned(id, portfolio));
        id
    }

    pub(crate) fn register_evaluation(
        &mut self,
        market: MarketStateId,
        portfolio: PortfolioStateId,
        profile: EvaluationProfile,
    ) -> Result<EvaluationId> {
        self.register_evaluation_with_execution(market, portfolio, profile, PositionExecution::Auto)
    }

    pub(crate) fn register_evaluation_with_execution(
        &mut self,
        market: MarketStateId,
        portfolio: PortfolioStateId,
        profile: EvaluationProfile,
        execution: PositionExecution,
    ) -> Result<EvaluationId> {
        self.validate_state_ids(market, portfolio)?;
        let key = EvaluationKey {
            market_state: market,
            portfolio_state: portfolio,
            profile,
        };
        if let Some(id) = self.jobs_by_key.get(&key) {
            return Ok(*id);
        }

        let id = EvaluationId(next_id(self.jobs.len()));
        self.jobs.push(EvaluationJob {
            id,
            key: key.clone(),
            execution,
            selective: None,
        });
        self.jobs_by_key.insert(key, id);
        Ok(id)
    }

    pub(crate) fn register_selective_evaluation(
        &mut self,
        market: MarketStateId,
        portfolio: PortfolioStateId,
        profile: EvaluationProfile,
        parent: ParentResult<'a>,
        invalidation: PositionInvalidation,
    ) -> Result<EvaluationId> {
        self.validate_state_ids(market, portfolio)?;
        let key = EvaluationKey {
            market_state: market,
            portfolio_state: portfolio,
            profile: profile.clone(),
        };
        if let Some(id) = self.jobs_by_key.get(&key) {
            return Ok(*id);
        }

        let portfolio_state = self.portfolio(portfolio)?;
        let market_state = self.market(market)?;
        let selective = match parent {
            ParentResult::External(valuation)
                if valuation.as_of == market_state.as_of
                    && valuation.provenance.as_ref().is_some_and(|provenance| {
                        provenance.profile == profile
                            && provenance.base_ccy == portfolio_state.portfolio.base_ccy
                            && (provenance.portfolio_state_id
                                == portfolio_state.portfolio.evaluation_state_id
                                || invalidation.authoritative_portfolio_change)
                    })
                    && prior_matches_portfolio(
                        valuation,
                        &portfolio_state.portfolio,
                        &invalidation.reprice_indices,
                    )
                    && invalidation
                        .reprice_indices
                        .iter()
                        .all(|index| *index < portfolio_state.portfolio.positions.len()) =>
            {
                Some(SelectiveJob {
                    prior: valuation,
                    reprice_indices: invalidation.reprice_indices,
                    refresh_base_currency: invalidation.refresh_base_currency,
                })
            }
            ParentResult::External(_) => None,
        };

        let id = EvaluationId(next_id(self.jobs.len()));
        self.jobs.push(EvaluationJob {
            id,
            key: key.clone(),
            execution: PositionExecution::Auto,
            selective,
        });
        self.jobs_by_key.insert(key, id);
        Ok(id)
    }

    pub(crate) fn execute(self) -> PortfolioEvaluationOutcome {
        let state_parallel = self.jobs.len() >= STATE_PARALLEL_MIN_JOBS
            && self.jobs.iter().all(|job| {
                self.portfolio(job.key.portfolio_state).is_ok_and(|state| {
                    state.portfolio.positions.len() <= STATE_PARALLEL_MAX_POSITIONS
                })
            });

        let ordered_results: Vec<Result<PortfolioValuation>> = if state_parallel {
            use rayon::prelude::*;
            self.jobs
                .par_iter()
                .map(|job| self.execute_job(job, PositionExecution::Serial))
                .collect()
        } else {
            self.jobs
                .iter()
                .map(|job| self.execute_job(job, job.execution))
                .collect()
        };

        let mut results = IndexMap::with_capacity(self.jobs.len());
        let mut failures = IndexMap::new();
        for (job, result) in self.jobs.iter().zip(ordered_results) {
            match result {
                Ok(valuation) => {
                    results.insert(job.id, Arc::new(valuation));
                }
                Err(error) => {
                    failures.insert(job.id, error);
                }
            }
        }

        PortfolioEvaluationOutcome {
            batch: PortfolioEvaluationBatch { results },
            failures,
        }
    }

    fn execute_job(
        &self,
        job: &EvaluationJob<'a>,
        execution: PositionExecution,
    ) -> Result<PortfolioValuation> {
        let market = self.market(job.key.market_state)?;
        let portfolio = self.portfolio(job.key.portfolio_state)?;
        evaluate_portfolio(EvaluationInput {
            portfolio: &portfolio.portfolio,
            market: &market.market,
            as_of: market.as_of,
            config: self.config,
            profile: &job.key.profile,
            pricing_options: &market.pricing_options,
            execution,
            seed: job.selective.as_ref().map(|selective| SelectiveSeed {
                prior: selective.prior,
                reprice_indices: &selective.reprice_indices,
                refresh_base_currency: selective.refresh_base_currency,
            }),
        })
    }

    fn validate_state_ids(&self, market: MarketStateId, portfolio: PortfolioStateId) -> Result<()> {
        self.market(market)?;
        self.portfolio(portfolio)?;
        Ok(())
    }

    fn market(&self, id: MarketStateId) -> Result<&PreparedMarketState<'a>> {
        self.markets
            .get(id.0 as usize)
            .filter(|state| state.id == id)
            .ok_or_else(|| Error::invalid_input(format!("unknown market state {}", id.0)))
    }

    fn portfolio(&self, id: PortfolioStateId) -> Result<&PreparedPortfolioState<'a>> {
        self.portfolios
            .get(id.0 as usize)
            .filter(|state| state.id == id)
            .ok_or_else(|| Error::invalid_input(format!("unknown portfolio state {}", id.0)))
    }
}

impl PortfolioEvaluationOutcome {
    pub(crate) fn get(&self, id: EvaluationId) -> Result<&Arc<PortfolioValuation>> {
        if let Some(result) = self.batch.results.get(&id) {
            return Ok(result);
        }
        if let Some(error) = self.failures.get(&id) {
            return Err(error.clone());
        }
        Err(Error::invalid_input(format!(
            "unknown evaluation result {}",
            id.0
        )))
    }

    pub(crate) fn into_valuation(mut self, id: EvaluationId) -> Result<PortfolioValuation> {
        self.take_valuation(id)
    }

    pub(crate) fn take_valuation(&mut self, id: EvaluationId) -> Result<PortfolioValuation> {
        if let Some(error) = self.failures.shift_remove(&id) {
            return Err(error);
        }
        let result =
            self.batch.results.shift_remove(&id).ok_or_else(|| {
                Error::invalid_input(format!("unknown evaluation result {}", id.0))
            })?;
        Ok(Arc::try_unwrap(result).unwrap_or_else(|shared| shared.as_ref().clone()))
    }
}

fn next_id(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

fn standard_metrics() -> Vec<MetricId> {
    vec![
        MetricId::Theta,
        MetricId::Dv01,
        MetricId::BucketedDv01,
        MetricId::Cs01,
        MetricId::BucketedCs01,
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Pv01,
    ]
}

fn stable_unique(metrics: &[MetricId]) -> Vec<MetricId> {
    let mut unique = Vec::with_capacity(metrics.len());
    extend_unique(&mut unique, metrics);
    unique
}

fn extend_unique(target: &mut Vec<MetricId>, metrics: &[MetricId]) {
    for metric in metrics {
        if !target.contains(metric) {
            target.push(metric.clone());
        }
    }
}

fn prior_matches_portfolio(
    prior: &PortfolioValuation,
    portfolio: &Portfolio,
    reprice_indices: &[usize],
) -> bool {
    if prior.position_values.len() != portfolio.positions.len()
        || prior.total_base_ccy.currency() != portfolio.base_ccy
    {
        return false;
    }

    // `PositionInvalidation::new` sorts and deduplicates these indices. Use a
    // cursor instead of allocating an O(N) dirty mask for every selective run.
    if reprice_indices
        .last()
        .is_some_and(|&index| index >= portfolio.positions.len())
    {
        return false;
    }

    let mut next_dirty = reprice_indices.iter().copied().peekable();
    for (index, position) in portfolio.positions.iter().enumerate() {
        let Some((prior_id, value)) = prior.position_values.get_index(index) else {
            return false;
        };
        if prior_id != &position.position_id || value.position_id != position.position_id {
            return false;
        }

        let dirty = next_dirty.peek().copied() == Some(index);
        if dirty {
            next_dirty.next();
            continue;
        }

        if value.entity_id != position.entity_id
            || value.metric_scale.to_bits() != position.scale_factor().to_bits()
            || value.value_base.currency() != portfolio.base_ccy
            || value
                .valuation_result
                .as_ref()
                .is_none_or(|result| result.instrument_id != position.instrument.id())
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::DUMMY_ENTITY_ID;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::{Attributes, Instrument, PricingOptions};
    use finstack_quant_valuations::pricer::InstrumentType;
    use finstack_quant_valuations::results::ValuationResult;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::macros::date;

    #[derive(Clone)]
    struct CountingInstrument {
        id: String,
        attributes: Attributes,
        base_calls: Arc<AtomicUsize>,
        metric_calls: Arc<AtomicUsize>,
        metrics_fail: bool,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        CountingInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for CountingInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Basket
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            self.base_calls.fetch_add(1, Ordering::SeqCst);
            Ok(Money::new(100.0, Currency::USD))
        }

        fn price_with_metrics(
            &self,
            _market: &MarketContext,
            as_of: Date,
            _metrics: &[MetricId],
            options: PricingOptions,
        ) -> finstack_quant_core::Result<ValuationResult> {
            self.metric_calls.fetch_add(1, Ordering::SeqCst);
            if self.metrics_fail {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "metric failure for {}",
                    self.id
                )));
            }
            let config = options.config.ok_or_else(|| {
                finstack_quant_core::Error::Validation("missing config".to_string())
            })?;
            Ok(ValuationResult::stamped_with_config(
                self.id(),
                as_of,
                Money::new(100.0, Currency::USD),
                config.as_ref(),
            ))
        }
    }

    fn counting_portfolio(
        id: &str,
        positions: usize,
        metrics_fail: bool,
    ) -> (Portfolio, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let base_calls = Arc::new(AtomicUsize::new(0));
        let metric_calls = Arc::new(AtomicUsize::new(0));
        let instrument_id = format!("{id}_INSTRUMENT");
        let instrument: Arc<dyn Instrument> = Arc::new(CountingInstrument {
            id: instrument_id.clone(),
            attributes: Attributes::new(),
            base_calls: Arc::clone(&base_calls),
            metric_calls: Arc::clone(&metric_calls),
            metrics_fail,
        });
        let mut builder = PortfolioBuilder::new(id)
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01));
        for index in 0..positions {
            builder = builder.position(
                Position::new(
                    format!("{id}_{index}"),
                    DUMMY_ENTITY_ID,
                    instrument_id.clone(),
                    Arc::clone(&instrument),
                    1.0,
                    PositionUnit::Units,
                )
                .expect("counting position"),
            );
        }
        (
            builder.build().expect("counting portfolio"),
            base_calls,
            metric_calls,
        )
    }

    fn empty_portfolio() -> Portfolio {
        PortfolioBuilder::new("EVALUATION_TEST")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .build()
            .expect("empty portfolio")
    }

    #[test]
    fn empty_only_profile_is_pv_only() {
        let profile = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: false,
            metrics: RequestedMetrics::Only(Vec::new()),
        });
        assert_eq!(profile.metrics, EvaluationMetricProfile::PvOnly);
        assert_eq!(profile.risk_policy, RiskFailurePolicy::BestEffort);
    }

    #[test]
    fn metric_profiles_are_stably_deduplicated() {
        let profile = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: true,
            metrics: RequestedMetrics::Only(vec![MetricId::Dv01, MetricId::Theta, MetricId::Dv01]),
        });
        let EvaluationMetricProfile::Metrics(metrics) = profile.metrics else {
            panic!("expected metrics profile");
        };
        assert_eq!(metrics.as_ref(), &[MetricId::Dv01, MetricId::Theta]);
        assert_eq!(profile.risk_policy, RiskFailurePolicy::Strict);
    }

    #[test]
    fn exact_jobs_are_deduplicated_but_state_and_policy_changes_are_not() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let portfolio = empty_portfolio();
        let mut plan = PortfolioEvaluationPlan::new(&config);
        let portfolio_state = plan.register_portfolio(&portfolio);
        let market_state = plan.register_market(&market, portfolio.as_of);
        let best_effort = EvaluationProfile::from_options(&Default::default());

        let first = plan
            .register_evaluation(market_state, portfolio_state, best_effort.clone())
            .expect("first job");
        let duplicate = plan
            .register_evaluation(market_state, portfolio_state, best_effort.clone())
            .expect("duplicate job");
        assert_eq!(first, duplicate);
        assert_eq!(plan.jobs.len(), 1);

        let strict = EvaluationProfile {
            risk_policy: RiskFailurePolicy::Strict,
            ..best_effort.clone()
        };
        let strict_job = plan
            .register_evaluation(market_state, portfolio_state, strict)
            .expect("strict job");
        assert_ne!(first, strict_job);

        let second_market_state =
            plan.register_market(&market, portfolio.as_of + time::Duration::days(1));
        assert_ne!(market_state, second_market_state);
        let dated_job = plan
            .register_evaluation(second_market_state, portfolio_state, best_effort)
            .expect("dated job");
        assert_ne!(first, dated_job);
    }

    #[test]
    fn duplicate_pv_only_jobs_price_once() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let (portfolio, base_calls, metric_calls) = counting_portfolio("PV_ONLY", 1, false);
        let mut plan = PortfolioEvaluationPlan::new(&config);
        let portfolio_state = plan.register_portfolio(&portfolio);
        let market_state = plan.register_market(&market, portfolio.as_of);
        let profile = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: true,
            metrics: RequestedMetrics::Only(Vec::new()),
        });

        let first = plan
            .register_evaluation(market_state, portfolio_state, profile.clone())
            .expect("first PV-only job");
        let duplicate = plan
            .register_evaluation_with_execution(
                market_state,
                portfolio_state,
                profile,
                PositionExecution::Parallel,
            )
            .expect("duplicate PV-only job");
        assert_eq!(
            first, duplicate,
            "execution policy does not change a job key"
        );

        let outcome = plan.execute();
        assert!(outcome.get(first).is_ok());
        assert_eq!(base_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            metric_calls.load(Ordering::SeqCst),
            1,
            "PV-only uses the canonical price_with_metrics(..., &[]) path"
        );
    }

    #[test]
    fn state_identity_keeps_distinct_as_of_dates() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let (portfolio, _, _) = counting_portfolio("DATED", 1, false);
        let mut plan = PortfolioEvaluationPlan::new(&config);
        let portfolio_state = plan.register_portfolio(&portfolio);
        let initial = plan.register_market(&market, portfolio.as_of);
        let next_date = portfolio.as_of + time::Duration::days(1);
        let next = plan.register_market(&market, next_date);
        let profile = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: true,
            metrics: RequestedMetrics::Only(Vec::new()),
        });
        let initial_job = plan
            .register_evaluation(initial, portfolio_state, profile.clone())
            .expect("initial job");
        let next_job = plan
            .register_evaluation(next, portfolio_state, profile)
            .expect("next-date job");

        let outcome = plan.execute();
        assert_ne!(initial_job, next_job);
        assert_eq!(
            outcome.get(initial_job).expect("initial result").as_of,
            portfolio.as_of
        );
        assert_eq!(outcome.get(next_job).expect("next result").as_of, next_date);
    }

    #[test]
    fn strict_metrics_fail_while_best_effort_degrades() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let (portfolio, base_calls, metric_calls) = counting_portfolio("METRICS", 1, true);
        let mut plan = PortfolioEvaluationPlan::new(&config);
        let portfolio_state = plan.register_portfolio(&portfolio);
        let market_state = plan.register_market(&market, portfolio.as_of);
        let best_effort = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: false,
            metrics: RequestedMetrics::Only(vec![MetricId::Dv01]),
        });
        let strict = EvaluationProfile::strict_metrics(&[MetricId::Dv01]);
        let best_effort_job = plan
            .register_evaluation(market_state, portfolio_state, best_effort)
            .expect("best-effort job");
        let strict_job = plan
            .register_evaluation(market_state, portfolio_state, strict)
            .expect("strict job");

        let outcome = plan.execute();
        let best_effort_result = outcome.get(best_effort_job).expect("best-effort result");
        assert!(best_effort_result.has_degraded_risk());
        assert!(outcome
            .get(strict_job)
            .expect_err("strict job must surface metric failure")
            .to_string()
            .contains("metric failure for METRICS_INSTRUMENT"));
        assert_eq!(metric_calls.load(Ordering::SeqCst), 2);
        assert_eq!(base_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn serial_and_parallel_position_evaluation_match_in_logical_order() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let (portfolio, _, _) = counting_portfolio("EXECUTION", 64, false);
        let profile = EvaluationProfile::from_options(&PortfolioValuationOptions {
            strict_risk: true,
            metrics: RequestedMetrics::Only(Vec::new()),
        });

        let mut serial_plan = PortfolioEvaluationPlan::new(&config);
        let serial_portfolio = serial_plan.register_portfolio(&portfolio);
        let serial_market = serial_plan.register_market(&market, portfolio.as_of);
        let serial_job = serial_plan
            .register_evaluation_with_execution(
                serial_market,
                serial_portfolio,
                profile.clone(),
                PositionExecution::Serial,
            )
            .expect("serial job");
        let serial = serial_plan
            .execute()
            .into_valuation(serial_job)
            .expect("serial result");

        let mut parallel_plan = PortfolioEvaluationPlan::new(&config);
        let parallel_portfolio = parallel_plan.register_portfolio(&portfolio);
        let parallel_market = parallel_plan.register_market(&market, portfolio.as_of);
        let parallel_job = parallel_plan
            .register_evaluation_with_execution(
                parallel_market,
                parallel_portfolio,
                profile,
                PositionExecution::Parallel,
            )
            .expect("parallel job");
        let parallel = parallel_plan
            .execute()
            .into_valuation(parallel_job)
            .expect("parallel result");

        assert_eq!(serial.as_of, parallel.as_of);
        assert_eq!(serial.total_base_ccy, parallel.total_base_ccy);
        assert_eq!(
            serial.position_values.keys().collect::<Vec<_>>(),
            parallel.position_values.keys().collect::<Vec<_>>()
        );
        for (position_id, serial_value) in &serial.position_values {
            assert_eq!(
                parallel.position_values[position_id].value_base,
                serial_value.value_base
            );
        }
    }

    #[test]
    fn state_parallel_failures_remain_in_registration_order() {
        let config = FinstackConfig::default();
        let market = build_test_market();
        let (first_portfolio, _, _) = counting_portfolio("FIRST", 1, true);
        let (second_portfolio, _, _) = counting_portfolio("SECOND", 1, true);
        let mut plan = PortfolioEvaluationPlan::new(&config);
        let first_state = plan.register_portfolio(&first_portfolio);
        let second_state = plan.register_portfolio(&second_portfolio);
        let market_state = plan.register_market(&market, first_portfolio.as_of);
        let profile = EvaluationProfile::strict_metrics(&[MetricId::Dv01]);
        let first = plan
            .register_evaluation(market_state, first_state, profile.clone())
            .expect("first failure");
        let second = plan
            .register_evaluation(market_state, second_state, profile.clone())
            .expect("second failure");
        for _ in 0..3 {
            let state = plan.register_portfolio(&first_portfolio);
            plan.register_evaluation(market_state, state, profile.clone())
                .expect("additional failure");
            let state = plan.register_portfolio(&second_portfolio);
            plan.register_evaluation(market_state, state, profile.clone())
                .expect("additional failure");
        }

        let outcome = plan.execute();
        let failure_ids = outcome.failures.keys().copied().collect::<Vec<_>>();
        assert_eq!(failure_ids[0], first);
        assert_eq!(failure_ids[1], second);
        assert!(outcome
            .get(first)
            .expect_err("first failure")
            .to_string()
            .contains("FIRST_INSTRUMENT"));
    }
}
