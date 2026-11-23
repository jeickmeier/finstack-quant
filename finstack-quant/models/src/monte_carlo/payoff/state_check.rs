//! Finite state-variable validation for Monte Carlo payoffs.

/// Read a named payoff input, returning an error when it is missing or non-finite.
///
/// A missing state key is a process/payoff wiring bug (wrong state key, wrong
/// `num_assets`, a process that does not populate `SPOT`, or a payoff grid
/// that does not match the engine grid). Silently defaulting to `0.0` turns
/// that bug into a systematically wrong price, so the simulation fails at the
/// first affected event.
pub(crate) fn require_finite_state(
    value: Option<f64>,
    key: &str,
    step: usize,
) -> finstack_quant_core::Result<f64> {
    let value = value.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "payoff input '{key}' missing at step {step}: process/payoff wiring mismatch"
        ))
    })?;
    if !value.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "payoff input '{key}' non-finite at step {step}: diverged process state"
        )));
    }
    Ok(value)
}
