//! Random number generation for Monte Carlo simulation.
//!
//! Provides counter-based RNGs (Philox) for deterministic parallel simulation
//! and quasi-Monte Carlo sequences (Sobol with Owen scrambling).
//!
//! # RNG Selection: Philox vs MT19937
//!
//! This implementation uses **Philox** as the default pseudo-random number generator
//! rather than MT19937 (Mersenne Twister), which is used by QuantLib. This design
//! decision is based on several considerations:
//!
//! ## Why Philox?
//!
//! | Feature | Philox | MT19937 |
//! |---------|--------|---------|
//! | Parallel safety | Lock-free, stateless | Requires per-thread state |
//! | Stream splitting | Native support | Requires skip-ahead |
//! | Memory footprint | ~64 bytes | ~2.5 KB |
//! | Statistical quality | TestU01 BigCrush | TestU01 BigCrush |
//! | Industry adoption | TensorFlow, JAX, cuRAND | QuantLib, NumPy |
//!
//! Key advantages of Philox for this codebase:
//!
//! 1. **Perfect parallelization**: Each path gets an independent stream without
//!    any synchronization overhead
//! 2. **Reproducibility**: Same (seed, stream_id) always produces identical results,
//!    regardless of thread count or execution order
//! 3. **Modern hardware optimization**: No memory lookups or table accesses
//! 4. **Low memory**: Important for GPU execution (future enhancement)
//!
//! ## QuantLib Compatibility
//!
//! For exact comparison with QuantLib results (e.g., validation testing), note:
//!
//! - QuantLib uses MT19937 with a specific seeding mechanism
//! - Different RNG sequences will produce different Monte Carlo estimates
//! - Statistical properties should converge to the same result with enough paths
//!
//! **For validation**: Compare converged prices and Greeks (e.g., 100K+ paths) rather
//! than matching intermediate random values. Both RNGs pass the same statistical tests
//! and should produce results within Monte Carlo standard error.
//!
//! **If exact MT19937 matching is required**: This could be added as a feature flag
//! in the future. File an issue with your use case.
//!
//! # Available Generators
//!
//! - [`philox::PhiloxRng`]: Counter-based PRNG for parallel pseudo-random simulation
//! - [`sobol::SobolRng`]: Low-discrepancy sequence for quasi-Monte Carlo
//! - [`brownian_bridge::BrownianBridge`]: Path construction with variance reduction
//! - [`fbm`]: True fractional-Brownian-motion increment generators
//!   (exact Cholesky and windowed conditional-Gaussian recursion)
//! - [`volterra`]: Riemann-Liouville Volterra process increment generator for
//!   the rough Bergomi model (Bennedsen-Lunde-Pakkanen hybrid scheme)
//!
//! For Sobol PCA utilities, see [`finstack_quant_core::math::random::sobol_pca`].
//!
//! # References
//!
//! - Salmon et al. (2011). "Parallel Random Numbers: As Easy as 1, 2, 3."
//!   SC '11 Proceedings. DOI: 10.1145/2063384.2063405
//! - Matsumoto & Nishimura (1998). "Mersenne Twister: A 623-dimensionally
//!   equidistributed uniform pseudo-random number generator."
//!   ACM TOMACS 8(1):3-30.

pub mod brownian_bridge;
pub mod fbm;
pub mod philox;
pub mod poisson;
pub mod sobol;
pub mod volterra;

pub use brownian_bridge::BrownianBridge;
pub use philox::PhiloxRng;
