use super::error::*;

#[test]
fn core_error_kind_is_selected_from_variant_not_message() {
    use finstack_quant_core::Error;
    use finstack_quant_core::InputError;

    let cases = [
        // A curve literally named "validation" must still classify as a
        // lookup miss — variant, not message sniffing.
        (
            Error::Input(InputError::MissingCurve {
                requested: "validation".to_string(),
                suggestions: vec![],
            }),
            "not_found",
        ),
        (
            Error::Input(InputError::CalendarNotFound {
                requested: "must be valid".to_string(),
                suggestions: vec![],
            }),
            "not_found",
        ),
        (
            Error::Input(InputError::NotFound {
                id: "invalid id containing 'not found'".to_string(),
            }),
            "not_found",
        ),
        (
            Error::Input(InputError::SolverConvergenceFailed {
                iterations: 1,
                residual: 1.0,
                last_x: 0.0,
                reason: "x".to_string(),
            }),
            "computation",
        ),
        (
            Error::CircularDependency {
                path: vec!["a".to_string()],
            },
            "computation",
        ),
        // Calibration stays "computation" even though its message contains
        // every keyword a message sniffer would match.
        (
            Error::Calibration {
                message: "not found: invalid value must be positive".to_string(),
                category: String::new(),
            },
            "computation",
        ),
        // Genuine validation keeps its kind even when an embedded
        // identifier contains "not found".
        (
            Error::Validation("curve 'not found quotes' rejected".to_string()),
            "validation",
        ),
        (Error::Input(InputError::NegativeValue), "validation"),
        (
            Error::Input(InputError::VolatilityConversionFailed {
                tolerance: 1e-8,
                residual: 1e-3,
            }),
            "computation",
        ),
        (
            Error::MetricCalculationFailed {
                metric_id: "dv01".to_string(),
                cause: Box::new(Error::Calibration {
                    message: "solver stalled".to_string(),
                    category: String::new(),
                }),
            },
            "computation",
        ),
        (
            Error::MetricCalculationFailed {
                metric_id: "dv01".to_string(),
                cause: Box::new(Error::Input(InputError::SolverConvergenceFailed {
                    iterations: 1,
                    residual: 1.0,
                    last_x: 0.0,
                    reason: "x".to_string(),
                })),
            },
            "computation",
        ),
        (
            Error::MetricCalculationFailed {
                metric_id: "ytm".to_string(),
                cause: Box::new(Error::Validation("bad quote".to_string())),
            },
            "validation",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.js_kind(), expected);
    }
}
