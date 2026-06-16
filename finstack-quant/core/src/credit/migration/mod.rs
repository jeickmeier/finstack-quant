//! Credit migration infrastructure: transition matrices, generator matrices,
//! matrix exponentiation, and CTMC simulation.
//!
//! This module provides the mathematical primitives for credit migration modeling
//! (JLT / CreditMetrics-style transition matrix approaches):
//!
//! - [`crate::credit::migration::scale::RatingScale`]: ordered state set; preset S&P/Fitch scales included.
//! - [`crate::credit::migration::matrix::TransitionMatrix`]: validated row-stochastic N×N matrix with time horizon.
//! - [`crate::credit::migration::generator::GeneratorMatrix`]: continuous-time intensity matrix Q, with extraction
//!   from annual transition matrices via matrix logarithm.
//! - [`crate::credit::migration::projection`]: compute P(t) = exp(Q · t) for arbitrary horizons.
//! - [`crate::credit::migration::simulation`]: Gillespie-algorithm CTMC path simulation.
//!
//! # Quick start
//!
//! ```
//! use finstack_quant_core::credit::migration::{
//!     RatingScale, GeneratorMatrix, projection,
//! };
//!
//! // 2-state generator (AAA defaults at rate 0.01/yr)
//! let scale = RatingScale::custom(vec!["AAA".to_string(), "D".to_string()])
//!     .expect("valid scale");
//! let gen = GeneratorMatrix::new(scale.clone(), &[-0.01, 0.01, 0.0, 0.0])
//!     .expect("valid generator");
//!
//! // 5-year transition matrix
//! let p5 = projection::project(&gen, 5.0).expect("projection succeeded");
//! let pd = p5.probability("AAA", "D").unwrap();
//! assert!(pd > 0.0 && pd < 1.0);
//! ```
//!
//! # References
//!
//! - Jarrow, R. A., Lando, D., & Turnbull, S. M. (1997). "A Markov Model for
//!   the Term Structure of Credit Risk Spreads." *Review of Financial Studies*,
//!   10(2), 481-523.
//! - Gupton, G. M., Finger, C. C., & Bhatia, M. (1997). *CreditMetrics —
//!   Technical Document*. J.P. Morgan.
//! - Israel, R., Rosenthal, J., & Wei, J. (2001). "Finding Generators for
//!   Markov Chains via Empirical Transition Matrices." *Mathematical Finance*,
//!   11(2), 245-265.

pub mod error;
pub mod generator;
pub mod matrix;
pub mod projection;
pub mod scale;
pub mod simulation;
#[cfg(test)]
mod tests;

pub use error::MigrationError;
pub use generator::GeneratorMatrix;
pub use matrix::TransitionMatrix;
pub use scale::RatingScale;
pub use simulation::{MigrationSimulator, RatingPath};
