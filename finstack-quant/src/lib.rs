#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Umbrella crate for the **Finstack Quant** quantitative-finance toolkit.
//!
//! Re-exports each sub-crate so downstream consumers can reach the full API
//! through a single dependency:
//!
//! | Re-export          | Sub-crate                         |
//! |--------------------|-----------------------------------|
//! | `core`             | [`finstack_quant_core`]                 |
//! | `analytics`        | [`finstack_quant_analytics`]            |
//! | `attribution`      | [`finstack_quant_attribution`]          |
//! | `calibration`      | [`finstack_quant_calibration`]          |
//! | `cashflows`        | [`finstack_quant_cashflows`]            |
//! | `covenants`        | [`finstack_quant_covenants`]            |
//! | `features`         | [`finstack_quant_features`]             |
//! | `margin`           | [`finstack_quant_margin`]               |
//! | `models`           | [`finstack_quant_models`]                |
//! | `valuations`       | [`finstack_quant_valuations`]           |
//! | `statements`       | [`finstack_quant_statements`]           |
//! | `statements_analytics` | [`finstack_quant_statements_analytics`] |
//! | `portfolio`        | [`finstack_quant_portfolio`]            |
//! | `scenarios`        | [`finstack_quant_scenarios`]            |
//!
//! [`schema`] is defined on this crate: it indexes JSON Schema artifacts from
//! every domain crate so a single validator can follow `$ref` across documents.
//!
//! # Quick start
//!
//! ```
//! use finstack_quant::core::currency::Currency;
//! use finstack_quant::core::money::Money;
//!
//! let amount = Money::from((100_i64, Currency::USD));
//! assert_eq!(amount.currency(), Currency::USD);
//! ```
//!
//! # Crates not re-exported
//!
//! Depend on these packages directly when you need them:
//!
//! - `finstack-quant-arrow` — Arrow `RecordBatch` export for `TableEnvelope`
//! - `finstack-quant-test-utils` — workspace golden-test helpers
//! - `finstack-quant-valuations-macros` — `FinancialBuilder` derive used by valuations

pub use finstack_quant_analytics as analytics;
pub use finstack_quant_attribution as attribution;
pub use finstack_quant_calibration as calibration;
pub use finstack_quant_cashflows as cashflows;
pub use finstack_quant_core as core;
pub use finstack_quant_covenants as covenants;
pub use finstack_quant_features as features;
pub use finstack_quant_margin as margin;
pub use finstack_quant_models as models;
pub use finstack_quant_portfolio as portfolio;
pub use finstack_quant_scenarios as scenarios;
pub use finstack_quant_statements as statements;
pub use finstack_quant_statements_analytics as statements_analytics;
pub use finstack_quant_valuations as valuations;

#[cfg(all(feature = "json-schema", feature = "jsonschema-validate"))]
pub mod schema;
