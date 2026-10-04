//! Independent historical SIMM v2.6 and Basel MAR21.93 regression vectors.
//! ISDA source: September 2023 methodology, paragraphs 33, 39, 69, 74, 79.

use finstack_quant_core::{currency::Currency, HashMap};
use finstack_quant_margin::regulatory::frtb::types::{
    CorrelationScenario, FrtbRiskClass, FrtbSensitivities,
};
use finstack_quant_margin::{SimmCalculator, SimmCreditSector, SimmVersion};

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn official_ir_weights_and_currency_concentration() {
    let calculator = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
    // USD regular, JPY low-volatility and BRL high-volatility; section D.1.
    for (currency, weight) in [
        (Currency::USD, 60.0),
        (Currency::JPY, 23.0),
        (Currency::BRL, 97.0),
    ] {
        close(
            ir_delta(
                &calculator,
                &HashMap::from_iter([((currency, "5Y".to_owned()), 100.0)]),
            ),
            weight * 100.0,
        );
    }
    // USD threshold 330 USD million per bp; raw DV01 is four times threshold.
    close(
        ir_delta(
            &calculator,
            &HashMap::from_iter([((Currency::USD, "5Y".to_owned()), 1_320_000_000.0)]),
        ),
        1_320_000_000.0 * 60.0 * 2.0,
    );
}

#[test]
fn credit_concentration_uses_raw_name_sensitivity() {
    let calculator = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
    // Financial bucket: RW 90bp, raw CS01 threshold USD170k/bp.
    close(
        credit_delta(
            &calculator,
            &HashMap::from_iter([(
                (
                    SimmCreditSector::Financial,
                    "BANK".to_owned(),
                    "5Y".to_owned(),
                ),
                100_000.0,
            )]),
        ),
        9_000_000.0,
    );
}

#[test]
fn fx_concentration_and_weights_use_percent_sensitivity_units() {
    let calculator = SimmCalculator::new(SimmVersion::V2_6).expect("embedded registry");
    // EUR/USD: 7.4 percentage-point RW; EUR category-one CT=USD3.3bn/%.
    close(
        fx_delta(
            &calculator,
            &HashMap::from_iter([(Currency::EUR, 10_000_000.0)]),
        ),
        74_000_000.0,
    );
    close(
        fx_delta(
            &calculator,
            &HashMap::from_iter([(Currency::BRL, 10_000_000.0)]),
        ),
        147_000_000.0,
    );
    // The calculation currency itself has no FX exposure.
    close(
        fx_delta(
            &calculator,
            &HashMap::from_iter([(Currency::USD, 10_000_000.0)]),
        ),
        0.0,
    );
}

#[test]
fn girr_vega_uses_one_percent_decay_for_both_maturities() {
    let mut sensitivities = FrtbSensitivities::new(Currency::USD);
    sensitivities
        .girr_vega
        .insert((Currency::USD, "5Y".into(), "1Y".into()), 100.0);
    sensitivities
        .girr_vega
        .insert((Currency::USD, "5Y".into(), "5Y".into()), 100.0);
    let expected = (20_000.0 + 20_000.0 * (-0.01_f64 * 4.0).exp()).sqrt();
    close(
        vega_charge(
            FrtbRiskClass::Girr,
            &sensitivities,
            CorrelationScenario::Medium,
        ),
        expected,
    );
}

#[test]
fn curvature_scales_expiries_before_factor_netting_and_ir_uses_hvr() {
    use finstack_quant_margin::{SimmCurvatureSensitivity, SimmRiskClass};
    let calc = SimmCalculator::new(finstack_quant_margin::SimmVersion::default())
        .expect("embedded registry");
    let mut inputs: Vec<_> = [("2W", 1000.0), ("1Y", -1000.0)]
        .into_iter()
        .map(|(expiry, vega)| SimmCurvatureSensitivity {
            risk_class: SimmRiskClass::Equity,
            bucket: "residual".into(),
            factor: "ACME".into(),
            risk_tenor: None,
            expiry_tenor: expiry.into(),
            volatility_weighted_vega: vega,
        })
        .collect();
    let z2 = 2.5758293035489004_f64.powi(2);
    let expected = (500.0 - 7000.0 / 365.0) * z2;
    close(curvature(&calc, &inputs).expect("curvature"), expected);
    inputs.reverse();
    close(curvature(&calc, &inputs).expect("permuted"), expected);
    for input in &mut inputs {
        input.risk_class = SimmRiskClass::InterestRate;
        input.bucket = "USD".into();
        input.factor = "OIS".into();
        input.risk_tenor = Some("5Y".into());
    }
    close(
        curvature(&calc, &inputs).expect("IR curvature"),
        expected / 0.47_f64.powi(2),
    );
    inputs[0].expiry_tenor = "7Y".into();
    assert!(curvature(&calc, &inputs).is_err());
}

fn component_charge(
    class: FrtbRiskClass,
    sens: &FrtbSensitivities,
    scenario: CorrelationScenario,
    component: &str,
) -> f64 {
    let result =
        finstack_quant_margin::regulatory::frtb::FrtbSbaEngine::new(vec![scenario], vec![class])
            .expect("selection")
            .calculate(sens)
            .expect("validated sensitivity fixture");
    let charges = match component {
        "delta" => result.delta_by_risk_class,
        "vega" => result.vega_by_risk_class,
        _ => result.curvature_by_risk_class,
    };
    charges.get(&class).copied().unwrap_or(0.0)
}
fn vega_charge(
    class: FrtbRiskClass,
    sens: &FrtbSensitivities,
    scenario: CorrelationScenario,
) -> f64 {
    component_charge(class, sens, scenario, "vega")
}

fn ir_delta(calc: &SimmCalculator, values: &HashMap<(Currency, String), f64>) -> f64 {
    let mut sens = finstack_quant_margin::SimmSensitivities::new(Currency::USD);
    sens.ir_delta = values.clone();
    calc.calculate_from_sensitivities_parts(&sens, Currency::USD)
        .expect("valid IR fixture")
        .0
}
fn credit_delta(
    calc: &SimmCalculator,
    values: &HashMap<(SimmCreditSector, String, String), f64>,
) -> f64 {
    let mut sens = finstack_quant_margin::SimmSensitivities::new(Currency::USD);
    sens.credit_qualifying_delta = values.clone();
    calc.calculate_from_sensitivities_parts(&sens, Currency::USD)
        .expect("valid credit fixture")
        .0
}
fn fx_delta(calc: &SimmCalculator, values: &HashMap<Currency, f64>) -> f64 {
    let mut sens = finstack_quant_margin::SimmSensitivities::new(Currency::USD);
    sens.fx_delta = values.clone();
    calc.calculate_from_sensitivities_parts(&sens, Currency::USD)
        .expect("valid FX fixture")
        .0
}
fn curvature(
    calc: &SimmCalculator,
    values: &[finstack_quant_margin::SimmCurvatureSensitivity],
) -> finstack_quant_core::Result<f64> {
    let mut sens = finstack_quant_margin::SimmSensitivities::new(Currency::USD);
    sens.curvature = values.to_vec();
    Ok(calc
        .calculate_from_sensitivities_parts(&sens, Currency::USD)?
        .0)
}
