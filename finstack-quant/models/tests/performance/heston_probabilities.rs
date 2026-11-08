//! Frozen pre-I6 probability arithmetic and bounded numerical panel.
//!
//! Attached as a child of `strip_pricer` so normal builds gain no API or code.
//! The reference deliberately recomputes the phase in two independent loops.

use super::*;
use crate::closed_form::heston::params::HESTON_TAIL_LOG_TARGET;
use crate::closed_form::heston_call_price_fourier;
use serde_json::{Value, json};

type Probability = (f64, f64, f64);

/// Original production loop, frozen before phase reuse; never call the candidate.
fn original_probability(
    pricer: &HestonStripPricer,
    log_strike: f64,
    cached_values: &[Complex<f64>],
) -> Probability {
    let i = Complex::new(0.0, 1.0);
    let mut integral = 0.0;
    let mut tail_abs_mass = 0.0;
    let tail_window_start = pricer.tail_window_start;

    for ((phi, weight), cached) in pricer.grid.iter().zip(cached_values.iter()) {
        let exp_term = (-i * *phi * log_strike).exp();
        let value = (exp_term * *cached).re;
        if !value.is_finite() {
            return (f64::NAN, f64::NAN, f64::INFINITY);
        }
        integral += *weight * value;
        if *phi >= tail_window_start {
            tail_abs_mass += weight.abs() * value.abs();
        }
    }

    let raw = 0.5 + integral / PI;
    (raw.clamp(0.0, 1.0), raw, tail_abs_mass / PI)
}

fn original_probabilities(pricer: &HestonStripPricer, strike: f64) -> [Probability; 2] {
    let log_strike = strike.ln();
    [
        original_probability(pricer, log_strike, &pricer.psi1_over_iphi),
        original_probability(pricer, log_strike, &pricer.psi2_over_iphi),
    ]
}

/// The only candidate adapter; replace these two calls when I6 is authorized.
fn current_probabilities(pricer: &HestonStripPricer, strike: f64) -> [Probability; 2] {
    let log_strike = strike.ln();
    [
        pricer.probability(log_strike, &pricer.psi1_over_iphi),
        pricer.probability(log_strike, &pricer.psi2_over_iphi),
    ]
}

/// Original price diagnostic order, independent of the candidate traversal.
fn original_price_call(pricer: &HestonStripPricer, strike: f64) -> Result<f64> {
    if !strike.is_finite() || strike <= 0.0 {
        return Err(Error::Validation(
            "Heston strike must be finite and positive".to_string(),
        ));
    }
    if pricer.integrand_corrupted {
        return Err(Error::Calibration {
            category: "heston_fourier".to_string(),
            message: format!(
                "Heston strip integration is corrupted for spot={}, strike={}, time={}",
                pricer.spot, strike, pricer.time
            ),
        });
    }
    let [(p1, raw_p1, tail_p1), (p2, raw_p2, tail_p2)] = original_probabilities(pricer, strike);
    let discounted_spot = pricer.spot * (-pricer.params.q * pricer.time).exp();
    let discounted_strike = strike * (-pricer.params.r * pricer.time).exp();
    let price_tolerance = HESTON_TAIL_DIAGNOSTIC_THRESHOLD * discounted_spot;
    let tail_price = discounted_spot * tail_p1 + discounted_strike * tail_p2;
    let probability_excursion =
        discounted_spot * (raw_p1 - p1).abs() + discounted_strike * (raw_p2 - p2).abs();
    if pricer.tail_cf_magnitude > 1e-8
        || tail_price > price_tolerance
        || probability_excursion > price_tolerance
        || !tail_price.is_finite()
    {
        return Err(Error::Calibration {
            category: "heston_fourier".to_string(),
            message: format!(
                "Heston integration tail is unresolved: price-tail estimate={tail_price}, CF magnitude={}",
                pricer.tail_cf_magnitude
            ),
        });
    }
    let call_price = pricer.spot * (-pricer.params.q * pricer.time).exp() * p1
        - strike * (-pricer.params.r * pricer.time).exp() * p2;
    let lower_bound = (discounted_spot - discounted_strike).max(0.0);
    if !call_price.is_finite()
        || call_price < lower_bound - price_tolerance
        || call_price > discounted_spot + price_tolerance
    {
        return Err(Error::Calibration {
            category: "heston_fourier".to_string(),
            message: format!(
                "Heston strip integration produced a non-finite price for \
                 spot={}, strike={}, time={}",
                pricer.spot, strike, pricer.time
            ),
        });
    }
    Ok(call_price.clamp(lower_bound, discounted_spot))
}

/// Original singleton adaptive driver, using the frozen probability reference.
fn original_public_call(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    validate_heston_inputs(spot, time, params)?;
    if !strike.is_finite() || strike <= 0.0 {
        return Err(Error::Validation(
            "Heston strikes must be finite and positive".to_string(),
        ));
    }
    if let Some(settings) = settings {
        settings.validate()?;
    }
    if time <= 0.0 {
        return Ok((spot - strike).max(0.0));
    }
    let mut grid = if let Some(settings) = settings {
        *settings
    } else {
        let mut grid =
            HestonFourierSettings::for_maturity(time, params.model.v0.min(params.model.theta));
        let c_inf = (1.0 - params.model.rho * params.model.rho).sqrt()
            * (params.model.v0 + params.model.kappa * params.model.theta * time)
            / params.model.sigma_v;
        grid.u_max = grid.u_max.max(HESTON_TAIL_LOG_TARGET / c_inf);
        grid.panels = grid.u_max.ceil() as usize;
        grid
    };
    grid.validate()?;
    let scale = spot * (-params.q * time).exp();
    let mut previous: Option<f64> = None;
    for _ in 0..6 {
        if let Some(pricer) = HestonStripPricer::new(spot, time, params, &grid) {
            if let Ok(price) = original_price_call(&pricer, strike) {
                if let Some(old) = previous
                    && (price - old).abs() <= 1e-9 * scale
                {
                    return Ok(price);
                }
                previous = Some(price);
            } else {
                previous = None;
            }
        }
        grid.u_max *= 2.0;
        let Some(panels) = grid.panels.checked_mul(4) else {
            break;
        };
        grid.panels = panels;
        if grid.validate().is_err() {
            break;
        }
    }
    Err(Error::Calibration {
        category: "heston_fourier".to_string(),
        message: format!(
            "Heston Fourier integration did not converge within the quadrature work budget for spot={spot}, time={time}"
        ),
    })
}

fn rejection_branch(error: &Error) -> &'static str {
    match error {
        Error::Validation(_) => "validation",
        Error::Calibration { category, message } => {
            assert_eq!(category, "heston_fourier");
            if message.starts_with("Heston strip integration is corrupted") {
                "corruption"
            } else if message.starts_with("Heston integration tail is unresolved") {
                "unresolved_tail_or_probability_excursion"
            } else if message.starts_with("Heston strip integration produced") {
                "nonfinite_or_no_arbitrage_bounds"
            } else if message.starts_with("Heston Fourier integration did not converge") {
                "quadrature_work_budget"
            } else {
                panic!("unexpected Heston calibration branch: {message}");
            }
        }
        _ => panic!("unexpected Heston error: {error}"),
    }
}

fn result_record(result: &Result<f64>) -> Value {
    match result {
        Ok(price) => json!({"status": "ok", "price_bits": bits(*price)}),
        Err(error) => {
            json!({"status": "error", "branch": rejection_branch(error), "error": error.to_string()})
        }
    }
}

fn assert_same_result(current: &Result<f64>, original: &Result<f64>, label: &str) {
    match (current, original) {
        (Ok(current), Ok(original)) => {
            assert!(current.is_finite() && original.is_finite(), "{label}");
            assert_eq!(current.to_bits(), original.to_bits(), "{label}");
        }
        (Err(current), Err(original)) => {
            assert_eq!(current.kind(), original.kind(), "{label}");
            assert_eq!(
                rejection_branch(current),
                rejection_branch(original),
                "{label}"
            );
        }
        _ => panic!("result mismatch for {label}: {current:?} vs {original:?}"),
    }
}

fn bits(value: f64) -> String {
    format!("0x{:016x}", value.to_bits())
}

/// Deterministic, non-cryptographic fingerprints of raw little-endian f64 bits.
fn fingerprint(values: impl IntoIterator<Item = f64>) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn probability_values(probabilities: [Probability; 2]) -> [f64; 6] {
    let [(p1, raw1, tail1), (p2, raw2, tail2)] = probabilities;
    [p1, raw1, tail1, p2, raw2, tail2]
}

fn record_probe(label: &str, pricer: &HestonStripPricer, strike: f64) -> Value {
    let original = probability_values(original_probabilities(pricer, strike));
    let current = probability_values(current_probabilities(pricer, strike));
    let mut max_abs = 0.0_f64;
    let mut squares = 0.0;
    let mut finite_count = 0_usize;
    for (index, (&actual, &expected)) in current.iter().zip(&original).enumerate() {
        if expected.is_nan() {
            assert!(actual.is_nan(), "{label}: diagnostic {index}");
        } else if expected.is_infinite() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "{label}: diagnostic {index}"
            );
        } else {
            assert!(actual.is_finite(), "{label}: diagnostic {index}");
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "{label}: diagnostic {index}"
            );
            let delta = (actual - expected).abs();
            max_abs = max_abs.max(delta);
            squares += delta * delta;
            finite_count += 1;
        }
    }
    let current_price = pricer.price_call(strike);
    let original_price = original_price_call(pricer, strike);
    assert_same_result(&current_price, &original_price, label);
    let params_bits = [
        pricer.params.r,
        pricer.params.q,
        pricer.params.model.kappa,
        pricer.params.model.theta,
        pricer.params.model.sigma_v,
        pricer.params.model.rho,
        pricer.params.model.v0,
    ]
    .map(bits);
    json!({
        "name": label,
        "spot_bits": bits(pricer.spot),
        "time_bits": bits(pricer.time),
        "strike_bits": bits(strike),
        "fixture_hash": fingerprint([pricer.spot, pricer.time, strike, pricer.params.r,
            pricer.params.q, pricer.params.model.kappa, pricer.params.model.theta,
            pricer.params.model.sigma_v, pricer.params.model.rho, pricer.params.model.v0]),
        "params_bits": params_bits,
        "grid_nodes": pricer.grid.len(),
        "grid_hash": fingerprint(pricer.grid.iter().flat_map(|&(phi, weight)| [phi, weight])),
        "cache_hash": fingerprint(pricer.psi1_over_iphi.iter().chain(&pricer.psi2_over_iphi)
            .flat_map(|value| [value.re, value.im])),
        "tail_window_bits": bits(pricer.tail_window_start),
        "tail_cf_bits": bits(pricer.tail_cf_magnitude),
        "integrand_corrupted": pricer.integrand_corrupted,
        "reference_bits": original.map(bits),
        "current_bits": current.map(bits),
        "reference_hash": fingerprint(original),
        "current_hash": fingerprint(current),
        "max_abs": max_abs,
        "rms": if finite_count == 0 { 0.0 } else { (squares / finite_count as f64).sqrt() },
        "cached_price": result_record(&current_price),
    })
}

struct PanelCase {
    name: &'static str,
    time: f64,
    params: HestonPricingParams,
    strikes: Vec<f64>,
    settings: HestonFourierSettings,
    public_settings: Option<HestonFourierSettings>,
    public_success: Option<bool>,
}

fn panel_case(
    name: &'static str,
    params: HestonPricingParams,
    time: f64,
    strikes: &[f64],
    settings: HestonFourierSettings,
) -> PanelCase {
    PanelCase {
        name,
        time,
        params,
        strikes: strikes.to_vec(),
        settings,
        public_settings: None,
        public_success: Some(true),
    }
}

fn panel_cases() -> Vec<PanelCase> {
    let benchmark = HestonPricingParams::new(0.03, 0.01, 1.5, 0.04, 0.3, -0.6, 0.04)
        .expect("benchmark parameters");
    let ordinary = HestonPricingParams::new(0.05, 0.0, 2.0, 0.04, 0.3, -0.7, 0.04)
        .expect("ordinary parameters");
    let mut cases = vec![panel_case(
        "ordinary_default",
        benchmark,
        1.0,
        &[70.0, 100.0, 130.0],
        HestonFourierSettings::default(),
    )];
    for (name, order) in [
        ("ordinary_gl2", 2),
        ("ordinary_gl4", 4),
        ("ordinary_gl8", 8),
        ("ordinary_gl16", 16),
    ] {
        cases.push(panel_case(
            name,
            benchmark,
            1.0,
            &[70.0, 100.0, 130.0],
            HestonFourierSettings::new(100.0, 100, order).expect("supported GL order"),
        ));
    }
    for (name, time) in [("short_expiry_001", 0.001), ("short_expiry_0001", 0.0001)] {
        cases.push(panel_case(
            name,
            ordinary,
            time,
            &[100.0],
            HestonFourierSettings::for_maturity(time, ordinary.model.v0),
        ));
    }
    cases.push(panel_case(
        "low_variance",
        HestonPricingParams::new(0.02, 0.0, 1.5, 0.0016, 0.2, -0.5, 0.0016).expect("low variance"),
        2.0,
        &[105.0],
        HestonFourierSettings::new(2000.0, 2000, 16).expect("reference grid"),
    ));
    cases.push(panel_case(
        "legitimate_underflow",
        HestonPricingParams::new(0.03, 0.0, 3.0, 0.09, 0.2, -0.6, 0.04).expect("long maturity"),
        15.0,
        &[100.0],
        HestonFourierSettings::for_maturity(15.0, 0.04),
    ));
    cases.push(panel_case(
        "near_deterministic",
        HestonPricingParams::new(0.05, 0.0, 2.0, 0.09, 1e-12, -0.5, 0.01)
            .expect("near deterministic"),
        1.0,
        &[100.0],
        HestonFourierSettings::for_maturity(1.0, 0.01),
    ));
    let mut positive = panel_case(
        "positive_correlation",
        HestonPricingParams::new(0.05, 0.0, 0.1, 0.04, 1.0, 0.9, 0.04)
            .expect("positive correlation"),
        1.0,
        &[100.0],
        HestonFourierSettings::default(),
    );
    positive.public_success = None;
    cases.push(positive);
    let coarse = HestonFourierSettings::new(8.0, 40, 16).expect("coarse grid");
    let mut unresolved = panel_case("unresolved_tail", ordinary, 0.0001, &[100.0], coarse);
    unresolved.public_settings = Some(coarse);
    unresolved.public_success = Some(false);
    cases.push(unresolved);
    let mut corrupted = panel_case(
        "corruption",
        HestonPricingParams::new(0.05, 0.0, 10.0, 100.0, 90.0, 0.99, 90.0)
            .expect("corruption parameters"),
        30.0,
        &[100.0],
        HestonFourierSettings::default(),
    );
    corrupted.public_settings = Some(corrupted.settings);
    corrupted.public_success = Some(false);
    cases.push(corrupted);
    cases
}

/// Deliberately synthetic caches isolate diagnostics; they are not model prices.
fn synthetic_cache(
    base: &HestonStripPricer,
    raw1: f64,
    raw2: f64,
    tail1: f64,
) -> HestonStripPricer {
    let mut pricer = base.clone();
    pricer.grid = vec![(1.0, 1.0), (2.0, 1.0)];
    pricer.psi1_over_iphi = vec![
        Complex::new(PI * (raw1 - 0.5) - PI * tail1, 0.0),
        Complex::new(PI * tail1, 0.0),
    ];
    pricer.psi2_over_iphi = vec![Complex::new(PI * (raw2 - 0.5), 0.0), Complex::new(0.0, 0.0)];
    pricer.tail_window_start = 1.5;
    pricer.tail_cf_magnitude = 0.0;
    pricer.integrand_corrupted = false;
    pricer
}

fn synthetic_probes(base: &HestonStripPricer, records: &mut Vec<Value>) {
    let original = original_probabilities(base, 100.0);
    for (position, index) in [
        ("first", 0),
        ("middle", base.grid.len() / 2),
        ("last", base.grid.len() - 1),
    ] {
        for stream in [1, 2, 3] {
            let mut pricer = base.clone();
            if stream != 2 {
                pricer.psi1_over_iphi[index] = Complex::new(f64::NAN, 0.0);
            }
            if stream != 1 {
                pricer.psi2_over_iphi[index] = Complex::new(f64::INFINITY, 0.0);
            }
            let actual = current_probabilities(&pricer, 100.0);
            if stream == 1 {
                assert_eq!(
                    probability_values([actual[1]; 2]).map(f64::to_bits),
                    probability_values([original[1]; 2]).map(f64::to_bits)
                );
            } else if stream == 2 {
                assert_eq!(
                    probability_values([actual[0]; 2]).map(f64::to_bits),
                    probability_values([original[0]; 2]).map(f64::to_bits)
                );
            }
            records.push(record_probe(
                &format!("nonfinite_stream{stream}_{position}"),
                &pricer,
                100.0,
            ));
        }
    }
    for (name, all) in [("partial_underflow", false), ("all_underflow", true)] {
        let mut pricer = base.clone();
        for (index, value) in pricer.psi1_over_iphi.iter_mut().enumerate() {
            if all || index % 2 == 0 {
                *value = Complex::new(0.0, 0.0);
            }
        }
        for (index, value) in pricer.psi2_over_iphi.iter_mut().enumerate() {
            if all || index % 2 == 0 {
                *value = Complex::new(0.0, 0.0);
            }
        }
        pricer.integrand_corrupted = all;
        records.push(record_probe(name, &pricer, 100.0));
    }
    for (name, strike) in [
        ("zero", 0.0),
        ("negative", -1.0),
        ("nan", f64::NAN),
        ("infinity", f64::INFINITY),
    ] {
        let mut pricer = base.clone();
        pricer.integrand_corrupted = true;
        let actual = pricer.price_call(strike);
        assert!(
            matches!(&actual, Err(Error::Validation(_))),
            "invalid strike precedes corruption"
        );
        assert_same_result(&actual, &original_price_call(&pricer, strike), name);
        records.push(json!({"name": format!("invalid_strike_{name}"), "strike_bits": bits(strike), "cached_price": result_record(&actual)}));
    }
    let threshold = 1e-8_f64;
    for (name, cf) in [
        ("cf_tail_below", f64::from_bits(threshold.to_bits() - 1)),
        ("cf_tail_at", threshold),
        ("cf_tail_above", f64::from_bits(threshold.to_bits() + 1)),
    ] {
        let mut pricer = synthetic_cache(base, 1.0, 0.0, 0.0);
        pricer.tail_cf_magnitude = cf;
        let result = pricer.price_call(1.0);
        assert_eq!(result.is_ok(), cf <= threshold, "{name}");
        records.push(record_probe(name, &pricer, 1.0));
    }
    for (name, delta, success) in [
        ("probability_excursion_below", 0.5e-9, true),
        ("probability_excursion_above", 2e-9, false),
    ] {
        let pricer = synthetic_cache(base, 1.0 + delta, 0.0, 0.0);
        assert_eq!(pricer.price_call(1.0).is_ok(), success, "{name}");
        records.push(record_probe(name, &pricer, 1.0));
    }
    for (name, tail, success) in [
        ("price_tail_below", 0.5e-9, true),
        ("price_tail_above", 2e-9, false),
    ] {
        let pricer = synthetic_cache(base, 1.0, 0.0, tail);
        assert_eq!(pricer.price_call(1.0).is_ok(), success, "{name}");
        records.push(record_probe(name, &pricer, 1.0));
    }
    let discounted_spot = base.spot * (-base.params.q * base.time).exp();
    let upper = synthetic_cache(base, 1.0, 0.0, 0.0);
    assert_eq!(
        upper.price_call(1.0).expect("exact upper bound").to_bits(),
        discounted_spot.to_bits()
    );
    records.push(record_probe("upper_bound_exact", &upper, 1.0));
    let lower = discounted_spot - (-base.params.r * base.time).exp();
    let tolerance = HESTON_TAIL_DIAGNOSTIC_THRESHOLD * discounted_spot;
    for (name, slack, success) in [
        ("lower_bound_clamp", 0.5, true),
        ("lower_bound_reject", 2.0, false),
    ] {
        let pricer = synthetic_cache(
            base,
            (lower - slack * tolerance) / discounted_spot,
            0.0,
            0.0,
        );
        let result = pricer.price_call(1.0);
        assert_eq!(result.is_ok(), success, "{name}");
        if let Ok(price) = result {
            assert_eq!(price.to_bits(), lower.to_bits(), "{name}");
        }
        records.push(record_probe(name, &pricer, 1.0));
    }
}

#[test]
#[ignore = "frozen CCM-I6 numerical panel; run explicitly before C0 and after candidate changes"]
fn ccm_heston_probability_panel() -> std::result::Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        HESTON_TAIL_DIAGNOSTIC_THRESHOLD.to_bits(),
        1e-9_f64.to_bits()
    );
    assert_eq!(
        HESTON_STRIP_MAX_CORRUPT_FRACTION.to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(HESTON_TAIL_WINDOW_FRACTION.to_bits(), 0.1_f64.to_bits());
    assert_eq!(
        crate::closed_form::heston::params::HESTON_MAX_NODES,
        1_048_576
    );
    assert_eq!(
        HESTON_TAIL_LOG_TARGET.to_bits(),
        27.631_021_115_928_547_f64.to_bits()
    );
    let mut records = Vec::new();
    for case in panel_cases() {
        let pricer = HestonStripPricer::new(100.0, case.time, &case.params, &case.settings)
            .expect("valid cache construction");
        for strike in case.strikes {
            let mut record = record_probe(case.name, &pricer, strike);
            let actual = heston_call_price_fourier(
                100.0,
                strike,
                case.time,
                &case.params,
                case.public_settings.as_ref(),
            );
            let reference = original_public_call(
                100.0,
                strike,
                case.time,
                &case.params,
                case.public_settings.as_ref(),
            );
            assert_same_result(&actual, &reference, case.name);
            if let Some(success) = case.public_success {
                assert_eq!(actual.is_ok(), success, "{}", case.name);
            }
            record["starting_settings"] = json!({"u_max_bits": bits(case.settings.u_max), "panels": case.settings.panels, "gl_order": case.settings.gl_order});
            record["public_settings"] = case.public_settings.map_or(Value::Null, |settings| json!({"u_max_bits": bits(settings.u_max), "panels": settings.panels, "gl_order": settings.gl_order}));
            record["public_price"] = result_record(&actual);
            records.push(record);
        }
    }
    let params = HestonPricingParams::new(0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04)
        .expect("synthetic-probe base parameters");
    let base = HestonStripPricer::new(100.0, 1.0, &params, &HestonFourierSettings::default())
        .expect("synthetic-probe base cache");
    synthetic_probes(&base, &mut records);
    let report = json!({"schema": "ccm-heston-probabilities-v1", "reference": "frozen original separate probability loops and singleton adaptive driver", "rng": "none", "fingerprint": "FNV-1a64 over raw little-endian f64 bits; diagnostic, not cryptographic", "hash_scope": "grid/cache hashes describe each fixed-cache probe; public adaptive calls compare final result bits and record initial settings, not intermediate grids", "record_count": records.len(), "records": records});
    if let Some(path) = std::env::var_os("CCM_HESTON_NUMERICS_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!(
        "CCM Heston probability panel: {} records",
        report["record_count"]
    );
    Ok(())
}
