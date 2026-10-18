//! Hull-White one-factor Monte Carlo and Longstaff-Schwartz engines.
//!
//! Product-independent simulation machinery for path-dependent and callable
//! rate products driven by an exact HW1F short-rate discretization. The
//! closed-form HW1F analytics (bond-price coefficients, caplet and bond-option
//! formulas, calibration parameters) live in [`crate::rates::hull_white`]; the
//! process and its exact scheme in [`crate::monte_carlo::process::ou`] and
//! [`crate::monte_carlo::discretization::exact_hw1f`]. This module adds what a
//! product pricer needs on top of them:
//!
//! - [`curve`]: θ(t) preparation from a discount curve and affine term-forward
//!   reconstruction ([`prepare_hw1f_params`], [`Hw1fTermForward`],
//!   [`PeriodForwardCoeffs`]).
//! - [`mc`]: the generic [`Hw1fMcPricer`] orchestrator for payoffs without
//!   exercise, with event-aligned grids and pathwise bank-account numeraire.
//! - [`lsmc`]: the [`Hw1fLsmcPricer`] Longstaff-Schwartz harness for payoffs
//!   implementing [`ExerciseBoundaryPayoff`].
//! - [`config`]: [`Hw1fMcConfig`] runtime settings with crate-level defaults
//!   from the Monte Carlo defaults registry.
//! - [`bank_account`], [`cumulative_coupon`], [`exercise`]: numeraire,
//!   coupon-tracking and regression-basis helpers shared by the payoffs.
//!
//! Instrument-level concerns (reading pricing overrides, resolving calibrated
//! market scalars, contract terms such as call schedules) stay in
//! `finstack-quant-valuations`.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_models::rates::hw1f::{Hw1fMcConfig, ShortRateBasis};
//!
//! let config = Hw1fMcConfig::default();
//! assert!(config.num_paths > 0);
//! assert_eq!(ShortRateBasis::new(config.basis_degree).row(0.5, 0.03).len(), 4);
//! ```

/// Pathwise money-market account (bank-account numeraire) helpers.
pub mod bank_account;
pub use bank_account::{accumulate_bank_factors, bank_step_factor};

/// Runtime Monte Carlo configuration shared by the HW1F engines.
pub mod config;
pub use config::{Hw1fMcConfig, SampleSplit};

/// Cumulative coupon tracker for path-dependent products (TARN, snowball).
pub mod cumulative_coupon;
pub use cumulative_coupon::{CouponEvent, CumulativeCouponTracker};

/// HW1F θ(t) preparation and term-forward bond reconstruction.
pub mod curve;
pub use curve::{
    initial_short_rate_from_curve, prepare_hw1f_params, Hw1fTermForward, PeriodForwardCoeffs,
};

/// Exercise-boundary protocol and regression basis for LSMC-priced rate exotics.
pub mod exercise;
pub use exercise::{ExerciseBoundaryPayoff, ShortRateBasis};

/// Generic HW1F Longstaff-Schwartz Monte Carlo pricer for callable rate exotics.
pub mod lsmc;
pub use lsmc::Hw1fLsmcPricer;

/// Generic HW1F Monte Carlo orchestrator for path-dependent rate exotics.
pub mod mc;
pub use mc::{build_event_aligned_grid, money_estimate_from_pairs, Hw1fMcPricer};
