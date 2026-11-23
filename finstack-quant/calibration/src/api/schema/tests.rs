use super::envelope::*;
use super::steps::*;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};

mod pointer_tests {
    use super::{FINAL_MARKET_POINTER, prefix_json_pointer};

    #[test]
    fn nested_diagnostic_pointer_join_is_rfc_6901_safe() {
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("/schema_version")),
            "/result/final_market/schema_version"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("/")),
            "/result/final_market/"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("desk/USD~CSA")),
            "/result/final_market/desk~1USD~0CSA"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, None),
            FINAL_MARKET_POINTER
        );
    }
}

mod interpolation_default_tests {
    use super::*;

    #[test]
    fn production_curve_interpolation_defaults_are_shape_safe() {
        let discount: DiscountCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-OIS",
            "currency": "USD",
            "base_date": "2025-01-01"
        }))
        .expect("discount params");
        assert_eq!(discount.interpolation, InterpStyle::LogLinear);

        let forward: ForwardCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-SOFR-3M",
            "currency": "USD",
            "base_date": "2025-01-01",
            "discount_curve_id": "USD-OIS"
        }))
        .expect("forward params");
        assert_eq!(forward.interpolation, InterpStyle::LogLinear);
        assert_eq!(forward.extrapolation, ExtrapolationPolicy::Extend);

        let xccy: XccyBasisParams = serde_json::from_value(serde_json::json!({
            "curve_id": "EUR-OIS",
            "currency": "EUR",
            "base_date": "2025-01-01",
            "fx_spot": 1.1,
            "domestic_discount_id": "USD-OIS"
        }))
        .expect("xccy params");
        assert_eq!(xccy.interpolation, InterpStyle::LogLinear);
    }
}

mod step_report_tests {
    use super::{CalibrationEnvelope, CalibrationPlan};

    #[test]
    fn step_report_names_the_available_steps_when_missing() {
        let plan = CalibrationPlan {
            id: "empty".to_string(),
            description: None,
            quote_sets: Default::default(),
            steps: Vec::new(),
            settings: Default::default(),
        };
        let envelope = CalibrationEnvelope::new(plan, Vec::new(), Vec::new());
        let result = crate::api::engine::calibrate(&envelope).expect("empty plan calibrates");
        let error = result
            .result
            .step_report("USD-OIS")
            .expect_err("no such step");
        assert_eq!(error.kind(), finstack_quant_core::ErrorKind::NotFound);
        assert!(
            error
                .to_string()
                .contains("calibration step 'USD-OIS'; available steps: []")
        );
    }
}
