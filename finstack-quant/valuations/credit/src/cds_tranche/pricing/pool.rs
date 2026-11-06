//! Immutable pool inputs shared by ordinary and prepared tranche pricing.
use super::heterogeneous::PreparedHeterogeneousPool;
use super::pricer::{CdsTranchePricer, HOMOGENEITY_TOLERANCE, PoolExposure};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::math::distributions::BinomialPmf;
use finstack_quant_core::{Error, Result};
use std::sync::{Arc, OnceLock};

pub(super) struct PreparedPool {
    kind: PoolKind,
    // Issuer pools historically integrate loss/recovery separately, including
    // uniform pools. Preserve that accumulation contract for Student-t.
    separate_pair: bool,
}
enum PoolKind {
    Homogeneous(HomogeneousPool),
    Heterogeneous(PreparedHeterogeneousPool),
}

impl PreparedPool {
    pub(super) fn new(
        pricer: &CdsTranchePricer,
        index_data: &CreditIndexData,
        maturity: Date,
        binomial: &mut Option<Arc<PreparedBinomial>>,
    ) -> Result<Self> {
        if !index_data.has_issuer_curves() {
            let t = pricer.years_from_base(index_data, maturity)?;
            return Ok(Self {
                kind: PoolKind::Homogeneous(HomogeneousPool::new(
                    pricer,
                    shared_binomial(usize::from(index_data.num_constituents()), binomial),
                    pricer.get_default_probability(index_data, t)?,
                    index_data.recovery_rate(),
                )),
                separate_pair: false,
            });
        }
        let curves = index_data.issuer_credit_curves().ok_or_else(|| {
            Error::Validation("heterogeneous pricing requires issuer curves".to_owned())
        })?;
        let count = usize::from(index_data.num_constituents());
        if count == 0 || curves.len() != count {
            return Err(Error::Validation(
                "heterogeneous pricing requires complete issuer coverage".to_owned(),
            ));
        }
        let mut probabilities = Vec::with_capacity(count);
        let mut recoveries = Vec::with_capacity(count);
        let mut weights = Vec::with_capacity(count);
        let mut total_weight = 0.0;
        for (id, curve) in curves {
            let weight = index_data.get_issuer_weight(id);
            if !weight.is_finite() || weight < 0.0 {
                return Err(Error::Validation(
                    "issuer weights must be finite and non-negative".to_owned(),
                ));
            }
            total_weight += weight;
            // Names without exposure must not change the loss distribution or
            // the choice between the binomial and heterogeneous algorithms.
            if weight == 0.0 {
                continue;
            }
            let time = curve.day_count().year_fraction(
                curve.base_date(),
                maturity,
                DayCountContext::default(),
            )?;
            probabilities.push((1.0 - curve.sp(time)).clamp(0.0, 1.0));
            recoveries.push(index_data.get_issuer_recovery(id));
            weights.push(weight);
        }
        if (total_weight - 1.0).abs() > 1e-9 {
            return Err(Error::Validation(
                "issuer weights must preserve the full pool notional".to_owned(),
            ));
        }
        let count = weights.len();
        let uniform = |values: &[f64]| {
            values
                .iter()
                .all(|x| (x - values[0]).abs() <= HOMOGENEITY_TOLERANCE)
        };
        if uniform(&probabilities) && uniform(&recoveries) && uniform(&weights) {
            return Ok(Self {
                kind: PoolKind::Homogeneous(HomogeneousPool::new(
                    pricer,
                    shared_binomial(count, binomial),
                    probabilities[0],
                    recoveries[0],
                )),
                separate_pair: true,
            });
        }
        let thresholds: Vec<_> = probabilities
            .iter()
            .map(|p| pricer.default_threshold_for_copula(*p))
            .collect();

        Ok(Self {
            kind: PoolKind::Heterogeneous(PreparedHeterogeneousPool::new(
                thresholds, weights, recoveries,
            )),
            separate_pair: true,
        })
    }

    pub(super) fn capped(
        &self,
        pricer: &CdsTranchePricer,
        cap: f64,
        correlation: f64,
        exposure: PoolExposure,
    ) -> Result<f64> {
        match &self.kind {
            PoolKind::Homogeneous(pool) => pool.capped(pricer, cap, correlation, exposure),
            PoolKind::Heterogeneous(pool) => pool.capped(pricer, cap, correlation, exposure),
        }
    }

    pub(super) fn pair(
        &self,
        pricer: &CdsTranchePricer,
        loss_cap: f64,
        recovery_cap: f64,
        correlation: f64,
    ) -> Result<(f64, f64)> {
        if !self.separate_pair
            && let PoolKind::Homogeneous(pool) = &self.kind
        {
            return pool.pair(pricer, loss_cap, recovery_cap, correlation);
        }
        Ok((
            self.capped(pricer, loss_cap, correlation, PoolExposure::Loss)?,
            self.capped(pricer, recovery_cap, correlation, PoolExposure::Recovery)?,
        ))
    }
}

pub(super) struct HomogeneousPool {
    binomial: Arc<PreparedBinomial>,
    default_prob: f64,
    recovery: f64,
    threshold: f64,
}
impl HomogeneousPool {
    pub(super) fn new(
        pricer: &CdsTranchePricer,
        binomial: Arc<PreparedBinomial>,
        default_prob: f64,
        recovery: f64,
    ) -> Self {
        Self {
            binomial,
            default_prob,
            recovery,
            threshold: pricer.default_threshold_for_copula(default_prob),
        }
    }
    fn conditional_pd<'a>(
        &'a self,
        pricer: &'a CdsTranchePricer,
        correlation: f64,
    ) -> impl Fn(&[f64]) -> f64 + 'a {
        move |factors| {
            pricer.conditional_default_prob_copula(
                pricer.copula(),
                self.threshold,
                factors,
                correlation,
            )
        }
    }
    pub(super) fn capped(
        &self,
        pricer: &CdsTranchePricer,
        cap: f64,
        correlation: f64,
        exposure: PoolExposure,
    ) -> Result<f64> {
        let exposure = match exposure {
            PoolExposure::Loss => 1.0 - self.recovery,
            PoolExposure::Recovery => self.recovery,
        };
        if let Some(expected) = pricer.gaussian_cap_identity(
            cap,
            self.binomial.count,
            self.default_prob,
            exposure,
            correlation,
        ) {
            return Ok(expected);
        }
        let binomial = self.binomial.get();
        let conditional_p = self.conditional_pd(pricer, correlation);
        pricer.integrate_factors(&|factors| {
            pricer.conditional_equity_tranche_capped(
                binomial,
                cap,
                conditional_p(factors),
                exposure,
            )
        })
    }
    pub(super) fn pair(
        &self,
        pricer: &CdsTranchePricer,
        loss_cap: f64,
        recovery_cap: f64,
        correlation: f64,
    ) -> Result<(f64, f64)> {
        let loss = pricer.gaussian_cap_identity(
            loss_cap,
            self.binomial.count,
            self.default_prob,
            1.0 - self.recovery,
            correlation,
        );
        let recovery = pricer.gaussian_cap_identity(
            recovery_cap,
            self.binomial.count,
            self.default_prob,
            self.recovery,
            correlation,
        );
        let conditional_p = self.conditional_pd(pricer, correlation);
        match (loss, recovery) {
            (Some(loss), Some(recovery)) => return Ok((loss, recovery)),
            (Some(loss), None) => {
                let binomial = self.binomial.get();
                return Ok((
                    loss,
                    pricer.integrate_factors(&|factors| {
                        pricer.conditional_equity_tranche_capped(
                            binomial,
                            recovery_cap,
                            conditional_p(factors),
                            self.recovery,
                        )
                    })?,
                ));
            }
            (None, Some(recovery)) => {
                let binomial = self.binomial.get();
                return Ok((
                    pricer.integrate_factors(&|factors| {
                        pricer.conditional_equity_tranche_capped(
                            binomial,
                            loss_cap,
                            conditional_p(factors),
                            1.0 - self.recovery,
                        )
                    })?,
                    recovery,
                ));
            }
            (None, None) => {}
        }
        let binomial = self.binomial.get();
        pricer.integrate_factors_pair(&|factors| {
            pricer.conditional_equity_tranche_pair(
                binomial,
                loss_cap,
                recovery_cap,
                conditional_p(factors),
                1.0 - self.recovery,
                self.recovery,
            )
        })
    }
}

// Preparation-local sharing only: coupon dates for one immutable index have
// the same active-name count. Keep at most one coefficient vector here.
fn shared_binomial(n: usize, cached: &mut Option<Arc<PreparedBinomial>>) -> Arc<PreparedBinomial> {
    match cached {
        Some(pmf) if pmf.count == n => Arc::clone(pmf),
        _ => {
            let pmf = Arc::new(PreparedBinomial::new(n));
            *cached = Some(Arc::clone(&pmf));
            pmf
        }
    }
}

pub(super) struct PreparedBinomial {
    count: usize,
    pmf: OnceLock<BinomialPmf>,
}
impl PreparedBinomial {
    pub(super) fn new(count: usize) -> Self {
        Self {
            count,
            pmf: OnceLock::new(),
        }
    }
    fn get(&self) -> &BinomialPmf {
        self.pmf.get_or_init(|| BinomialPmf::new(self.count))
    }
}
