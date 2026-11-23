use super::*;

/// `avg_daily_volume` was validated and then never used: the model was
/// built directly from the caller's raw coefficients, so an execution in
/// a name trading 1M shares/day cost bit-identically the same as one
/// trading 1B shares/day. ADV must be live: deeper markets => strictly
/// smaller impact cost, all else equal.
#[test]
fn uniform_impact_responds_to_avg_daily_volume() {
    let cost = |adv: f64| {
        almgren_chriss_impact(100_000.0, adv, 0.02, 5.0, 1.0, 1.0, Some(100.0))
            .expect("valid inputs")
            .total_cost
    };
    let thin = cost(1e6);
    let deep = cost(1e9);
    assert!(
        (thin - deep).abs() > 0.0,
        "ADV must change the cost (thin {thin}, deep {deep})"
    );
    assert!(
        deep < thin,
        "a deeper market must cost less (thin {thin}, deep {deep})"
    );
}

/// The caller coefficients scale the ADV-derived calibration
/// multiplicatively, so doubling a coefficient doubles its component.
#[test]
fn uniform_impact_coefficients_scale_their_components() {
    let base = almgren_chriss_impact(100_000.0, 1e6, 0.02, 5.0, 1.0, 1.0, Some(100.0))
        .expect("valid inputs");
    let scaled = almgren_chriss_impact(100_000.0, 1e6, 0.02, 5.0, 2.0, 1.0, Some(100.0))
        .expect("valid inputs");
    assert!(
        (scaled.permanent_impact - 2.0 * base.permanent_impact).abs()
            <= 1e-9 * base.permanent_impact,
        "doubling the permanent coefficient must double the permanent cost"
    );
    assert!(
        (scaled.temporary_impact - base.temporary_impact).abs()
            <= 1e-9 * base.temporary_impact.abs().max(1.0),
        "the temporary component must be untouched"
    );
}
