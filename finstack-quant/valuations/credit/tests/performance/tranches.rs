//! Offline numerical regression evidence for frozen tranche cases.
//!
//! The ignored regression reads sealed independent references and validates the
//! selected pricing implementation. Current diagnostics are separate from the
//! immutable historical benchmark evidence.

use super::CdsTranchePricer;
use super::pricer::{
    GRID_STEP, HOMOGENEITY_TOLERANCE, MAX_GRID_POINTS, PROBABILITY_CLIP, PoolExposure,
};
use crate::cds_tranche::CdsTranche;
use finstack_quant_core::canonical::content_hash;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_models::correlation::{Copula, CopulaSpec, FactorPairIntegrand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const INPUT_FIXTURE: &str = "valuation_b4_tranches.json";
const REFERENCE_FIXTURE: &str = "valuation_b4_tranche_reference.json";
const GAUSSIAN_CAP_BUDGET: f64 = 2e-10;
const STUDENT_CAP_BUDGET: f64 = 1e-12;
const STUDENT_NODE_CAP_BUDGET: f64 = 5e-13;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenInputs {
    schema_version: u32,
    source_revision: String,
    markets: BTreeMap<String, MarketContext>,
    cases: Vec<FrozenCase>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenCase {
    id: String,
    aliases: Vec<String>,
    market_id: String,
    instrument: CdsTranche,
    as_of: Date,
    copula: CopulaSpec,
    correlation_bumps: bool,
    expect_loss: bool,
    expect_writedown: bool,
}

#[derive(Serialize)]
struct Pool {
    ids: Vec<String>,
    weights: Vec<f64>,
    recoveries: Vec<f64>,
    pds: Vec<f64>,
    has_issuer_curves: bool,
}

impl Pool {
    fn at_date(index: &CreditIndexData, date: Date) -> Self {
        let mut result = Self {
            ids: Vec::new(),
            weights: Vec::new(),
            recoveries: Vec::new(),
            pds: Vec::new(),
            has_issuer_curves: index.issuer_credit_curves().is_some(),
        };
        if let Some(curves) = index.issuer_credit_curves() {
            for (id, curve) in curves {
                let weight = index.get_issuer_weight(id);
                if weight > 0.0 {
                    result.ids.push(id.clone());
                    result.weights.push(weight);
                    result.recoveries.push(index.get_issuer_recovery(id));
                    result.pds.push(
                        (1.0 - curve.sp_on_date(date).expect("issuer survival")).clamp(0.0, 1.0),
                    );
                }
            }
        } else {
            let count = usize::from(index.num_constituents());
            let pd = (1.0
                - index
                    .index_credit_curve()
                    .sp_on_date(date)
                    .expect("index survival"))
            .clamp(0.0, 1.0);
            // These label anonymous equal model slots, not real issuer names.
            result.ids = (0..count)
                .map(|i| format!("anonymous_index_slot_{i}"))
                .collect();
            result.weights = vec![1.0 / count as f64; count];
            result.recoveries = vec![index.recovery_rate(); count];
            result.pds = vec![pd; count];
        }
        assert!(!result.weights.is_empty());
        result
    }

    fn equal_exposure(&self) -> bool {
        uniform(&self.weights, 0.0) && uniform(&self.recoveries, 0.0)
    }

    fn baseline_branch(&self) -> &'static str {
        if !self.has_issuer_curves {
            "homogeneous_index_binomial"
        } else if uniform(&self.weights, HOMOGENEITY_TOLERANCE)
            && uniform(&self.recoveries, HOMOGENEITY_TOLERANCE)
            && uniform(&self.pds, HOMOGENEITY_TOLERANCE)
        {
            "issuer_homogeneous_binomial"
        } else if self.exact_states().is_some() {
            "heterogeneous_exact_subsets"
        } else {
            "heterogeneous_fractional_grid"
        }
    }

    fn exact_states(&self) -> Option<usize> {
        u32::try_from(self.weights.len())
            .ok()
            .and_then(|bits| 1usize.checked_shl(bits))
            .filter(|states| *states <= MAX_GRID_POINTS)
    }

    fn grid_points(&self, recovery: bool) -> usize {
        self.weights
            .iter()
            .zip(&self.recoveries)
            .map(|(weight, r)| {
                let exposure = if recovery { *r } else { 1.0 - r };
                (weight * exposure / GRID_STEP).ceil() as usize
            })
            .sum::<usize>()
            + 1
    }
}

fn uniform(values: &[f64], tolerance: f64) -> bool {
    values.first().is_some_and(|first| {
        values
            .iter()
            .all(|value| (value - first).abs() <= tolerance)
    })
}

/// Counts existing conditional-PD calls only in this ignored diagnostic test.
struct CountedCopula {
    inner: Box<dyn Copula>,
    calls: Arc<AtomicUsize>,
}

impl Copula for CountedCopula {
    fn conditional_default_prob(&self, threshold: f64, factors: &[f64], rho: f64) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.conditional_default_prob(threshold, factors, rho)
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        threshold: f64,
        systematic: f64,
        mixing: f64,
        rho: f64,
    ) -> f64 {
        self.inner
            .conditional_default_prob_given_systematic_and_mixing(
                threshold, systematic, mixing, rho,
            )
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        self.inner.integrate_fn(f)
    }

    fn try_integrate_fn(
        &self,
        f: &dyn Fn(&[f64]) -> finstack_quant_core::Result<f64>,
    ) -> finstack_quant_core::Result<f64> {
        self.inner.try_integrate_fn(f)
    }

    fn try_integrate_pair(
        &self,
        f: &FactorPairIntegrand<'_>,
    ) -> finstack_quant_core::Result<(f64, f64)> {
        self.inner.try_integrate_pair(f)
    }

    fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    fn model_name(&self) -> &'static str {
        self.inner.model_name()
    }

    fn tail_dependence(&self, rho: f64) -> f64 {
        self.inner.tail_dependence(rho)
    }
}

/// A test double removes factor integration while exercising the real pool
/// distribution and capping code at supplied conditional probabilities.
/// Its Student-t dispatch tag selects the overridable fixed-rule integrator;
/// it does not represent, calibrate or price a different financial model.
struct ConditionalInputs {
    by_threshold: BTreeMap<u64, f64>,
}

impl Copula for ConditionalInputs {
    fn conditional_default_prob(&self, threshold: f64, _: &[f64], _: f64) -> f64 {
        *self
            .by_threshold
            .get(&threshold.to_bits())
            .expect("frozen conditional probability")
    }

    fn conditional_default_prob_given_systematic_and_mixing(
        &self,
        threshold: f64,
        _: f64,
        _: f64,
        _: f64,
    ) -> f64 {
        self.conditional_default_prob(threshold, &[], 0.0)
    }

    fn integrate_fn(&self, f: &dyn Fn(&[f64]) -> f64) -> f64 {
        f(&[0.0, 1.0])
    }

    fn num_factors(&self) -> usize {
        2
    }

    fn model_name(&self) -> &'static str {
        "test-only fixed conditional inputs"
    }

    fn tail_dependence(&self, _: f64) -> f64 {
        0.0
    }
}

fn conditional_pricer(pool: &Pool, probabilities: &[f64]) -> CdsTranchePricer {
    assert_eq!(pool.pds.len(), probabilities.len());
    let spec = CopulaSpec::student_t(6.0).expect("test dispatch tag");
    let mut by_threshold = BTreeMap::new();
    for (&pd, &probability) in pool.pds.iter().zip(probabilities) {
        let threshold = spec
            .default_threshold(pd.clamp(PROBABILITY_CLIP, 1.0 - PROBABILITY_CLIP))
            .expect("test threshold key");
        if let Some(previous) = by_threshold.insert(threshold.to_bits(), probability) {
            assert_eq!(previous.to_bits(), probability.to_bits());
        }
    }
    CdsTranchePricer {
        copula_spec: spec,
        copula: Arc::new(ConditionalInputs { by_threshold }),
    }
}

#[derive(Clone)]
struct FactorNode {
    factors: Vec<f64>,
    weight: Option<f64>,
}

fn factor_nodes(pricer: &CdsTranchePricer) -> Vec<FactorNode> {
    match pricer.get_copula_spec() {
        CopulaSpec::Gaussian => [-9.0, -6.0, -3.0, -1.0, 0.0, 1.0, 3.0, 6.0, 9.0]
            .into_iter()
            .map(|z| FactorNode {
                factors: vec![z],
                weight: None,
            })
            .collect(),
        CopulaSpec::StudentT { .. } => {
            let collected = RefCell::new(Vec::new());
            pricer
                .copula()
                .try_integrate_fn(&|factors| {
                    collected.borrow_mut().push(factors.to_vec());
                    Ok(0.0)
                })
                .expect("fixed Student-t nodes");
            let factors = collected.into_inner();
            let nodes: Vec<_> = factors
                .into_iter()
                .map(|point| {
                    // Query the unchanged finite rule's basis weight once;
                    // no private model fields or production getters needed.
                    let weight = pricer
                        .copula()
                        .try_integrate_fn(&|candidate| {
                            Ok(if candidate == point.as_slice() {
                                1.0
                            } else {
                                0.0
                            })
                        })
                        .expect("fixed Student-t node weight");
                    assert!(weight.is_finite() && weight >= 0.0);
                    FactorNode {
                        factors: point,
                        weight: Some(weight),
                    }
                })
                .collect();
            let mass: f64 = nodes.iter().map(|node| node.weight.expect("weight")).sum();
            assert!((mass - 1.0).abs() < 1e-8);
            nodes
        }
        other => panic!("B4 frozen tranche cases do not use {other:?}"),
    }
}

struct Cap {
    label: &'static str,
    percent: f64,
    correlation: f64,
    exposure: PoolExposure,
}

fn caps(pricer: &CdsTranchePricer, index: &CreditIndexData, tranche: &CdsTranche) -> [Cap; 4] {
    let effective = pricer.calculate_effective_structure(tranche, index.recovery_rate());
    let attach = pricer.smooth_correlation_boundary(
        index
            .base_correlation_curve()
            .correlation(tranche.attach_pct),
    );
    let detach = pricer.smooth_correlation_boundary(
        index
            .base_correlation_curve()
            .correlation(tranche.detach_pct),
    );
    // The frozen B4 matrices use flat base correlation, including parallel
    // risk bumps. Do not silently treat a future skew as one shared node PMF.
    assert_eq!(attach.to_bits(), detach.to_bits());
    [
        Cap {
            label: "loss_attach",
            percent: effective.eff_attach * 100.0,
            correlation: attach,
            exposure: PoolExposure::Loss,
        },
        Cap {
            label: "recovery_attach",
            percent: (1.0 - effective.eff_attach) * 100.0,
            correlation: attach,
            exposure: PoolExposure::Recovery,
        },
        Cap {
            label: "loss_detach",
            percent: effective.eff_detach * 100.0,
            correlation: detach,
            exposure: PoolExposure::Loss,
        },
        Cap {
            label: "recovery_detach",
            percent: (1.0 - effective.eff_detach) * 100.0,
            correlation: detach,
            exposure: PoolExposure::Recovery,
        },
    ]
}

fn cap_values(
    pricer: &CdsTranchePricer,
    index: &CreditIndexData,
    date: Date,
    caps: &[Cap; 4],
) -> [f64; 4] {
    std::array::from_fn(|i| {
        let cap = &caps[i];
        pricer
            .calculate_equity_tranche_capped(
                cap.percent,
                cap.correlation,
                index,
                date,
                cap.exposure,
            )
            .expect("capped pool expectation")
    })
}

fn gamma(operations: usize) -> f64 {
    let n_epsilon = operations as f64 * f64::EPSILON;
    assert!(n_epsilon < 1.0);
    n_epsilon / (1.0 - n_epsilon)
}

fn scenario_market(
    case: &FrozenCase,
    market: &MarketContext,
    pricer: &CdsTranchePricer,
    bump: f64,
) -> MarketContext {
    if bump == 0.0 {
        return market.clone();
    }
    let original = market
        .get_credit_index(&case.instrument.credit_index_id)
        .expect("index");
    let curve = Arc::new(
        pricer
            .bump_base_correlation(original.base_correlation_curve(), bump)
            .expect("parallel correlation bump"),
    );
    let index = original
        .to_builder()
        .base_correlation_curve(Arc::clone(&curve))
        .build()
        .expect("bumped index");
    market
        .clone()
        .insert(curve)
        .insert_credit_index(&case.instrument.credit_index_id, index)
        .expect("bumped market")
}

fn scenario_record(case: &FrozenCase, market: &MarketContext, name: &str, bump: f64) -> Value {
    let calls = Arc::new(AtomicUsize::new(0));
    let pricer = CdsTranchePricer {
        copula_spec: case.copula.clone(),
        copula: Arc::new(CountedCopula {
            inner: case.copula.build().expect("frozen copula"),
            calls: Arc::clone(&calls),
        }),
    };
    let market = scenario_market(case, market, &pricer, bump);
    let tranche = &case.instrument;
    assert!(
        tranche.upfront.is_none(),
        "frozen B4 tranche inputs have no upfront"
    );
    assert_eq!(
        tranche.realized_loss, 0.0,
        "frozen B4 tranche inputs are unseasoned"
    );
    let projection = pricer
        .prepare_projection_inputs(tranche, &market, case.as_of)
        .expect("actual projected periods");
    let el_wd = projection
        .el_curve(&pricer, projection.index.base_correlation_curve())
        .expect("actual projected EL/WD curve");
    let projection_conditional_pd_calls = calls.swap(0, Ordering::Relaxed);
    let index = projection.index;
    let settlement = projection.valuation_date;
    let periods = projection.periods;
    assert_eq!(periods.len(), el_wd.len());
    assert!(!periods.is_empty());
    assert!(
        periods
            .windows(2)
            .all(|pair| pair[0].accrual_end <= pair[1].accrual_end)
    );
    if case.expect_loss {
        assert!(
            el_wd.iter().any(|point| point.el_fraction > 0.0),
            "{} has no expected loss",
            case.id
        );
    }
    if case.expect_writedown {
        assert!(
            el_wd.iter().any(|point| point.wd_fraction > 0.0),
            "{} has no recovery writedown",
            case.id
        );
    }
    assert!(el_wd.iter().all(|point| point.el_fraction >= 0.0
        && point.wd_fraction >= 0.0
        && point.el_fraction + point.wd_fraction <= 1.0 + 1e-12));
    assert!(
        el_wd
            .windows(2)
            .all(|pair| pair[1].el_fraction >= pair[0].el_fraction - 1e-12
                && pair[1].wd_fraction >= pair[0].wd_fraction - 1e-12)
    );
    let effective = pricer.calculate_effective_structure(tranche, index.recovery_rate());
    let width = (tranche.detach_pct - tranche.attach_pct) / 100.0;
    let cap_budget = if matches!(case.copula, CopulaSpec::StudentT { .. }) {
        STUDENT_CAP_BUDGET
    } else {
        GAUSSIAN_CAP_BUDGET
    };
    let period_error_bound = 2.0 * cap_budget * effective.pool_factor / width;
    if !case.expect_loss {
        assert!(
            el_wd
                .iter()
                .all(|point| point.el_fraction.abs() <= period_error_bound)
        );
    }
    if !case.expect_writedown {
        assert!(
            el_wd
                .iter()
                .all(|point| point.wd_fraction.abs() <= period_error_bound)
        );
    }
    let discount = market
        .get_discount(&tranche.discount_curve_id)
        .expect("discount curve");
    let coupon = tranche.coupon_bp / 10_000.0;
    let notional = tranche.notional.amount();
    let definitions = caps(&pricer, &index, tranche);
    let nodes = factor_nodes(&pricer);
    let student_operations = 4 * nodes.len() + 2;
    let student_accumulation_allowance = if matches!(case.copula, CopulaSpec::StudentT { .. }) {
        let mass: f64 = nodes
            .iter()
            .map(|node| node.weight.expect("Student-t weight"))
            .sum();
        let max_cap = definitions
            .iter()
            .map(|cap| cap.percent / 100.0)
            .fold(0.0_f64, f64::max);
        // Covers nested positive sums, products, normalization and converting
        // the queried basis weights to a flattened independent reference.
        let allowance = gamma(student_operations) * max_cap * mass;
        assert!(STUDENT_NODE_CAP_BUDGET * mass + allowance < STUDENT_CAP_BUDGET);
        Some(allowance)
    } else {
        None
    };
    let mut coefficients = 0.0;
    let mut absolute_cashflow_scale = 0.0;
    let records: Vec<_> = periods.iter().zip(&el_wd).map(|(period, point)| {
        assert!(period.accrual_end > case.as_of, "frozen B4 periods must be prospective");
        let protection_start = period.accrual_start.max(case.as_of).min(period.accrual_end);
        let prior_time = if protection_start <= case.as_of { 0.0 } else { pricer.years_from_base(&index, protection_start).expect("prior time") };
        let end_time = pricer.years_from_base(&index, period.accrual_end).expect("end time");
        let default_fraction = pricer.within_period_default_fraction(&index, prior_time, end_time);
        let remaining_accrual = tranche.day_count.year_fraction(protection_start, period.accrual_end, DayCountContext::default()).expect("remaining accrual");
        let days = (period.accrual_end - protection_start).whole_days().max(0);
        let default_date = protection_start.checked_add(time::Duration::days((default_fraction * days as f64).round() as i64)).unwrap_or(period.accrual_end).clamp(case.as_of, period.accrual_end);
        let dpay = discount.df_between_dates(case.as_of, period.payment_date).expect("payment DF");
        let ddef = discount.df_between_dates(case.as_of, default_date).expect("default DF");
        coefficients += 2.0 * ddef + coupon.abs() * dpay * (2.0 * period.accrual_year_fraction + 4.0 * (1.0 - default_fraction) * remaining_accrual);
        absolute_cashflow_scale += ddef + coupon.abs() * dpay * (period.accrual_year_fraction + 2.0 * remaining_accrual);
        let pool = Pool::at_date(&index, period.accrual_end);
        calls.store(0, Ordering::Relaxed);
        let paired_caps = pricer.paired_capped_expectations_for_test(tranche, &index, period.accrual_end).expect("actual projection caps");
        let paired_conditional_pd_calls = calls.swap(0, Ordering::Relaxed);
        let raw_caps: Vec<_> = definitions.iter().enumerate().map(|(i, cap)| {
            calls.store(0, Ordering::Relaxed);
            let scalar_actual = pricer.calculate_equity_tranche_capped(cap.percent, cap.correlation, &index, period.accrual_end, cap.exposure).expect("scalar raw capped expectation");
            let pd_calls = calls.swap(0, Ordering::Relaxed);
            let pd_calls_per_node = if pool.baseline_branch().contains("binomial") { 1 } else { pool.weights.len() };
            assert_eq!(pd_calls % pd_calls_per_node, 0);
            json!({
                "label": cap.label, "cap": cap.percent / 100.0, "cap_pct": cap.percent,
                "correlation_input": cap.correlation,
                "correlation_effective": pricer.smooth_correlation_boundary(cap.correlation),
                "exposure": if cap.exposure == PoolExposure::Loss { "loss" } else { "recovery" },
                "actual": paired_caps[i], "scalar_actual": scalar_actual,
                "counted_operation": "scalar_cap", "conditional_pd_calls": pd_calls,
                "factor_integrand_evaluations": pd_calls / pd_calls_per_node,
            })
        }).collect();
        let node_records: Vec<_> = nodes.iter().map(|node| {
            let probabilities: Vec<_> = pool.pds.iter().map(|pd| pricer.conditional_default_prob_copula(pricer.copula(), pricer.default_threshold_for_copula(*pd), &node.factors, pricer.smooth_correlation_boundary(definitions[0].correlation))).collect();
            let isolated = conditional_pricer(&pool, &probabilities);
            let actual_caps = cap_values(&isolated, &index, period.accrual_end, &definitions);
            let conditioning_hash = content_hash(&json!({
                "probability_bits": probabilities.iter().map(|p| p.to_bits()).collect::<Vec<_>>(),
                "factor_bits": node.factors.iter().map(|f| f.to_bits()).collect::<Vec<_>>(),
                "weight_bits": node.weight.map(f64::to_bits),
            })).expect("conditioning hash");
            json!({"factors": node.factors, "weight": node.weight, "conditioning_hash": conditioning_hash, "conditional_probabilities": probabilities, "actual_caps": actual_caps})
        }).collect();
        let loss_points = pool.grid_points(false);
        let recovery_points = pool.grid_points(true);
        json!({
            "accrual_start": period.accrual_start, "accrual_end": period.accrual_end,
            "payment_date": period.payment_date, "known": false,
            "protection_start": protection_start, "default_date": default_date,
            "accrual": period.accrual_year_fraction, "remaining_accrual": remaining_accrual,
            "default_fraction": default_fraction, "discount_payment": dpay, "discount_default": ddef,
            "el": point.el_fraction, "wd": point.wd_fraction,
            "pool": pool, "branch": pool.baseline_branch(),
            "branch_basis": "B0 input-selection rule; not a candidate dispatch claim",
            "raw_cap_source": "actual_projection_pair",
            "paired_conditional_pd_calls": paired_conditional_pd_calls,
            "exact_equal_exposure": pool.equal_exposure(),
            "work_dimensions": {
                "positive_names": pool.weights.len(), "exact_subset_states": pool.exact_states(),
                "grid_step_pool_fraction": GRID_STEP,
                "loss_grid_points": loss_points, "recovery_grid_points": recovery_points,
                "loss_grid_visits_bound": pool.weights.len() * loss_points,
                "recovery_grid_visits_bound": pool.weights.len() * recovery_points,
            },
            "raw_caps": raw_caps, "nodes": node_records,
        })
    }).collect();
    // A conservative fixed arithmetic ledger for both production and the
    // reference cashflow reconstruction: at most 128 basic f64 operations
    // per period plus 128 setup operations, applied to a bounded absolute
    // cashflow scale. Curves/timing are unchanged inputs, not recomputed math.
    let operations = 128 * periods.len() + 128;
    let pv_roundoff_allowance = gamma(operations) * notional * absolute_cashflow_scale;
    let pv_error_bound = notional * period_error_bound * coefficients + pv_roundoff_allowance;
    let actual_pv = pricer
        .price_tranche(tranche, &market, case.as_of)
        .expect("diagnostic PV")
        .amount();
    json!({
        "name": name, "correlation_bump": bump, "settlement": settlement,
        "actual_pv": actual_pv, "cap_error_bound": cap_budget,
        "period_error_bound": period_error_bound, "pv_error_bound": pv_error_bound,
        "pv_roundoff_allowance": pv_roundoff_allowance, "roundoff_operation_budget": operations,
        "student_accumulation_allowance": student_accumulation_allowance,
        "student_accumulation_operation_budget": if student_accumulation_allowance.is_some() { Some(student_operations) } else { None },
        "pv_coefficient_sum": coefficients, "absolute_cashflow_scale": absolute_cashflow_scale,
        "prior_el": pricer.calculate_prior_tranche_loss(tranche),
        "prior_wd": pricer.calculate_prior_tranche_writedown(tranche, index.recovery_rate()),
        "notional": notional, "coupon_decimal": coupon,
        "pool_factor": effective.pool_factor, "original_tranche_width": width,
        "premium_sign": tranche.side.sign(), "protection_sign": -tranche.side.sign(),
        "projection_conditional_pd_calls": projection_conditional_pd_calls,
        "periods": records,
    })
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn expected_aliases() -> BTreeSet<String> {
    let mut expected: BTreeSet<_> = [
        "cds_tranche_npv/equity_0_3",
        "cds_tranche_npv/junior_mezz_3_7",
        "cds_tranche_npv/senior_mezz_7_10",
        "cds_tranche_npv/senior_10_15",
        "cds_tranche_cs01/cs01",
        "cds_tranche_correlation_delta/correlation_delta",
        "cds_tranche_jump_to_default/jump_to_default",
        "cds_tranche_par_spread/par_spread",
        "cds_tranche_all_metrics/all_metrics",
        "cds_tranche_student_t_npv/junior_mezz_3_7_df6",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for names in [10, 16, 17, 18, 50, 125] {
        expected.insert(format!("cds_tranche_heterogeneous/npv_hetero/{names}"));
    }
    for names in [18, 125] {
        expected.insert(format!(
            "cds_tranche_heterogeneous/correlation_delta/{names}"
        ));
    }
    for name in [
        "gaussian_heterogeneous_four_3_7",
        "gaussian_concentrated_50_3_7",
        "gaussian_recovery_dispersion_50_3_7",
        "gaussian_equal_exposure_125_40_80",
        "gaussian_homogeneous_125_40_80",
        "gaussian_homogeneous_125_60_100",
        "student_t_equal_exposure_50_3_7_df6",
    ] {
        expected.insert(format!("cds_tranche_p5_controls/{name}"));
    }
    expected
}

fn read_inputs() -> FrozenInputs {
    let bytes = std::fs::read(fixture_path(INPUT_FIXTURE))
        .expect("root must first freeze the B4 captured input fixture");
    let inputs: FrozenInputs = serde_json::from_slice(&bytes).expect("frozen B4 inputs");
    assert_eq!(inputs.schema_version, 1);
    let aliases: BTreeSet<_> = inputs
        .cases
        .iter()
        .flat_map(|case| case.aliases.iter().cloned())
        .collect();
    assert_eq!(
        aliases,
        expected_aliases(),
        "all 25 frozen case IDs are required"
    );
    assert_eq!(
        inputs
            .cases
            .iter()
            .map(|case| case.aliases.len())
            .sum::<usize>(),
        25,
        "case aliases must not overlap"
    );
    let ids: BTreeSet<_> = inputs.cases.iter().map(|case| &case.id).collect();
    assert_eq!(ids.len(), inputs.cases.len());
    assert!(inputs.cases.iter().all(|case| !case.aliases.is_empty()));
    for case in &inputs.cases {
        let needs_bumps = case.aliases.iter().any(|id| {
            id.contains("/correlation_delta") || id == "cds_tranche_all_metrics/all_metrics"
        });
        assert_eq!(
            case.correlation_bumps, needs_bumps,
            "risk scenario coverage changed"
        );
        // These are assertions about the specific frozen matrix, not a new
        // product-wide rule for loss/recovery exposure.
        assert_eq!(case.expect_loss, case.instrument.attach_pct < 60.0);
        assert_eq!(case.expect_writedown, case.instrument.detach_pct > 60.0);
    }
    inputs
}

fn diagnostics(inputs: &FrozenInputs) -> Value {
    let records: Vec<_> = inputs.cases.iter().map(|case| {
        let market = inputs.markets.get(&case.market_id).expect("frozen market ID");
        let mut scenarios = vec![scenario_record(case, market, "base", 0.0)];
        if case.correlation_bumps {
            scenarios.push(scenario_record(case, market, "correlation_up", 0.01));
            scenarios.push(scenario_record(case, market, "correlation_down", -0.01));
        }
        json!({
            "id": case.id, "aliases": case.aliases, "copula": case.copula,
            "fixture_hash": content_hash(&json!({"instrument": case.instrument, "market": market, "as_of": case.as_of, "copula": case.copula})).expect("fixture hash"),
            "scenarios": scenarios,
        })
    }).collect();
    json!({"schema_version": 1, "input_hash": content_hash(inputs).expect("input hash"), "cases": records})
}

fn array(value: &Value) -> &[Value] {
    value.as_array().expect("reference array")
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .expect("finite reference number")
}

fn write_checked_evidence(diagnostics: Value) {
    let mut evidence = json!({
        "evidence_schema_version": 1,
        "candidate_policy": "P5-combined",
        "numerical_checks_passed": true,
        "policy_source": "Invoked canonical test cds_tranche::pricing::performance_diagnostics::tranche_check_exact_reference; archived source and binary hashes establish the implementation, not an observed dispatch trace.",
        "field_conventions": {
            "branch": "B0 input-selection classification, as stated by each period's branch_basis; not observed candidate dispatch.",
            "work_dimensions": "Input-derived subset/grid capacities and bounds; not executed allocations or state visits.",
            "conditional_pd_calls": "Measured wrapper calls for the separately named scalar-cap, paired-cap or projection diagnostic invocation; not total pricing work.",
            "factor_integrand_evaluations": "Derived scalar-cap evaluations from measured PD calls divided by one or the positive-name count; not paired-node or PMF-work counts.",
            "operation_budgets": "Numerical error-budget assumptions, not measured work.",
        },
    });
    evidence["diagnostics"] = diagnostics;
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/valuation-benchmarks")
        .join("current");
    std::fs::create_dir_all(&directory).expect("current evidence directory");
    let path = directory.join("tranche-checked-diagnostics.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&evidence).expect("checked evidence JSON"),
    )
    .expect("checked current evidence export");
    eprintln!("P5 checked tranche diagnostics: {}", path.display());
}

#[test]
#[ignore = "offline numerical regression against sealed independent tranche references"]
fn tranche_check_exact_reference() {
    let reference: Value = serde_json::from_slice(
        &std::fs::read(fixture_path(REFERENCE_FIXTURE))
            .expect("root must first seal independent reference pins"),
    )
    .expect("reference fixture");
    let actual = diagnostics(&read_inputs());
    assert_eq!(reference["schema_version"], json!(1));
    assert_eq!(actual["input_hash"], reference["input_hash"]);
    assert_eq!(
        array(&actual["cases"]).len(),
        array(&reference["cases"]).len()
    );
    for (case, pin) in array(&actual["cases"])
        .iter()
        .zip(array(&reference["cases"]))
    {
        assert_eq!(case["id"], pin["id"]);
        assert_eq!(case["fixture_hash"], pin["fixture_hash"]);
        assert_eq!(case["aliases"], pin["aliases"]);
        let student = case["copula"]["type"] == json!("student_t");
        assert_eq!(
            array(&case["scenarios"]).len(),
            array(&pin["scenarios"]).len()
        );
        let mut strict_delta = true;
        for (scenario, expected) in array(&case["scenarios"])
            .iter()
            .zip(array(&pin["scenarios"]))
        {
            assert_eq!(scenario["name"], expected["name"]);
            for key in [
                "period_error_bound",
                "pv_error_bound",
                "pv_roundoff_allowance",
                "student_accumulation_allowance",
                "student_accumulation_operation_budget",
            ] {
                assert_eq!(
                    scenario[key], expected[key],
                    "sealed numerical gate {key} changed"
                );
            }
            assert_eq!(
                array(&scenario["periods"]).len(),
                array(&expected["periods"]).len()
            );
            let mut strict_pv = true;
            let mut unchanged_pv = true;
            for (period, reference_period) in array(&scenario["periods"])
                .iter()
                .zip(array(&expected["periods"]))
            {
                for key in ["accrual_start", "accrual_end", "payment_date"] {
                    assert_eq!(period[key], reference_period[key]);
                }
                let binomial = matches!(
                    period["branch"].as_str(),
                    Some("homogeneous_index_binomial" | "issuer_homogeneous_binomial")
                );
                let strict = binomial
                    || period["branch"] == json!("heterogeneous_exact_subsets")
                    || period["exact_equal_exposure"] == json!(true);
                strict_pv &= strict;
                let unchanged_count_kernel =
                    period["exact_equal_exposure"] != json!(true) || binomial;
                // Gaussian index pools use paired integration; issuer pools
                // retain their two scalar integration calls.
                let affected_pair = case["copula"]["type"] == json!("gaussian")
                    && period["branch"] == json!("homogeneous_index_binomial");
                let unchanged_projection = unchanged_count_kernel && !affected_pair;
                unchanged_pv &= unchanged_projection;
                assert_eq!(period["raw_cap_source"], json!("actual_projection_pair"));
                assert_eq!(
                    scenario["cap_error_bound"], reference_period["candidate_raw_cap_tolerance"],
                    "sealed capped-expectation tolerance changed"
                );
                assert_eq!(array(&period["raw_caps"]).len(), 4);
                assert_eq!(array(&reference_period["raw_caps"]).len(), 4);
                for (cap, cap_pin) in array(&period["raw_caps"])
                    .iter()
                    .zip(array(&reference_period["raw_caps"]))
                {
                    let value = number(&cap["actual"]);
                    if strict {
                        assert_eq!(reference_period["exact_reference_available"], json!(true));
                        let uncertainty = number(&cap_pin["uncertainty"]);
                        for (operation, amount) in [
                            ("projection pair", value),
                            ("scalar", number(&cap["scalar_actual"])),
                        ] {
                            let error = (amount - number(&cap_pin["expected"])).abs() + uncertainty;
                            assert!(
                                error < number(&reference_period["candidate_raw_cap_tolerance"]),
                                "{} {} {} {operation} raw-cap error {error:e}",
                                case["id"],
                                scenario["name"],
                                cap["label"]
                            );
                        }
                    }
                    if unchanged_projection {
                        assert_eq!(
                            format!("{:016x}", value.to_bits()),
                            cap_pin["baseline_bits"]
                                .as_str()
                                .expect("baseline cap bits")
                        );
                    }
                }
                assert_eq!(
                    array(&period["nodes"]).len(),
                    array(&reference_period["conditional"]).len()
                );
                assert_eq!(
                    reference_period["conditional_node_policy"],
                    json!(if student {
                        "student_fixed_rule_arithmetic"
                    } else {
                        "gaussian_diagnostic_probes"
                    })
                );
                for (node, node_pin) in array(&period["nodes"])
                    .iter()
                    .zip(array(&reference_period["conditional"]))
                {
                    assert_eq!(node["conditioning_hash"], node_pin["conditioning_hash"]);
                    assert_eq!(array(&node["actual_caps"]).len(), 4);
                    if strict {
                        if student {
                            assert_eq!(
                                number(&node_pin["candidate_node_tolerance"]),
                                STUDENT_NODE_CAP_BUDGET,
                                "sealed Student-t conditional-cap tolerance changed"
                            );
                        } else {
                            assert!(node_pin.get("candidate_node_tolerance").is_none());
                        }
                        for i in 0..4 {
                            let uncertainty = number(&node_pin["uncertainties"][i]);
                            assert!(uncertainty <= 1e-13);
                            let error = (number(&node["actual_caps"][i])
                                - number(&node_pin["expected_caps"][i]))
                            .abs()
                                + uncertainty;
                            if student {
                                assert!(
                                    error < STUDENT_NODE_CAP_BUDGET,
                                    "Student-t conditional-cap error {error:e}"
                                );
                            }
                        }
                    }
                    if unchanged_count_kernel {
                        for i in 0..4 {
                            assert_eq!(
                                format!("{:016x}", number(&node["actual_caps"][i]).to_bits()),
                                node_pin["baseline_caps_bits"][i]
                                    .as_str()
                                    .expect("baseline conditional bits")
                            );
                        }
                    }
                }
                if strict {
                    for (actual_key, expected_key) in [("el", "expected_el"), ("wd", "expected_wd")]
                    {
                        assert!(
                            (number(&period[actual_key]) - number(&reference_period[expected_key]))
                                .abs()
                                + number(&reference_period["component_reference_uncertainty"])
                                <= number(&expected["period_error_bound"])
                        );
                    }
                }
                if unchanged_projection {
                    for (actual_key, baseline_key) in
                        [("el", "baseline_el_bits"), ("wd", "baseline_wd_bits")]
                    {
                        assert_eq!(
                            format!("{:016x}", number(&period[actual_key]).to_bits()),
                            reference_period[baseline_key]
                                .as_str()
                                .expect("baseline period bits")
                        );
                    }
                }
            }
            strict_delta &= strict_pv;
            if strict_pv {
                let error = (number(&scenario["actual_pv"]) - number(&expected["expected_pv"]))
                    .abs()
                    + number(&expected["reference_pv_uncertainty"]);
                assert!(
                    error <= number(&expected["pv_error_bound"]),
                    "{} {} PV error {error:e}",
                    case["id"],
                    scenario["name"]
                );
            }
            if unchanged_pv {
                assert_eq!(
                    format!("{:016x}", number(&scenario["actual_pv"]).to_bits()),
                    expected["baseline_pv_bits"]
                        .as_str()
                        .expect("baseline PV bits")
                );
            }
        }
        if array(&case["scenarios"]).len() == 3 && strict_delta {
            let scenarios = array(&case["scenarios"]);
            let pins = array(&pin["scenarios"]);
            let actual_delta =
                (number(&scenarios[1]["actual_pv"]) - number(&scenarios[2]["actual_pv"])) / 2.0;
            let expected_delta =
                (number(&pins[1]["expected_pv"]) - number(&pins[2]["expected_pv"])) / 2.0;
            let bound =
                (number(&pins[1]["pv_error_bound"]) + number(&pins[2]["pv_error_bound"])) / 2.0;
            let uncertainty = (number(&pins[1]["reference_pv_uncertainty"])
                + number(&pins[2]["reference_pv_uncertainty"]))
                / 2.0;
            assert!((actual_delta - expected_delta).abs() + uncertainty <= bound);
        }
    }
    write_checked_evidence(actual);
}
