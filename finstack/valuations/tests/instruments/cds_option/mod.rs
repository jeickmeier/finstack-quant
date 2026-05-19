//! `CDSOption` test suite, organised around the Bloomberg CDSO numerical-
//! quadrature pricer.
//!
//! - [`common`]: shared fixtures and a builder for setup-heavy tests.
//! - [`test_parameters`] / [`test_types`]: construction and validation.
//! - [`test_pricing`]: end-to-end pricing scenarios.
//! - [`test_greeks`]: Δ, Γ, Vega, Θ via bump-and-reprice on `npv`.
//! - [`test_implied_vol`]: σ recovery from the live pricer.
//! - [`test_option_bounds`]: no-arbitrage value bounds.
//! - [`test_moneyness`]: ITM/ATM/OTM behaviour.
//! - [`test_metrics_registry`]: metric-framework wiring.
//!
//! Tests covering the legacy Black-on-spreads model (decommissioned per
//! DOCS 2055833 §1.2) — `test_black_model_properties`, `quantlib_parity`,
//! the FEP-via-flag tests in `test_index_options` — were removed when
//! the Bloomberg-quadrature model became the default.

mod common;

mod test_parameters;
mod test_types;

mod test_greeks;
mod test_implied_vol;
mod test_knockout_convention;
mod test_pricing;
mod test_public_properties;
mod test_recovery01_par_invariance;

mod test_moneyness;
mod test_option_bounds;

mod test_metrics_registry;

// Phase-3 reconciliation: spot 5Y CDX.NA.IG.46 against Bloomberg's
// CDSW screen values, isolating the CDS-pricer layer from the CDSO
// option-pricer layer.
mod test_bloomberg_cdsw_parity;
mod test_cdx_ig_46_cdso_diagnostics;
