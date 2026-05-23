#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Umbrella crate for the **Finstack** quantitative-finance toolkit.
//!
//! Re-exports each sub-crate so downstream consumers can reach the full API
//! through a single dependency:
//!
//! | Re-export          | Sub-crate                         |
//! |--------------------|-----------------------------------|
//! | `core`             | [`finstack_core`]                 |
//! | `analytics`        | [`finstack_analytics`]            |
//! | `cashflows`        | [`finstack_cashflows`]            |
//! | `covenants`        | [`finstack_covenants`]            |
//! | `margin`           | [`finstack_margin`]               |
//! | `monte_carlo`      | [`finstack_monte_carlo`]          |
//! | `valuations`       | [`finstack_valuations`]           |
//! | `statements`       | [`finstack_statements`]           |
//! | `statements_analytics` | [`finstack_statements_analytics`] |
//! | `portfolio`        | [`finstack_portfolio`]            |
//! | `scenarios`        | [`finstack_scenarios`]            |

pub use finstack_analytics as analytics;
pub use finstack_cashflows as cashflows;
pub use finstack_core as core;
pub use finstack_covenants as covenants;
pub use finstack_margin as margin;
pub use finstack_monte_carlo as monte_carlo;
pub use finstack_portfolio as portfolio;
pub use finstack_scenarios as scenarios;
pub use finstack_statements as statements;
pub use finstack_statements_analytics as statements_analytics;
pub use finstack_valuations as valuations;
