//! Golden test framework.
//!
//! Fixtures live under `tests/golden/data/` and use the `finstack.golden/2`
//! schema defined in [`schema`]. Each fixture is validated by [`walk`] and
//! executed by [`runner`].

mod pricing;
mod pricing_common;
mod runner;
mod sabr;
mod schema;
mod tolerance;
mod walk;
