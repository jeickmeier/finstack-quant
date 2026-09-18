//! Stochastic Monte Carlo revolving-credit pricing.

use super::path_generator::generate_three_factor_paths;
use super::path_pricing::resolve_fixings;
use super::results::EnhancedMonteCarloResult;
use super::unified::{
    RevolvingCreditPricer, DEFAULT_CREDIT_SPREAD_IMPLIED_VOL, DEFAULT_UTIL_CREDIT_CORR,
};
use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule, CashflowRepresentation};
use crate::cashflow::traits::{schedule_from_classified_flows, ScheduleBuildOpts};
use crate::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
use crate::instruments::fixed_income::revolving_credit::types::{DrawRepaySpec, RevolvingCredit};
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::{HashMap, Result};
use finstack_quant_models::monte_carlo::estimate::Estimate;
use finstack_quant_models::monte_carlo::results::{MoneyEstimate, MonteCarloResult};

/// Accumulator for one expected flow across Monte Carlo paths.
struct ExpectedFlow {
    amount: Money,
    rate_sum: f64,
    rate_count: usize,
    accrual_factor: f64,
}

/// Monte Carlo estimate of per-path values with Bessel-corrected variance.
///
/// Antithetic paths are NOT i.i.d. — each `(z, −z)` pair is negatively
/// correlated by construction. Treating the `2N` pathwise values as
/// independent overstates the effective sample size and misstates the
/// standard error, so each adjacent antithetic pair is averaged into ONE
/// i.i.d. sample first. The 95% interval assumes asymptotic normality.
fn path_estimate(values: &[f64], antithetic: bool) -> Estimate {
    let samples: Vec<f64> = if antithetic {
        values
            .chunks(2)
            .map(|pair| pair.iter().sum::<f64>() / pair.len() as f64)
            .collect()
    } else {
        values.to_vec()
    };
    let n = samples.len() as f64;
    let mean = if samples.is_empty() {
        0.0
    } else {
        samples.iter().sum::<f64>() / n
    };
    let variance = if samples.len() > 1 {
        samples.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    let stderr = if samples.is_empty() {
        0.0
    } else {
        (variance / n).sqrt()
    };
    let z_95 = 1.96;
    Estimate::new(
        mean,
        stderr,
        (mean - z_95 * stderr, mean + z_95 * stderr),
        samples.len(),
    )
}

impl RevolvingCreditPricer {
    /// Expected cashflow schedule of a stochastic facility.
    ///
    /// Runs the Monte Carlo valuation and averages the per-path schedules flow
    /// by flow. Every path books its flows on the same dates (the adjusted
    /// payment dates of the accrual periods and the midpoint funding dates of
    /// the observation grid), so averaging amounts and coupon rates per
    /// `(date, reset date, kind)` is a well-defined expectation. A flow absent
    /// on some paths (a commitment fee on a fully drawn path, for example)
    /// contributes zero on those paths. This backs `CashflowScheduleSource`
    /// for stochastic facilities, i.e. theta carry and the JSON cashflow
    /// exporters, and is deterministic under the fixed simulation seed.
    ///
    /// # Arguments
    ///
    /// * `facility` - Revolving credit facility; must carry a stochastic
    ///   draw/repay spec.
    /// * `market` - Curves used to generate and project each path.
    /// * `as_of` - Valuation date; flows paid on or before it are excluded.
    pub fn expected_cashflows(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        facility.validate()?;
        let enhanced = Self::price_monte_carlo(facility, market, as_of)?;
        let num_paths = enhanced.path_results.len() as f64;
        let ccy = facility.commitment_amount.currency();

        // Insertion order of the first appearance keeps the schedule stable
        // (paths share dates and emission order).
        let mut order: Vec<(Date, Option<Date>, CFKind)> = Vec::new();
        let mut buckets: HashMap<(Date, Option<Date>, CFKind), ExpectedFlow> = HashMap::default();
        for path in &enhanced.path_results {
            for cf in path.cashflows.get_flows() {
                let key = (cf.date, cf.reset_date, cf.kind);
                let entry = buckets.entry(key).or_insert_with(|| {
                    order.push(key);
                    ExpectedFlow {
                        amount: Money::from((0_i64, ccy)),
                        rate_sum: 0.0,
                        rate_count: 0,
                        accrual_factor: cf.accrual_factor,
                    }
                });
                entry.amount = entry.amount.checked_add(cf.amount)?;
                if let Some(rate) = cf.rate {
                    entry.rate_sum += rate;
                    entry.rate_count += 1;
                }
            }
        }

        let flows = order
            .into_iter()
            .map(|key| {
                let flow = &buckets[&key];
                let rate = (flow.rate_count > 0).then(|| flow.rate_sum / flow.rate_count as f64);
                CashFlow::new(
                    key.0,
                    key.1,
                    flow.amount * (1.0 / num_paths),
                    key.2,
                    flow.accrual_factor,
                    rate,
                )
            })
            .collect();

        Ok(schedule_from_classified_flows(
            flows,
            facility.day_count,
            ScheduleBuildOpts {
                notional_hint: Some(Money::from((0_i64, ccy))),
                meta: CashFlowMeta {
                    projected_fixings: Vec::new(),
                    representation: CashflowRepresentation::Projected,
                    calendar_ids: Vec::new(),
                    facility_limit: Some(facility.commitment_amount),
                    issue_date: Some(facility.commitment_date),
                    maturity_date: None,
                },
            },
        ))
    }

    /// Price with full MC path capture for analysis.
    ///
    /// # Arguments
    ///
    /// * `facility` - Revolving credit facility; must carry a stochastic draw/repay spec.
    /// * `market` - Curves used to generate and discount each path.
    /// * `as_of` - Valuation date for the simulation.
    pub fn price_with_paths(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<EnhancedMonteCarloResult> {
        facility.validate()?;
        Self::require_default_model_for_contingent_exposure(facility)?;
        match &facility.draw_repay_spec {
            DrawRepaySpec::Stochastic(_) => Self::price_monte_carlo(facility, market, as_of),
            DrawRepaySpec::Deterministic(_) => Err(finstack_quant_core::Error::Validation(
                "Path capture requires stochastic spec".into(),
            )),
        }
    }

    /// Internal MC pricing with 3-factor path generation and aggregation.
    ///
    /// This method:
    /// 1. Generates 3-factor MC paths (utilization, rate, spread)
    /// 2. Generates cashflows for each path
    /// 3. Prices each path deterministically
    /// 4. Computes MC statistics across all paths
    pub(crate) fn price_monte_carlo(
        facility: &RevolvingCredit,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<EnhancedMonteCarloResult> {
        let stoch_spec = match &facility.draw_repay_spec {
            DrawRepaySpec::Stochastic(spec) => spec.as_ref(),
            DrawRepaySpec::Deterministic(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Stochastic spec required for MC pricing".to_string(),
                ))
            }
        };

        use super::super::types::{CreditSpreadProcessSpec, McConfig};
        let mc_config_to_use;
        let mc_config = if let Some(ref mc_config) = stoch_spec.mc_config {
            mc_config.validate()?;
            mc_config
        } else {
            // Synthesize minimal McConfig
            // If facility has hazard curve, use market-anchored process; otherwise constant zero
            //
            // NOTE: when the facility carries a hazard curve, the synthesized
            // config defaults to a moderate positive utilization–credit
            // correlation (`DEFAULT_UTIL_CREDIT_CORR`) and a genuinely
            // stochastic credit spread (`DEFAULT_CREDIT_SPREAD_IMPLIED_VOL`),
            // so the default stochastic valuation embeds adverse selection:
            // spread up ⇒ utilization up ⇒ higher exposure-at-default. Without
            // a hazard curve there is no credit factor, so no correlation is
            // applied. Supply an explicit `McConfig` to override either
            // default (e.g. `util_credit_corr: Some(0.0)` to disable adverse
            // selection).
            let (credit_process, util_credit_corr) =
                if let Some(ref hazard_id) = facility.credit_curve_id {
                    (
                        CreditSpreadProcessSpec::MarketAnchored {
                            credit_curve_id: hazard_id.clone(),
                            kappa: 0.1,
                            implied_vol: DEFAULT_CREDIT_SPREAD_IMPLIED_VOL,
                            tenor_years: None,
                        },
                        Some(DEFAULT_UTIL_CREDIT_CORR),
                    )
                } else {
                    // No credit factor → a utilization–credit correlation
                    // would be inert; leave it unset.
                    (CreditSpreadProcessSpec::Constant(0.0), None)
                };

            mc_config_to_use = McConfig {
                correlation_matrix: None,
                credit_spread_process: credit_process,
                interest_rate_process: None,
                util_credit_corr,
            };
            mc_config_to_use.validate()?;
            tracing::debug!(
                facility_id = facility.id.as_str(),
                util_credit_corr = ?mc_config_to_use.util_credit_corr,
                "auto-synthesized revolver McConfig: default utilization/credit \
                 correlation embeds adverse selection when a hazard curve is \
                 present; supply an explicit McConfig (e.g. util_credit_corr: \
                 Some(0.0)) to override"
            );
            &mc_config_to_use
        };

        // Historical fixings remain contractual in stochastic valuation. The
        // short-rate process drives only reset dates that have not fixed yet.
        let fixings = resolve_fixings(facility, market);
        let engine = CashflowEngine::new(facility, Some(market), as_of, fixings)?;
        // Factor state is observed on accrual boundaries plus term-index
        // reset dates so intra-period resets re-fix the coupon.
        let observation_dates = super::super::utils::build_observation_dates(facility)?;

        // Generate 3-factor paths (simulation starts at as_of for seasoned facilities)
        let paths = generate_three_factor_paths(
            stoch_spec,
            mc_config,
            facility,
            market,
            &observation_dates,
            as_of,
        )?;

        // Price each path. Paths carry their own pre-generated randomness and
        // `generate_stochastic_path` / `price_single_path` are pure functions of
        // `path_data` plus the shared (immutable) engine/facility/market, so the
        // valuation is parallelised. `into_par_iter().collect()` preserves path
        // order, keeping the antithetic pairing and the PV statistics identical
        // to the serial implementation.
        let price_path = |path_data| {
            let schedule = engine.generate_stochastic_path(path_data)?;
            Self::price_single_path(facility, market, as_of, &schedule)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let path_results: Vec<_> = {
            use rayon::prelude::*;
            paths
                .into_par_iter()
                .map(price_path)
                .collect::<Result<Vec<_>>>()?
        };

        #[cfg(target_arch = "wasm32")]
        let path_results: Vec<_> = paths
            .into_iter()
            .map(price_path)
            .collect::<Result<Vec<_>>>()?;

        // Compute MC statistics using Bessel-corrected variance (N-1 denominator)
        // for unbiased standard error estimation.
        //
        // Antithetic paths are NOT i.i.d. — each (z, −z) pair is negatively
        // correlated by construction. Treating the 2N pathwise PVs as
        // independent overstates the effective sample size and misstates the
        // standard error. The correct estimator averages each antithetic
        // pair into ONE i.i.d. sample first (pairs are adjacent in path
        // order), then applies the usual sample statistics.
        let pvs: Vec<f64> = path_results.iter().map(|r| r.pv.amount()).collect();
        let costs: Vec<f64> = path_results
            .iter()
            .map(|r| r.draw_option_cost.amount())
            .collect();
        let use_antithetic = stoch_spec.antithetic && !stoch_spec.use_sobol_qmc;
        let currency = facility.commitment_amount.currency();
        let estimate = MoneyEstimate::from_estimate(path_estimate(&pvs, use_antithetic), currency)?;
        let draw_option_cost =
            MoneyEstimate::from_estimate(path_estimate(&costs, use_antithetic), currency)?;

        let result = EnhancedMonteCarloResult {
            mc_result: MonteCarloResult {
                estimate,
                paths: None,
                run: None,
            },
            path_results,
            draw_option_cost,
        };

        // Touch exported details so they are live under `-D dead-code`.
        let _ = result.mc_result.estimate.num_paths;
        let _ = result.path_results.len();

        Ok(result)
    }
}
