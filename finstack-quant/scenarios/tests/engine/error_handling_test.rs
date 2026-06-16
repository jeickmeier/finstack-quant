//! Tests for error construction and display formatting.

use finstack_quant_scenarios::Error;

#[test]
fn test_market_data_not_found_error() {
    let error = Error::MarketDataNotFound {
        id: "USD_SOFR".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Market data not found"));
    assert!(display.contains("USD_SOFR"));
}

#[test]
fn test_node_not_found_error() {
    let error = Error::NodeNotFound {
        node_id: "Revenue".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Node not found"));
    assert!(display.contains("Revenue"));
}

#[test]
fn test_tenor_not_found_error() {
    let error = Error::TenorNotFound {
        tenor: "3Y".to_string(),
        curve_id: "USD_SOFR".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Tenor not found"));
    assert!(display.contains("3Y"));
    assert!(display.contains("USD_SOFR"));
}

#[test]
fn test_invalid_tenor_error() {
    let error = Error::InvalidTenor("bad_tenor".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Invalid tenor string"));
    assert!(display.contains("bad_tenor"));
}

#[test]
fn test_invalid_period_error() {
    let error = Error::InvalidPeriod("bad_period".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Invalid time period"));
    assert!(display.contains("bad_period"));
}

#[test]
fn test_unsupported_operation_error() {
    let error = Error::UnsupportedOperation {
        operation: "parallel_bump".to_string(),
        target: "curve_xyz".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Unsupported operation"));
    assert!(display.contains("parallel_bump"));
    assert!(display.contains("curve_xyz"));
}

#[test]
fn test_validation_error() {
    let error = Error::Validation("Invalid input".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Validation error"));
    assert!(display.contains("Invalid input"));
}

#[test]
fn test_internal_error() {
    let error = Error::Internal("Something went wrong".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Internal error"));
    assert!(display.contains("Something went wrong"));
}

#[test]
fn test_instrument_not_found_error() {
    let error = Error::InstrumentNotFound("BOND123".to_string());

    let display = format!("{}", error);
    assert!(display.contains("Instrument not found"));
    assert!(display.contains("BOND123"));
}

#[test]
fn test_curve_type_mismatch_error() {
    let error = Error::CurveTypeMismatch {
        expected: "Discount".to_string(),
        actual: "Forward".to_string(),
    };

    let display = format!("{}", error);
    assert!(display.contains("Curve type mismatch"));
    assert!(display.contains("Discount"));
    assert!(display.contains("Forward"));
}

#[test]
fn test_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Error>();
}

#[test]
fn test_error_is_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<Error>();
}
