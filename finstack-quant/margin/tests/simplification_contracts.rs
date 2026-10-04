//! Regressions for the simplified canonical margin boundaries.
use finstack_quant_core::currency::Currency;
use finstack_quant_margin::regulatory::frtb::{
    DrcPosition, DrcSector, DrcSeniority, FrtbSbaEngine, FrtbSensitivities,
};
use finstack_quant_margin::{CsaSpec, MarginTenor, OtcMarginSpec};

fn position() -> DrcPosition {
    DrcPosition {
        issuer: "ACME".into(),
        jtd_amount: 1_000_000.0,
        rating_bucket: 4,
        sector: DrcSector::Corporate,
        seniority: DrcSeniority::SeniorUnsecured,
        maturity_years: 2.0,
        pnl_adjustment: 0.0,
    }
}

#[test]
fn container_and_engine_share_complete_drc_validation() {
    let invalid = [
        DrcPosition {
            maturity_years: -1.0,
            ..position()
        },
        DrcPosition {
            maturity_years: f64::NAN,
            ..position()
        },
        DrcPosition {
            seniority: DrcSeniority::Equity,
            sector: DrcSector::Sovereign,
            ..position()
        },
        DrcPosition {
            rating_bucket: 0,
            ..position()
        },
        DrcPosition {
            pnl_adjustment: f64::INFINITY,
            ..position()
        },
    ];
    for pos in invalid {
        assert!(pos.validate().is_err());
        let mut sens = FrtbSensitivities::new(Currency::USD);
        sens.add_drc_position(pos);
        assert!(sens.validate().is_err());
        assert!(FrtbSbaEngine::default().calculate(&sens).is_err());
    }
    let mut sens = FrtbSensitivities::new(Currency::USD);
    sens.add_drc_position(position());
    assert_eq!(
        FrtbSbaEngine::default()
            .calculate(&sens)
            .expect("supported DRC")
            .drc,
        45_000.0
    );
}

#[test]
fn drc_wire_rejects_retired_or_unsupported_classification() {
    let mut value = serde_json::to_value(position()).expect("serialize position");
    value["asset_type"] = serde_json::json!("corporate");
    assert!(serde_json::from_value::<DrcPosition>(value).is_err());
    let mut value = serde_json::to_value(position()).expect("serialize position");
    value["seniority"] = serde_json::json!("securitization");
    assert!(serde_json::from_value::<DrcPosition>(value).is_err());
}

#[test]
fn otc_uses_csa_elections_and_preserves_no_im() {
    let mut csa = CsaSpec::usd_regulatory().expect("embedded registry");
    csa.im_params = None;
    csa.vm_params.frequency = MarginTenor::Weekly;
    csa.vm_params.settlement_lag = 3;
    let spec = OtcMarginSpec::bilateral_simm(csa.clone());
    assert_eq!(spec.get_im_methodology(), None);
    spec.validate().expect("no IM election is valid");
    assert_eq!(spec.csa, csa);
    let value = serde_json::to_value(&spec).expect("serialize OTC");
    for key in ["im_methodology", "vm_frequency", "settlement_lag"] {
        assert!(value.get(key).is_none());
    }
    let mut retired = value;
    retired["im_methodology"] = serde_json::json!("simm");
    assert!(serde_json::from_value::<OtcMarginSpec>(retired).is_err());
}

#[test]
fn generic_ccp_wire_describes_only_the_actual_proxy() {
    use finstack_quant_margin::calculators::CcpMethodology;
    assert_eq!(
        CcpMethodology::from_ccp_name("unknown CCP"),
        CcpMethodology::GenericProxy
    );
    assert_eq!(
        serde_json::to_value(CcpMethodology::GenericProxy).expect("serialize proxy"),
        serde_json::json!("generic_proxy")
    );
    assert!(serde_json::from_value::<CcpMethodology>(
        serde_json::json!({"generic_var": {"confidence": 0.99, "lookback_days": 250}})
    )
    .is_err());
}

#[test]
fn published_simm_wire_examples_are_accepted_by_the_calculator() {
    use finstack_quant_margin::types::SimmSensitivitiesJson;
    use finstack_quant_margin::{SimmCalculator, SimmSensitivities, SimmVersion};
    let artifact = finstack_quant_margin::schema::ARTIFACTS
        .iter()
        .find(|a| {
            a.relative_path
                .ends_with("simm_sensitivities_json.schema.json")
        })
        .expect("canonical SIMM wire artifact");
    let schema = artifact.generate().expect("generate schema");
    let calculator = SimmCalculator::new(SimmVersion::default()).expect("embedded registry");
    for example in schema["examples"].as_array().expect("published examples") {
        let wire: SimmSensitivitiesJson =
            serde_json::from_value(example.clone()).expect("decode wire example");
        let sensitivities: SimmSensitivities = wire.into();
        calculator
            .calculate_from_sensitivities_parts(&sensitivities, Currency::USD)
            .expect("example accepted by model");
    }
}
