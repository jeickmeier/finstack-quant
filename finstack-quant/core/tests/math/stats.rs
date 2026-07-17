//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::math::stats::{
    log_returns, realized_variance, realized_variance_ohlc, RealizedVarMethod,
};
use finstack_quant_core::math::{correlation, covariance, mean, mean_var, variance};

#[test]
fn mean_and_variance_basic() {
    let xs = [1.0, 2.0, 3.0, 4.0];
    let m = mean(&xs);
    let v = variance(&xs);
    let (m2, v2) = mean_var(&xs);
    assert!((m - 2.5).abs() < 1e-12);
    // Sample variance of [1,2,3,4]: SS=5, n-1=3, var=5/3
    assert!((v - 5.0 / 3.0).abs() < 1e-12);
    assert!((m - m2).abs() < 1e-12);
    assert!((v - v2).abs() < 1e-12);
}

#[test]
fn covariance_and_correlation() {
    let x = [1.0, 2.0, 3.0, 4.0];
    let y = [2.0, 4.0, 6.0, 8.0];
    let cov = covariance(&x, &y);
    let corr = correlation(&x, &y);
    // Perfect linear relationship ⇒ correlation 1
    assert!((corr - 1.0).abs() < 1e-12);
    // Covariance should be positive and consistent with scaling
    assert!(cov > 0.0);
}

#[test]
fn log_returns_and_realized_variance_close_to_close() {
    let prices = [100.0, 102.0, 101.0, 105.0];
    let returns = log_returns(&prices);
    assert_eq!(returns.len(), prices.len() - 1);

    let rv = realized_variance(&prices, RealizedVarMethod::CloseToClose, 252.0)
        .expect("CloseToClose should succeed");
    assert!(rv.is_finite() && rv >= 0.0);
    let expected = returns.iter().map(|r| r * r).sum::<f64>() / returns.len() as f64 * 252.0;
    assert!(
        (rv - expected).abs() < 1e-12,
        "close-to-close RV should use squared log returns"
    );

    // OHLC-only methods must be rejected on the close-only API
    for method in [
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
        RealizedVarMethod::YangZhang,
    ] {
        let result = realized_variance(&prices, method, 252.0);
        assert!(
            result.is_err(),
            "realized_variance with {} must return Err for close-only input",
            method.label()
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("OHLC"),
            "error message should mention OHLC: {msg}"
        );
    }
}

#[test]
fn realized_variance_close_to_close_rejects_invalid_prices() {
    for prices in [[100.0, 0.0], [100.0, -5.0], [100.0, f64::NAN]] {
        let err = realized_variance(&prices, RealizedVarMethod::CloseToClose, 252.0)
            .expect_err("invalid close prices should be rejected");
        assert!(
            err.to_string()
                .contains("prices must be finite and positive"),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn realized_variance_close_to_close_single_price_is_zero() {
    let result = realized_variance(&[100.0], RealizedVarMethod::CloseToClose, 252.0)
        .expect("single close price should use early zero return");
    assert_eq!(result, 0.0);
}

#[test]
fn realized_variance_ohlc_estimators_behave() {
    let open = [100.0, 101.0, 102.0];
    let high = [102.0, 103.0, 104.0];
    let low = [99.0, 100.0, 101.0];
    let close = [101.0, 102.0, 103.0];

    for method in [
        RealizedVarMethod::CloseToClose,
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
        RealizedVarMethod::YangZhang,
    ] {
        let value = realized_variance_ohlc(&open, &high, &low, &close, method, 252.0)
            .expect("realized_variance_ohlc should succeed for valid OHLC input");
        assert!(value.is_finite() && value >= 0.0);
    }
}

#[test]
fn realized_variance_ohlc_rejects_mismatched_lengths() {
    let err = realized_variance_ohlc(
        &[100.0, 101.0],
        &[102.0],
        &[99.0, 100.0],
        &[101.0, 102.0],
        RealizedVarMethod::Parkinson,
        252.0,
    )
    .expect_err("mismatched OHLC vectors should be rejected");

    assert!(
        err.to_string().contains("same length"),
        "unexpected error: {err}"
    );
}
