use super::*;

#[test]
fn error_kind_labels_match_serde() {
    for kind in [
        ErrorKind::NotFound,
        ErrorKind::Validation,
        ErrorKind::Computation,
    ] {
        assert_eq!(
            serde_json::to_value(kind).expect("kind serializes"),
            serde_json::Value::String(kind.as_str().to_string())
        );
    }
}

#[test]
fn format_chain_joins_sources_without_repeating_embedded_text() {
    #[derive(Debug, thiserror::Error)]
    #[error("outer")]
    struct Outer(#[source] Error);
    let chain = format_chain(&Outer(Error::Validation("bad".to_string())));
    assert_eq!(chain, "outer: Validation error: bad");

    #[derive(Debug, thiserror::Error)]
    #[error("wrapped: {0}")]
    struct Embedding(#[source] Error);
    let chain = format_chain(&Embedding(Error::Validation("bad".to_string())));
    assert_eq!(chain, "wrapped: Validation error: bad");
}

#[test]
fn test_display() {
    let err: Error = InputError::NegativeValue.into();
    assert_eq!(format!("{}", err), "Values must be non-negative");

    let not_found_err: Error = InputError::NotFound {
        id: "test_curve".to_string(),
    }
    .into();
    assert_eq!(
        format!("{}", not_found_err),
        "Requested item not found: test_curve"
    );
}

#[test]
fn test_missing_curve_with_suggestions() {
    let available = vec![
        "USD_OIS".to_string(),
        "USD_GOVT".to_string(),
        "EUR_OIS".to_string(),
        "GBP_GILT".to_string(),
    ];

    let err = Error::missing_curve_with_suggestions("USD_OS", &available);
    let msg = format!("{}", err);
    assert!(msg.contains("USD_OIS") || msg.contains("Did you mean"));

    let err2 = Error::missing_curve_with_suggestions("USD", &available);
    let msg2 = format!("{}", err2);
    assert!(msg2.contains("USD_OIS") || msg2.contains("USD_GOVT"));

    let err3 = Error::missing_curve_with_suggestions("JPY_UNKNOWN", &available);
    let msg3 = format!("{}", err3);
    assert!(msg3.contains("Curve not found"));
}

#[test]
fn test_unknown_metric() {
    let available = vec![
        "dv01".to_string(),
        "duration_mod".to_string(),
        "ytm".to_string(),
    ];
    let err = Error::UnknownMetric {
        metric_id: "dv1".to_string(),
        available,
    };

    let msg = format!("{}", err);
    assert!(msg.contains("Unknown metric 'dv1'"));

    match err {
        Error::UnknownMetric {
            metric_id,
            available: avail,
        } => {
            assert_eq!(metric_id, "dv1");
            assert_eq!(avail.len(), 3);
            assert!(avail.contains(&"dv01".to_string()));
        }
        _ => panic!("Expected UnknownMetric variant"),
    }
}

#[test]
fn test_metric_not_applicable() {
    let err = Error::MetricNotApplicable {
        metric_id: "ytm".to_string(),
        instrument_type: "Swap".to_string(),
    };

    let msg = format!("{}", err);
    assert!(msg.contains("ytm"));
    assert!(msg.contains("Swap"));
    assert!(msg.contains("not applicable"));

    match err {
        Error::MetricNotApplicable {
            metric_id,
            instrument_type,
        } => {
            assert_eq!(metric_id, "ytm");
            assert_eq!(instrument_type, "Swap");
        }
        _ => panic!("Expected MetricNotApplicable variant"),
    }
}

#[test]
fn test_metric_calculation_failed() {
    let cause = Error::Input(InputError::NotFound {
        id: "missing_curve".to_string(),
    });
    let err = Error::MetricCalculationFailed {
        metric_id: "dv01".to_string(),
        cause: Box::new(cause.clone()),
    };

    let msg = format!("{}", err);
    assert!(msg.contains("dv01"));
    assert!(msg.contains("calculation failed"));
    assert!(msg.contains("missing_curve"));

    match err {
        Error::MetricCalculationFailed {
            metric_id,
            cause: boxed_cause,
        } => {
            assert_eq!(metric_id, "dv01");
            assert_eq!(*boxed_cause, cause);
        }
        _ => panic!("Expected MetricCalculationFailed variant"),
    }
}

#[test]
fn test_circular_dependency() {
    let path = vec![
        "metric_a".to_string(),
        "metric_b".to_string(),
        "metric_c".to_string(),
        "metric_a".to_string(),
    ];
    let err = Error::CircularDependency { path: path.clone() };

    let msg = format!("{}", err);
    assert!(msg.contains("Circular dependency"));
    assert!(msg.contains("metric_a -> metric_b -> metric_c -> metric_a"));

    match err {
        Error::CircularDependency { path: cycle_path } => {
            assert_eq!(cycle_path, path);
            assert_eq!(cycle_path.len(), 4);
            assert_eq!(cycle_path[0], cycle_path[3]); // Cycle back to start
        }
        _ => panic!("Expected CircularDependency variant"),
    }
}

#[test]
fn test_metric_errors_are_clonable() {
    let err1 = Error::UnknownMetric {
        metric_id: "test".to_string(),
        available: vec!["dv01".to_string()],
    };
    let err2 = err1.clone();
    assert_eq!(err1, err2);

    let err3 = Error::MetricNotApplicable {
        metric_id: "ytm".to_string(),
        instrument_type: "Swap".to_string(),
    };
    let err4 = err3.clone();
    assert_eq!(err3, err4);

    let cause = Error::Internal("metric test failure".into());
    let err5 = Error::MetricCalculationFailed {
        metric_id: "test".to_string(),
        cause: Box::new(cause),
    };
    let err6 = err5.clone();
    assert_eq!(err5, err6);

    let err7 = Error::CircularDependency {
        path: vec!["a".to_string(), "b".to_string()],
    };
    let err8 = err7.clone();
    assert_eq!(err7, err8);
}

#[test]
fn internal_error_carries_context() {
    let err = Error::Internal("bootstrap diverged".into());
    let msg = err.to_string();
    assert!(
        msg.contains("bootstrap diverged"),
        "Error should contain context: {msg}"
    );
}
