//! Display defaults that do not supply financial calculation inputs.

/// Whether [`crate::Performance::mean_return`] and
/// [`crate::Performance::volatility`] annualize when the caller omits the flag.
/// Annualized requests still require explicit observation frequency.
pub const DEFAULT_ANNUALIZE: bool = true;
