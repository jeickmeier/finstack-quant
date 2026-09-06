//! Hand-derived numeric FRTB SBA charges for the risk classes that previously
//! had no numeric coverage at all: CSR securitisation (CTP), CSR
//! securitisation (non-CTP), and commodity.
//!
//! Every test drives [`FrtbSbaEngine`] end-to-end (`new()` ->
//! `calculate()`) and asserts the resulting charge against a value derived by
//! hand. Each derivation is written out in the comment above the assertion
//! with every risk weight and correlation quoted as a literal, so a change to
//! any prescribed parameter in `regulatory::frtb::params` fails the test
//! rather than silently moving capital.
//!
//! # Standards reference
//!
//! - Basel Committee on Banking Supervision, *Minimum capital requirements for
//!   market risk* (BCBS **d457**), published 14 January 2019, corrected version
//!   25 February 2019; consolidated as Basel Framework chapter **MAR21**,
//!   "Standardised approach: sensitivities-based method", version effective
//!   1 January 2023.
//! - Aggregation: MAR21.4 (intra-bucket `K_b`), MAR21.5 (curvature `psi` and
//!   the curvature aggregation), MAR21.6 (inter-bucket, the alternative
//!   capped-`S_b` fallback, and the three correlation scenarios), MAR21.7
//!   (maximum across scenarios).
//! - Correlation scenarios (MAR21.6): `rho_high = min(1.25 * rho, 1)`,
//!   `rho_low = max(2 * rho - 1, 0.75 * rho)`.
//!
//! # Scale convention
//!
//! Delta risk weights are quoted **in percent** exactly as published (`30.0`
//! means 30%), and the engine expects sensitivities scaled to match — see the
//! table in the `regulatory::frtb` module documentation. The literals below
//! therefore multiply directly.
//!
//! # Curvature inputs
//!
//! `CVR+` / `CVR-` are supplied by the caller already shocked and already net
//! of the delta-hedged component, in the loss-positive convention. The engine
//! applies **no** further risk weight to curvature inputs; the tests below pin
//! that behaviour.

use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::frtb::{
    CorrelationScenario, FrtbRiskClass, FrtbSbaEngine, FrtbSbaResult, FrtbSensitivities,
};

/// Relative tolerance for comparing a computed charge with the hand-derived
/// value. The arithmetic is a handful of `f64` multiplications and one square
/// root, so agreement is far tighter than this; the tolerance exists only to
/// avoid pinning the last bit of an IEEE-754 result.
const REL_TOL: f64 = 1e-9;

/// Assert `actual` matches `expected` to [`REL_TOL`] relative precision.
fn assert_charge(actual: f64, expected: f64, what: &str) {
    let tol = expected.abs() * REL_TOL;
    assert!(
        (actual - expected).abs() <= tol,
        "{what}: expected {expected}, got {actual} (diff {})",
        actual - expected
    );
}

/// Build an engine restricted to one risk class under the Medium (prescribed)
/// correlation scenario, so the hand derivation has no scenario ambiguity.
fn medium_engine(risk_class: FrtbRiskClass) -> FrtbSbaEngine {
    FrtbSbaEngine::new(vec![CorrelationScenario::Medium], vec![risk_class])
        .expect("engine builds for a single risk class and scenario")
}

fn delta_charge_of(result: &FrtbSbaResult, rc: FrtbRiskClass) -> f64 {
    result.delta_by_risk_class.get(&rc).copied().unwrap_or(0.0)
}

fn vega_charge_of(result: &FrtbSbaResult, rc: FrtbRiskClass) -> f64 {
    result.vega_by_risk_class.get(&rc).copied().unwrap_or(0.0)
}

fn curvature_charge_of(result: &FrtbSbaResult, rc: FrtbRiskClass) -> f64 {
    result
        .curvature_by_risk_class
        .get(&rc)
        .copied()
        .unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Commodity
// ---------------------------------------------------------------------------

#[test]
fn commodity_delta_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::Commodity);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    // Bucket 1 = solid combustibles, RW = 30% (MAR21.82, Table 11).
    sens.add_commodity_delta("COAL_A", 1, "1Y", "location", 100_000.0);
    sens.add_commodity_delta("COAL_B", 1, "1Y", "location", 50_000.0);
    // Bucket 3 = electricity and carbon trading, RW = 60% (MAR21.82, Table 11).
    sens.add_commodity_delta("POWER", 3, "1Y", "location", 50_000.0);

    let result = engine.calculate(&sens).expect("commodity delta calculates");

    // Derivation (MAR21.4 / MAR21.6). Every parameter used here matches
    // MAR21 as published:
    //
    //   Weighted sensitivities (WS_k = s_k * RW_b), MAR21.82 Table 11:
    //     WS(COAL_A) = 100_000 * 30.0 = 3_000_000
    //     WS(COAL_B) =  50_000 * 30.0 = 1_500_000
    //     WS(POWER)  =  50_000 * 60.0 = 3_000_000
    //
    //   Intra-bucket (MAR21.83): rho_kl = rho_cty * rho_tenor * rho_basis.
    //   The two bucket-1 factors are different commodities within bucket 1
    //   (rho_cty = 55%, MAR21.83 Table 12 bucket 1), share a tenor
    //   (rho_tenor = 1) and a delivery location (rho_basis = 1), so
    //   rho = 55%:
    //     K_1^2 = 3.0e6^2 + 1.5e6^2 + 2 * 0.55 * 3.0e6 * 1.5e6
    //           = 9.00e12 + 2.25e12 + 4.95e12 = 1.62e13
    //     K_1   = sqrt(1.62e13) = 4_024_922.3594996217
    //     S_1   = 3.0e6 + 1.5e6 = 4_500_000
    //   Bucket 3 holds a single factor: K_3 = S_3 = 3_000_000.
    //
    //   Inter-bucket (MAR21.85(1)): gamma = 20% because buckets 1 and 3 are
    //   both within buckets 1-10:
    //     Delta^2 = K_1^2 + K_3^2 + 2 * 0.20 * S_1 * S_3
    //             = 1.62e13 + 9.0e12 + 0.4 * 4.5e6 * 3.0e6
    //             = 2.52e13 + 5.4e12 = 3.06e13
    //     Delta   = sqrt(3.06e13) = 5_531_726.674375732
    //
    let expected = 5_531_726.674_375_732;
    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::Commodity),
        expected,
        "commodity delta",
    );
    // No vega/curvature/DRC/RRAO in this portfolio, so the total is the delta
    // charge under the single configured scenario.
    assert_charge(result.total, expected, "commodity delta total");
}

#[test]
fn commodity_vega_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::Commodity);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    // Deliberately non-unit sensitivities: the vega risk weight genuinely
    // multiplies here, so the assertion moves if COMMODITY_VEGA_RISK_WEIGHT
    // changes. (The pre-existing unit-input tests in `vega.rs` fed 100.0 and
    // asserted 100.0, which would have passed with no weight applied at all.)
    sens.commodity_vega
        .insert(("COAL_A".to_string(), 1, "1Y".to_string()), 200_000.0);
    sens.commodity_vega
        .insert(("COAL_B".to_string(), 1, "1Y".to_string()), 100_000.0);
    // Bucket 5 = non-precious metals.
    sens.commodity_vega
        .insert(("COPPER".to_string(), 5, "1Y".to_string()), 100_000.0);

    let result = engine.calculate(&sens).expect("commodity vega calculates");

    // Derivation (MAR21.4 / MAR21.6 with the vega risk weight):
    //
    //   Commodity vega risk weight = 100%. MAR21.92 footnote 24 sets
    //   RW_k = min(RW_sigma * sqrt(LH_risk class) / sqrt(10), 100%) with
    //   RW_sigma = 55%; MAR21.92 Table 13 gives the commodity liquidity
    //   horizon LH = 120 days, so 0.55 * sqrt(120/10) = 0.55 * sqrt(12)
    //   = 1.9053, which binds at the 100% cap. Table 13 publishes the
    //   resulting 100% directly.
    //
    //     WS(COAL_A) = 200_000 * 1.00 = 200_000
    //     WS(COAL_B) = 100_000 * 1.00 = 100_000
    //     WS(COPPER) = 100_000 * 1.00 = 100_000
    //
    //   Intra-bucket rho = 55% (MAR21.83 Table 12 bucket 1; same tenor and
    //   location so the other two factors are 1):
    //     K_1^2 = 200_000^2 + 100_000^2 + 2 * 0.55 * 200_000 * 100_000
    //           = 4.0e10 + 1.0e10 + 2.2e10 = 7.2e10
    //     K_1   = sqrt(7.2e10) = 268_328.15729997476, S_1 = 300_000
    //     K_5   = 100_000, S_5 = 100_000
    //
    //   Inter-bucket gamma = 20% (MAR21.85(1)):
    //     Vega^2 = 7.2e10 + 1.0e10 + 2 * 0.20 * 300_000 * 100_000
    //            = 8.2e10 + 1.2e10 = 9.4e10
    //     Vega   = sqrt(9.4e10) = 306_594.1943351178
    let expected = 306_594.194_335_117_8;
    let charge = vega_charge_of(&result, FrtbRiskClass::Commodity);
    assert_charge(charge, expected, "commodity vega");
    assert_charge(result.total, expected, "commodity vega total");

    // Guard against the failure mode called out in the FRTB coverage audit:
    // a 100% weight makes a unit-input test indistinguishable from applying
    // no weight and no aggregation at all.
    let raw_sum: f64 = sens.commodity_vega.values().sum();
    assert!(
        (charge - raw_sum).abs() > 1.0,
        "commodity vega charge must be an aggregation, not a plain sum of sensitivities"
    );
}

#[test]
fn commodity_curvature_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::Commodity);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    // (CVR+, CVR-), loss-positive, supplied already shocked.
    sens.commodity_curvature
        .insert(("COAL_A".to_string(), 1), (500_000.0, -100_000.0));
    sens.commodity_curvature
        .insert(("COAL_B".to_string(), 1), (200_000.0, -40_000.0));
    sens.commodity_curvature
        .insert(("COPPER".to_string(), 5), (300_000.0, -50_000.0));

    let result = engine
        .calculate(&sens)
        .expect("commodity curvature calculates");

    // MAR21.100/101: squared name and bucket correlations; S_1 is the raw 700k.
    let expected = (500_000.0_f64.powi(2)
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.55_f64.powi(2) * 500_000.0 * 200_000.0
        + 300_000.0_f64.powi(2)
        + 2.0 * 0.2_f64.powi(2) * 700_000.0 * 300_000.0)
        .sqrt();
    assert_charge(
        curvature_charge_of(&result, FrtbRiskClass::Commodity),
        expected,
        "commodity curvature",
    );
    assert_charge(result.total, expected, "commodity curvature total");
}

// ---------------------------------------------------------------------------
// CSR securitisation - correlation trading portfolio (CTP)
// ---------------------------------------------------------------------------

#[test]
fn csr_sec_ctp_delta_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    // Bucket 1, RW = 4% and bucket 3, RW = 8% — both match MAR21.59 Table 6
    // as published. Two distinct tranche names share bucket 1 and a tenor, so
    // the intra-bucket name correlation applies.
    sens.csr_sec_ctp_delta.insert(
        (
            "CTP_A".to_string(),
            1,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        100_000.0,
    );
    sens.csr_sec_ctp_delta.insert(
        (
            "CTP_B".to_string(),
            1,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        50_000.0,
    );
    sens.csr_sec_ctp_delta.insert(
        (
            "CTP_C".to_string(),
            3,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        25_000.0,
    );

    let result = engine
        .calculate(&sens)
        .expect("CSR sec CTP delta calculates");

    // MAR21.60/61: different names rho=.35; same tenor/basis; sectors 1/3 gamma=.10.
    let expected = (400_000.0_f64.powi(2)
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.35 * 400_000.0 * 200_000.0
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.10 * 600_000.0 * 200_000.0)
        .sqrt();
    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::CsrSecCtp),
        expected,
        "CSR sec CTP delta",
    );
    assert_charge(result.total, expected, "CSR sec CTP delta total");
}

#[test]
fn csr_sec_ctp_vega_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_ctp_vega
        .insert(("CTP_A".to_string(), 1, "1Y".to_string()), 300_000.0);
    sens.csr_sec_ctp_vega
        .insert(("CTP_B".to_string(), 1, "1Y".to_string()), 100_000.0);
    sens.csr_sec_ctp_vega
        .insert(("CTP_C".to_string(), 5, "1Y".to_string()), 200_000.0);

    let result = engine
        .calculate(&sens)
        .expect("CSR sec CTP vega calculates");

    // MAR21.60/61/94: same expiry; different names rho=.35; sectors 1/5 gamma=.25.
    let expected = (300_000.0_f64.powi(2)
        + 100_000.0_f64.powi(2)
        + 2.0 * 0.35 * 300_000.0 * 100_000.0
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.25 * 400_000.0 * 200_000.0)
        .sqrt();
    let charge = vega_charge_of(&result, FrtbRiskClass::CsrSecCtp);
    assert_charge(charge, expected, "CSR sec CTP vega");
    assert_charge(result.total, expected, "CSR sec CTP vega total");

    let raw_sum: f64 = sens.csr_sec_ctp_vega.values().sum();
    assert!(
        (charge - raw_sum).abs() > 1.0,
        "CSR sec CTP vega charge must be an aggregation, not a plain sum"
    );
}

#[test]
fn csr_sec_ctp_curvature_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_ctp_curvature
        .insert(("CTP_A".to_string(), 1), (400_000.0, -50_000.0));
    sens.csr_sec_ctp_curvature
        .insert(("CTP_B".to_string(), 1), (200_000.0, -20_000.0));
    sens.csr_sec_ctp_curvature
        .insert(("CTP_C".to_string(), 3), (300_000.0, -30_000.0));

    let result = engine
        .calculate(&sens)
        .expect("CSR sec CTP curvature calculates");

    // MAR21.100/101: rho=.35 squared; sectors 1/3 gamma=.10 squared; raw S_1=600k.
    let expected = (400_000.0_f64.powi(2)
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.35_f64.powi(2) * 400_000.0 * 200_000.0
        + 300_000.0_f64.powi(2)
        + 2.0 * 0.1_f64.powi(2) * 600_000.0 * 300_000.0)
        .sqrt();
    assert_charge(
        curvature_charge_of(&result, FrtbRiskClass::CsrSecCtp),
        expected,
        "CSR sec CTP curvature",
    );
    assert_charge(result.total, expected, "CSR sec CTP curvature total");
}

// ---------------------------------------------------------------------------
// CSR securitisation - non-CTP
// ---------------------------------------------------------------------------

#[test]
fn csr_sec_nonctp_delta_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_nonctp_delta.insert(
        (
            "RMBS_A".to_string(),
            1,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        1_000_000.0,
    );
    sens.csr_sec_nonctp_delta.insert(
        (
            "RMBS_B".to_string(),
            1,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        500_000.0,
    );
    sens.csr_sec_nonctp_delta.insert(
        (
            "CMBS_C".to_string(),
            5,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        1_000_000.0,
    );

    let result = engine
        .calculate(&sens)
        .expect("CSR sec non-CTP delta calculates");

    // MAR21.68/70: different tranches rho=.4; same tenor/basis; zero inter-bucket correlation.
    let expected = (900_000.0_f64.powi(2)
        + 450_000.0_f64.powi(2)
        + 2.0 * 0.4 * 900_000.0 * 450_000.0
        + 800_000.0_f64.powi(2))
    .sqrt();
    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
        expected,
        "CSR sec non-CTP delta",
    );
    assert_charge(result.total, expected, "CSR sec non-CTP delta total");
}

#[test]
fn csr_sec_nonctp_vega_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_nonctp_vega
        .insert(("RMBS_A".to_string(), 1, "1Y".to_string()), 300_000.0);
    sens.csr_sec_nonctp_vega
        .insert(("RMBS_B".to_string(), 1, "1Y".to_string()), 100_000.0);
    sens.csr_sec_nonctp_vega
        .insert(("CMBS_C".to_string(), 5, "1Y".to_string()), 200_000.0);

    let result = engine
        .calculate(&sens)
        .expect("CSR sec non-CTP vega calculates");

    // MAR21.68/70/94: different tranches rho=.4; same expiry; zero inter-bucket correlation.
    let expected = (300_000.0_f64.powi(2)
        + 100_000.0_f64.powi(2)
        + 2.0 * 0.4 * 300_000.0 * 100_000.0
        + 200_000.0_f64.powi(2))
    .sqrt();
    let charge = vega_charge_of(&result, FrtbRiskClass::CsrSecNonCtp);
    assert_charge(charge, expected, "CSR sec non-CTP vega");
    assert_charge(result.total, expected, "CSR sec non-CTP vega total");

    let raw_sum: f64 = sens.csr_sec_nonctp_vega.values().sum();
    assert!(
        (charge - raw_sum).abs() > 1.0,
        "CSR sec non-CTP vega charge must be an aggregation, not a plain sum"
    );
}

#[test]
fn csr_sec_nonctp_curvature_matches_mar21_derivation() {
    let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_nonctp_curvature
        .insert(("RMBS_A".to_string(), 1), (400_000.0, -50_000.0));
    sens.csr_sec_nonctp_curvature
        .insert(("RMBS_B".to_string(), 1), (200_000.0, -20_000.0));
    sens.csr_sec_nonctp_curvature
        .insert(("CMBS_C".to_string(), 5), (300_000.0, -30_000.0));

    let result = engine
        .calculate(&sens)
        .expect("CSR sec non-CTP curvature calculates");

    // MAR21.100: rho=.4 squared; zero inter-bucket correlation.
    let expected = (400_000.0_f64.powi(2)
        + 200_000.0_f64.powi(2)
        + 2.0 * 0.4_f64.powi(2) * 400_000.0 * 200_000.0
        + 300_000.0_f64.powi(2))
    .sqrt();
    assert_charge(
        curvature_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
        expected,
        "CSR sec non-CTP curvature",
    );
    assert_charge(result.total, expected, "CSR sec non-CTP curvature total");
}

// ---------------------------------------------------------------------------
// Full engine path: all three correlation scenarios, and cross-class summation
// ---------------------------------------------------------------------------

#[test]
fn commodity_delta_scenario_maximum_binds_at_high_correlation() {
    // Default engine: all three correlation scenarios, maximum taken
    // (MAR21.7).
    let engine = FrtbSbaEngine::new(
        CorrelationScenario::ALL.to_vec(),
        vec![FrtbRiskClass::Commodity],
    )
    .expect("default-scenario engine builds");

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_commodity_delta("COAL_A", 1, "1Y", "location", 100_000.0);
    sens.add_commodity_delta("COAL_B", 1, "1Y", "location", 50_000.0);
    sens.add_commodity_delta("POWER", 3, "1Y", "location", 50_000.0);

    let result = engine.calculate(&sens).expect("commodity delta calculates");

    // Same portfolio as `commodity_delta_matches_mar21_derivation`. All
    // sensitivities share a sign, so a higher correlation can only raise the
    // charge and the High scenario must bind.
    //
    //   High (MAR21.6(2)): rho = min(1.25 * 0.55, 1) = 0.6875,
    //                      gamma = min(1.25 * 0.20, 1) = 0.25
    //     K_1^2 = 9.0e12 + 2.25e12 + 2 * 0.6875 * 3.0e6 * 1.5e6 = 1.74375e13
    //     Delta = 5_760_859.3109014565
    //   Medium: 5_531_726.674375732 (derived above)
    //   Low (MAR21.6(3)): rho = max(2*0.55 - 1, 0.75*0.55) = 0.4125,
    //                     gamma = max(2*0.20 - 1, 0.75*0.20) = 0.15
    //     Delta = 5_292_683.629313205
    let high = 5_760_859.310_901_456_5;
    let medium = 5_531_726.674_375_732;
    let low = 5_292_683.629_313_205;

    let charge_for = |scenario: CorrelationScenario| {
        result
            .scenario_charges
            .get(&scenario)
            .copied()
            .unwrap_or(0.0)
    };

    assert_charge(
        charge_for(CorrelationScenario::High),
        high,
        "commodity delta, high scenario",
    );
    assert_charge(
        charge_for(CorrelationScenario::Medium),
        medium,
        "commodity delta, medium scenario",
    );
    assert_charge(
        charge_for(CorrelationScenario::Low),
        low,
        "commodity delta, low scenario",
    );

    assert_eq!(
        result.binding_scenario,
        CorrelationScenario::High,
        "same-sign sensitivities must bind at the high-correlation scenario"
    );
    assert_charge(result.total, high, "commodity delta total (max scenario)");
}

#[test]
fn previously_uncovered_risk_classes_contribute_additively() {
    // MAR21.7: within a scenario the SBA charge is the plain sum of the
    // per-risk-class delta/vega/curvature charges — the SBA has no
    // cross-risk-class correlation matrix. Pin that all three previously
    // uncovered risk classes reach the result breakdown and add up.
    let engine = FrtbSbaEngine::new(
        vec![CorrelationScenario::Medium],
        vec![
            FrtbRiskClass::Commodity,
            FrtbRiskClass::CsrSecCtp,
            FrtbRiskClass::CsrSecNonCtp,
        ],
    )
    .expect("engine builds");

    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_commodity_delta("COAL_A", 1, "1Y", "location", 100_000.0);
    sens.add_commodity_delta("COAL_B", 1, "1Y", "location", 50_000.0);
    sens.add_commodity_delta("POWER", 3, "1Y", "location", 50_000.0);
    sens.csr_sec_ctp_vega
        .insert(("CTP_A".to_string(), 1, "1Y".to_string()), 300_000.0);
    sens.csr_sec_ctp_vega
        .insert(("CTP_B".to_string(), 1, "1Y".to_string()), 100_000.0);
    sens.csr_sec_ctp_vega
        .insert(("CTP_C".to_string(), 5, "1Y".to_string()), 200_000.0);
    sens.csr_sec_nonctp_curvature
        .insert(("RMBS_A".to_string(), 1), (400_000.0, -50_000.0));
    sens.csr_sec_nonctp_curvature
        .insert(("RMBS_B".to_string(), 1), (200_000.0, -20_000.0));
    sens.csr_sec_nonctp_curvature
        .insert(("CMBS_C".to_string(), 5), (300_000.0, -30_000.0));

    let result = engine.calculate(&sens).expect("multi-class calculates");

    // Component values are the ones derived in the single-class tests above.
    let commodity_delta = 5_531_726.674_375_732;
    let ctp_vega = (201_000_000_000.0_f64).sqrt();
    let nonctp_curvature = (315_600_000_000.0_f64).sqrt();

    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::Commodity),
        commodity_delta,
        "commodity delta component",
    );
    assert_charge(
        vega_charge_of(&result, FrtbRiskClass::CsrSecCtp),
        ctp_vega,
        "CSR sec CTP vega component",
    );
    assert_charge(
        curvature_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
        nonctp_curvature,
        "CSR sec non-CTP curvature component",
    );
    assert_charge(
        result.total,
        commodity_delta + ctp_vega + nonctp_curvature,
        "SBA total is the sum of risk-class components",
    );
}

// ---------------------------------------------------------------------------
// Single-bucket risk-weight coverage
//
// Each uses a single sensitivity in a single bucket, so the charge reduces
// to `|WS|` and is independent of intra- and inter-bucket correlations.
// ---------------------------------------------------------------------------

/// MAR21.67 sets bucket 25 ("other sector") at 3.5%.
#[test]
fn csr_sec_nonctp_bucket_25_uses_the_published_three_and_a_half_percent() {
    let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);
    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.csr_sec_nonctp_delta.insert(
        (
            "OTHER_A".to_string(),
            25,
            "5Y".to_string(),
            "basis".to_string(),
        ),
        1_000_000.0,
    );

    let result = engine.calculate(&sens).expect("bucket 25 delta calculates");

    // 1_000_000 * 3.5 = 3_500_000.
    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
        3_500_000.0,
        "CSR sec non-CTP bucket 25 delta (MAR21.67)",
    );
}

/// MAR21.64 Table 8 publishes buckets 7 and 8 at 1.2% and 1.4%.
#[test]
fn csr_sec_nonctp_buckets_7_and_8_use_published_table_8_weights() {
    for (bucket, weight) in [(7u8, 1.2), (8u8, 1.4)] {
        let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.csr_sec_nonctp_delta.insert(
            (
                "ABS".to_string(),
                bucket,
                "5Y".to_string(),
                "basis".to_string(),
            ),
            1_000_000.0,
        );

        let result = engine.calculate(&sens).expect("delta calculates");
        assert_charge(
            delta_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
            1_000_000.0 * weight,
            "CSR sec non-CTP Table 8 weight",
        );
    }
}

/// MAR21.65 derives bucket 13 as 1.25 x bucket 5 (0.8%) = 1.0%; it was 5.0%.
/// MAR21.66 derives bucket 21 as 1.75 x bucket 5 = 1.4%; it was 5.0%.
#[test]
fn csr_sec_nonctp_derived_buckets_scale_from_the_base_row() {
    for (bucket, weight) in [(13u8, 0.8 * 1.25), (21u8, 0.8 * 1.75)] {
        let engine = medium_engine(FrtbRiskClass::CsrSecNonCtp);
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.csr_sec_nonctp_delta.insert(
            (
                "ABS".to_string(),
                bucket,
                "5Y".to_string(),
                "basis".to_string(),
            ),
            1_000_000.0,
        );

        let result = engine.calculate(&sens).expect("delta calculates");
        assert_charge(
            delta_charge_of(&result, FrtbRiskClass::CsrSecNonCtp),
            1_000_000.0 * weight,
            "CSR sec non-CTP derived weight (MAR21.65/21.66)",
        );
    }
}

/// MAR21.53 Table 4 publishes bucket 8 (covered bonds) at 2.5%; it was
/// implemented at 1.0%, understating the charge by 60%.
#[test]
fn csr_nonsec_bucket_8_covered_bonds_uses_the_published_weight() {
    let engine = medium_engine(FrtbRiskClass::CsrNonSec);
    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_csr_nonsec_delta("COVERED_A", 8, "5Y", "bond", 1_000_000.0);

    let result = engine.calculate(&sens).expect("bucket 8 delta calculates");

    // 1_000_000 * 2.5 = 2_500_000.
    assert_charge(
        delta_charge_of(&result, FrtbRiskClass::CsrNonSec),
        2_500_000.0,
        "CSR non-sec bucket 8 delta (MAR21.53 Table 4)",
    );
}
