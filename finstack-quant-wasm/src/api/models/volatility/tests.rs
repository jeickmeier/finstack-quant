use super::sabr::*;

use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_models::volatility::sabr::{
    SabrCalibrator, SabrParameters, SabrShift, SabrSmile,
};

#[test]
fn sabr_params_equity_default_roundtrip() {
    let p = JsSabrParameters::equity_default();
    assert!((p.alpha() - 0.20).abs() < 1e-12);
    assert!((p.beta() - 1.0).abs() < 1e-12);
}

#[test]
fn sabr_auto_shift_fits_negative_rate_smile() {
    // Native tests cannot construct `JsValue` strings or Debug a `JsValue`
    // error: both abort off wasm32. Drive the same `"auto"` policy through
    // the Rust keyword parser the binding calls and the domain calibrator.
    // Synthetic quotes use the documented 2% ladder rung
    // (`-min(strike)+10bp = 1.6%` rounds up to 2%), matching the JS facade
    // and Python bindings.
    let policy = "auto".parse::<SabrShift>().expect("auto shift policy");
    let p = SabrParameters::new_with_shift(0.05, 0.5, 0.4, -0.1, 0.02).expect("params");
    let forward = -0.005;
    let strikes = vec![-0.015, -0.01, -0.005, 0.0, 0.005];
    let vols = SabrSmile::new(p, forward, 1.0, VolQuoteType::ShiftedBlackLognormal)
        .generate_smile(&strikes)
        .expect("smile");

    let fitted = SabrCalibrator::new()
        .with_shift(policy)
        .calibrate(
            forward,
            &strikes,
            &vols,
            1.0,
            0.5,
            VolQuoteType::ShiftedBlackLognormal,
        )
        .expect("auto-shift calibrate")
        .parameters;
    let shift = fitted
        .shift()
        .expect("negative-rate fit must carry a shift");
    assert!(shift > 0.0);
    assert!(fitted.is_shifted());
    assert!((shift - 0.02).abs() < 1e-12);
}
