//! Longstaff-Schwartz Monte Carlo (LSMC) for American/Bermudan options.
//!
//! Implements backward induction with least-squares regression to price
//! options with early exercise features.
//!
//! Reference: Longstaff & Schwartz (2001) - "Valuing American Options by Simulation"
//!
//! # In-sample upward bias
//!
//! This implementation estimates the continuation-value regression and the
//! resulting option price on the **same set of simulated paths** ("in-sample"
//! LSMC). The exercise policy is therefore fit to the noise of those paths,
//! which systematically biases the reported price *upward* relative to the
//! true American value. The magnitude of the bias is typically small (a few
//! basis points for smooth payoffs with well-chosen basis functions and
//! `num_paths ≳ 10⁴`) but grows with:
//!
//! - richer basis families (over-fitting is easier);
//! - fewer paths (less regression stability);
//! - payoff kinks near at-the-money states.
//!
//! For mission-critical pricing the standard remedy is to fit the regression
//! on one independent path set ("training") and apply the frozen exercise
//! policy to a separate path set ("pricing"). Use
//! [`LsmcEngine::price_unbiased`] for that two-pass workflow, or complement
//! this estimator with an Andersen-Broadie dual upper bound to bracket the true
//! value.
//!
//! # Exercise grid
//!
//! Early exercise is evaluated on the discrete simulation steps listed in
//! [`LsmcConfig::exercise_dates`] (the GBM convenience constructors use every
//! step `0..=num_steps`). That is a **Bermudan** option on the time grid, not a
//! continuous American. In the in-sample [`LsmcEngine::price`] method, when index
//! `0` is included, immediate exercise is applied as a floor so the estimate cannot print below
//! intrinsic. When that floor binds, the reported mean is the intrinsic
//! value while stderr and sample standard deviation stay those of the
//! unfloored path present values. The 95% CI is the unfloored interval
//! with its lower bound clamped to intrinsic so the published interval
//! still contains the reported mean.
//! The two-pass methods instead choose immediate exercise on the training
//! paths and freeze that decision with the other exercise dates. Their pricing
//! estimates can fall below intrinsic and are unbiased for the fitted policy,
//! whose value can be below the optimal Bermudan value.

mod engine;
mod pricer;
#[cfg(test)]
mod tests;
mod types;

pub use engine::LsmcEngine;
pub use pricer::LsmcPricer;
pub use types::{
    AmericanExercise, ExerciseBoundaryPoint, ExercisePolicy, LsmcConfig, LsmcInputs, LsmcResult,
};
