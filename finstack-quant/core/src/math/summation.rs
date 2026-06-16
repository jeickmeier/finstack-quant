//! Numerically stable summation algorithms.
//!
//! Implements compensated summation methods to minimize floating-point
//! rounding errors when summing large sequences. Critical for maintaining
//! deterministic results in financial calculations.
//!
//! # Algorithms
//!
//! - [`kahan_sum`]: Compensated summation with error tracking (best for same-sign values)
//! - [`neumaier_sum`]: Improved compensated summation (best for mixed-sign values)
//! - [`NeumaierAccumulator`]: Incremental accumulator for streaming summation
//!
//! # Choosing an Algorithm
//!
//! For most use cases, prefer [`neumaier_sum`] or [`NeumaierAccumulator`] as they
//! handle both same-sign and mixed-sign values correctly.
//!
//! # References
//!
//! - Kahan, W. (1965). "Further Remarks on Reducing Truncation Errors."
//!   *Communications of the ACM*, 8(1), 40.
//! - Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren zur Summation
//!   endlicher Summen." *Zeitschrift für Angewandte Mathematik und Mechanik*, 54(1), 39-51.
//! - Higham, N. J. (1993). "The Accuracy of Floating Point Summation."
//!   *SIAM Journal on Scientific Computing*, 14(4), 783-799.

/// Kahan compensated summation – improves numerical stability while preserving a fixed iteration order.
///
/// Best for sequences where all values have the same sign. For mixed-sign values,
/// prefer [`neumaier_sum`] which handles magnitude differences better.
///
/// # Arguments
///
/// * `iter` - Iterator yielding `f64` values to sum. **All values must be finite.**
///   NaN or infinity in the input silently corrupts the compensation term,
///   producing an incorrect result even after the non-finite value has passed.
///
/// # Returns
///
/// The sum of all values with reduced floating-point error.
///
/// # Complexity
///
/// - **Time**: O(n) where n is the number of elements
/// - **Space**: O(1) auxiliary space (single compensation variable)
///
/// # Algorithm
///
/// Maintains a running compensation `c` that tracks lost low-order bits:
/// ```text
/// for x in iter:
///     y = x - c          // Compensate for previous error
///     t = sum + y        // Add compensated value
///     c = (t - sum) - y  // New error term
///     sum = t
/// ```
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::kahan_sum;
///
/// // Summing many small values where naive summation loses precision
/// let values: Vec<f64> = (0..1_000_000).map(|_| 0.1).collect();
/// let sum = kahan_sum(values.iter().copied());
/// assert!((sum - 100_000.0).abs() < 0.01);
/// ```
#[inline]
pub fn kahan_sum<I>(iter: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let mut sum: f64 = 0.0;
    let mut c: f64 = 0.0; // compensation
    for x in iter {
        let y = x - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }
    sum
}

/// Neumaier compensated summation – handles both positive and negative values better than Kahan.
///
/// This algorithm improves upon Kahan summation by better handling cases where
/// the sum and the next value have similar magnitudes but opposite signs.
/// **Recommended for financial calculations with mixed-sign cashflows.**
///
/// # Arguments
///
/// * `iter` - Iterator yielding `f64` values to sum. **All values must be finite.**
///   NaN or infinity in the input silently corrupts the compensation term,
///   producing an incorrect result even after the non-finite value has passed.
///
/// # Returns
///
/// The sum of all values with reduced floating-point error.
///
/// # Complexity
///
/// - **Time**: O(n) where n is the number of elements
/// - **Space**: O(1) auxiliary space (single compensation variable)
///
/// # Algorithm
///
/// Improves on Kahan by adaptively tracking error based on relative magnitudes:
/// ```text
/// for x in iter:
///     t = sum + x
///     if |sum| >= |x|:
///         c += (sum - t) + x   // sum is bigger; track error from sum
///     else:
///         c += (x - t) + sum   // x is bigger; track error from x
///     sum = t
/// return sum + c
/// ```
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::neumaier_sum;
///
/// // Mixed-sign values where naive summation loses precision
/// let values = [1e16, 1.0, -1e16];
/// let sum = neumaier_sum(values.iter().copied());
/// assert!((sum - 1.0).abs() < 1e-10);
/// ```
///
/// # References
///
/// - Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren zur Summation
///   endlicher Summen." *Zeitschrift für Angewandte Mathematik und Mechanik*, 54(1), 39-51.
#[inline]
pub fn neumaier_sum<I>(iter: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let mut sum = 0.0;
    let mut c = 0.0; // running compensation
    for x in iter {
        let t = sum + x;
        c += if sum.abs() >= x.abs() {
            (sum - t) + x
        } else {
            (x - t) + sum
        };
        sum = t;
    }
    sum + c
}

/// Incremental Neumaier compensated summation.
///
/// Useful when you want stable accumulation without allocating a temporary
/// `Vec<f64>` of terms (e.g., PV loops over cashflows). Handles both same-sign
/// and mixed-sign values correctly.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::NeumaierAccumulator;
///
/// let mut acc = NeumaierAccumulator::new();
/// for x in [1e16, 1.0, -1e16] {
///     acc.add(x);
/// }
/// // Result is more accurate than naive summation
/// assert!((acc.total() - 1.0).abs() < 1e-10);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NeumaierAccumulator {
    sum: f64,
    c: f64,
}

impl NeumaierAccumulator {
    /// Create a new accumulator with zero state.
    #[inline]
    pub const fn new() -> Self {
        Self { sum: 0.0, c: 0.0 }
    }

    /// Add a value to the running total.
    ///
    /// `x` must be finite. NaN or infinity corrupts the compensation term.
    #[inline]
    pub fn add(&mut self, x: f64) {
        let t = self.sum + x;
        self.c += if self.sum.abs() >= x.abs() {
            (self.sum - t) + x
        } else {
            (x - t) + self.sum
        };
        self.sum = t;
    }

    /// Return the compensated total.
    #[inline]
    pub fn total(self) -> f64 {
        self.sum + self.c
    }

    /// Return the current sum without consuming the accumulator.
    #[inline]
    pub fn current(&self) -> f64 {
        self.sum + self.c
    }
}
