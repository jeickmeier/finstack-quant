//! Curve dependency declaration tests for TRS instruments.
//!
//! These tests ensure TRS instruments fully declare the curves they rely on so that
//! risk calculators (DV01, bucketed DV01, etc.) can correctly bump all relevant
//! rate curves (discount + forward) in line with market convention.

use finstack_quant_valuations::instruments::{
    CurveDependencies, EquityTotalReturnSwap, FIIndexTotalReturnSwap,
};

#[test]
fn equity_trs_declares_discount_and_forward_curves() {
    let trs = EquityTotalReturnSwap::example().unwrap();
    let deps = trs.curve_dependencies().expect("curve_dependencies");

    assert!(
        deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"),
        "Equity TRS should declare its discount curve"
    );
    assert!(
        deps.forward_curves
            .iter()
            .any(|c| c.as_str() == "USD-SOFR-3M"),
        "Equity TRS should declare its financing forward curve"
    );
}

#[test]
fn fi_index_trs_declares_discount_and_forward_curves() {
    let trs = FIIndexTotalReturnSwap::example().unwrap();
    let deps = trs.curve_dependencies().expect("curve_dependencies");

    assert!(
        deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"),
        "FI index TRS should declare its discount curve"
    );
    assert!(
        deps.forward_curves
            .iter()
            .any(|c| c.as_str() == "USD-SOFR-3M"),
        "FI index TRS should declare its financing forward curve"
    );
}
