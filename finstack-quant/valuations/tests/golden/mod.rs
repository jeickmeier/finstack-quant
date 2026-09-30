//! Golden test framework.
//!
//! Fixtures live under `tests/golden/data/` and use the `finstack_quant.golden/1`
//! schema defined in [`schema`]. Each fixture is validated by [`walk`] and
//! executed by [`runner`].

mod pricing;
mod pricing_common;
mod runner;
mod schema;
mod theta_horizons;
mod tolerance;
mod walk;
