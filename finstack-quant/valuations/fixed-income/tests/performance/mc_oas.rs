//! Full-size, fixed-seed numerical records for the MBS OAS performance work.
//!
//! These ignored tests intentionally retain the pre-aggregation path formula.
//! They are numerical diagnostics, never latency or peak-memory measurements.

use super::*;
use crate::mbs_passthrough::{AgencyProgram, PoolType};
use finstack_quant_cashflows::builder::PrepaymentModelSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DateExt;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::stats::{OnlineCovariance, OnlineStats};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_test_support::discount_forward_curves;
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use time::macros::date;

const SPREADS: [f64; 8] = [-0.10, -0.05, -0.01, 0.0, 0.01, 0.05, 0.10, 0.20];
const QUOTE_PCT: f64 = 95.0;

struct Fixture {
    case: &'static str,
    pool: AgencyMbsPassthrough,
    market: MarketContext,
    as_of: Date,
}

fn fixture(index: usize) -> Fixture {
    if index == 0 {
        let as_of = date!(2025 - 01 - 01);
        let mut pool = AgencyMbsPassthrough::example().expect("original MBS fixture");
        pool.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(QUOTE_PCT);
        return Fixture {
            case: "agency_mbs_metrics/oas",
            pool,
            market: MarketContext::new().insert(discount_forward_curves::flat_discount(
                "USD-OIS", as_of, 0.05,
            )),
            as_of,
        };
    }
    let (case, paths, months, agency, delay, act360, seasoned) = [
        (
            "agency_mbs_oas_scaling/paths_512_months_360",
            512,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/paths_2048_months_360",
            2048,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/paths_8192_months_360",
            8192,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/paths_2048_months_60",
            2048,
            60,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/paths_2048_months_180",
            2048,
            180,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/gnma_i_paths_2048_months_360",
            2048,
            360,
            AgencyProgram::GnmaI,
            None,
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/delay_15_paths_2048_months_360",
            2048,
            360,
            AgencyProgram::Fnma,
            Some(15),
            false,
            false,
        ),
        (
            "agency_mbs_oas_scaling/fresh_act360_paths_2048_months_300",
            2048,
            300,
            AgencyProgram::Fnma,
            None,
            true,
            false,
        ),
        (
            "agency_mbs_oas_scaling/seasoned_act360_paths_2048_months_300",
            2048,
            300,
            AgencyProgram::Fnma,
            None,
            true,
            true,
        ),
    ][index - 1];
    let as_of = if act360 {
        date!(2026 - 01 - 15)
    } else {
        date!(2025 - 01 - 01)
    };
    let accrual_start = as_of.replace_day(1).expect("accrual month");
    let issue = if seasoned {
        date!(2021 - 01 - 01)
    } else {
        accrual_start
    };
    let mut pool = AgencyMbsPassthrough::builder()
        .id(InstrumentId::new("MBS-OAS-B4"))
        .pool_id("MBS-OAS-B4-POOL".into())
        .agency(agency)
        .pool_type(PoolType::Generic)
        .original_face(Money::from((1_000_000_i64, Currency::USD)))
        .current_face(Money::from((
            if act360 { 850_000_i64 } else { 1_000_000_i64 },
            Currency::USD,
        )))
        .wac(0.045)
        .coupon(0.04)
        .servicing_fee_bp(25.0)
        .guarantee_fee_bp(25.0)
        .wam_months(months)
        .issue_date(issue)
        .maturity(
            accrual_start
                .add_months(i32::try_from(months).expect("bounded WAM"))
                .expect("contractual maturity"),
        )
        .stated_delay_days_opt(delay)
        .last_paid_accrual_end_opt(None)
        .prepayment_spec(PrepaymentModelSpec::psa(1.0))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(if act360 {
            DayCount::Act360
        } else {
            DayCount::Thirty360
        })
        .build()
        .expect("B4 MBS fixture");
    pool.instrument_pricing_overrides
        .market_quotes
        .quoted_clean_price_pct = Some(QUOTE_PCT);
    let model = &mut pool.instrument_pricing_overrides.model_config;
    model.mc_paths = Some(paths);
    model.hw1f_mean_reversion = Some(0.05);
    model.hw1f_sigma = Some(0.01);
    model.mc_seed_scenario = Some("base".into());
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .knots([(0.0, 1.0), (40.0, (-0.05_f64 * 40.0).exp())])
        .build()
        .expect("B4 flat discount curve");
    Fixture {
        case,
        pool,
        market: MarketContext::new().insert(curve),
        as_of,
    }
}

/// Pre-optimization direct path oracle: reproject at every trial spread.
/// Keep its discount multiplication order independent of coefficient reduction.
fn direct_path_pv(
    pool: &AgencyMbsPassthrough,
    path: &RatePath,
    base_rate: f64,
    spread: f64,
    steps: &McStepSchedule,
) -> f64 {
    let dt = 1.0 / 12.0;
    let mut balance = pool.current_face.amount();
    let mut pv = 0.0;
    let mut cumulative_df = 1.0;
    let wam = pool.wam_months as usize;
    let count = path.rates.len().saturating_sub(1).min(wam).min(steps.len());
    for month in 0..count {
        if balance < 0.01 {
            break;
        }
        let rate = path.rates[month];
        cumulative_df *= (-(rate + spread) * dt).exp();
        let smm = rate_adjusted_smm(
            steps.base_smms[month],
            rate,
            base_rate,
            super::super::PREPAY_RATE_SENSITIVITY,
        );
        let step = pool_month_step(
            balance,
            wam.saturating_sub(month) as u32,
            pool.wac,
            smm,
            pool.coupon,
            steps.accrual_fractions[month],
        );
        let cashflow = step.scheduled_principal + step.prepayment + step.interest;
        pv += cashflow * cumulative_df * (-(rate + spread) * steps.payment_extras[month]).exp();
        balance = step.ending_balance;
    }
    pv
}

/// The actual production preparation/evaluation route with one-path normalization.
/// Kept distinct from the direct oracle and the test-only factorization.
fn current_path_pv(
    pool: &AgencyMbsPassthrough,
    path: &RatePath,
    base_rate: f64,
    spread: f64,
    steps: &McStepSchedule,
) -> f64 {
    let coefficients = mean_zero_oas_cashflows(
        pool,
        std::slice::from_ref(path),
        base_rate,
        super::super::PREPAY_RATE_SENSITIVITY,
        steps,
    );
    let payment_times = oas_payment_times(steps);
    spread_value(&coefficients, &payment_times, spread)
}

fn path_pvs(
    pool: &AgencyMbsPassthrough,
    paths: &[RatePath],
    base_rate: f64,
    spread: f64,
    steps: &McStepSchedule,
    direct: bool,
) -> Vec<f64> {
    let price = if direct {
        direct_path_pv
    } else {
        current_path_pv
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| price(pool, path, base_rate, spread, steps))
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    paths
        .iter()
        .map(|path| price(pool, path, base_rate, spread, steps))
        .collect()
}

fn direct_mean(
    pool: &AgencyMbsPassthrough,
    paths: &[RatePath],
    rate: f64,
    spread: f64,
    steps: &McStepSchedule,
) -> f64 {
    path_pvs(pool, paths, rate, spread, steps, true)
        .iter()
        .sum::<f64>()
        / paths.len() as f64
}

/// One-path test-only factorization; no paths-by-months cashflow matrix is kept.
fn zero_spread_cashflows(
    pool: &AgencyMbsPassthrough,
    path: &RatePath,
    rate: f64,
    steps: &McStepSchedule,
) -> (Vec<f64>, usize) {
    let mut cashflows = vec![0.0; steps.len()];
    let mut active_months = 0;
    let mut balance = pool.current_face.amount();
    let mut df = 1.0;
    let wam = pool.wam_months as usize;
    for (month, discounted_cashflow) in cashflows
        .iter_mut()
        .enumerate()
        .take(path.rates.len().saturating_sub(1).min(wam))
    {
        if balance < 0.01 {
            break;
        }
        let current_rate = path.rates[month];
        active_months += 1;
        df *= (-current_rate * (1.0 / 12.0)).exp();
        let smm = rate_adjusted_smm(
            steps.base_smms[month],
            current_rate,
            rate,
            super::super::PREPAY_RATE_SENSITIVITY,
        );
        let step = pool_month_step(
            balance,
            wam.saturating_sub(month) as u32,
            pool.wac,
            smm,
            pool.coupon,
            steps.accrual_fractions[month],
        );
        let cf = step.scheduled_principal + step.prepayment + step.interest;
        assert!(cf.is_finite() && cf >= 0.0);
        *discounted_cashflow = cf * df * (-current_rate * steps.payment_extras[month]).exp();
        balance = step.ending_balance;
    }
    (cashflows, active_months)
}

fn factorized_value(coefficients: &[f64], taus: &[f64], spread: f64) -> f64 {
    coefficients
        .iter()
        .zip(taus)
        .map(|(&cf, &tau)| cf * (-spread * tau).exp())
        .sum()
}

fn paired_record(
    reference: &[f64],
    candidate: &[f64],
    reference_value: f64,
    candidate_value: f64,
) -> Value {
    assert_eq!(reference.len(), candidate.len());
    let mut pairs = OnlineCovariance::new();
    let mut differences = OnlineStats::new();
    let mut max_abs_difference = 0.0_f64;
    for (&x, &y) in reference.iter().zip(candidate) {
        assert!(x.is_finite() && y.is_finite());
        pairs.update(x, y);
        differences.update(y - x);
        max_abs_difference = max_abs_difference.max((y - x).abs());
    }
    let n = reference.len();
    assert!(n >= 2);
    assert_eq!(pairs.count(), n);
    assert_eq!(differences.count(), n);
    let vx = pairs.variance_x();
    let vy = pairs.variance_y();
    let vd = differences.variance();
    assert!([vx, vy, vd].iter().all(|v| v.is_finite() && *v >= 0.0));
    assert!(pairs.covariance().is_finite());
    json!({
        "physical_paths": n, "independent_estimators": n, "paired_observations": n,
        "mean_reference": pairs.mean_x(), "mean_candidate_path": pairs.mean_y(),
        "mean_difference": differences.mean(), "variance_reference": vx,
        "variance_candidate": vy, "covariance": pairs.covariance(), "variance_difference": vd,
        "se_reference": (vx / n as f64).sqrt(), "se_candidate": (vy / n as f64).sqrt(),
        "se_difference": (vd / n as f64).sqrt(), "max_abs_path_difference": max_abs_difference,
        "reference_reduction_residual": reference_value - pairs.mean_x(),
        "candidate_reduction_residual": candidate_value - pairs.mean_y(),
    })
}

fn solver() -> BrentSolver {
    BrentSolver::new()
        .with_tolerance(OAS_TOLERANCE)
        .with_max_iterations(200)
        .with_bracket_bounds(-0.10, 0.20)
        .with_initial_bracket_size(Some(0.05))
}

fn run_case(index: usize, scenario: &str) {
    let mut fixture = fixture(index);
    if scenario != "base" {
        fixture
            .pool
            .instrument_pricing_overrides
            .model_config
            .mc_seed_scenario = Some(scenario.into());
    }
    let config = McOasConfig::resolve(&fixture.pool).expect("MC configuration");
    let pool = quote_basis_pool(&fixture.pool, fixture.as_of).expect("quote-basis pool");
    let target = QUOTE_PCT / 100.0 * pool.current_face.amount()
        + settlement_accrued_interest(&pool, fixture.as_of).expect("settlement accrued interest");
    let discount = fixture
        .market
        .get_discount(&pool.discount_curve_id)
        .expect("discount curve");
    let months = pool.wam_months as usize;
    let steps = mc_step_schedule(&pool, fixture.as_of, months, discount.day_count())
        .expect("monthly schedule");
    let rate = initial_short_rate_from_curve(discount.as_ref(), fixture.as_of)
        .expect("initial short rate");
    let hw = prepare_hw1f_params(
        HullWhiteCalibrationParams::new(config.hw1f_mean_reversion, config.hw1f_sigma)
            .expect("HW parameters"),
        discount.as_ref(),
        fixture.as_of,
        months as f64 / 12.0,
    )
    .expect("curve-consistent HW model");
    let paths = simulate_rate_paths(rate, &hw, config.num_paths, months, config.seed);
    assert_eq!(paths.len(), config.num_paths);
    let taus: Vec<f64> = steps
        .payment_extras
        .iter()
        .enumerate()
        .map(|(m, extra)| (m as f64 + 1.0) * (1.0 / 12.0) + extra)
        .collect();
    assert!(taus.iter().all(|t| t.is_finite() && *t > 0.0));
    let production_times = oas_payment_times(&steps);
    assert_eq!(
        production_times
            .iter()
            .map(|t| t.to_bits())
            .collect::<Vec<_>>(),
        taus.iter().map(|t| t.to_bits()).collect::<Vec<_>>(),
        "production spread times must match the independent reference"
    );
    // Prepare all paths through the real production reduction once. Averaging
    // one-path candidate PVs would omit the block/month reduction's rounding.
    let production_coefficients = mean_zero_oas_cashflows(
        &pool,
        &paths,
        rate,
        super::super::PREPAY_RATE_SENSITIVITY,
        &steps,
    );
    assert_eq!(production_coefficients.len(), production_times.len());
    assert!(
        production_coefficients
            .iter()
            .all(|cf| cf.is_finite() && *cf >= 0.0)
    );
    let tau_max = taus.iter().copied().fold(0.0_f64, f64::max);
    let mut mean_coefficients = vec![0.0; months];
    let mut prototype_path_values = Vec::with_capacity(paths.len());
    let mut active_path_months = 0_usize;
    let mut slope_lower_bound = 0.0;
    let mut path_bits = DefaultHasher::new();
    for path in &paths {
        assert_eq!(path.rates.len(), months + 1);
        for value in &path.rates {
            assert!(value.is_finite());
            value.to_bits().hash(&mut path_bits);
        }
        let (coefficients, active_months) = zero_spread_cashflows(&pool, path, rate, &steps);
        active_path_months += active_months;
        for (aggregate, &cf) in mean_coefficients.iter_mut().zip(&coefficients) {
            *aggregate += cf;
        }
        // Computed from each direct path's monthly terms, never from mean coefficients.
        slope_lower_bound += coefficients
            .iter()
            .zip(&taus)
            .map(|(&cf, &tau)| tau * cf * (-0.20 * tau).exp())
            .sum::<f64>();
        prototype_path_values
            .push(SPREADS.map(|spread| factorized_value(&coefficients, &taus, spread)));
    }
    for coefficient in &mut mean_coefficients {
        *coefficient /= paths.len() as f64;
    }
    slope_lower_bound /= paths.len() as f64;
    assert!(slope_lower_bound.is_finite() && slope_lower_bound > 0.0);
    let mut points = Vec::new();
    for (column, spread) in SPREADS.into_iter().enumerate() {
        let reference = path_pvs(&pool, &paths, rate, spread, &steps, true);
        let current = path_pvs(&pool, &paths, rate, spread, &steps, false);
        let prototype: Vec<f64> = prototype_path_values
            .iter()
            .map(|row| row[column])
            .collect();
        let ref_pv = reference.iter().sum::<f64>() / paths.len() as f64;
        let current_pv = spread_value(&production_coefficients, &production_times, spread);
        let prototype_pv = factorized_value(&mean_coefficients, &taus, spread);
        let tolerance = 1e-8_f64.max(1e-10 * ref_pv.abs());
        assert!(
            (current_pv - ref_pv).abs() <= tolerance,
            "{} / {scenario} / {spread}: production PV differs",
            fixture.case
        );
        assert!(
            (prototype_pv - ref_pv).abs() <= tolerance,
            "{} / {scenario} / {spread}: factorization exceeds sealed PV bound",
            fixture.case
        );
        points.push(json!({
            "spread": spread, "reference_pv": ref_pv, "production_pv": current_pv,
            "test_factorization_pv": prototype_pv, "pv_tolerance": tolerance,
            "production_pairs": paired_record(&reference, &current, ref_pv, current_pv),
            "test_factorization_pairs": paired_record(&reference, &prototype, ref_pv, prototype_pv),
        }));
    }
    let reference_calls = Cell::new(0_usize);
    let reference_root = solver()
        .solve(
            |spread| {
                reference_calls.set(reference_calls.get() + 1);
                direct_mean(&pool, &paths, rate, spread, &steps) - target
            },
            0.0,
        )
        .expect("direct-reference OAS root");
    assert!(reference_root.is_finite() && (-0.10..=0.20).contains(&reference_root));
    let prototype_calls = Cell::new(0_usize);
    let prototype_root = solver()
        .solve(
            |spread| {
                prototype_calls.set(prototype_calls.get() + 1);
                factorized_value(&mean_coefficients, &taus, spread) - target
            },
            0.0,
        )
        .expect("test-factorization OAS root");
    let production_helper_calls = Cell::new(0_usize);
    let production_helper_root = solver()
        .solve(
            |spread| {
                production_helper_calls.set(production_helper_calls.get() + 1);
                spread_value(&production_coefficients, &production_times, spread) - target
            },
            0.0,
        )
        .expect("production-helper OAS root");
    let production_root = calculate_mc_oas(
        &fixture.pool,
        QUOTE_PCT,
        &fixture.market,
        fixture.as_of,
        &config,
    )
    .expect("production OAS root");
    assert_eq!(
        production_root.to_bits(),
        production_helper_root.to_bits(),
        "{} / {scenario}: end-to-end OAS must use the measured production-helper objective",
        fixture.case
    );
    let eps_global = 1e-8_f64.max(1e-10 * direct_mean(&pool, &paths, rate, -0.10, &steps));
    let root_bound =
        2.0 * OAS_TOLERANCE + 2.0 * eps_global / slope_lower_bound + 16.0 * f64::EPSILON;
    let mut roots = Vec::new();
    for (label, root) in [
        ("production", production_root),
        ("test_factorization", prototype_root),
    ] {
        assert!(root.is_finite() && (-0.10..=0.20).contains(&root));
        assert!(
            (root - reference_root).abs() <= root_bound,
            "{} / {scenario}: {label} root exceeds sealed bound",
            fixture.case
        );
        let repriced = direct_mean(&pool, &paths, rate, root, &steps);
        let residual_bound = OAS_TOLERANCE
            * tau_max
            * direct_mean(&pool, &paths, rate, root - OAS_TOLERANCE, &steps)
            + 2.0 * eps_global
            + 16.0 * f64::EPSILON * target.abs().max(1.0);
        assert!(
            (repriced - target).abs() <= residual_bound,
            "{} / {scenario}: {label} reference residual exceeds sealed bound",
            fixture.case
        );
        roots.push(json!({"implementation": label, "root": root, "reference_residual": repriced - target, "residual_bound": residual_bound}));
    }
    let mut root_neighborhood = Vec::new();
    for spread in [
        reference_root - OAS_TOLERANCE,
        reference_root,
        reference_root + OAS_TOLERANCE,
    ] {
        if !(-0.10..=0.20).contains(&spread) {
            continue;
        }
        let reference = direct_mean(&pool, &paths, rate, spread, &steps);
        let current = spread_value(&production_coefficients, &production_times, spread);
        let prototype = factorized_value(&mean_coefficients, &taus, spread);
        let tolerance = 1e-8_f64.max(1e-10 * reference.abs());
        assert!((current - reference).abs() <= tolerance);
        assert!((prototype - reference).abs() <= tolerance);
        root_neighborhood.push(json!({"spread": spread, "reference_pv": reference, "production_pv": current, "test_factorization_pv": prototype, "pv_tolerance": tolerance}));
    }
    let record = json!({
        "case": fixture.case, "scenario": scenario, "seed": config.seed,
        "fixture": {"instrument": fixture.pool, "market": fixture.market, "as_of": fixture.as_of, "quote_pool": pool, "quote_pct": QUOTE_PCT},
        "grid": {"months": months, "times_bits": (0..=months).map(|m| (m as f64 * (1.0 / 12.0)).to_bits()).collect::<Vec<_>>(), "payment_extras": steps.payment_extras, "accrual_fractions": steps.accrual_fractions, "base_smms": steps.base_smms, "oas_discount_times": taus},
        "physical_paths": config.num_paths, "independent_estimators": config.num_paths,
        "initial_short_rate": rate, "prepared_hw_model": hw,
        "rate_value_count": paths.len() * (months + 1), "same_toolchain_path_bits_hash": path_bits.finish(),
        "active_path_months_per_projection": active_path_months,
        "reference_solver_objective_calls": reference_calls.get(),
        "reference_solver_projected_path_months": reference_calls.get() * active_path_months,
        "test_factorization_projection_path_months": active_path_months,
        "test_factorization_solver_objective_calls": prototype_calls.get(),
        "production_helper_solver_objective_calls": production_helper_calls.get(),
        "production_helper_projection_passes": 1,
        "production_helper_projected_path_months": active_path_months,
        "production_helper_root": production_helper_root,
        "production_objective_calls": null,
        "objective_count_note": "Reference, test prototype and production-helper solves are counted separately. The production-helper projection occurs once; its active path-month count is independently counted by the unchanged reference traversal. The separate end-to-end production solve has no counter and must return the identical root.",
        "target_dirty_pv": target, "reference_root": reference_root, "root_bound_decimal": root_bound,
        "root_bound_bp": root_bound * 10_000.0, "slope_lower_bound": slope_lower_bound,
        "eps_global": eps_global, "roots": roots, "fixed_spread_points": points,
        "reference_root_neighborhood": root_neighborhood,
        "uncertainty_note": "Path PV SE describes pseudo-random sampling; paired difference SE never relaxes the deterministic PV/root bounds. Production path pairs use one-path preparation, while production PV/root checks use the actual full-path aggregate. Test factorization remains a separate independent prototype.",
    });
    println!("FQ_MBS_DIAGNOSTIC {record}");
}

/// Exercise the production factorization with negative payment adjustments,
/// incomplete paths and a payoff that ends before the available rate horizon.
#[test]
fn production_factorization_preserves_negative_extra_and_early_exit() {
    let fixture = fixture(7);
    let pool = quote_basis_pool(&fixture.pool, fixture.as_of).expect("quote-basis pool");
    let discount = fixture
        .market
        .get_discount(&pool.discount_curve_id)
        .expect("discount curve");
    let steps = mc_step_schedule(&pool, fixture.as_of, 12, discount.day_count())
        .expect("short monthly schedule");
    assert!(steps.payment_extras[0] < 0.0);
    let path = RatePath {
        rates: (0..=12)
            .map(|month| 0.04 + 0.001 * f64::from(month % 3))
            .collect(),
    };
    let short_path = RatePath {
        rates: path.rates[..3].to_vec(),
    };
    let base_rate = 0.05;
    let mut early_pool = pool.clone();
    early_pool.current_face = Money::from((100_i64, Currency::USD));
    early_pool.prepayment_spec = PrepaymentModelSpec::constant_cpr(1.0);
    let early_steps = mc_step_schedule(&early_pool, fixture.as_of, 12, discount.day_count())
        .expect("early-payoff schedule");
    assert_eq!(
        zero_spread_cashflows(&early_pool, &path, base_rate, &early_steps).1,
        1,
        "the early-payoff control must exit after one active month"
    );
    assert_eq!(
        zero_spread_cashflows(&pool, &short_path, base_rate, &steps).1,
        2,
        "the truncated path must project exactly its two available intervals"
    );
    for (label, candidate_pool, candidate_path, candidate_steps) in [
        ("negative_extra", &pool, &path, &steps),
        ("short_path", &pool, &short_path, &steps),
        ("early_payoff", &early_pool, &path, &early_steps),
    ] {
        for spread in SPREADS {
            let reference = direct_path_pv(
                candidate_pool,
                candidate_path,
                base_rate,
                spread,
                candidate_steps,
            );
            let candidate = current_path_pv(
                candidate_pool,
                candidate_path,
                base_rate,
                spread,
                candidate_steps,
            );
            assert!(reference.is_finite() && candidate.is_finite());
            let tolerance = 1e-8_f64.max(1e-10 * reference.abs());
            assert!(
                (candidate - reference).abs() <= tolerance,
                "{label} / {spread}: production PV {candidate} differs from direct {reference}"
            );
        }
    }
}

macro_rules! accuracy_seeds {
    ($module:ident, $fixture:expr) => {
        mod $module {
            use super::*;
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn base() {
                run_case($fixture, "base");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_0() {
                run_case($fixture, "b4-mbs-seed-0");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_1() {
                run_case($fixture, "b4-mbs-seed-1");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_2() {
                run_case($fixture, "b4-mbs-seed-2");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_3() {
                run_case($fixture, "b4-mbs-seed-3");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_4() {
                run_case($fixture, "b4-mbs-seed-4");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_5() {
                run_case($fixture, "b4-mbs-seed-5");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_6() {
                run_case($fixture, "b4-mbs-seed-6");
            }
            #[test]
            #[ignore = "full-size MBS OAS accuracy and work diagnostic"]
            fn seed_7() {
                run_case($fixture, "b4-mbs-seed-7");
            }
        }
    };
}
accuracy_seeds!(original_512_348, 0);
accuracy_seeds!(paths_512_months_360, 1);
accuracy_seeds!(paths_2048_months_360, 2);
accuracy_seeds!(paths_8192_months_360, 3);
accuracy_seeds!(paths_2048_months_60, 4);
accuracy_seeds!(paths_2048_months_180, 5);
accuracy_seeds!(gnma_i_paths_2048_months_360, 6);
accuracy_seeds!(delay_15_paths_2048_months_360, 7);
accuracy_seeds!(fresh_act360_paths_2048_months_300, 8);
accuracy_seeds!(seasoned_act360_paths_2048_months_300, 9);
