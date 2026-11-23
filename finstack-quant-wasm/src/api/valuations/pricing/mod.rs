//! WASM bindings for instrument pricing and metric introspection.
//!
//! Typed instruments own their canonical example payloads.
//! Structural credit-model factories live under `crate::api::models::credit`.
//!
//! # Monte-Carlo determinism
//!
//! `priceInstrument` (and its `MarketContext`-handle variant) accept Monte-Carlo models
//! (e.g. `monte_carlo_gbm`,
//! `monte_carlo_hull_white_1f`). These bindings deliberately expose **no**
//! explicit RNG-seed parameter: the seed is part of the *instrument*
//! contract, not the pricing call.
//!
//! The determinism guarantee is provided by the Rust core, not by these
//! wrappers: when an instrument's `instrument_pricing_overrides.model_config.mc_seed_scenario`
//! is `None`, the core MC pricers derive a **stable** seed deterministically
//! from the instrument ID (see
//! `finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides`). Repricing the same
//! instrument JSON on the same build target therefore yields bit-identical
//! results without the caller supplying a seed. The wasm32 build and the
//! native (Python) build can differ at the `f64` rounding level, because their
//! transcendental math libraries differ, and Monte Carlo and finite-difference
//! metrics amplify that (see `INVARIANTS.md` §2.1); compare across hosts with
//! a tolerance. Callers who need a distinct deterministic stream set
//! that label inside the instrument JSON. This contract is verified by
//! `tests::price_instrument_mc_is_deterministic_without_explicit_seed`.

mod engine;
#[cfg(test)]
pub(super) mod tests;

pub(crate) use engine::{PriceRequest, parse_market, parse_market_text};

pub use engine::*;
