//! Numerical integration methods for financial mathematics.
//!
//! Implements deterministic quadrature rules and adaptive algorithms for
//! computing integrals that arise in option pricing, risk-neutral expectations,
//! and probability calculations.
//!
//! # Algorithms
//!
//! - **Gauss-Hermite**: Integration over normal distribution (Heston, SABR)
//! - **Gauss-Legendre**: Integration over finite intervals (payoff expectations)
//! - **Simpson's rule**: Classic adaptive quadrature
//!
//! # Use Cases
//!
//! - **Option pricing**: Semi-analytical methods requiring characteristic function integration
//! - **Heston model**: Fourier inversion for vanilla options
//! - **SABR**: Probability density integration for digital payoffs
//! - **Risk-neutral expectations**: Integrate payoff × density
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::integration::GaussHermiteQuadrature;
//!
//! // Integrate x² over standard normal (expected value = 1.0)
//! let quad = GaussHermiteQuadrature::new(7)?;
//! let integral = quad.integrate(|x| x * x);
//! assert!((integral - 1.0).abs() < 0.1);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! See [`docs/REFERENCES.md`](../../../../docs/REFERENCES.md) for canonical
//! anchors:
//!
//! - **Gaussian Quadrature**:
//!   - Abramowitz, M., & Stegun, I. A. (1964). *Handbook of Mathematical Functions*.
//!     Chapter 25 (Numerical Integration).
//! - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 4.5.
//!   ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes)) `docs/REFERENCES.md#press-numerical-recipes`
//!
//! - **Adaptive Methods**:
//!   - Davis, P. J., & Rabinowitz, P. (1984). *Methods of Numerical Integration*
//!     (2nd ed.). Academic Press.
//!
//! - **Financial Applications**:
//!   - Lewis, A. L. (2000). *Option Valuation under Stochastic Volatility*.
//!     Finance Press. (Fourier methods and quadrature)

use crate::Error;
use crate::InputError;
use crate::math::summation::NeumaierAccumulator;

/// Gauss-Hermite quadrature points and weights for numerical integration
/// over the standard normal distribution.
///
/// These are pre-computed for common quadrature orders to avoid runtime
/// computation of the nodes and weights.
#[derive(Debug)]
pub struct GaussHermiteQuadrature {
    /// Quadrature points (x-coordinates)
    points: &'static [f64],
    /// Quadrature weights
    weights: &'static [f64],
}

// Pre-computed Gauss-Hermite quadrature points and weights.
// Nodes and weights are the RAW physicist's values from
// numpy.polynomial.hermite.hermgauss(N) (weight function exp(-x^2)).
// The probabilist's conversion (x -> sqrt(2)·z, result / sqrt(pi)) is applied
// at evaluation time inside `integrate()`, not baked into these tables.

static GAUSS_HERMITE_5_POINTS: &[f64] = &[
    -2.0201828704560856,
    -0.9585724646138185,
    0.0,
    0.9585724646138185,
    2.0201828704560856,
];
static GAUSS_HERMITE_5_WEIGHTS: &[f64] = &[
    0.019_953_242_059_045_913,
    0.393_619_323_152_241_2,
    0.945_308_720_482_941_9,
    0.393_619_323_152_241_2,
    0.019_953_242_059_045_913,
];

static GAUSS_HERMITE_7_POINTS: &[f64] = &[
    -2.6519613568352334,
    -1.6735516287674718,
    -0.8162878828589647,
    0.0,
    0.8162878828589647,
    1.6735516287674718,
    2.6519613568352334,
];
static GAUSS_HERMITE_7_WEIGHTS: &[f64] = &[
    0.0009717812450995192,
    0.05451558281912703,
    0.4256072526101278,
    0.8102646175568073,
    0.4256072526101278,
    0.05451558281912703,
    0.0009717812450995192,
];

static GAUSS_HERMITE_10_POINTS: &[f64] = &[
    -3.4361591188377376,
    -2.5327316742327897,
    -1.7566836492998817,
    -1.0366108297895136,
    -0.3429013272237046,
    0.3429013272237046,
    1.0366108297895136,
    1.7566836492998817,
    2.5327316742327897,
    3.4361591188377376,
];
static GAUSS_HERMITE_10_WEIGHTS: &[f64] = &[
    7.640_432_855_232_62e-6,
    0.001_343_645_746_781_272_8,
    0.033_874_394_455_481_063,
    0.240_138_611_082_314_67,
    0.610_862_633_735_325_8,
    0.610_862_633_735_325_8,
    0.240_138_611_082_314_67,
    0.033_874_394_455_481_063,
    0.001_343_645_746_781_272_8,
    7.640_432_855_232_62e-6,
];

static GAUSS_HERMITE_15_POINTS: &[f64] = &[
    -4.499_990_707_309_392,
    -3.669_950_373_404_453,
    -2.967_166_927_905_603,
    -2.325_732_486_173_858,
    -1.719_992_575_186_489,
    -1.136_115_585_210_921,
    -0.565_069_583_255_576,
    0.0,
    0.565_069_583_255_576,
    1.136_115_585_210_921,
    1.719_992_575_186_489,
    2.325_732_486_173_858,
    2.967_166_927_905_603,
    3.669_950_373_404_453,
    4.499_990_707_309_392,
];
static GAUSS_HERMITE_15_WEIGHTS: &[f64] = &[
    1.522_475_804_253_517e-9,
    1.059_115_547_711_067e-6,
    1.000_044_412_325_025e-4,
    2.778_068_842_912_776e-3,
    3.078_003_387_254_608e-2,
    1.584_889_157_959_357e-1,
    4.120_286_874_988_986e-1,
    5.641_003_087_264_175e-1,
    4.120_286_874_988_986e-1,
    1.584_889_157_959_357e-1,
    3.078_003_387_254_608e-2,
    2.778_068_842_912_776e-3,
    1.000_044_412_325_025e-4,
    1.059_115_547_711_067e-6,
    1.522_475_804_253_517e-9,
];

static GAUSS_HERMITE_20_POINTS: &[f64] = &[
    -5.387_480_890_011_233,
    -4.603_682_449_550_744,
    -3.944_764_040_115_625,
    -3.347_854_567_383_216,
    -2.788_806_058_428_13,
    -2.254_974_002_089_276,
    -1.738_537_712_116_586,
    -1.234_076_215_395_323,
    -0.737_473_728_545_394,
    -0.245_340_708_300_901,
    0.245_340_708_300_901,
    0.737_473_728_545_394,
    1.234_076_215_395_323,
    1.738_537_712_116_586,
    2.254_974_002_089_276,
    2.788_806_058_428_13,
    3.347_854_567_383_216,
    3.944_764_040_115_625,
    4.603_682_449_550_744,
    5.387_480_890_011_233,
];
static GAUSS_HERMITE_20_WEIGHTS: &[f64] = &[
    2.229_393_645_534_151e-13,
    4.399_340_992_273_181e-10,
    1.086_069_370_769_281e-7,
    7.802_556_478_532_064e-6,
    2.283_386_360_163_528e-4,
    3.243_773_342_237_853e-3,
    2.481_052_088_746_361e-2,
    1.090_172_060_200_233e-1,
    2.866_755_053_628_342e-1,
    4.622_436_696_006_101e-1,
    4.622_436_696_006_101e-1,
    2.866_755_053_628_342e-1,
    1.090_172_060_200_233e-1,
    2.481_052_088_746_361e-2,
    3.243_773_342_237_853e-3,
    2.283_386_360_163_528e-4,
    7.802_556_478_532_064e-6,
    1.086_069_370_769_281e-7,
    4.399_340_992_273_181e-10,
    2.229_393_645_534_151e-13,
];

impl GaussHermiteQuadrature {
    /// Get the immutable physicist's Hermite nodes for this quadrature order.
    #[must_use]
    pub fn get_points(&self) -> &'static [f64] {
        self.points
    }

    /// Get the immutable weights for `exp(-x²)` integration, paired with the nodes.
    #[must_use]
    pub fn get_weights(&self) -> &'static [f64] {
        self.weights
    }

    /// Create a Gauss-Hermite quadrature with the specified order.
    ///
    /// This is the canonical constructor for Gauss-Hermite quadrature.
    ///
    /// # Arguments
    /// * `order` - Quadrature order (supported: 5, 7, 10, 15, 20)
    ///
    /// # Returns
    /// `Ok(Self)` if order is supported, `Err` with descriptive message otherwise.
    ///
    /// # Precision Guidelines
    ///
    /// | Order | Polynomial Exactness | Recommended Use |
    /// |-------|---------------------|-----------------|
    /// | 5 | Degree 9 | Quick estimates, smooth functions |
    /// | 7 | Degree 13 | Standard option pricing |
    /// | 10 | Degree 19 | General Monte Carlo validation |
    /// | 15 | Degree 29 | High-precision Heston pricing |
    /// | 20 | Degree 39 | Long-dated options, high vol-of-vol |
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the order is not
    /// one of the supported values (5, 7, 10, 15, 20).
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::math::integration::GaussHermiteQuadrature;
    ///
    /// let quad = GaussHermiteQuadrature::new(7)?;
    /// let integral = quad.integrate(|x| x * x);
    /// assert!((integral - 1.0).abs() < 0.1); // E[X²] = 1 for standard normal
    ///
    /// // High-precision quadrature for demanding applications
    /// let high_precision = GaussHermiteQuadrature::new(20)?;
    ///
    /// // Unsupported orders return Err
    /// assert!(GaussHermiteQuadrature::new(3).is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn new(order: usize) -> crate::Result<Self> {
        match order {
            5 => Ok(Self {
                points: GAUSS_HERMITE_5_POINTS,
                weights: GAUSS_HERMITE_5_WEIGHTS,
            }),
            7 => Ok(Self {
                points: GAUSS_HERMITE_7_POINTS,
                weights: GAUSS_HERMITE_7_WEIGHTS,
            }),
            10 => Ok(Self {
                points: GAUSS_HERMITE_10_POINTS,
                weights: GAUSS_HERMITE_10_WEIGHTS,
            }),
            15 => Ok(Self {
                points: GAUSS_HERMITE_15_POINTS,
                weights: GAUSS_HERMITE_15_WEIGHTS,
            }),
            20 => Ok(Self {
                points: GAUSS_HERMITE_20_POINTS,
                weights: GAUSS_HERMITE_20_WEIGHTS,
            }),
            _ => Err(crate::Error::Validation(format!(
                "Unsupported Gauss-Hermite quadrature order: {}. Supported orders are: 5, 7, 10, 15, 20",
                order
            ))),
        }
    }

    /// Integrate a function over the standard normal distribution.
    ///
    /// # Arguments
    /// * `f` - Function to integrate, takes x (standard normal variate) as input
    ///
    /// # Returns
    /// The approximate integral of f(x) * φ(x) dx from -∞ to +∞,
    /// where φ(x) is the standard normal PDF.
    ///
    /// # Convention
    ///
    /// The stored nodes and weights follow the physicists' Gauss-Hermite rule
    /// for `∫ e^{-z^2} g(z) dz`. This helper converts that convention to a
    /// standard-normal expectation by evaluating `f(√2 z)` and dividing the
    /// weighted sum by `√π`.
    pub fn integrate<F2>(&self, f: F2) -> f64
    where
        F2: Fn(f64) -> f64,
    {
        let sqrt_2 = std::f64::consts::SQRT_2;
        let mut acc = NeumaierAccumulator::new();
        let mut fallback = false;
        let mut raw = 0.0_f64;
        for (i, &z) in self.points.iter().enumerate() {
            let term = self.weights[i] * f(sqrt_2 * z);
            raw += term;
            if !fallback && acc.add(term).is_err() {
                fallback = true;
            }
        }
        let sum = if fallback {
            raw
        } else {
            acc.get_total().unwrap_or(raw)
        };
        sum / std::f64::consts::PI.sqrt()
    }
}

/// Largest count that converts to `f64` without losing its identity.
const MAX_EXACT_COUNT: u64 = 1 << 53;

/// Samples the root comparison of an adaptive Simpson panel needs.
const SIMPSON_ROOT_EVALS: usize = 5;

/// Build the computation failure shared by the Simpson rules.
///
/// `evaluations` is the number of callback invocations actually made and
/// `last_x` the coordinate the failure refers to, never a configured limit.
fn quadrature_failure(
    rule: &str,
    evaluations: usize,
    residual: f64,
    last_x: f64,
    reason: &str,
) -> Error {
    InputError::SolverConvergenceFailed {
        iterations: evaluations,
        residual,
        last_x,
        reason: format!("{rule}: {reason}"),
    }
    .into()
}

/// Multiply a weighted average by the interval width without forming an
/// overflowing `b - a` when the product itself is representable.
#[inline]
fn scale_by_width(average: f64, a: f64, b: f64) -> f64 {
    let width = b - a;
    if width.is_finite() {
        average * width
    } else {
        average * b - average * a
    }
}

/// Interior node at fraction `t` of the increasing interval `[lo, hi]`.
#[inline]
fn interior_node(lo: f64, hi: f64, t: f64) -> f64 {
    if (lo >= 0.0) == (hi >= 0.0) {
        lo + t * (hi - lo)
    } else {
        lo * (1.0 - t) + hi * t
    }
}

/// Simpson's rule on `n` equal subintervals.
///
/// Evaluates `(b - a) * sum(w_i * f(x_i))` with endpoint weights `1/(3n)` and
/// alternating interior weights `4/(3n)` and `2/(3n)`. The weights sum to one,
/// so the weighted sum is an average of the samples and cannot overflow when
/// the samples are representable. Exact for polynomials of degree three or
/// less; no accuracy claim is made for other integrands.
///
/// # Arguments
///
/// * `f` - Fallible integrand evaluated once at each of the `n + 1` nodes, in
///   increasing coordinate order. It must describe one deterministic function
///   for the whole call. The first error it returns stops the rule and is
///   returned unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` evaluates the same
///   increasing-bound rule and negates the result; `b == a` returns `+0.0`
///   without calling `f`.
/// * `n` - Positive even number of equal subintervals, at most `2^53` so the
///   count converts to `f64` exactly.
///
/// # Returns
///
/// The finite Simpson estimate of the signed integral of `f` from `a` to `b`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint or an `n` that is
/// zero, odd or above `2^53`. Returns the callback's own error unchanged.
/// Returns [`InputError::SolverConvergenceFailed`] (a computation error) when
/// a sample is NaN or infinite, when a nonzero weighted sample underflows to
/// zero, when interior nodes cannot be told apart in binary64, or when the
/// result is not representable; `iterations` is the number of callbacks made
/// and `last_x` the coordinate concerned.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::simpson_rule;
///
/// // Integrate x² from 0 to 1 (exact answer: 1/3)
/// let integral = simpson_rule(|x| Ok(x * x), 0.0, 1.0, 100)?;
/// assert!((integral - 1.0 / 3.0).abs() < 1e-12);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn simpson_rule<F>(mut f: F, a: f64, b: f64, n: usize) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<f64, Error>,
{
    const RULE: &str = "simpson_rule";
    if !a.is_finite() || !b.is_finite() {
        return Err(Error::Validation(format!(
            "{RULE}: endpoints must be finite, got [{a}, {b}]"
        )));
    }
    if n == 0 || !n.is_multiple_of(2) {
        return Err(Error::Validation(format!(
            "Simpson's rule needs a positive even number of intervals, got {n}"
        )));
    }
    if n as u64 > MAX_EXACT_COUNT {
        return Err(Error::Validation(format!(
            "{RULE}: interval count {n} does not convert to f64 exactly"
        )));
    }
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok(0.0);
    }
    let (lo, hi, negate) = if a < b { (a, b, false) } else { (b, a, true) };
    let count = n as f64;
    let unit = 1.0 / (3.0 * count);
    let mut average = NeumaierAccumulator::new();
    let mut previous = lo;
    for i in 0..=n {
        let x = if i == 0 {
            lo
        } else if i == n {
            hi
        } else {
            interior_node(lo, hi, i as f64 / count)
        };
        if i > 0 && x <= previous {
            return Err(quadrature_failure(
                RULE,
                i,
                f64::NAN,
                x,
                "adjacent nodes are not distinct in binary64 on a nonzero interval",
            ));
        }
        previous = x;
        let sample = f(x)?;
        let made = i + 1;
        if !sample.is_finite() {
            return Err(quadrature_failure(
                RULE,
                made,
                f64::NAN,
                x,
                "integrand was not finite",
            ));
        }
        let weight = if i == 0 || i == n {
            unit
        } else if i % 2 == 1 {
            4.0 * unit
        } else {
            2.0 * unit
        };
        let term = sample * weight;
        if sample != 0.0 && term == 0.0 {
            return Err(quadrature_failure(
                RULE,
                made,
                f64::NAN,
                x,
                "a nonzero weighted sample underflowed to zero",
            ));
        }
        average.add(term).map_err(|_| {
            quadrature_failure(RULE, made, f64::NAN, x, "weighted sum is not representable")
        })?;
    }
    let evaluations = n + 1;
    let average = average.get_total().map_err(|_| {
        quadrature_failure(
            RULE,
            evaluations,
            f64::NAN,
            hi,
            "weighted sum is not representable",
        )
    })?;
    let value = scale_by_width(average, lo, hi);
    if !value.is_finite() || (average != 0.0 && value == 0.0) {
        return Err(quadrature_failure(
            RULE,
            evaluations,
            f64::NAN,
            hi,
            "integral is not representable in binary64",
        ));
    }
    Ok(oriented(value, negate))
}

/// Apply the requested orientation, keeping an exact zero as `+0.0`.
#[inline]
fn oriented(value: f64, negate: bool) -> f64 {
    if value == 0.0 {
        0.0
    } else if negate {
        -value
    } else {
        value
    }
}

#[inline]
fn midpoint(a: f64, b: f64) -> f64 {
    if (a >= 0.0) == (b >= 0.0) {
        a + 0.5 * (b - a)
    } else {
        0.5 * a + 0.5 * b
    }
}

#[inline]
fn half_width(a: f64, b: f64) -> f64 {
    if (a >= 0.0) == (b >= 0.0) {
        0.5 * (b - a)
    } else {
        0.5 * b - 0.5 * a
    }
}

/// Explicit work controls of one adaptive Simpson run.
#[derive(Clone, Copy)]
struct SimpsonLimits {
    /// Deepest bisection level allowed, with the root panel at depth zero.
    max_depth: usize,
    /// Hard limit on callback invocations.
    max_evals: usize,
}

/// Actual work done so far by one adaptive Simpson run.
struct SimpsonWork {
    rule: &'static str,
    max_evals: usize,
    evaluations: usize,
    last_x: f64,
    depth: usize,
}

impl SimpsonWork {
    fn failure(&self, residual: f64, x: f64, reason: &str) -> Error {
        quadrature_failure(
            self.rule,
            self.evaluations,
            residual,
            x,
            &format!("{reason} (panel depth {})", self.depth),
        )
    }

    /// Evaluate the callback once at `x`, enforcing the evaluation limit and
    /// rejecting a non-finite component.
    fn sample<const N: usize, F>(&mut self, f: &mut F, x: f64) -> Result<[f64; N], Error>
    where
        F: FnMut(f64) -> Result<[f64; N], Error>,
    {
        if self.evaluations >= self.max_evals {
            return Err(self.failure(
                f64::NAN,
                x,
                &format!(
                    "evaluation limit {} reached before the required sample",
                    self.max_evals
                ),
            ));
        }
        self.evaluations += 1;
        self.last_x = x;
        let values = f(x)?;
        if let Some(component) = values.iter().position(|value| !value.is_finite()) {
            return Err(self.failure(
                f64::NAN,
                x,
                &format!(
                    "integrand{} was not finite",
                    component_label::<N>(component)
                ),
            ));
        }
        Ok(values)
    }
}

/// Label a pair component in tuple order; a scalar rule has no label.
fn component_label<const N: usize>(component: usize) -> String {
    if N == 1 {
        String::new()
    } else {
        format!(" component {}", component + 1)
    }
}

/// One pending Simpson panel `[a, b]` with its cached samples at `a`, `b` and
/// the midpoint (`fc`), its one-panel estimate and its local error share.
struct SimpsonPanel<const N: usize> {
    a: f64,
    b: f64,
    fa: [f64; N],
    fb: [f64; N],
    fc: [f64; N],
    whole: [f64; N],
    tol: [f64; N],
    depth: usize,
}

/// Largest sample magnitude for which `fa + 4 fm + fb` cannot overflow.
const SIMPSON_DIRECT_LIMIT: f64 = f64::MAX / 8.0;

/// One-panel Simpson value per component, `(b - a) / 6 * (fa + 4 fm + fb)`.
///
/// Ordinary samples use the classical weighted sum. Samples large enough for
/// `4 * f` to overflow use the equivalent average `fa/6 + 2 fm/3 + fb/6`
/// scaled by the width, so a representable result is not lost. Returns the
/// failing component when a nonzero product underflows to zero or the value
/// is not representable.
fn simpson_panel<const N: usize>(
    a: f64,
    b: f64,
    fa: &[f64; N],
    fm: &[f64; N],
    fb: &[f64; N],
) -> Result<[f64; N], usize> {
    let mut out = [0.0; N];
    for i in 0..N {
        let direct = fa[i].abs() <= SIMPSON_DIRECT_LIMIT
            && fm[i].abs() <= SIMPSON_DIRECT_LIMIT
            && fb[i].abs() <= SIMPSON_DIRECT_LIMIT;
        let (factor, value) = if direct {
            let sum = fa[i] + 4.0 * fm[i] + fb[i];
            (sum, (half_width(a, b) / 3.0) * sum)
        } else {
            let average = fa[i] / 6.0 + fm[i] * (2.0 / 3.0) + fb[i] / 6.0;
            (average, scale_by_width(average, a, b))
        };
        if !value.is_finite() || (factor != 0.0 && value == 0.0) {
            return Err(i);
        }
        out[i] = value;
    }
    Ok(out)
}

/// Shared adaptive Simpson engine for `N` integrands on one mesh.
///
/// Iterative, left panel before right, with an internal stack bounded by
/// `max_depth + 1`. Each component has its own budget, split as
/// `left = tol / 2`, `right = tol - left`.
fn adaptive_simpson_core<const N: usize, F>(
    rule: &'static str,
    f: &mut F,
    a: f64,
    b: f64,
    tol: [f64; N],
    limits: SimpsonLimits,
) -> Result<[f64; N], Error>
where
    F: FnMut(f64) -> Result<[f64; N], Error>,
{
    let SimpsonLimits {
        max_depth,
        max_evals,
    } = limits;
    if !a.is_finite() || !b.is_finite() {
        return Err(Error::Validation(format!(
            "{rule}: endpoints must be finite, got [{a}, {b}]"
        )));
    }
    if tol.iter().any(|t| !t.is_finite() || *t < 0.0) {
        return Err(Error::Validation(format!(
            "{rule}: every tolerance must be finite and nonnegative, got {tol:?}"
        )));
    }
    if max_evals == 0 {
        return Err(Error::Validation(format!(
            "{rule}: max_evals must be positive"
        )));
    }
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok([0.0; N]);
    }
    let (lo, hi, negate) = if a < b { (a, b, false) } else { (b, a, true) };
    let mut work = SimpsonWork {
        rule,
        max_evals,
        evaluations: 0,
        last_x: lo,
        depth: 0,
    };
    if max_evals < SIMPSON_ROOT_EVALS {
        return Err(work.failure(
            f64::NAN,
            lo,
            &format!(
                "evaluation limit {max_evals} is below the {SIMPSON_ROOT_EVALS} samples the \
                 root comparison needs"
            ),
        ));
    }
    const NOT_BISECTABLE: &str = "panel cannot be bisected in binary64 on a nonzero interval";
    const NOT_REPRESENTABLE: &str = "panel estimate is not representable";

    let root_mid = midpoint(lo, hi);
    if !(lo < root_mid && root_mid < hi) {
        return Err(work.failure(f64::NAN, root_mid, NOT_BISECTABLE));
    }
    let fa = work.sample(f, lo)?;
    let fb = work.sample(f, hi)?;
    let fc = work.sample(f, root_mid)?;
    let whole = simpson_panel(lo, hi, &fa, &fc, &fb).map_err(|component| {
        work.failure(
            f64::NAN,
            root_mid,
            &format!("{NOT_REPRESENTABLE}{}", component_label::<N>(component)),
        )
    })?;

    let mut stack = vec![SimpsonPanel {
        a: lo,
        b: hi,
        fa,
        fb,
        fc,
        whole,
        tol,
        depth: 0,
    }];
    let mut values: [NeumaierAccumulator; N] = std::array::from_fn(|_| NeumaierAccumulator::new());
    let mut errors: [NeumaierAccumulator; N] = std::array::from_fn(|_| NeumaierAccumulator::new());
    let mut deepest = 0;

    while let Some(panel) = stack.pop() {
        work.depth = panel.depth;
        deepest = deepest.max(panel.depth);
        let c = midpoint(panel.a, panel.b);
        let d = midpoint(panel.a, c);
        let e = midpoint(c, panel.b);
        if !(panel.a < d && d < c && c < e && e < panel.b) {
            return Err(work.failure(f64::NAN, c, NOT_BISECTABLE));
        }
        let fd = work.sample(f, d)?;
        let fe = work.sample(f, e)?;
        let unusable = |component: usize| {
            work.failure(
                f64::NAN,
                c,
                &format!("{NOT_REPRESENTABLE}{}", component_label::<N>(component)),
            )
        };
        let left = simpson_panel(panel.a, c, &panel.fa, &fd, &panel.fc).map_err(unusable)?;
        let right = simpson_panel(c, panel.b, &panel.fc, &fe, &panel.fb).map_err(unusable)?;
        let mut total = [0.0; N];
        let mut estimate = [0.0; N];
        for i in 0..N {
            total[i] = left[i] + right[i];
            estimate[i] = (total[i] - panel.whole[i]).abs() / 15.0;
            if !total[i].is_finite() || !estimate[i].is_finite() {
                return Err(unusable(i));
            }
        }

        let within_share = (0..N).all(|i| estimate[i] <= panel.tol[i]);
        if within_share || panel.depth >= max_depth {
            for i in 0..N {
                values[i].add(total[i]).map_err(|_| unusable(i))?;
                errors[i].add(estimate[i]).map_err(|_| unusable(i))?;
            }
            continue;
        }

        let left_tol: [f64; N] = std::array::from_fn(|i| panel.tol[i] / 2.0);
        let right_tol: [f64; N] = std::array::from_fn(|i| panel.tol[i] - left_tol[i]);
        // Right is pushed first so the left panel is refined first.
        stack.push(SimpsonPanel {
            a: c,
            b: panel.b,
            fa: panel.fc,
            fb: panel.fb,
            fc: fe,
            whole: right,
            tol: right_tol,
            depth: panel.depth + 1,
        });
        stack.push(SimpsonPanel {
            a: panel.a,
            b: c,
            fa: panel.fa,
            fb: panel.fc,
            fc: fd,
            whole: left,
            tol: left_tol,
            depth: panel.depth + 1,
        });
    }

    work.depth = deepest;
    let mut out = [0.0; N];
    for i in 0..N {
        let label = component_label::<N>(i);
        let value = values[i].get_total().map_err(|_| {
            work.failure(
                f64::NAN,
                work.last_x,
                &format!("summed integral{label} is not representable"),
            )
        })?;
        let estimate = errors[i].get_total().map_err(|_| {
            work.failure(
                f64::NAN,
                work.last_x,
                &format!("summed error estimate{label} is not representable"),
            )
        })?;
        if estimate > tol[i] {
            return Err(work.failure(
                estimate,
                work.last_x,
                &format!(
                    "summed error estimate{label} {estimate:.2e} exceeds tolerance {:.2e} on \
                     [{a:.6e}, {b:.6e}] with max_depth {max_depth}",
                    tol[i]
                ),
            ));
        }
        out[i] = oriented(value, negate);
    }
    Ok(out)
}

/// Adaptive Simpson quadrature with an absolute global error budget.
///
/// Each panel compares its one-panel Simpson value `S1` with the sum `S2` of
/// its two half panels, uses `e = |S2 - S1| / 15` as the error estimate and
/// keeps the uncorrected `S2`. A panel whose estimate exceeds its share of the
/// budget is bisected, the left half receiving `tol / 2` and the right half
/// the remainder, until `max_depth`. Terminal values and estimates are summed
/// with compensated totals; the call succeeds only when the summed estimate is
/// at most `tol`. Endpoint and midpoint samples are cached and reused, so the
/// root costs five evaluations and every bisection two more.
///
/// The estimate is an operational difference, not a bound on the true error.
///
/// # Arguments
///
/// * `f` - Fallible integrand. It must describe one deterministic function for
///   the whole call; mutable diagnostics are allowed. The first error it
///   returns stops the run and is returned unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` runs the same
///   increasing-bound refinement and negates the result; `b == a` returns
///   `+0.0` without calling `f`.
/// * `tol` - Finite, nonnegative absolute budget for the sum of all terminal
///   error estimates, in integral units. Zero is allowed and requests a zero
///   computed estimate, not exact integration.
/// * `max_depth` - Deepest bisection level, with the root panel at depth zero.
///   Depth zero still performs the root comparison. A depth-limited panel may
///   exceed its local share if the summed estimate still fits `tol`.
/// * `max_evals` - Positive hard limit on callback invocations, at least five
///   for the root comparison. The run never exceeds it and returns no partial
///   integral when it is reached.
///
/// # Returns
///
/// The finite signed integral estimate whose summed error estimate is at most
/// `tol`.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, a negative or
/// non-finite `tol`, or `max_evals == 0`. Returns the callback's own error
/// unchanged. Returns [`InputError::SolverConvergenceFailed`] (a computation
/// error) when the summed estimate exceeds `tol`, when `max_evals` is reached
/// or is below five, when a sample or panel value is NaN, infinite or lost to
/// underflow, or when a panel cannot be bisected in binary64. Its `iterations`
/// is the number of callbacks actually made, `last_x` the coordinate concerned
/// and `reason` names the cause and the panel depth reached.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::adaptive_simpson;
///
/// let integral = adaptive_simpson(
///     |x| Ok((10.0 * x).sin()),
///     0.0,
///     std::f64::consts::PI,
///     1e-6,
///     40,
///     100_000,
/// )?;
/// assert!(integral.abs() < 1e-5);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn adaptive_simpson<F>(
    mut f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_depth: usize,
    max_evals: usize,
) -> Result<f64, Error>
where
    F: FnMut(f64) -> Result<f64, Error>,
{
    let mut scalar = |x: f64| f(x).map(|value| [value]);
    let limits = SimpsonLimits {
        max_depth,
        max_evals,
    };
    adaptive_simpson_core("adaptive_simpson", &mut scalar, a, b, [tol], limits).map(|[value]| value)
}

/// Integrate two functions on one shared adaptive Simpson mesh.
///
/// The callback returns both integrands at each node and one call counts as
/// one evaluation. The mesh is refined when either component exceeds its own
/// share; each component's budget is split and summed independently, so a
/// large value or spare tolerance in one component cannot consume the other's
/// budget. The formulas are those of [`adaptive_simpson`], applied per
/// component. Results need not match two independent scalar runs bit for bit,
/// because the refined meshes can differ.
///
/// # Arguments
///
/// * `f` - Fallible callback returning both integrands in tuple order at one
///   domain coordinate. It must describe one deterministic function for the
///   whole call. The first error it returns stops the run and is returned
///   unchanged.
/// * `a` - Finite lower endpoint, in the domain units accepted by `f`.
/// * `b` - Finite upper endpoint in the same units. `b < a` runs the same
///   increasing-bound refinement and negates both results; `b == a` returns
///   `(+0.0, +0.0)` without calling `f`.
/// * `tol` - `[tol0, tol1]`: finite, nonnegative absolute budgets, one per
///   component in tuple order, each in its own component's integral units.
/// * `max_depth` - Deepest bisection level, with the root panel at depth zero.
/// * `max_evals` - Positive hard limit on callback invocations, at least five
///   for the root comparison.
///
/// # Returns
///
/// The two finite signed integral estimates in tuple order, each with a summed
/// error estimate at most its own budget.
///
/// # Errors
///
/// Returns [`Error::Validation`] for a non-finite endpoint, a negative or
/// non-finite budget, or `max_evals == 0`. Returns the callback's own error
/// unchanged. Returns [`InputError::SolverConvergenceFailed`] (a computation
/// error) under the conditions listed for [`adaptive_simpson`]; `reason`
/// names the failing component, the first in tuple order when both fail at
/// one sample or gate.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::adaptive_simpson_pair;
///
/// let (square, cube) = adaptive_simpson_pair(
///     |x| Ok((x * x, x * x * x)),
///     0.0,
///     1.0,
///     [1e-10, 1e-10],
///     20,
///     10_000,
/// )?;
/// assert!((square - 1.0 / 3.0).abs() < 1e-10);
/// assert!((cube - 0.25).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn adaptive_simpson_pair<F>(
    mut f: F,
    a: f64,
    b: f64,
    tol: [f64; 2],
    max_depth: usize,
    max_evals: usize,
) -> Result<(f64, f64), Error>
where
    F: FnMut(f64) -> Result<(f64, f64), Error>,
{
    let mut paired = |x: f64| f(x).map(|(first, second)| [first, second]);
    let limits = SimpsonLimits {
        max_depth,
        max_evals,
    };
    adaptive_simpson_core("adaptive_simpson_pair", &mut paired, a, b, tol, limits)
        .map(|[first, second]| (first, second))
}

// Gauss–Legendre Quadrature (finite intervals)

/// Return Gauss–Legendre nodes and weights for supported orders.
fn gl_nodes_weights(order: usize) -> Result<(&'static [f64], &'static [f64]), Error> {
    // Nodes/weights for symmetric [-1,1] intervals
    // Orders supported: 2, 4, 8, 16
    match order {
        2 => Ok((
            &[-0.577_350_269_189_625_7, 0.577_350_269_189_625_7],
            &[1.0, 1.0],
        )),
        4 => Ok((
            &[
                -0.861_136_311_594_052_6,
                -0.339_981_043_584_856_3,
                0.339_981_043_584_856_3,
                0.861_136_311_594_052_6,
            ],
            &[
                0.347_854_845_137_453_85,
                0.652_145_154_862_546_1,
                0.652_145_154_862_546_1,
                0.347_854_845_137_453_85,
            ],
        )),
        8 => Ok((
            &[
                -0.960_289_856_497_536_3,
                -0.796_666_477_413_626_7,
                -0.525_532_409_916_329,
                -0.183_434_642_495_649_8,
                0.183_434_642_495_649_8,
                0.525_532_409_916_329,
                0.796_666_477_413_626_7,
                0.960_289_856_497_536_3,
            ],
            &[
                0.101_228_536_290_376_26,
                0.222_381_034_453_374_48,
                0.313_706_645_877_887_27,
                0.362_683_783_378_361_96,
                0.362_683_783_378_361_96,
                0.313_706_645_877_887_27,
                0.222_381_034_453_374_48,
                0.101_228_536_290_376_26,
            ],
        )),
        16 => Ok((
            &[
                -0.989_400_934_991_649_9,
                -0.944_575_023_073_232_6,
                -0.865_631_202_387_831_8,
                -0.755_404_408_355_003,
                -0.617_876_244_402_643_8,
                -0.458_016_777_657_227_37,
                -0.281_603_550_779_258_9,
                -0.095_012_509_837_637_44,
                0.095_012_509_837_637_44,
                0.281_603_550_779_258_9,
                0.458_016_777_657_227_37,
                0.617_876_244_402_643_8,
                0.755_404_408_355_003,
                0.865_631_202_387_831_8,
                0.944_575_023_073_232_6,
                0.989_400_934_991_649_9,
            ],
            &[
                0.027_152_459_411_754_095,
                0.062_253_523_938_647_894,
                0.095_158_511_682_492_78,
                0.124_628_971_255_533_88,
                0.149_595_988_816_576_73,
                0.169_156_519_395_002_54,
                0.182_603_415_044_923_58,
                0.189_450_610_455_068_5,
                0.189_450_610_455_068_5,
                0.182_603_415_044_923_58,
                0.169_156_519_395_002_54,
                0.149_595_988_816_576_73,
                0.124_628_971_255_533_88,
                0.095_158_511_682_492_78,
                0.062_253_523_938_647_894,
                0.027_152_459_411_754_095,
            ],
        )),
        _ => Err(crate::Error::Validation(
            "unsupported Gauss-Legendre order; supported orders are 2, 4, 8 and 16".to_string(),
        )),
    }
}

/// Gauss–Legendre quadrature over finite interval \[a,b\].
///
/// This is a low-level building block for numerical integration. For most use
/// cases requiring automatic error control, prefer [`gauss_legendre_integrate_adaptive`].
///
/// # When to Use
///
/// - **Use this function** when you need precise control over quadrature order
///   and have verified the function is smooth over the interval.
/// - **Use [`gauss_legendre_integrate_adaptive`]** when you need automatic error
///   control and aren't sure about function smoothness.
///
/// # Arguments
///
/// * `f` - Function to integrate
/// * `a` - Lower bound of integration (must be finite)
/// * `b` - Upper bound of integration (must be finite)
/// * `order` - Quadrature order (supported: 2, 4, 8, 16)
///
/// # Returns
///
/// - `Ok(0.0)` if `a == b`
/// - `Ok(integral)` for the approximate integral value
///
/// # Errors
///
/// Returns [`Error::Validation`] when:
/// - `a` or `b` is not finite (NaN or infinity)
/// - `order` is not one of the supported values (2, 4, 8, 16)
///
/// # Complexity
///
/// - **Time**: O(order) function evaluations
/// - **Space**: O(1) auxiliary space (nodes/weights are static)
///
/// # Precision Guidelines
///
/// | Order | Polynomial Exactness | Recommended Use |
/// |-------|---------------------|-----------------|
/// | 2 | Degree 3 | Very rough estimates |
/// | 4 | Degree 7 | Quick calculations |
/// | 8 | Degree 15 | Standard accuracy |
/// | 16 | Degree 31 | High precision |
///
fn gauss_legendre_integrate<F2>(f: F2, a: f64, b: f64, order: usize) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64,
{
    if !(a.is_finite() && b.is_finite()) {
        return Err(crate::Error::Validation(
            "Gauss-Legendre integration bounds must be finite".to_string(),
        ));
    }
    let (xs, ws) = gl_nodes_weights(order)?;
    // Exact comparison: zero-width integration interval is a standard degenerate case.
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok(if f(a).is_finite() { 0.0 } else { f64::NAN });
    }
    let half = half_width(a, b);
    let mid = midpoint(a, b);
    let mut acc = 0.0;
    for i in 0..xs.len() {
        let x = mid + half * xs[i];
        let fx = f(x);
        if !fx.is_finite() {
            return Ok(f64::NAN);
        }
        acc += ws[i] * fx;
    }
    Ok(acc * half)
}

/// Build a composite Gauss-Legendre grid over a finite interval.
///
/// Each returned pair contains an integration abscissa and its fully scaled
/// weight. Summing `weight * f(abscissa)` therefore evaluates the composite
/// rule without a second panel-width factor.
///
/// # Arguments
///
/// * `a` - Finite lower integration bound.
/// * `b` - Finite upper integration bound, strictly greater than `a`.
/// * `order` - Nodes per panel; supported values are 2, 4, 8, and 16.
/// * `panels` - Positive number of equal-width panels.
///
/// # Errors
///
/// Returns [`Error::Validation`] when either bound is non-finite, the
/// interval is empty or reversed, `panels` is zero, or `order` is unsupported.
pub fn gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Result<Vec<(f64, f64)>, Error> {
    if panels == 0 || !(a.is_finite() && b.is_finite()) || b <= a {
        return Err(crate::Error::Validation(
            "Gauss-Legendre grid needs finite bounds with b > a and at least one panel".to_string(),
        ));
    }
    let (nodes, weights) = gl_nodes_weights(order)?;
    let width = (b - a) / panels as f64;
    let half = 0.5 * width;
    let mut grid = Vec::with_capacity(nodes.len() * panels);
    for panel in 0..panels {
        let midpoint = a + (panel as f64 + 0.5) * width;
        grid.extend(
            nodes
                .iter()
                .zip(weights)
                .map(|(&node, &weight)| (midpoint + half * node, half * weight)),
        );
    }
    Ok(grid)
}

/// Composite Gauss–Legendre over \[a,b\] using `panels` sub-intervals.
///
/// Divides the integration interval into `panels` equal sub-intervals and applies
/// Gauss-Legendre quadrature to each. This improves accuracy for functions that
/// are not well-approximated by polynomials over the full interval.
///
/// # Arguments
///
/// * `f` - Function to integrate
/// * `a` - Lower bound of integration
/// * `b` - Upper bound of integration
/// * `order` - Quadrature order per panel (supported: 2, 4, 8, 16)
/// * `panels` - Number of sub-intervals (must be > 0)
///
/// # Returns
///
/// Approximate integral value.
///
/// # Errors
///
/// Returns [`Error::Validation`] under the same conditions as
/// [`gauss_legendre_grid`]: non-finite bounds, `b <= a`, zero `panels`, or an
/// unsupported `order` (supported orders: 2, 4, 8, 16).
///
/// # Complexity
///
/// - **Time**: O(panels × order) function evaluations
/// - **Space**: O(panels × order) for the node grid
pub fn gauss_legendre_integrate_composite<F2>(
    f: F2,
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64,
{
    Ok(gauss_legendre_grid(a, b, order, panels)?
        .into_iter()
        .map(|(x, w)| w * f(x))
        .sum())
}

/// Adaptive Gauss–Legendre integration with automatic refinement.
///
/// Recursively bisects the interval and refines where the error estimate
/// exceeds tolerance, providing automatic accuracy control.
///
/// # Arguments
///
/// * `f` - Function to integrate (must implement `Copy` for recursive calls)
/// * `a` - Finite first endpoint, in the domain units accepted by `f`.
/// * `b` - Finite second endpoint in the same units as `a`; values below `a`
///   produce a signed integral.
/// * `order` - Quadrature order per subinterval (supported: 2, 4, 8, 16)
/// * `tol` - Finite, non-negative absolute budget for the sum of all terminal
///   quadrature error estimates, in integral units.
/// * `max_depth` - Maximum recursion depth to prevent infinite refinement
///
/// # Returns
///
/// Approximate integral value whose summed quadrature error estimate is at most
/// `tol`. The estimate is a refinement diagnostic, not a rigorous error bound
/// for arbitrary integrands.
///
/// # Errors
///
/// Returns [`Error::Validation`] for non-finite endpoints, a negative or
/// non-finite tolerance, or an unsupported quadrature order. Returns
/// [`InputError::SolverConvergenceFailed`] when the summed error estimate
/// exceeds `tol` at `max_depth`, or the value or error estimate is non-finite.
///
/// # Algorithm
///
/// At each level:
/// 1. Compute integral over `[a, b]` using single Gauss-Legendre
/// 2. Compute integral over `[a, mid]` + `[mid, b]`
/// 3. If difference ≤ `tol` (halved per side at each level so the leaf
///    budgets sum to the caller's tolerance), return composite result
/// 4. Otherwise, recursively refine each half until `max_depth`.
/// 5. Sum the absolute error estimates across all terminal intervals and
///    accept only when their total meets the original `tol`. This allows a
///    difficult region to use unused budget from smooth regions without
///    spending the same global budget independently on every leaf.
///
/// # Complexity
///
/// - **Time**: O(order × 2^max_depth) evaluations worst case
/// - **Space**: O(max_depth) stack frames
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive;
///
/// // Integrate a function with a peak
/// let integral = gauss_legendre_integrate_adaptive(
///     |x| (-x * x).exp(),
///     -5.0, 5.0,
///     8,    // order
///     1e-8, // tolerance
///     20    // max depth
/// ).expect("Integration succeeds");
///
/// // Should be close to sqrt(π) ≈ 1.7725
/// assert!((integral - std::f64::consts::PI.sqrt()).abs() < 1e-6);
/// ```
pub fn gauss_legendre_integrate_adaptive<F2>(
    f: F2,
    a: f64,
    b: f64,
    order: usize,
    tol: f64,
    max_depth: usize,
) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64 + Copy,
{
    if !a.is_finite() || !b.is_finite() || !tol.is_finite() || tol < 0.0 {
        return Err(crate::Error::Validation(
            "adaptive Gauss-Legendre endpoints and tolerance must be finite with tol >= 0"
                .to_string(),
        ));
    }
    gl_nodes_weights(order)?;
    #[allow(clippy::float_cmp)]
    if a == b {
        return if f(a).is_finite() {
            Ok(0.0)
        } else {
            Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: f64::NAN,
                last_x: a,
                reason: "gauss_legendre_integrate_adaptive integrand evaluated to NaN or infinity"
                    .to_string(),
            }
            .into())
        };
    }

    fn recurse<F2>(
        f: F2,
        a: f64,
        b: f64,
        order: usize,
        tol: f64,
        depth: usize,
        max_depth: usize,
    ) -> Result<(f64, f64), Error>
    where
        F2: Fn(f64) -> f64 + Copy,
    {
        let mid = midpoint(a, b);
        #[allow(clippy::float_cmp)]
        if mid == a || mid == b {
            return Ok((f64::NAN, f64::INFINITY));
        }
        let i1 = gauss_legendre_integrate(f, a, b, order)?;
        let i2_left = gauss_legendre_integrate(f, a, mid, order)?;
        let i2_right = gauss_legendre_integrate(f, mid, b, order)?;
        let i2 = i2_left + i2_right;
        let err = (i2 - i1).abs();
        if err <= tol || !err.is_finite() || depth >= max_depth {
            return Ok((i2, err));
        }
        let (left, left_error) = recurse(f, a, mid, order, tol * 0.5, depth + 1, max_depth)?;
        let (right, right_error) = recurse(f, mid, b, order, tol * 0.5, depth + 1, max_depth)?;
        Ok((left + right, left_error + right_error))
    }

    let (value, error_estimate) = recurse(f, a, b, order, tol, 0, max_depth)?;
    if value.is_finite() && error_estimate <= tol {
        Ok(value)
    } else {
        Err(InputError::SolverConvergenceFailed {
            iterations: max_depth,
            residual: error_estimate,
            last_x: midpoint(a, b),
            reason: format!(
                "gauss_legendre_integrate_adaptive did not meet total tolerance {tol:.2e} at \
                 max_depth {max_depth} (interval [{a:.6e}, {b:.6e}], summed error estimate {error_estimate:.2e})"
            ),
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauss_legendre_basic() {
        // x² on [0,1] = 1/3
        let f = |x: f64| x * x;

        // Test different orders
        let result2 = gauss_legendre_integrate(f, 0.0, 1.0, 2).expect("supported order");
        let result4 = gauss_legendre_integrate(f, 0.0, 1.0, 4).expect("supported order");
        let result8 = gauss_legendre_integrate(f, 0.0, 1.0, 8).expect("supported order");

        let exact = 1.0 / 3.0;

        // All should be accurate for polynomial
        assert!((result2 - exact).abs() < 1e-10);
        assert!((result4 - exact).abs() < 1e-10);
        assert!((result8 - exact).abs() < 1e-10);
    }
    #[test]
    fn test_gauss_legendre_invalid_bounds() {
        let f = |x: f64| x;

        // Infinite bounds
        assert!(gauss_legendre_integrate(f, 0.0, f64::INFINITY, 4).is_err());
        assert!(gauss_legendre_integrate(f, f64::NEG_INFINITY, 0.0, 4).is_err());

        // NaN bounds
        assert!(gauss_legendre_integrate(f, 0.0, f64::NAN, 4).is_err());
    }
    #[test]
    fn test_gauss_legendre_equal_bounds() {
        let f = |x: f64| x * x;

        // Equal bounds should give 0
        let result = gauss_legendre_integrate(f, 5.0, 5.0, 4).expect("supported order");
        assert!(result.abs() < 1e-12);
    }
    #[test]
    fn test_gauss_legendre_invalid_order() {
        let f = |x: f64| x;

        // Unsupported order
        assert!(gauss_legendre_integrate(f, 0.0, 1.0, 3).is_err());
        assert!(gauss_legendre_integrate(f, 0.0, 1.0, 7).is_err());
    }
    // ── H3 regression: large-bounds/small-width midpoint safety ──
    //
    // With a = 1e14 and b = 1e14 + 1 the naive midpoint (a+b)/2 has catastrophic
    // cancellation on 64-bit floats; a + 0.5*(b-a) is stable. Integrand is
    // constant 1, so the exact answer is 1.0.
    #[test]
    fn test_gauss_legendre_large_bounds_small_width_midpoint() {
        let a = 1.0e14_f64;
        let b = a + 1.0;
        let result = gauss_legendre_integrate(|_| 1.0, a, b, 4).expect("valid");
        assert!(
            (result - 1.0).abs() < 1e-6,
            "Integral of 1 over [1e14, 1e14+1] should be 1.0, got {result}"
        );
    }
    // ── H2 guard: tiny-but-nonzero interval must not collapse to zero ──
    //
    // The `a == b` shortcut is intentional for exact zero-width intervals.
    // Confirm a very small but nonzero interval is still integrated.
    #[test]
    fn test_gauss_legendre_tiny_nonzero_interval_not_collapsed() {
        // Interval width = 1e-12; integrand = 1; exact answer ≈ 1e-12
        let a = 1.0_f64;
        let b = 1.0 + 1.0e-12;
        let result = gauss_legendre_integrate(|_| 1.0, a, b, 4).expect("valid");
        // Must be positive — interval must not be treated as zero-width.
        assert!(
            result > 0.0,
            "Tiny nonzero interval must not be treated as zero-width; got {result}"
        );
        // Result should match (b - a) to within floating-point precision of that subtraction.
        let width = b - a;
        assert!(
            (result - width).abs() / width < 1.0e-10,
            "Expected ~{width}, got {result}"
        );
    }

    #[test]
    fn test_simpson_rule() {
        // Test Simpson's rule on a simple polynomial x² on [0, 1]
        // Exact integral = 1/3
        let f = |x: f64| Ok(x * x);
        let integral = simpson_rule(f, 0.0, 1.0, 100)
            .expect("Simpson rule integration should succeed in test");
        assert!((integral - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_gauss_hermite_new_returns_result() {
        // Valid orders should succeed
        assert!(GaussHermiteQuadrature::new(5).is_ok());
        assert!(GaussHermiteQuadrature::new(7).is_ok());
        assert!(GaussHermiteQuadrature::new(10).is_ok());
        assert!(GaussHermiteQuadrature::new(15).is_ok());
        assert!(GaussHermiteQuadrature::new(20).is_ok());

        // Invalid orders should return Err with helpful message
        let err = GaussHermiteQuadrature::new(3);
        assert!(err.is_err());
        let msg = err
            .expect_err("Expected error for unsupported order")
            .to_string();
        assert!(msg.contains("3"), "Error should mention the invalid order");
        assert!(
            msg.contains("Supported orders") || msg.contains("5, 7, 10, 15, 20"),
            "Error should list supported orders"
        );

        assert!(GaussHermiteQuadrature::new(42).is_err());
        assert!(GaussHermiteQuadrature::new(100).is_err());
    }

    #[test]
    fn gauss_legendre_adaptive_shares_unused_budget_only_within_total_tolerance() {
        // The right leaf misses its half-budget (3.25e-6), but the summed
        // terminal error estimate (about 6.29e-6) fits the global budget.
        let tolerance = 6.5e-6;
        let value = gauss_legendre_integrate_adaptive(|x| x.sin(), 0.0, 1.0, 2, tolerance, 1)
            .expect("summed leaf error is within total tolerance");
        let exact = 1.0 - 1.0_f64.cos();

        assert!((value - exact).abs() <= tolerance);
        assert!(gauss_legendre_integrate_adaptive(|x| x.sin(), 0.0, 1.0, 2, 4.7e-6, 1).is_err());
    }

    #[test]
    fn gauss_legendre_adaptive_enforces_global_polynomial_error_budget() {
        let tolerance = 2e-10;
        let result = gauss_legendre_integrate_adaptive(|x| x.powi(4), 0.0, 1.0, 2, tolerance, 5);
        match result {
            Err(Error::Input(InputError::SolverConvergenceFailed { residual, .. })) => {
                assert!(residual > tolerance);
            }
            other => panic!("expected an error for the combined leaf budget, got {other:?}"),
        }

        for (a, b, exact) in [(0.0, 1.0, 0.2), (1.0, 0.0, -0.2)] {
            let value = gauss_legendre_integrate_adaptive(|x| x.powi(4), a, b, 2, tolerance, 8)
                .expect("sufficient refinement meets the global budget");
            assert!((value - exact).abs() <= tolerance);
        }
    }

    #[test]
    fn gauss_legendre_adaptive_rejects_invalid_budget_and_non_finite_values() {
        for tolerance in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(gauss_legendre_integrate_adaptive(|x| x, 0.0, 1.0, 2, tolerance, 5).is_err());
        }
        assert!(gauss_legendre_integrate_adaptive(|_| f64::NAN, 0.0, 1.0, 2, 1e-8, 5).is_err());
        assert!(gauss_legendre_integrate_adaptive(|x| x, 0.0, 0.0, 3, 1e-8, 5).is_err());
    }

    #[test]
    fn adaptive_quadrature_fails_closed_on_midpoint_collapse_and_handles_extreme_bounds() {
        // Non-polynomial cusp with tol = 0.0 and large max_depth hits ULP midpoint
        // collapse before `error_estimate <= 0.0`; must fail as a geometric
        // computation error, well inside the evaluation limit.
        let cusp = |x: f64| (x - 0.3).abs().sqrt();
        let scalar = adaptive_simpson(|x| Ok(cusp(x)), 0.0, 1.0, 0.0, 200, 1_000_000)
            .expect_err("midpoint collapse must fail");
        assert!(
            scalar.to_string().contains("cannot be bisected"),
            "{scalar}"
        );
        let pair =
            adaptive_simpson_pair(|x| Ok((cusp(x), x)), 0.0, 1.0, [0.0, 0.0], 200, 1_000_000)
                .expect_err("midpoint collapse must fail");
        assert!(pair.to_string().contains("cannot be bisected"), "{pair}");
        assert!(gauss_legendre_integrate_adaptive(cusp, 0.0, 1.0, 4, 0.0, 200).is_err());

        // Same-sign large bounds where `(a + b) / 2.0` would overflow `f64::MAX`
        let a = 1.0e308_f64;
        let b = 1.0e308_f64 + 1.0e295_f64;
        let res =
            adaptive_simpson(|_| Ok(1.0), a, b, 1.0e285, 10, 100).expect("overflow-safe midpoint");
        assert!((res - (b - a)).abs() <= 1.0e285);
    }
}
