//! Independent transcription of historical ISDA SIMM v2.6 tables.
//!
//! Source: https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf
//! PDF SHA256 57e9d2e9e080e27abc92197923b019e40eb2fe60d9c82ce2a0f8238b7c4bf8e0.
//! Reread 2026-09-09. Provenance: tests/fixtures/simm_v26_provenance.md.
//! These replace incorrectly attributed self-captured values, not external
//! Bloomberg/QuantLib vectors. Formula checks live in production_simm_csa_audit.

use finstack_quant_margin::{SimmCalculator, SimmRiskClass, SimmVersion, SIMM_TENORS};

#[test]
fn published_ir_weights_and_correlations() {
    let calc = SimmCalculator::new(SimmVersion::V2_6).expect("registry");
    let p = &calc.params;
    // D.1 tables 1-3, printed page 14.
    for (table, expected) in [
        (
            &p.ir_delta_weights,
            [109., 105., 90., 71., 66., 66., 64., 60., 60., 61., 61., 67.],
        ),
        (
            &p.ir_delta_weights_low,
            [15., 18., 9., 11., 13., 15., 19., 23., 23., 22., 22., 23.],
        ),
        (
            &p.ir_delta_weights_high,
            [
                163., 109., 87., 89., 102., 96., 101., 97., 97., 102., 106., 101.,
            ],
        ),
    ] {
        assert_eq!(table.len(), 12);
        for (tenor, weight) in SIMM_TENORS.iter().zip(expected) {
            assert_eq!(table[*tenor], weight);
        }
    }
    // D.2 paragraph 36, printed pages 14-15, strict upper triangle in tenor order.
    let rows: &[&[f64]] = &[
        &[77., 67., 59., 48., 39., 34., 30., 25., 23., 21., 20.],
        &[84., 74., 56., 43., 36., 31., 26., 21., 19., 19.],
        &[88., 69., 55., 47., 40., 34., 27., 25., 25.],
        &[86., 73., 65., 57., 49., 40., 38., 37.],
        &[94., 87., 79., 68., 60., 57., 55.],
        &[96., 91., 80., 74., 70., 69.],
        &[97., 88., 81., 77., 76.],
        &[95., 90., 86., 85.],
        &[97., 94., 94.],
        &[98., 97.],
        &[99.],
    ];
    assert_eq!(p.ir_tenor_correlations.len(), 66);
    for (i, row) in rows.iter().enumerate() {
        for (offset, percent) in row.iter().enumerate() {
            let a = SIMM_TENORS[i].to_owned();
            let b = SIMM_TENORS[i + offset + 1].to_owned();
            let actual = p
                .ir_tenor_correlations
                .get(&(a.clone(), b.clone()))
                .or_else(|| p.ir_tenor_correlations.get(&(b, a)))
                .expect("pair");
            assert_eq!(*actual, percent / 100.0);
        }
    }
    assert_eq!(p.ir_inter_currency_correlation, 0.32);
    assert_eq!(p.ir_subcurve_correlation, 0.993);
    assert_eq!(p.ir_historical_volatility_ratio, 0.47);
    assert_eq!(p.ir_vega_weight, 0.23);
}

#[test]
fn published_non_ir_weights_and_historical_ratios() {
    let calc = SimmCalculator::default();
    let p = &calc.params;
    // E through I: selected supported residual classes and every commodity bucket.
    assert_eq!(p.cnq_delta_weight, 1300.0);
    assert_eq!(p.equity_delta_weight, 50.0);
    assert_eq!(p.fx_delta_weight, 7.4);
    assert_eq!(p.fx_high_delta_weight, 14.7);
    assert_eq!(p.cq_vega_weight, 0.76);
    assert_eq!(p.cnq_vega_weight, 0.76);
    assert_eq!(p.equity_vega_weight, 0.45);
    assert_eq!(p.fx_vega_weight, 0.48);
    assert_eq!(p.commodity_vega_weight, 0.55);
    assert_eq!(p.equity_historical_volatility_ratio, 0.60);
    assert_eq!(p.fx_historical_volatility_ratio, 0.57);
    assert_eq!(p.commodity_historical_volatility_ratio, 0.74);
    assert_eq!(p.cq_same_issuer_correlation, 0.93);
    assert_eq!(p.cq_intra_bucket_correlation, 0.46);
    assert_eq!(p.credit_residual_correlation, 0.50);
    assert_eq!(p.fx_intra_bucket_correlation, 0.50);
    assert_eq!(p.fx_regular_high_correlation, 0.25);
    assert_eq!(p.fx_high_high_correlation, -0.05);
    let weights = [
        48., 29., 33., 25., 35., 30., 60., 52., 68., 63., 21., 21., 15., 16., 13., 68., 17.,
    ];
    let correlations = [
        83., 97., 93., 97., 98., 90., 98., 49., 80., 46., 58., 53., 62., 16., 18., 0., 38.,
    ];
    for i in 0..17 {
        let key = (i + 1).to_string();
        assert_eq!(p.commodity_bucket_weights[&key], weights[i]);
        assert_eq!(
            p.commodity_intra_bucket_correlations[&key],
            correlations[i] / 100.0
        );
    }
}

#[test]
fn published_raw_concentration_thresholds() {
    let calc = SimmCalculator::default();
    let p = &calc.params;
    // J, printed pages 26-28. Table units USD millions, per bp/% for delta.
    for (key, delta, vega) in [
        ("high", 30., 74.),
        ("regular_well_traded", 330., 4900.),
        ("regular_less_traded", 130., 520.),
        ("low", 61., 970.),
    ] {
        assert_eq!(p.ir_delta_concentration_thresholds[key], delta * 1e6);
        assert_eq!(p.ir_vega_concentration_thresholds[key], vega * 1e6);
    }
    for (key, threshold) in [("1", 3300.), ("2", 880.), ("3", 170.)] {
        assert_eq!(p.fx_delta_concentration_thresholds[key], threshold * 1e6);
    }
    for (key, threshold) in [
        ("1_1", 2800.),
        ("1_2", 1400.),
        ("1_3", 590.),
        ("2_2", 520.),
        ("2_3", 340.),
        ("3_3", 210.),
    ] {
        assert_eq!(p.fx_vega_concentration_thresholds[key], threshold * 1e6);
    }
    let delta = [
        310., 2100., 1700., 1700., 1700., 2800., 2800., 2700., 2700., 52., 530., 1300., 100., 100.,
        100., 52., 4000.,
    ];
    let vega = [
        390., 2900., 310., 310., 310., 6300., 6300., 1200., 1200., 120., 390., 1300., 590., 590.,
        590., 69., 69.,
    ];
    for i in 0..17 {
        let key = (i + 1).to_string();
        assert_eq!(
            p.commodity_delta_concentration_thresholds[&key],
            delta[i] * 1e6
        );
        assert_eq!(
            p.commodity_vega_concentration_thresholds[&key],
            vega[i] * 1e6
        );
    }
    for (class, delta, vega) in [
        (SimmRiskClass::Equity, 0.37, 39.),
        (SimmRiskClass::CreditNonQualifying, 0.50, 70.),
    ] {
        assert_eq!(p.concentration_thresholds[&class], delta * 1e6);
        assert_eq!(p.vega_concentration_thresholds[&class], vega * 1e6);
    }
    assert_eq!(
        p.vega_concentration_thresholds[&SimmRiskClass::CreditQualifying],
        360e6
    );
}

#[test]
fn published_cross_risk_class_correlations() {
    use SimmRiskClass::*;
    let calc = SimmCalculator::default();
    let p = &calc.params;
    let classes = [
        InterestRate,
        CreditQualifying,
        CreditNonQualifying,
        Equity,
        Commodity,
        Fx,
    ];
    let rows: &[&[f64]] = &[
        &[4., 4., 7., 37., 14.],
        &[54., 70., 27., 37.],
        &[46., 24., 15.],
        &[35., 39.],
        &[35.],
    ];
    for (i, row) in rows.iter().enumerate() {
        for (j, percent) in row.iter().enumerate() {
            let a = classes[i];
            let b = classes[i + j + 1];
            assert_eq!(
                *p.risk_class_correlations
                    .get(&(a, b))
                    .or_else(|| p.risk_class_correlations.get(&(b, a)))
                    .expect("pair"),
                percent / 100.0
            );
        }
    }
}
