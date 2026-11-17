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
//!   *Communications of the ACM*, 8(1), 40. `docs/REFERENCES.md#kahan-1965`
//! - Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren zur Summation
//!   endlicher Summen." *Zeitschrift für Angewandte Mathematik und Mechanik*, 54(1), 39-51.
//! - Higham, N. J. (1993). "The Accuracy of Floating Point Summation."
//!   *SIAM Journal on Scientific Computing*, 14(4), 783-799. `docs/REFERENCES.md#higham-accuracy-and-stability`

/// Kahan compensated summation – improves numerical stability while preserving a fixed iteration order. `docs/REFERENCES.md#kahan-1965`
///
/// Best for sequences where all values have the same sign. For mixed-sign values,
/// prefer [`neumaier_sum`] which handles magnitude differences better.
///
/// # Arguments
///
/// * `iter` - Iterator yielding `f64` values to sum. All values must be finite.
///
/// # Returns
///
/// The finite sum of all values with reduced floating-point error (or `+0.0`
/// for an empty iterator).
///
/// # Errors
///
/// - Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any input element is `NaN` or infinite.
/// - Returns [`crate::Error::Internal`] when intermediate compensation or
///   running sum arithmetic overflows to a non-finite value.
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
/// use finstack_quant_core::math::summation::kahan_sum;
///
/// // Summing many small values where naive summation loses precision
/// let values: Vec<f64> = (0..1_000_000).map(|_| 0.1).collect();
/// let sum = kahan_sum(values.iter().copied())?;
/// assert!((sum - 100_000.0).abs() < 0.01);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[inline]
pub fn kahan_sum<I>(iter: I) -> crate::Result<f64>
where
    I: IntoIterator<Item = f64>,
{
    let mut sum: f64 = 0.0;
    let mut c: f64 = 0.0; // compensation
    for x in iter {
        if !x.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x),
            }
            .into());
        }
        let y = x - c;
        let t = sum + y;
        let next_c = (t - sum) - y;
        if !(y.is_finite() && t.is_finite() && next_c.is_finite()) {
            return Err(crate::Error::Internal(
                "kahan_sum overflowed to non-finite value".to_string(),
            ));
        }
        c = next_c;
        sum = t;
    }
    Ok(if sum == 0.0 { 0.0 } else { sum })
}

/// Neumaier compensated summation – handles both positive and negative values better than Kahan.
///
/// This algorithm improves upon Kahan summation by better handling cases where
/// the sum and the next value have similar magnitudes but opposite signs.
/// **Recommended for financial calculations with mixed-sign cashflows.**
///
/// # Arguments
///
/// * `iter` - Iterator yielding `f64` values to sum. All values must be finite.
///
/// # Returns
///
/// The finite sum of all values with reduced floating-point error (or `+0.0`
/// for an empty iterator).
///
/// # Errors
///
/// - Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any input element is `NaN` or infinite.
/// - Returns [`crate::Error::Internal`] when running accumulation or the final
///   compensated total overflows to a non-finite value.
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
/// use finstack_quant_core::math::summation::neumaier_sum;
///
/// // Mixed-sign values where naive summation loses precision
/// let values = [1e16, 1.0, -1e16];
/// let sum = neumaier_sum(values.iter().copied())?;
/// assert!((sum - 1.0).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Neumaier, A. (1974). "Rundungsfehleranalyse einiger Verfahren zur Summation
///   endlicher Summen." *Zeitschrift für Angewandte Mathematik und Mechanik*, 54(1), 39-51.
#[inline]
pub fn neumaier_sum<I>(iter: I) -> crate::Result<f64>
where
    I: IntoIterator<Item = f64>,
{
    let mut acc = NeumaierAccumulator::new();
    for x in iter {
        acc.add(x)?;
    }
    acc.get_total()
}

/// Incremental Neumaier compensated summation.
///
/// Useful when you want stable accumulation without allocating a temporary
/// `Vec<f64>` of terms (e.g., PV loops over cashflows). Handles both same-sign
/// and mixed-sign values correctly.
///
/// Updates via [`Self::add`] are atomic: if an input is non-finite or causes
/// intermediate overflow, the accumulator state is left unchanged.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::summation::NeumaierAccumulator;
///
/// let mut acc = NeumaierAccumulator::new();
/// for x in [1e16, 1.0, -1e16] {
///     acc.add(x)?;
/// }
/// // Result is more accurate than naive summation
/// assert!((acc.get_total()? - 1.0).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
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

    /// Add a finite value to the running total atomically.
    ///
    /// On error, the running state (`sum` and compensation) is unchanged.
    ///
    /// # Arguments
    ///
    /// * `x` - Finite value to add in the accumulator's units.
    ///
    /// # Errors
    ///
    /// - Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
    ///   when `x` is `NaN` or infinite.
    /// - Returns [`crate::Error::Internal`] when the updated running sum or
    ///   compensation term overflows to a non-finite value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::math::summation::NeumaierAccumulator;
    ///
    /// let mut acc = NeumaierAccumulator::new();
    /// acc.add(1e16)?;
    /// acc.add(1.0)?;
    /// acc.add(-1e16)?;
    /// assert_eq!(acc.get_total()?, 1.0);
    /// assert!(acc.add(f64::MAX).and_then(|()| acc.add(f64::MAX)).is_err());
    /// assert_eq!(acc.get_total()?, f64::MAX);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    #[inline]
    pub fn add(&mut self, x: f64) -> crate::Result<()> {
        if !x.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x),
            }
            .into());
        }
        let t = self.sum + x;
        let delta_c = if self.sum.abs() >= x.abs() {
            (self.sum - t) + x
        } else {
            (x - t) + self.sum
        };
        let next_c = self.c + delta_c;
        if !(t.is_finite() && next_c.is_finite()) {
            return Err(crate::Error::Internal(
                "compensated sum overflowed".to_string(),
            ));
        }
        self.sum = t;
        self.c = next_c;
        Ok(())
    }

    #[inline]
    pub(crate) fn sum_and_compensation(&self) -> (f64, f64) {
        (self.sum, self.c)
    }

    /// Return the finite compensated total without consuming the accumulator.
    ///
    /// Returns `+0.0` for an empty or zero-sum accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Internal`] if `sum + c` overflows to a
    /// non-finite value.
    #[inline]
    pub fn get_total(&self) -> crate::Result<f64> {
        let total = self.sum + self.c;
        if !total.is_finite() {
            return Err(crate::Error::Internal(
                "compensated sum total is not finite".to_string(),
            ));
        }
        Ok(if total == 0.0 { 0.0 } else { total })
    }
}
