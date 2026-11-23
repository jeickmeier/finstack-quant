//! Random number generation for Monte Carlo simulations.
//!
//! Provides a small RNG trait, a deterministic PCG64 implementation, Sobol
//! utilities, and sampling helpers used by simulation code.
//!
//! # Components
//!
//! - [`RandomNumberGenerator`]: Trait for pluggable RNG implementations
//! - [`Pcg64Rng`]: Deterministic PCG64 generator
//! - [`box_muller`]: Normal variate generation from uniform samples
//! - [`SobolRng`]: Sobol low-discrepancy sequence with optional prefix hash scrambling
//! - [`BrownianBridge`]: Brownian bridge construction order
//! - [`poisson_inverse_cdf`]: Poisson sampling utilities
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};
//!
//! // Create RNG with seed for reproducibility
//! let mut rng = Pcg64Rng::new(42, 0);
//!
//! // Generate samples
//! let uniform = rng.uniform();       // U(0, 1)
//! let normal = rng.normal(0.0, 1.0)?; // N(0, 1)
//! let event = rng.uniform() < 0.05;   // 5% probability
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # Statistical properties
//!
//! [`Pcg64Rng`] wraps `rand_pcg::Pcg64`:
//! - **Period**: 2^128
//! - **Streams**: [`Pcg64Rng::new`] uses a stream-specific increment
//! - **Deterministic**: Equal seed, stream, and call order reproduce the same sequence in the same build
//!
//! # Parallel Simulations
//!
//! For parallel Monte Carlo, use distinct stream selectors:
//!
//! ```rust
//! use finstack_quant_core::math::random::Pcg64Rng;
//!
//! // Each thread gets a distinct stream selector (e.g., 4 threads)
//! let num_threads = 4;
//! let rngs: Vec<_> = (0..num_threads)
//!     .map(|thread_id| Pcg64Rng::new(42, thread_id as u64))
//!     .collect();
//! ```
//!
//! # References
//!
//! - **PCG**: O'Neill, M. E. (2014). "PCG: A Family of Simple Fast Space-Efficient
//!   Statistically Good Algorithms for Random Number Generation." `docs/REFERENCES.md#o-neill-2014-pcg`
//! - **Box-Muller**: Box, G. E. P., & Muller, M. E. (1958). "A Note on the
//!   Generation of Random Normal Deviates." `docs/REFERENCES.md#box-muller-1958`

mod brownian_bridge;
mod pcg;
mod poisson;
mod sobol;
#[cfg(test)]
mod tests;

pub use brownian_bridge::BrownianBridge;
pub use pcg::{Pcg64Rng, RandomNumberGenerator, box_muller};
pub use poisson::poisson_inverse_cdf;
pub use sobol::{MAX_SOBOL_DIMENSION, SOBOL_PERIOD, SobolRng};
