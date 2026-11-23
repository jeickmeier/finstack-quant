//! Shared JSON pricing helpers for host-language bindings.
//!
//! This module centralizes the tagged-instrument JSON pipeline used by the
//! Python and WASM bindings: parse instrument JSON, optionally merge metric
//! pricing overrides, parse the as-of date and model key, and dispatch through
//! the caller-selected registry or the shared standard registry.

pub(crate) mod dispatch;
mod overrides;
#[cfg(test)]
mod tests;
