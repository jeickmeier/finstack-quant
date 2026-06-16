//! FRTB Sensitivity-Based Approach (SBA) for standardized market risk capital.
//!
//! Implements the Basel III standardized market risk capital charge per
//! BCBS d457. Computes delta, vega, curvature, default risk, and residual
//! risk add-on components across prescribed risk classes.
//!
//! # Scale convention (IMPORTANT)
//!
//! The prescribed risk weights in [`params`] reproduce the Basel tables
//! **as published**, which means the scale varies by risk class and
//! sensitivity type. Callers must provide sensitivities in the matching
//! scale so that `WS = sensitivity * RW` produces a dollar-denominated
//! weighted sensitivity.
//!
//! | Risk class     | RW scale              | Expected sensitivity unit      |
//! |----------------|-----------------------|--------------------------------|
//! | GIRR delta     | percent (`1.7` = 1.7%)| $ per **1 percentage-point** yield shift (i.e. 100x DV01) |
//! | GIRR vega      | decimal (`0.55`)      | $ per 1 unit of implied vol    |
//! | CSR delta      | percent               | $ per 1 pp spread shift        |
//! | Equity delta   | percent (`55` = 55%)  | $ per 1% price move (i.e. `100 * dV/dP * P`) |
//! | Equity vega    | decimal (`0.78`)      | $ per 1 unit of implied vol    |
//! | Commodity delta| percent               | $ per 1% price move            |
//! | FX delta       | percent (`15` = 15%)  | $ per 1% FX rate move          |
//! | Curvature      | decimal shock (e.g. `0.5`) | raw CVR up / down in $ |
//!
//! The convention is internally consistent (delta RWs are in percent
//! across risk classes, vega RWs are in decimal across risk classes) but
//! intentionally preserves Basel's published numbers so that capital
//! figures match vendor tools and regulatory test packs without scaling.
//! If your sensitivity feed is in a different convention (e.g. decimal
//! DV01 per 1bp, raw dollar delta per $1 of price), **pre-scale the
//! sensitivities before calling into this engine** — do not change the
//! weight tables.

pub mod aggregation;
pub mod curvature;
pub mod delta;
pub mod drc;
pub mod engine;
pub mod params;
pub mod rrao;
pub mod types;
pub mod vega;

pub use engine::{FrtbSbaEngine, FrtbSbaEngineBuilder};
pub use types::{
    CorrelationScenario, DrcAssetType, DrcPosition, DrcSector, DrcSeniority, FrtbRiskClass,
    FrtbSbaResult, FrtbSensitivities, RraoPosition,
};
