//! `CdsOption` test suite, organised around the Bloomberg CDSO numerical-
//! quadrature pricer.
//!
//! - [`common`]: shared fixtures and a builder for setup-heavy tests.
//! - [`test_parameters`] / [`test_types`]: construction and validation.
//! - End-to-end and kernel-level pricing tests that require crate-private
//!   implementation details live beside the pricer as unit tests.
//! - [`test_greeks`]: Δ, Γ, Vega, Θ via bump-and-reprice on `npv`.
//! - [`test_implied_vol`]: σ recovery from the live pricer.
//! - No-arbitrage bounds are covered beside the pricer as unit tests.
//! - [`test_moneyness`]: ITM/ATM/OTM behaviour.
//! - [`test_metrics_registry`]: metric-framework wiring.
//!
//! Tests covering the legacy Black-on-spreads model (decommissioned per
//! DOCS 2055833 §1.2) — `test_black_model_properties`, `quantlib_parity`,
//! the FEP-via-flag tests in `test_index_options` — were removed when
//! the Bloomberg-quadrature model became the default.

mod test_parameters;
