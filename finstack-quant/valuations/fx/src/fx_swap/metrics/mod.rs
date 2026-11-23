//! FX Swap metrics module.
//!
//! Provides metric calculators specific to `FxSwap`.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_fx_swap_metrics`.
//!
//! Exposed metrics:
//! - Forward points (far rate - near rate)
//! - FX01 (sensitivity to a 1% relative spot move) — shared
//!   `GenericFx01Calculator`
//! - DV01 (domestic) and DV01 (foreign)

mod ir01;
mod register;

pub(crate) use register::register_fx_swap_metrics;
