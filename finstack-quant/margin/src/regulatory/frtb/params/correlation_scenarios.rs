//! Correlation scenario scaling factors (MAR21.6).
//!
//! # Provenance
//!
//! | Item | Value |
//! |------|-------|
//! | Source document | BCBS **d457**, *Minimum capital requirements for market risk* |
//! | Publication date | 14 January 2019; corrected version 25 February 2019 |
//! | Consolidated as | Basel Framework chapter **MAR21** |
//! | MAR21 version | Effective 1 January 2023; incorporates the FAQs published 5 July 2024 and 23 March 2026 |
//! | Paragraphs used | MAR21.6 (the three scenarios), MAR21.7 (maximum across scenarios), MAR21.100-21.101 (curvature correlations are the squares of the delta correlations) |
//! | Primary sources verified | <https://www.bis.org/bcbs/publ/d457.pdf> and <https://www.bis.org/baselframework/BaselFramework.pdf> |
//! | Last reviewed | 2026-08-20 |
//! | Review procedure | See `data/margin/README.md`, "FRTB parameter review" |
//!
//! The three correlation scenarios are applied to the base (Medium)
//! prescribed correlations to produce Low and High stress correlations:
//!
//! ```text
//! MAR21.6(1) Medium: rho as prescribed in MAR21.39 to MAR21.101
//! MAR21.6(2) High:   rho_high = min(1.25 * rho_medium, 100%)
//! MAR21.6(3) Low:    rho_low  = max(2 * rho_medium - 100%, 75% * rho_medium)
//! ```
//!
//! MAR21.7 then takes the **largest** of the three scenario totals, each
//! computed by summing delta, vega and curvature across all risk classes
//! within that scenario.
//!
//! These scaling operations are implemented directly on the
//! [`CorrelationScenario`](crate::regulatory::frtb::CorrelationScenario) enum, and pinned by
//! `correlation_scenario_tests` there.
//!
//! # Known deviations from MAR21
//!
//! - **Curvature correlations are only half-squared** (MAR21.100). Curvature
//!   `rho_kl` and `gamma_bc` are the squares of the corresponding delta
//!   parameters. The curvature engine squares the inter-bucket gamma but
//!   passes the intra-bucket rho through unsquared.
//! - **Scenario scaling is applied before squaring** rather than to the
//!   already-squared curvature parameters, so the High scenario multiplies
//!   the curvature gamma by `1.25^2 = 1.5625` where MAR21.100 read with
//!   MAR21.6(2) implies `1.25`.
