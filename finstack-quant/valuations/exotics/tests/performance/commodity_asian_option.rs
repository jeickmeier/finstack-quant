//! Frozen B4 commodity Asian numerical panel; no production pricing hooks.
//!
//! Fixtures match the 17 Asian identities in `benches/commodity_pricing.rs`.
//! Quadratic compensated moments and independently expressed lognormal prices
//! establish the existing TW/Kemna-Vorst contract, not exact arithmetic-Asian
//! distribution accuracy. The core normal CDF is a shared numerical primitive.

use crate::commodity::commodity_asian_option::types::CommodityAsianOption;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::special_functions::{erf, norm_cdf};
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::OptionType;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{CommodityUnderlyingParams, Instrument};
use finstack_quant_valuations_test_support::commodity_curves::{
    contango_price_curve, flat_price_curve,
};
use finstack_quant_valuations_test_support::volatility::flat_vol_surface;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use time::macros::date;

struct Case {
    id: String,
    method: AveragingMethod,
    observations: usize,
    horizon_days: usize,
    realized: usize,
    volatility: f64,
    flat_forward: bool,
}

fn cases() -> Vec<Case> {
    let mut cases = vec![Case {
        id: "commodity_asian_option/obs_count/12obs".into(),
        method: AveragingMethod::Arithmetic,
        observations: 12,
        horizon_days: 360,
        realized: 0,
        volatility: 0.30,
        flat_forward: false,
    }];
    for (name, method) in [
        ("arithmetic", AveragingMethod::Arithmetic),
        ("geometric", AveragingMethod::Geometric),
    ] {
        cases.push(Case {
            id: format!("commodity_asian_option/method/{name}"),
            method,
            observations: 12,
            horizon_days: 360,
            realized: 0,
            volatility: 0.30,
            flat_forward: false,
        });
        for observations in [12, 60, 252, 1260] {
            cases.push(Case {
                id: format!("commodity_asian_option/fixing_count/{name}/{observations}"),
                method,
                observations,
                horizon_days: 1260,
                realized: 0,
                volatility: 0.30,
                flat_forward: false,
            });
        }
        cases.push(Case {
            id: format!("commodity_asian_option/seasoned/{name}_252obs_126realized"),
            method,
            observations: 252,
            horizon_days: 1260,
            realized: 126,
            volatility: 0.30,
            flat_forward: false,
        });
        for (label, volatility) in [("zero", 0.0), ("small", 1.0e-5)] {
            cases.push(Case {
                id: format!("commodity_asian_option/volatility/{name}_252obs_{label}"),
                method,
                observations: 252,
                horizon_days: 1260,
                realized: 0,
                volatility,
                flat_forward: true,
            });
        }
    }
    cases
}

fn market(case: &Case) -> MarketContext {
    let base = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.975),
            (1.0, 0.95),
            (2.0, 0.90),
            (5.0, 0.78),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("benchmark discount curve");
    let mut market = MarketContext::new().insert(discount);
    for (id, spot, carry) in [
        ("WTI-FWD", 75.0, 0.05),
        ("CL-FORWARD", 75.0, 0.05),
        ("NG-FORWARD", 3.5, 0.02),
        ("RBOB-FORWARD", 90.0, 0.04),
    ] {
        market = market.insert(if id == "CL-FORWARD" && case.flat_forward {
            flat_price_curve(id, base, spot, 5.0)
        } else {
            contango_price_curve(id, base, spot, carry, 5.0)
        });
    }
    for (id, strikes, sigma) in [
        ("WTI-VOL", [50.0, 60.0, 70.0, 80.0, 90.0], 0.30),
        ("CL-VOL", [50.0, 60.0, 70.0, 80.0, 90.0], case.volatility),
        ("NG-VOL", [2.0, 3.0, 4.0, 5.0, 6.0], 0.35),
        ("RBOB-VOL", [60.0, 75.0, 90.0, 105.0, 120.0], 0.32),
    ] {
        let expiries: &[f64] = if id == "CL-VOL" && case.horizon_days == 1260 {
            &[0.25, 0.5, 1.0, 2.0, 5.0]
        } else {
            &[0.25, 0.5, 1.0, 2.0]
        };
        market = market.insert_surface(flat_vol_surface(id, expiries, &strikes, sigma));
    }
    market
}

fn instrument(case: &Case) -> (CommodityAsianOption, Date) {
    let base = date!(2025 - 01 - 01);
    assert_eq!(case.horizon_days % case.observations, 0);
    let spacing = case.horizon_days / case.observations;
    let fixing_dates: Vec<_> = (1..=case.observations)
        .map(|index| base + time::Duration::days((spacing * index) as i64))
        .collect();
    let as_of = base + time::Duration::days((spacing * case.realized) as i64);
    let past_fixings = fixing_dates
        .iter()
        .take(case.realized)
        .enumerate()
        .map(|(index, &date)| (date, 75.0 + 0.02 * (index + 1) as f64))
        .collect();
    let option = CommodityAsianOption::builder()
        .id(InstrumentId::new(if case.horizon_days == 360 {
            "CL-ASIAN-BENCH"
        } else {
            "CL-ASIAN-MATRIX"
        }))
        .underlying(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .strike(if case.flat_forward { 70.0 } else { 75.0 })
        .option_type(OptionType::Call)
        .averaging_method(case.method)
        .fixing_dates(fixing_dates)
        .past_fixings(past_fixings)
        .quantity(1000.0)
        .expiry(base + time::Duration::days((case.horizon_days + 2) as i64))
        .forward_curve_id(CurveId::new("CL-FORWARD"))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .vol_surface_id(CurveId::new("CL-VOL"))
        .day_count(DayCount::Act365F)
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::default())
        .build()
        .expect("benchmark commodity Asian fixture");
    (option, as_of)
}

#[derive(serde::Serialize)]
struct Moments {
    arithmetic_m1: f64,
    arithmetic_m2: f64,
    geometric_log_variance: f64,
    geometric_forward: f64,
}

/// Independent compensated quadratic oracle, separate from production reductions.
fn reference_moments(
    option: &CommodityAsianOption,
    forwards: &[(f64, f64)],
    sigma: f64,
) -> Moments {
    let n = option.fixing_dates.len() as f64;
    let m = forwards.len() as f64;
    let historical_log = neumaier_sum(option.past_fixings.iter().map(|(_, value)| value.ln()));
    let log_sum = neumaier_sum(forwards.iter().map(|(_, forward)| forward.ln()));
    let sum_t = neumaier_sum(forwards.iter().map(|(time, _)| *time));
    let mut second_moment = NeumaierAccumulator::new();
    let mut min_times = NeumaierAccumulator::new();
    for &(ti, fi) in forwards {
        for &(tj, fj) in forwards {
            let minimum = ti.min(tj);
            let product = fi * fj * (sigma * sigma * minimum).exp();
            second_moment.add(product);
            min_times.add(minimum);
        }
    }
    let variance = sigma * sigma * min_times.total() / (n * n);
    let mu = (historical_log + log_sum - 0.5 * sigma * sigma * sum_t) / n;
    Moments {
        arithmetic_m1: neumaier_sum(forwards.iter().map(|(_, forward)| *forward)) / m,
        arithmetic_m2: second_moment.total() / (m * m),
        geometric_log_variance: variance,
        geometric_forward: (mu + 0.5 * variance).exp(),
    }
}

/// Undiscounted call and put expectations from a mean and log variance.
/// This does not call either commodity pricing function.
fn lognormal_payoffs(mean: f64, log_variance: f64, strike: f64) -> (f64, f64) {
    if log_variance <= 0.0 {
        return ((mean - strike).max(0.0), (strike - mean).max(0.0));
    }
    let deviation = log_variance.sqrt();
    let mean_log = mean.ln() - 0.5 * log_variance;
    let d2 = (mean_log - strike.ln()) / deviation;
    let d1 = d2 + deviation;
    (
        mean * norm_cdf(d1) - strike * norm_cdf(d2),
        strike * norm_cdf(-d2) - mean * norm_cdf(-d1),
    )
}

fn reference_values(
    option: &CommodityAsianOption,
    forwards: &[(f64, f64)],
    sigma: f64,
    df: f64,
    moments: &Moments,
) -> (f64, f64, f64) {
    let n = option.fixing_dates.len() as f64;
    let m = forwards.len() as f64;
    let historical_sum = neumaier_sum(option.past_fixings.iter().map(|(_, value)| *value));
    let arithmetic_average =
        (historical_sum + neumaier_sum(forwards.iter().map(|(_, value)| *value))) / n;
    let (call, put, parity) = match option.averaging_method {
        AveragingMethod::Arithmetic => {
            let effective_strike = (n * option.strike - historical_sum) / m;
            assert!(
                effective_strike > 0.0,
                "B4 fixtures must exercise the moment path"
            );
            let variance = if sigma == 0.0 {
                0.0
            } else {
                (moments.arithmetic_m2 / moments.arithmetic_m1.powi(2))
                    .ln()
                    .max(0.0)
            };
            let (call, put) = lognormal_payoffs(moments.arithmetic_m1, variance, effective_strike);
            (
                call * m / n,
                put * m / n,
                arithmetic_average - option.strike,
            )
        }
        AveragingMethod::Geometric => {
            let (call, put) = lognormal_payoffs(
                moments.geometric_forward,
                moments.geometric_log_variance,
                option.strike,
            );
            (call, put, moments.geometric_forward - option.strike)
        }
    };
    let scale = option.quantity * df;
    (call * scale, put * scale, parity * scale)
}

#[derive(serde::Serialize)]
struct Bound {
    actual: f64,
    reference: f64,
    absolute_error: f64,
    absolute_limit: f64,
    passed: bool,
}

impl Bound {
    fn new(actual: f64, reference: f64, absolute_limit: f64) -> Self {
        let absolute_error = (actual - reference).abs();
        Self {
            actual,
            reference,
            absolute_error,
            absolute_limit,
            passed: actual.is_finite()
                && reference.is_finite()
                && absolute_limit.is_finite()
                && absolute_limit >= 0.0
                && absolute_error <= absolute_limit,
        }
    }
}

fn run_case(case: &Case) -> (Value, bool) {
    let (option, as_of) = instrument(case);
    let market = market(case);
    option
        .validate_past_fixings(as_of)
        .expect("complete benchmark history");
    let curve = market
        .get_price_curve(&option.forward_curve_id)
        .expect("benchmark forward curve");
    let forwards: Vec<_> = option
        .fixing_dates
        .iter()
        .filter(|date| **date > as_of)
        .map(|date| {
            (
                option
                    .day_count
                    .year_fraction(as_of, *date, DayCountContext::default())
                    .expect("fixing model time"),
                curve.price_on_date(*date).expect("fixing forward"),
            )
        })
        .collect();
    assert_eq!(forwards.len(), case.observations - case.realized);
    assert!(
        forwards
            .iter()
            .all(|(time, value)| *time > 0.0 && value.is_finite() && *value > 0.0)
    );
    let df = market
        .get_discount(&option.discount_curve_id)
        .expect("benchmark discount curve")
        .df_between_dates(as_of, option.expiry)
        .expect("date-based discount factor");
    let reference = reference_moments(&option, &forwards, case.volatility);
    let (arithmetic_m1, arithmetic_variance) =
        super::arithmetic_moments(&forwards, case.volatility);
    let arithmetic_m2 = arithmetic_m1 * arithmetic_m1 + arithmetic_variance;
    let (_, historical_log, _) = option.accumulated_state(as_of);
    let geometric = super::geometric_moments(
        &forwards,
        case.volatility,
        historical_log,
        option.fixing_dates.len(),
    );
    let production = Moments {
        arithmetic_m1,
        arithmetic_m2,
        geometric_log_variance: geometric.log_variance,
        geometric_forward: geometric.forward,
    };
    let (reference_call, reference_put, reference_parity) =
        reference_values(&option, &forwards, case.volatility, df, &reference);
    let call = option
        .value(&market, as_of)
        .expect("public benchmark call pricing");
    let mut put_option = option.clone();
    put_option.option_type = OptionType::Put;
    let put = put_option
        .value(&market, as_of)
        .expect("public companion put pricing");

    let scale1 = forwards
        .iter()
        .map(|(_, value)| value.abs())
        .chain(option.past_fixings.iter().map(|(_, value)| value.abs()))
        .fold(option.strike.abs(), f64::max);
    let scale2 = reference
        .arithmetic_m2
        .abs()
        .max(reference.arithmetic_m1.powi(2));
    let max_time = forwards.iter().map(|(time, _)| *time).fold(0.0, f64::max);
    let ratio = forwards.len() as f64 / option.fixing_dates.len() as f64;
    let variance_scale = reference
        .geometric_log_variance
        .abs()
        .max(case.volatility.powi(2) * max_time * ratio.powi(2));
    let currency_scale = option.quantity * df * scale1;
    let price_limit = if case.volatility == 0.0 {
        1.0e-12
    } else {
        1.0e-9
    } * currency_scale;
    let mut bounds = BTreeMap::new();
    let reduction_factor = 32.0 * f64::EPSILON * (forwards.len() + 1) as f64;
    match case.method {
        AveragingMethod::Arithmetic => {
            bounds.insert(
                "production_m1",
                Bound::new(
                    production.arithmetic_m1,
                    reference.arithmetic_m1,
                    1.0e-12 * scale1,
                ),
            );
            bounds.insert(
                "production_m2",
                Bound::new(
                    production.arithmetic_m2,
                    reference.arithmetic_m2,
                    5.0e-10 * scale2,
                ),
            );
            bounds.insert(
                "linear_reduction_m2",
                Bound::new(
                    production.arithmetic_m2,
                    reference.arithmetic_m2,
                    reduction_factor * scale2,
                ),
            );
        }
        AveragingMethod::Geometric => {
            bounds.insert(
                "production_geometric_variance",
                Bound::new(
                    production.geometric_log_variance,
                    reference.geometric_log_variance,
                    5.0e-10 * variance_scale,
                ),
            );
            bounds.insert(
                "production_geometric_forward",
                Bound::new(
                    production.geometric_forward,
                    reference.geometric_forward,
                    1.0e-12 * scale1,
                ),
            );
            bounds.insert(
                "linear_reduction_geometric_variance",
                Bound::new(
                    production.geometric_log_variance,
                    reference.geometric_log_variance,
                    reduction_factor * variance_scale,
                ),
            );
        }
    }
    bounds.insert(
        "production_call_pv",
        Bound::new(call.amount(), reference_call, price_limit),
    );
    bounds.insert(
        "production_put_pv",
        Bound::new(put.amount(), reference_put, price_limit),
    );
    bounds.insert(
        "production_put_call_parity",
        Bound::new(
            call.amount() - put.amount(),
            reference_parity,
            2.0e-12 * currency_scale,
        ),
    );
    let currency_ok = call.currency() == Currency::USD && put.currency() == Currency::USD;
    let values_ok = call.amount().is_finite()
        && put.amount().is_finite()
        && call.amount() >= 0.0
        && put.amount() >= 0.0;
    let passed = currency_ok && values_ok && bounds.values().all(|bound| bound.passed);
    // Keep this object identical to benchmark capture, so the root's fixture
    // hash can establish that the numerical panel used the timed workload.
    let mut dimensions = json!({
        "total_fixings": case.observations, "realized_fixing_count": case.realized,
        "future_fixings": forwards.len(), "fixing_horizon_days": case.horizon_days,
        "payment_lag_days": 2, "averaging_method": case.method, "volatility": case.volatility
    });
    if case.horizon_days == 1260 {
        dimensions["fixing_spacing_days"] = json!(case.horizon_days / case.observations);
        dimensions["forward_shape"] = json!(if case.flat_forward {
            "flat"
        } else {
            "linear_contango"
        });
    }
    if case.realized > 0 {
        dimensions["remaining_fixing_horizon_days"] =
            json!(case.horizon_days * (case.observations - case.realized) / case.observations);
    }
    let record = json!({
        "case": case.id,
        "fixture": {
            "instrument": &option,
            "market": &market, "as_of": as_of.to_string(), "model": option.default_model(),
            "requested_metrics": [],
            "dimensions": dimensions
        },
        "companion_put_instrument": &put_option,
        "projection": { "future_time_forward_pairs": &forwards, "discount_factor": df,
            "historical_sum": neumaier_sum(option.past_fixings.iter().map(|(_, value)| *value)),
            "historical_log_sum": neumaier_sum(option.past_fixings.iter().map(|(_, value)| value.ln())) },
        "moments": { "compensated_quadratic_reference": reference, "production": production,
            "production_arithmetic_variance": arithmetic_variance },
        "output": { "value": call.amount(), "currency": call.currency(), "production_call": call.amount(),
            "production_put": put.amount(), "reference_call": reference_call, "reference_put": reference_put,
            "reference_put_call_parity": reference_parity },
        "scales": { "price_per_unit": scale1, "second_moment": scale2,
            "geometric_variance": variance_scale, "currency": currency_scale },
        "bounds": bounds,
        "currency_ok": currency_ok, "finite_nonnegative_values": values_ok, "passed": passed,
        "moment_observation": "Actual private production helpers evaluated on the same projected inputs and production history accumulation used by pricing. Arithmetic M2 is reconstructed as production M1 squared plus production variance; pricing uses the variance directly. Public call and put prices are checked against independent compensated quadratic references.",
        "accuracy_contract": "Frozen B4 bounds plus the predeclared 32*EPSILON*(m+1) linear-reduction moment gate; arithmetic reference preserves the Turnbull-Wakeman approximation."
    });
    (record, passed)
}

#[test]
fn commodity_asian_b4_numerical_panel() {
    let cases = cases();
    assert_eq!(cases.len(), 17);
    let mut failures = Vec::new();
    for case in cases {
        let (record, passed) = run_case(&case);
        println!("FQ_COMMODITY_DIAGNOSTIC {record}");
        if !passed {
            failures.push(case.id);
        }
    }
    assert!(
        failures.is_empty(),
        "commodity B4 frozen numerical bounds failed: {failures:?}"
    );
}

fn assert_edge_bound(label: &str, actual: f64, reference: f64, limit: f64) {
    let bound = Bound::new(actual, reference, limit);
    assert!(
        bound.passed,
        "{label}: actual={actual}, reference={reference}, error={}, limit={limit}",
        bound.absolute_error
    );
}

/// Dates establish a valid history partition; private numerical model times
/// are supplied separately so equal times do not require duplicate dates.
fn edge_option(
    method: AveragingMethod,
    future_count: usize,
    history: &[f64],
    strike: f64,
) -> (CommodityAsianOption, Date) {
    let base = date!(2025 - 01 - 01);
    let total_count = history.len() + future_count;
    let mut option = CommodityAsianOption::example().expect("edge option");
    option.averaging_method = method;
    option.strike = strike;
    option.quantity = 1000.0;
    option.fixing_dates = (1..=total_count)
        .map(|index| base + time::Duration::days(index as i64))
        .collect();
    option.past_fixings = option
        .fixing_dates
        .iter()
        .copied()
        .zip(history.iter().copied())
        .collect();
    option.expiry = base + time::Duration::days((total_count + 2) as i64);
    let as_of = base + time::Duration::days(history.len() as i64);
    option
        .validate_past_fixings(as_of)
        .expect("valid edge history");
    (option, as_of)
}

fn check_moment_edge(
    label: &str,
    forwards: &[(f64, f64)],
    sigma: f64,
    history: &[f64],
    strike: f64,
) {
    assert!(!forwards.is_empty());
    assert!(forwards.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    let df = 0.96;
    let m = forwards.len() as f64;
    let n = (forwards.len() + history.len()) as f64;
    let scale1 = forwards
        .iter()
        .map(|(_, forward)| forward.abs())
        .chain(history.iter().map(|value| value.abs()))
        .fold(strike.abs(), f64::max);
    let reduction_factor = 32.0 * f64::EPSILON * (m + 1.0);
    let max_time = forwards.iter().map(|(time, _)| *time).fold(0.0, f64::max);
    for method in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
        let (option, as_of) = edge_option(method, forwards.len(), history, strike);
        let (historical_sum, historical_log, historical_count) = option.accumulated_state(as_of);
        assert_eq!(historical_count, history.len());
        let reference = reference_moments(&option, forwards, sigma);
        match method {
            AveragingMethod::Arithmetic => {
                let (m1, variance) = super::arithmetic_moments(forwards, sigma);
                let m2 = m1 * m1 + variance;
                let scale2 = reference
                    .arithmetic_m2
                    .abs()
                    .max(reference.arithmetic_m1.powi(2));
                assert_edge_bound(
                    &format!("{label}: arithmetic M1"),
                    m1,
                    reference.arithmetic_m1,
                    1.0e-12 * scale1,
                );
                assert_edge_bound(
                    &format!("{label}: arithmetic M2 frozen B4"),
                    m2,
                    reference.arithmetic_m2,
                    5.0e-10 * scale2,
                );
                assert_edge_bound(
                    &format!("{label}: arithmetic M2 linear reduction"),
                    m2,
                    reference.arithmetic_m2,
                    reduction_factor * scale2,
                );
            }
            AveragingMethod::Geometric => {
                let observed = super::geometric_moments(
                    forwards,
                    sigma,
                    historical_log,
                    option.fixing_dates.len(),
                );
                let variance_scale = reference
                    .geometric_log_variance
                    .abs()
                    .max(sigma.powi(2) * max_time * (m / n).powi(2));
                assert_edge_bound(
                    &format!("{label}: geometric forward"),
                    observed.forward,
                    reference.geometric_forward,
                    1.0e-12 * scale1,
                );
                assert_edge_bound(
                    &format!("{label}: geometric variance frozen B4"),
                    observed.log_variance,
                    reference.geometric_log_variance,
                    5.0e-10 * variance_scale,
                );
                assert_edge_bound(
                    &format!("{label}: geometric variance linear reduction"),
                    observed.log_variance,
                    reference.geometric_log_variance,
                    reduction_factor * variance_scale,
                );
            }
        }
        let (reference_call, reference_put, reference_parity) =
            reference_values(&option, forwards, sigma, df, &reference);
        let price = |kind| {
            let per_unit = match method {
                AveragingMethod::Arithmetic => super::price_arithmetic_tw_commodity(
                    forwards,
                    strike,
                    sigma,
                    df,
                    kind,
                    historical_sum,
                    option.fixing_dates.len(),
                ),
                AveragingMethod::Geometric => super::price_geometric_commodity(
                    forwards,
                    strike,
                    sigma,
                    df,
                    kind,
                    historical_log,
                    option.fixing_dates.len(),
                ),
            };
            let value = option.quantity * per_unit;
            assert!(
                value.is_finite() && value >= 0.0,
                "{label}: {method:?} {kind:?} price {value}"
            );
            value
        };
        let call = price(OptionType::Call);
        let put = price(OptionType::Put);
        let currency_scale = option.quantity * df * scale1;
        let price_limit = if sigma == 0.0 { 1.0e-12 } else { 1.0e-9 } * currency_scale;
        assert_edge_bound(
            &format!("{label}: {method:?} call"),
            call,
            reference_call,
            price_limit,
        );
        assert_edge_bound(
            &format!("{label}: {method:?} put"),
            put,
            reference_put,
            price_limit,
        );
        assert_edge_bound(
            &format!("{label}: {method:?} put-call parity"),
            call - put,
            reference_parity,
            2.0e-12 * currency_scale,
        );
    }
}

#[test]
fn moment_single_fixing() {
    check_moment_edge("single fixing", &[(0.5, 75.0)], 0.30, &[], 75.0);
}

#[test]
fn moment_equal_times() {
    check_moment_edge(
        "equal model times",
        &[(0.25, 70.0), (0.25, 75.0), (0.75, 80.0), (0.75, 85.0)],
        0.30,
        &[],
        77.0,
    );
}

#[test]
fn moment_irregular_times() {
    check_moment_edge(
        "irregular model times",
        &[(0.01, 70.0), (0.125, 80.0), (0.51, 76.0), (1.75, 91.0)],
        0.30,
        &[],
        80.0,
    );
}

#[test]
fn moment_high_volatility() {
    check_moment_edge(
        "high volatility",
        &[(0.01, 70.0), (0.125, 80.0), (0.51, 76.0), (1.75, 91.0)],
        1.0,
        &[],
        80.0,
    );
}

#[test]
fn zero_volatility_adjacent_forwards_has_no_roundoff_premium() {
    // Independently rounded raw moments can leave M2 > M1^2 even though zero
    // volatility makes the average certain. Direct covariance must stay zero.
    check_moment_edge(
        "zero volatility adjacent forwards",
        &[(0.25, 1.0), (0.75, 1.0 + f64::EPSILON)],
        0.0,
        &[],
        1.0,
    );
}

fn adjacent_forward_option_market(volatility: f64) -> (CommodityAsianOption, MarketContext, Date) {
    let as_of = date!(2025 - 01 - 01);
    let fixing_dates = vec![
        as_of + time::Duration::days(365),
        as_of + time::Duration::days(730),
    ];
    let forward = PriceCurve::builder("CL-FORWARD")
        .base_date(as_of)
        .spot_price(1.0)
        .knots([(0.0, 1.0), (1.0, 1.0), (2.0, 1.0 + f64::EPSILON)])
        .build()
        .expect("adjacent forwards curve");
    for (&fixing, expected) in fixing_dates.iter().zip([1.0, 1.0 + f64::EPSILON]) {
        assert_eq!(
            forward.price_on_date(fixing).expect("fixing forward"),
            expected
        );
    }
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (3.0, 1.0)])
        .build()
        .expect("unit discount curve");
    let market = MarketContext::new()
        .insert(forward)
        .insert(discount)
        .insert_surface(flat_vol_surface(
            "CL-VOL",
            &[1.0, 3.0],
            &[0.5, 1.5],
            volatility,
        ));
    let mut option = CommodityAsianOption::example().expect("adjacent forwards option");
    option.averaging_method = AveragingMethod::Arithmetic;
    option.option_type = OptionType::Call;
    option.fixing_dates = fixing_dates;
    option.past_fixings.clear();
    option.expiry = as_of + time::Duration::days(732);
    option.strike = 1.0;
    option.quantity = 1000.0;
    option.day_count = DayCount::Act365F;
    option.forward_curve_id = CurveId::new("CL-FORWARD");
    option.discount_curve_id = CurveId::new("USD-OIS");
    option.vol_surface_id = CurveId::new("CL-VOL");
    (option, market, as_of)
}

#[test]
fn public_zero_volatility_adjacent_forwards_has_no_roundoff_premium() {
    let (mut option, market, as_of) = adjacent_forward_option_market(0.0);
    let call = option.value(&market, as_of).expect("zero volatility call");
    option.option_type = OptionType::Put;
    let put = option.value(&market, as_of).expect("zero volatility put");
    assert_eq!(call.currency(), Currency::USD);
    assert_eq!(put.currency(), Currency::USD);
    let currency_scale = option.quantity * (1.0 + f64::EPSILON);
    assert_edge_bound(
        "public zero volatility call",
        call.amount(),
        0.0,
        1.0e-12 * currency_scale,
    );
    assert_edge_bound(
        "public zero volatility put",
        put.amount(),
        0.0,
        1.0e-12 * currency_scale,
    );
    assert_edge_bound(
        "public zero volatility parity",
        call.amount() - put.amount(),
        0.0,
        2.0e-12 * currency_scale,
    );
}

struct TinyAtmReference {
    variance: f64,
    log_variance: f64,
    payoff: f64,
}

/// Quadratic covariance sum, independent of the ordered production reduction.
/// At strike equal to the mean, the erf identity avoids subtracting two CDFs
/// close to one half and retains the positive tiny-volatility expectation.
fn tiny_atm_reference(forwards: &[(f64, f64)], sigma: f64) -> TinyAtmReference {
    let count = forwards.len() as f64;
    let mean = neumaier_sum(forwards.iter().map(|(_, forward)| *forward)) / count;
    assert_eq!(
        mean, 1.0,
        "adjacent-forward fixture must remain exactly ATM"
    );
    let mut covariance = NeumaierAccumulator::new();
    for &(ti, fi) in forwards {
        for &(tj, fj) in forwards {
            covariance.add(fi * fj * (sigma * sigma * ti.min(tj)).exp_m1());
        }
    }
    let variance = covariance.total() / (count * count);
    assert!(variance.is_finite() && variance > 0.0);
    let log_variance = (variance / mean.powi(2)).ln_1p();
    assert!(log_variance.is_finite() && log_variance > 0.0);
    let payoff = mean * erf(log_variance.sqrt() / (2.0 * std::f64::consts::SQRT_2));
    assert!(payoff.is_finite() && payoff > 0.0);
    TinyAtmReference {
        variance,
        log_variance,
        payoff,
    }
}

fn check_tiny_volatility_prices(
    label: &str,
    call: f64,
    put: f64,
    reference: f64,
    currency_scale: f64,
) {
    for (kind, value) in [("call", call), ("put", put)] {
        assert!(
            value.is_finite() && value > 0.0,
            "{label}: {kind} must retain positive tiny-volatility time value, got {value}"
        );
        assert_edge_bound(
            &format!("{label}: {kind} frozen reference bound"),
            value,
            reference,
            1.0e-9 * currency_scale,
        );
        // At sigma=1e-10 every legacy exp term rounds to one. Its naive
        // quadratic numerator rounds to four, hence the legacy ATM PV is zero.
        assert_edge_bound(
            &format!("{label}: {kind} frozen baseline bound"),
            value,
            0.0,
            1.0e-9 * currency_scale,
        );
        assert_edge_bound(
            &format!("{label}: {kind} tiny-volatility accuracy"),
            value,
            reference,
            1.0e-12 * currency_scale,
        );
    }
    assert_edge_bound(
        &format!("{label}: put-call parity"),
        call - put,
        0.0,
        2.0e-12 * currency_scale,
    );
}

#[test]
fn tiny_positive_volatility_adjacent_forwards_retains_time_value() {
    let forwards = [(0.25, 1.0), (0.75, 1.0 + f64::EPSILON)];
    let sigma = 1.0e-10;
    let df = 0.96;
    let quantity = 1000.0;
    let price = |kind| {
        quantity
            * super::price_arithmetic_tw_commodity(
                &forwards,
                1.0,
                sigma,
                df,
                kind,
                0.0,
                forwards.len(),
            )
    };
    let reference = tiny_atm_reference(&forwards, sigma);
    let (mean, variance) = super::arithmetic_moments(&forwards, sigma);
    assert!(mean.is_finite() && mean > 0.0);
    assert_eq!(mean, 1.0);
    assert!(variance.is_finite() && variance > 0.0);
    let log_variance = (variance / mean / mean).ln_1p();
    assert!(log_variance.is_finite() && log_variance > 0.0);
    let reduction_factor = 32.0 * f64::EPSILON * (forwards.len() + 1) as f64;
    assert_edge_bound(
        "tiny covariance linear reduction",
        variance,
        reference.variance,
        reduction_factor * reference.variance,
    );
    assert_edge_bound(
        "tiny log variance linear reduction",
        log_variance,
        reference.log_variance,
        reduction_factor * reference.log_variance,
    );
    check_tiny_volatility_prices(
        "private adjacent forwards",
        price(OptionType::Call),
        price(OptionType::Put),
        quantity * df * reference.payoff,
        quantity * df * (1.0 + f64::EPSILON),
    );
}

#[test]
fn public_tiny_positive_volatility_adjacent_forwards_retains_time_value() {
    let sigma = 1.0e-10;
    let (mut option, market, as_of) = adjacent_forward_option_market(sigma);
    let call = option.value(&market, as_of).expect("tiny volatility call");
    option.option_type = OptionType::Put;
    let put = option.value(&market, as_of).expect("tiny volatility put");
    assert_eq!(call.currency(), Currency::USD);
    assert_eq!(put.currency(), Currency::USD);
    let df = market
        .get_discount(&option.discount_curve_id)
        .expect("unit discount curve")
        .df_between_dates(as_of, option.expiry)
        .expect("unit payment discount factor");
    let reference = tiny_atm_reference(&[(1.0, 1.0), (2.0, 1.0 + f64::EPSILON)], sigma);
    check_tiny_volatility_prices(
        "public adjacent forwards",
        call.amount(),
        put.amount(),
        option.quantity * df * reference.payoff,
        option.quantity * df * (1.0 + f64::EPSILON),
    );
}

#[test]
fn moment_scale_homogeneity() {
    for factor in [1.0e-6, 1.0, 1.0e6] {
        let forwards: Vec<_> = [(0.25, 80.0), (0.5, 82.0), (0.75, 84.0), (1.0, 86.0)]
            .into_iter()
            .map(|(time, forward)| (time, factor * forward))
            .collect();
        check_moment_edge(
            &format!("fresh price scale {factor}"),
            &forwards,
            0.30,
            &[],
            factor * 83.0,
        );
        let history = [77.0 * factor, 79.0 * factor, 76.0 * factor];
        check_moment_edge(
            &format!("seasoned price scale {factor}"),
            &forwards,
            0.30,
            &history,
            factor * 83.0,
        );
    }
}

#[test]
fn seasoned_moment_reference() {
    check_moment_edge(
        "seasoned moments",
        &[(0.20, 78.0), (0.45, 81.0), (0.70, 85.0)],
        0.35,
        &[77.0, 79.0, 76.0],
        80.0,
    );
}

#[test]
fn one_remaining_fixing() {
    check_moment_edge(
        "one remaining fixing",
        &[(0.5, 85.0)],
        0.30,
        &[70.0, 72.0, 74.0, 76.0, 78.0],
        75.0,
    );
}

#[test]
fn nonpositive_effective_strike() {
    let forwards = [(0.5, 80.0)];
    let history = [120.0, 120.0];
    let total_count = history.len() + forwards.len();
    let historical_sum = neumaier_sum(history);
    let df = 0.96;
    let quantity = 1000.0;
    let average = (historical_sum + 80.0) / total_count as f64;
    for strike in [80.0, 75.0] {
        assert!(total_count as f64 * strike - historical_sum <= 0.0);
        let call = quantity
            * super::price_arithmetic_tw_commodity(
                &forwards,
                strike,
                0.30,
                df,
                OptionType::Call,
                historical_sum,
                total_count,
            );
        let put = quantity
            * super::price_arithmetic_tw_commodity(
                &forwards,
                strike,
                0.30,
                df,
                OptionType::Put,
                historical_sum,
                total_count,
            );
        let currency_scale = quantity * df * 120.0;
        assert_edge_bound(
            "nonpositive effective strike call",
            call,
            quantity * df * (average - strike),
            1.0e-9 * currency_scale,
        );
        assert_edge_bound(
            "nonpositive effective strike put",
            put,
            0.0,
            1.0e-9 * currency_scale,
        );
        assert_edge_bound(
            "nonpositive effective strike parity",
            call - put,
            quantity * df * (average - strike),
            2.0e-12 * currency_scale,
        );
        assert_eq!(put, 0.0);
    }
}

#[test]
fn fully_observed_before_payment() {
    let as_of = date!(2025 - 01 - 10);
    let expiry = date!(2025 - 01 - 15);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, (-0.05_f64).exp())])
        .build()
        .expect("fully observed discount curve");
    let df = discount
        .df_between_dates(as_of, expiry)
        .expect("payment discount factor");
    assert!(df < 1.0);
    // No forward curve or volatility surface: a fully observed average must
    // reach only the realized-payoff and payment-discounting path.
    let market = MarketContext::new().insert(discount);
    for method in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
        let average = match method {
            AveragingMethod::Arithmetic => neumaier_sum([70.0, 80.0, 90.0]) / 3.0,
            AveragingMethod::Geometric => {
                (neumaier_sum([70.0_f64.ln(), 80.0_f64.ln(), 90.0_f64.ln()]) / 3.0).exp()
            }
        };
        for strike in [75.0, 85.0] {
            let mut option = CommodityAsianOption::example().expect("fully observed option");
            option.averaging_method = method;
            option.option_type = OptionType::Call;
            option.quantity = 1000.0;
            option.strike = strike;
            option.expiry = expiry;
            option.fixing_dates = vec![
                date!(2025 - 01 - 05),
                date!(2025 - 01 - 06),
                date!(2025 - 01 - 07),
            ];
            option.past_fixings = option
                .fixing_dates
                .iter()
                .copied()
                .zip([70.0, 80.0, 90.0])
                .collect();
            let call = option
                .value(&market, as_of)
                .expect("fully observed call with discount data only");
            option.option_type = OptionType::Put;
            let put = option
                .value(&market, as_of)
                .expect("fully observed put with discount data only");
            assert_eq!(call.currency(), Currency::USD);
            assert_eq!(put.currency(), Currency::USD);
            assert!(call.amount().is_finite() && call.amount() >= 0.0);
            assert!(put.amount().is_finite() && put.amount() >= 0.0);
            let currency_scale = option.quantity * df * 90.0;
            assert_edge_bound(
                "fully observed call",
                call.amount(),
                option.quantity * df * (average - strike).max(0.0),
                1.0e-12 * currency_scale,
            );
            assert_edge_bound(
                "fully observed put",
                put.amount(),
                option.quantity * df * (strike - average).max(0.0),
                1.0e-12 * currency_scale,
            );
            assert_edge_bound(
                "fully observed parity",
                call.amount() - put.amount(),
                option.quantity * df * (average - strike),
                2.0e-12 * currency_scale,
            );
        }
    }
}
