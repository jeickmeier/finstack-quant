//! Monte Carlo sensitivity estimators.
//!
//! This module groups the three Greek estimators used in the crate:
//! pathwise differentiation, likelihood-ratio / score-function methods, and
//! finite differences with common random numbers.
//!
//! Start with `pathwise` for smooth payoffs, use `lrm` for discontinuous
//! payoffs such as digitals or barriers, and use `finite_diff` when you need a
//! general bump-and-reprice fallback.

pub mod finite_diff;
pub(crate) mod lrm;
#[cfg(test)]
pub(crate) mod pathwise;
