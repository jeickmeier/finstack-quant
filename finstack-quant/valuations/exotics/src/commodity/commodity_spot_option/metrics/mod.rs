//! Commodity option metrics module.
//!
//! Registers option greeks and rate sensitivities for commodity options.
//!
//! # Forward-Based Greeks
//!
//! Commodity options are priced using Black-76, which is forward/futures-based.
//! The gamma and vanna calculators use **forward-based** sensitivities, bumping
//! the forward price driver in priority order:
//!
//! 1. **`quoted_forward`**: If the instrument has a quoted forward override, bump that
//! 2. **`PriceCurve`**: If a PriceCurve exists for `forward_curve_id`, bump it (parallel percent)
//! Observed spot is required separately for early exercise and simulation;
//! it does not substitute for the contractual delivery forward.
//!
//! This ensures greeks are consistent with the Black-76 pricing model, regardless of
//! how the forward price is specified in the market data.

mod register;
pub(crate) mod selected_model;

pub(crate) use register::register_commodity_spot_option_metrics;
