use finstack_core::math::stats::{
    log_returns, realized_variance, realized_variance_ohlc, RealizedVarMethod,
};
use finstack_core::math::{correlation, covariance, mean, mean_var, variance};

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
fn parkinson_variance_golden() {
    // Golden-value test for Parkinson (1980) high-low range estimator
    // Formula: σ² = [1/(4·ln(2))] · (1/n) · Σ[ln(H/L)]²
    //
    // Reference: Parkinson, M. (1980). "The Extreme Value Method for
    // Estimating the Variance of the Rate of Return."
    // Journal of Business, 53(1), 61-65.

    // Two-day data with H/L ratios of 1.1 and 1.2
    let open = [100.0, 100.0];
    let high = [110.0, 120.0];
    let low = [100.0, 100.0];
    let close = [105.0, 110.0];

    // Hand-calculated expected value:
    // ln(110/100) = ln(1.1) ≈ 0.09531
    // ln(120/100) = ln(1.2) ≈ 0.18232
    // sum_sq = 0.09531² + 0.18232² = 0.00908 + 0.03324 = 0.04232
    // factor = 1 / (4 * ln(2)) ≈ 0.3607
    // daily_var = 0.3607 * 0.04232 / 2 ≈ 0.007633
    // annual_var = 0.007633 * 252 ≈ 1.9236

    let ln_hl_1 = (110.0_f64 / 100.0).ln();
    let ln_hl_2 = (120.0_f64 / 100.0).ln();
    let sum_sq = ln_hl_1.powi(2) + ln_hl_2.powi(2);
    let factor = 1.0 / (4.0 * 2.0_f64.ln());
    let expected_daily = factor * sum_sq / 2.0;
    let expected_annual = expected_daily * 252.0;

    let result = realized_variance_ohlc(
        &open,
        &high,
        &low,
        &close,
        RealizedVarMethod::Parkinson,
        252.0,
    )
    .expect("Parkinson should succeed for valid OHLC input");

    assert!(
        (result - expected_annual).abs() < 1e-10,
        "Parkinson variance {} vs expected {} (diff: {})",
        result,
        expected_annual,
        (result - expected_annual).abs()
    );
}

#[test]
fn garman_klass_variance_golden() {
    // Golden-value test for Garman-Klass (1980) OHLC estimator
    // Formula: σ² = (1/n) · Σ[0.5·[ln(H/L)]² - (2·ln(2) - 1)·[ln(C/O)]²]
    //
    // Reference: Garman, M. B., & Klass, M. J. (1980). "On the Estimation of
    // Security Price Volatilities from Historical Data."
    // Journal of Business, 53(1), 67-78.

    // Two-day data
    let open = [100.0, 105.0];
    let high = [110.0, 115.0];
    let low = [95.0, 100.0];
    let close = [105.0, 110.0];

    // Hand-calculated expected value:
    // Day 1: ln(H/L) = ln(110/95) ≈ 0.1466, ln(C/O) = ln(105/100) ≈ 0.0488
    // Day 2: ln(H/L) = ln(115/100) ≈ 0.1398, ln(C/O) = ln(110/105) ≈ 0.0465
    // coeff = 2*ln(2) - 1 ≈ 0.3863
    // Day 1 contrib: 0.5 * 0.1466² - 0.3863 * 0.0488² = 0.01074 - 0.00092 = 0.00982
    // Day 2 contrib: 0.5 * 0.1398² - 0.3863 * 0.0465² = 0.00977 - 0.00084 = 0.00893
    // daily_var = (0.00982 + 0.00893) / 2 ≈ 0.009375
    // annual_var = 0.009375 * 252 ≈ 2.3625

    let coeff = 2.0 * 2.0_f64.ln() - 1.0;

    let hl_1 = (110.0_f64 / 95.0).ln();
    let co_1 = (105.0_f64 / 100.0).ln();
    let contrib_1 = 0.5 * hl_1.powi(2) - coeff * co_1.powi(2);

    let hl_2 = (115.0_f64 / 100.0).ln();
    let co_2 = (110.0_f64 / 105.0).ln();
    let contrib_2 = 0.5 * hl_2.powi(2) - coeff * co_2.powi(2);

    let expected_daily = (contrib_1 + contrib_2) / 2.0;
    let expected_annual = expected_daily * 252.0;

    let result = realized_variance_ohlc(
        &open,
        &high,
        &low,
        &close,
        RealizedVarMethod::GarmanKlass,
        252.0,
    )
    .expect("GarmanKlass should succeed for valid OHLC input");

    assert!(
        (result - expected_annual).abs() < 1e-10,
        "Garman-Klass variance {} vs expected {} (diff: {})",
        result,
        expected_annual,
        (result - expected_annual).abs()
    );
}

#[test]
fn yang_zhang_includes_open_to_close_component() {
    let open = [100.0, 105.0, 95.0, 110.0];
    let high = [102.0, 108.0, 98.0, 112.0];
    let low = [99.0, 103.0, 93.0, 108.0];
    let close = [101.0, 104.0, 97.0, 111.0];
    let annualization = 252.0;

    let yz = realized_variance_ohlc(
        &open,
        &high,
        &low,
        &close,
        RealizedVarMethod::YangZhang,
        annualization,
    )
    .expect("YangZhang should succeed for valid OHLC input");

    let n = open.len();
    // Yang & Zhang (2000): both k and the RS average are defined in terms of
    // the number of return periods m = n − 1 (bar 0 has no previous close),
    // not the bar count n.
    let m = n - 1;
    let k = 0.34 / (1.34 + (m + 1) as f64 / (m - 1) as f64);
    let overnight: Vec<f64> = (1..n).map(|i| (open[i] / close[i - 1]).ln()).collect();
    let open_close: Vec<f64> = (1..n).map(|i| (close[i] / open[i]).ln()).collect();
    let rs_sum: f64 = (1..n)
        .map(|i| {
            let hc = (high[i] / close[i]).ln();
            let ho = (high[i] / open[i]).ln();
            let lc = (low[i] / close[i]).ln();
            let lo = (low[i] / open[i]).ln();
            hc * ho + lc * lo
        })
        .sum();

    let overnight_mean = overnight.iter().sum::<f64>() / overnight.len() as f64;
    let open_close_mean = open_close.iter().sum::<f64>() / open_close.len() as f64;
    let var_overnight = overnight
        .iter()
        .map(|r| {
            let d = r - overnight_mean;
            d * d
        })
        .sum::<f64>()
        / (overnight.len() - 1) as f64;
    let var_open_close = open_close
        .iter()
        .map(|r| {
            let d = r - open_close_mean;
            d * d
        })
        .sum::<f64>()
        / (open_close.len() - 1) as f64;
    let var_rs = rs_sum / m as f64;
    let expected = (var_overnight + k * var_open_close + (1.0 - k) * var_rs) * annualization;

    assert!((yz - expected).abs() < 1e-12, "Yang-Zhang formula mismatch");
}
