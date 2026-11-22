//! Scenario result aggregation, tranche statistics, and tail-risk metrics.

use crate::structured_credit::types::{StructuredCredit, Tranche, TrancheSeniority};
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::estimate::independent_estimator_stats;
use finstack_quant_valuations_core::results::structured_credit::{
    StochasticPricingResult, StructuredCreditPricingMode, TranchePricingResult,
};

/// Confidence level of the reported expected shortfall (95% ES) of the deal
/// and per-tranche path losses.
const ES_CONFIDENCE: f64 = 0.95;

pub(super) struct PathScenarioOutput {
    pub(super) deal_pv: f64,
    pub(super) deal_loss: f64,
    pub(super) tranches: Vec<(usize, PathTrancheMetrics)>,
    /// A collateral draw could not be funded somewhere on the path.
    pub(super) unfunded_draws: bool,
    /// Collateral draws funded on the path.
    pub(super) collateral_draws: f64,
    /// Draw option cost of the path (sum of the tranche shares).
    pub(super) draw_option_cost: f64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct PathTrancheMetrics {
    pub(super) pv: f64,
    pub(super) loss: f64,
    pub(super) wal: f64,
    /// Principal the tranche received on the path after the valuation date.
    pub(super) principal: f64,
    pub(super) duration: f64,
    /// Present value less the counterfactual (fair draw interest) present value.
    pub(super) option_cost: f64,
}

impl PathTrancheMetrics {
    pub(super) fn from_cashflows(
        cashflows: &crate::structured_credit::TrancheCashflows,
        as_of: Date,
        discount_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
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
        for flow in &cashflows.detailed_flows {
            if flow.get_kind() == finstack_quant_core::cashflow::CFKind::Notional
                && flow.get_amount().amount() < 0.0
                && flow.get_date() > as_of
            {
                let df = discount_curve.df_between_dates(as_of, flow.get_date())?;
                pv += flow.get_amount().amount() * df;
            }
        }

        let wal = crate::structured_credit::metrics::calculate_tranche_wal(cashflows, as_of)?;
        let principal: f64 = cashflows
            .principal_flows
            .iter()
            .filter(|(date, _)| *date > as_of)
            .map(|(_, amount)| amount.amount())
            .sum();

        Ok(Self {
            pv,
            loss: cashflows.total_writedown.amount(),
            wal,
            principal,
            duration: if positive_pv > f64::EPSILON {
                weighted_duration / positive_pv
            } else {
                0.0
            },
            option_cost: 0.0,
        })
    }
}

struct TrancheScenarioStats {
    tranche_id: String,
    seniority: TrancheSeniority,
    attach_pct: f64,
    detach_pct: f64,
    /// Current note balance at the valuation date: the face the tranche
    /// price is quoted on.
    current_balance: f64,
    pv_stats: OnlineStats,
    loss_stats: OnlineStats,
    losses: Vec<f64>,
    /// Sum of path WALs over the paths that returned principal.
    wal_sum: f64,
    /// Paths on which the tranche received any principal: a wiped-out path
    /// has no WAL and must not pull the average toward zero.
    paths_with_principal: usize,
    duration_sum: f64,
    option_cost_stats: OnlineStats,
}

impl TrancheScenarioStats {
    fn new(tranche: &Tranche, num_paths: usize) -> Self {
        Self {
            tranche_id: tranche.id.to_string(),
            seniority: tranche.seniority,
            attach_pct: tranche.effective_attach_pct(),
            detach_pct: tranche.effective_detach_pct(),
            current_balance: tranche.current_balance.amount(),
            pv_stats: OnlineStats::new(),
            loss_stats: OnlineStats::new(),
            losses: Vec::with_capacity(num_paths),
            wal_sum: 0.0,
            paths_with_principal: 0,
            duration_sum: 0.0,
            option_cost_stats: OnlineStats::new(),
        }
    }

    fn record(&mut self, metrics: PathTrancheMetrics) -> Result<()> {
        self.pv_stats.update(metrics.pv)?;
        self.loss_stats.update(metrics.loss)?;
        self.losses.push(metrics.loss);
        if metrics.principal > 0.0 {
            self.wal_sum += metrics.wal;
            self.paths_with_principal += 1;
        }
        self.duration_sum += metrics.duration;
        self.option_cost_stats.update(metrics.option_cost)?;
        Ok(())
    }

    fn finalize(
        mut self,
        currency: finstack_quant_core::currency::Currency,
        num_paths: usize,
    ) -> Result<TranchePricingResult> {
        let paths = num_paths.max(1) as f64;
        let mean_pv = self.pv_stats.get_mean()?.unwrap_or(0.0);
        let mean_loss = self.loss_stats.get_mean()?.unwrap_or(0.0);
        // Use population variance (n denominator), the established convention
        // for these Monte Carlo loss estimators.
        let loss_std = self
            .loss_stats
            .get_population_variance()?
            .unwrap_or(0.0)
            .sqrt();
        let es = expected_shortfall(&mut self.losses, ES_CONFIDENCE);

        let price_pct = if self.current_balance > f64::EPSILON {
            mean_pv / self.current_balance * 100.0
        } else {
            0.0
        };

        Ok(TranchePricingResult::new(
            self.tranche_id,
            self.seniority,
            Money::new(mean_pv, currency)?,
        )
        .with_price_pct(price_pct)
        .with_subordination(self.attach_pct, self.detach_pct)
        .with_risk_metrics(
            Money::new(mean_loss, currency)?,
            Money::new(loss_std, currency)?,
            Money::new(es, currency)?,
        )
        .with_wal(if self.paths_with_principal > 0 {
            self.wal_sum / self.paths_with_principal as f64
        } else {
            0.0
        })
        .with_paths_with_principal(self.paths_with_principal)
        .with_credit_duration(self.duration_sum / paths)
        .with_draw_option_cost(Money::new(
            self.option_cost_stats.get_mean()?.unwrap_or(0.0),
            currency,
        )?))
    }
}

pub(super) struct ScenarioCollector {
    currency: finstack_quant_core::currency::Currency,
    num_paths: usize,
    /// `true` when paths were generated as antithetic pairs `(2k, 2k+1)`.
    /// Each pair is one negatively-correlated draw, *not* two i.i.d. samples,
    /// so the deal-PV std-error must be computed over the `n/2` pair means.
    antithetic: bool,
    deal_loss_stats: OnlineStats,
    deal_losses: Vec<f64>,
    /// Per-path deal PVs, recorded in path order. Retained so sampling
    /// statistics can use independent antithetic pair means; the order matches
    /// the antithetic pairing `(2k, 2k+1)` because `record_output` is fed in
    /// path order (see `price_monte_carlo`).
    deal_pvs: Vec<f64>,
    tranche_stats: Vec<TrancheScenarioStats>,
    /// Paths on which a collateral draw could not be funded.
    unfunded_paths: usize,
    /// Collateral draws funded per path.
    draw_stats: OnlineStats,
    /// Per-path draw option cost, retained only for pools with stochastic
    /// revolvers.
    option_cost_paths: Option<Vec<f64>>,
}

impl ScenarioCollector {
    pub(super) fn new(
        instrument: &StructuredCredit,
        num_paths: usize,
        antithetic: bool,
        track_option_cost: bool,
    ) -> Result<Self> {
        if num_paths == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "stochastic scenario collector requires at least one path".to_string(),
            ));
        }
        Ok(Self {
            currency: instrument.pool.get_currency(),
            num_paths,
            antithetic,
            deal_loss_stats: OnlineStats::new(),
            deal_losses: Vec::with_capacity(num_paths),
            deal_pvs: Vec::with_capacity(num_paths),
            tranche_stats: instrument
                .tranches
                .tranches
                .iter()
                .map(|tranche| TrancheScenarioStats::new(tranche, num_paths))
                .collect(),
            unfunded_paths: 0,
            draw_stats: OnlineStats::new(),
            option_cost_paths: track_option_cost.then(|| Vec::with_capacity(num_paths)),
        })
    }

    pub(super) fn record_tranche(&mut self, idx: usize, metrics: PathTrancheMetrics) -> Result<()> {
        if let Some(stats) = self.tranche_stats.get_mut(idx) {
            stats.record(metrics)?;
        }
        Ok(())
    }

    pub(super) fn record_deal(&mut self, pv: f64, loss: f64) -> Result<()> {
        self.deal_loss_stats.update(loss)?;
        self.deal_losses.push(loss);
        self.deal_pvs.push(pv);
        Ok(())
    }

    pub(super) fn record_output(&mut self, output: PathScenarioOutput) -> Result<()> {
        for (idx, metrics) in output.tranches {
            self.record_tranche(idx, metrics)?;
        }
        self.record_deal(output.deal_pv, output.deal_loss)?;
        if output.unfunded_draws {
            self.unfunded_paths += 1;
        }
        self.draw_stats.update(output.collateral_draws)?;
        if let Some(paths) = self.option_cost_paths.as_mut() {
            paths.push(output.draw_option_cost);
        }
        Ok(())
    }

    pub(super) fn finalize(
        mut self,
        pricing_mode: StructuredCreditPricingMode,
    ) -> Result<StochasticPricingResult> {
        let pv_stats = independent_estimator_stats(&self.deal_pvs, self.antithetic)?;
        let mean_pv = pv_stats.get_mean()?.unwrap_or(0.0);
        let mean_loss = self.deal_loss_stats.get_mean()?.unwrap_or(0.0);
        // Welford population variance avoids catastrophic cancellation when
        // tranche PVs are large (≥ 1e7) and relative dispersion is small.
        let loss_pop_var = self
            .deal_loss_stats
            .get_population_variance()?
            .unwrap_or(0.0);
        let es = expected_shortfall(&mut self.deal_losses, ES_CONFIDENCE);

        let mut result = StochasticPricingResult::new(
            Money::new(mean_pv, self.currency)?,
            Money::new(mean_loss, self.currency)?,
            self.num_paths,
            pricing_mode,
        )
        .with_unexpected_loss(Money::new(loss_pop_var.sqrt(), self.currency)?)
        .with_expected_shortfall(Money::new(es, self.currency)?, ES_CONFIDENCE);

        // Prices are quoted per CURRENT note balance, the same face the
        // deterministic `dirty_price` metric uses, not per pool balance: a
        // deal whose notes total 60M against a 100M pool prices at 100, not 60.
        let notional: f64 = self
            .tranche_stats
            .iter()
            .map(|stats| stats.current_balance)
            .sum();
        if notional > f64::EPSILON {
            // `mean_pv` is the present value of all FUTURE cashflows
            // from the valuation date, which is by definition the DIRTY price —
            // it already contains the accrued portion of the next coupon.
            // Assigning it to `clean_price` first and then copying to
            // `dirty_price` had the labels backwards on the primary quantity.
            //
            // Clean = dirty − accrued. The deal-level stochastic result carries
            // no per-tranche interest flows, so accrued cannot be computed here
            // (the same constraint the accrued calculator documents). Rather than fabricate one, `clean_price` is set
            // equal to dirty and flagged as UNADJUSTED: understating accrued by
            // at most one period's interest, in a known direction, instead of
            // silently mislabelling which of the two is authoritative. Use
            // `tranche_metrics` for an accrued-adjusted clean price.
            result.dirty_price = mean_pv / notional * 100.0;
            result.clean_price = result.dirty_price;
        }
        result.pv_std_error = pv_stats.get_stderr()?.unwrap_or(f64::NAN);
        result.pv_confidence_interval =
            match (pv_stats.get_mean()?, pv_stats.get_ci_half_width(0.05)?) {
                (Some(m), Some(h)) => {
                    let lo = m - h;
                    let hi = m + h;
                    if !lo.is_finite() || !hi.is_finite() {
                        return Err(finstack_quant_core::Error::Internal(
                            "PV 95% confidence interval bounds overflowed".into(),
                        ));
                    }
                    (lo, hi)
                }
                _ => (f64::NAN, f64::NAN),
            };
        result.unfunded_draw_path_fraction =
            self.unfunded_paths as f64 / self.num_paths.max(1) as f64;
        result.expected_collateral_draws =
            Money::new(self.draw_stats.get_mean()?.unwrap_or(0.0), self.currency)?;
        if let Some(paths) = self.option_cost_paths.take() {
            let mean = if paths.is_empty() {
                0.0
            } else {
                paths.iter().sum::<f64>() / paths.len() as f64
            };
            result.draw_option_cost = Money::new(mean, self.currency)?;
            result.draw_option_cost_paths = paths;
        }
        result.tranche_results = self
            .tranche_stats
            .into_iter()
            .map(|stats| stats.finalize(self.currency, self.num_paths))
            .collect::<Result<Vec<_>>>()?;

        Ok(result)
    }
}

pub(super) fn expected_shortfall(losses: &mut [f64], confidence: f64) -> f64 {
    if losses.is_empty() {
        return 0.0;
    }
    // `total_cmp`, not `partial_cmp(..).unwrap_or(Equal)`.
    //
    // The old comparator is not a total order when a NaN is present — NaN
    // compares Equal to everything while the finite values retain their own
    // ordering — which violates `sort_by`'s contract. Since Rust 1.81 the
    // standard sort detects that and PANICS rather than producing garbage.
    // A NaN can reach here through `convert_clamped` (utils/rates.rs), so this
    // was a live panic path in a pricing loop, not a theoretical one.
    //
    // `total_cmp` is a genuine total order over all f64 including NaN, so the
    // sort is always well-defined. NaN sorts to the front under the reversed
    // comparator, which is the conservative placement for a tail-loss measure:
    // a corrupted path shows up as an extreme loss rather than being silently
    // averaged into the middle of the distribution.
    losses.sort_by(|a, b| b.total_cmp(a));
    let tail = (1.0 - confidence).clamp(0.0, 1.0);
    let tail_count = (tail * losses.len() as f64).ceil().max(1.0) as usize;
    let tail_count = tail_count.min(losses.len());
    losses.iter().take(tail_count).sum::<f64>() / tail_count as f64
}
