//! Shared rough-volatility level scaling helper.

/// Scale a deterministic variance (power 1) or amplitude (power 1/2) by
/// the normalized lognormal Volterra driver at the left endpoint.
pub(super) fn rough_level(
    base: f64,
    eta: f64,
    history: f64,
    t: f64,
    h: f64,
    power: f64,
) -> finstack_quant_core::Result<f64> {
    if !history.is_finite() || !eta.is_finite() || eta <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "invalid rough volatility history or vol-of-vol".into(),
        ));
    }
    if base == 0.0 || (t == 0.0 && history == 0.0) {
        return Ok(base);
    }
    let exponent = power * (eta * history - 0.5 * eta * eta * t.powf(2.0 * h));
    let mut level = base * exponent.exp();
    if level == 0.0 || !level.is_finite() {
        level = (base.ln() + exponent).exp();
    }
    if !level.is_finite() || level <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "rough volatility level is outside the representable range".into(),
        ));
    }
    Ok(level)
}
