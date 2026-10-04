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

use crate::error::InputError;
use crate::Error;

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
        use crate::math::summation::NeumaierAccumulator;
        let sqrt_2 = std::f64::consts::SQRT_2;
        let mut acc = NeumaierAccumulator::new();
        for (i, &z) in self.points.iter().enumerate() {
            acc.add(self.weights[i] * f(sqrt_2 * z));
        }
        acc.total() / std::f64::consts::PI.sqrt()
    }
}

/// Simpson's rule for numerical integration.
///
/// Provides good accuracy for smooth functions. Requires an even number of intervals.
///
/// # Arguments
///
/// * `f` - Function to integrate
/// * `a` - Lower endpoint of the integration interval, in the domain units
///   accepted by `f`.
/// * `b` - Upper endpoint of the integration interval, in the domain units
///   accepted by `f`; values below `a` produce a signed integral.
/// * `n` - Number of intervals (must be even and > 0)
///
/// # Returns
///
/// Approximate integral value.
///
/// # Errors
///
/// Returns [`InputError::Invalid`] when:
/// - `n` is zero
/// - `n` is not an even number
///
/// # Complexity
///
/// - **Time**: O(n) function evaluations
/// - **Space**: O(1) auxiliary space
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::simpson_rule;
///
/// // Integrate x² from 0 to 1 (exact answer: 1/3)
/// let integral = simpson_rule(|x| x * x, 0.0, 1.0, 100).expect("Valid parameters");
/// assert!((integral - 1.0/3.0).abs() < 1e-6);
/// ```
pub fn simpson_rule<F2>(f: F2, a: f64, b: f64, n: usize) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64,
{
    if n == 0 || !n.is_multiple_of(2) {
        return Err(InputError::Invalid.into());
    }

    let h = (b - a) / n as f64;
    let mut sum = f(a) + f(b);

    // Add even terms (coefficient 2)
    for i in (2..n).step_by(2) {
        let x = a + i as f64 * h;
        sum += 2.0 * f(x);
    }

    // Add odd terms (coefficient 4)
    for i in (1..n).step_by(2) {
        let x = a + i as f64 * h;
        sum += 4.0 * f(x);
    }

    Ok(sum * h / 3.0)
}

/// Adaptive quadrature using recursive Simpson's rule.
///
/// This method automatically refines the integration grid in areas where
/// the function changes rapidly, providing better accuracy with fewer
/// function evaluations for smooth functions.
///
/// # Arguments
///
/// * `f` - Function to integrate (must implement `Copy` for recursive calls)
/// * `a` - Lower endpoint of the integration interval, in the domain units
///   accepted by `f`.
/// * `b` - Upper endpoint of the integration interval, in the domain units
///   accepted by `f`; values below `a` produce a signed integral.
/// * `tol` - Non-negative absolute error budget for the sum of all subinterval
///   estimates, in integral units; local refinement uses proportional shares.
/// * `max_depth` - Maximum recursion depth to prevent infinite refinement
///
/// # Returns
///
/// Approximate integral value whose summed quadrature error estimate is at most `tol`.
///
/// # Errors
///
/// Returns [`InputError::Invalid`] for non-finite endpoints, or a negative or
/// non-finite tolerance.
///
/// Returns [`Error::Input`] wrapping [`InputError::SolverConvergenceFailed`] if the
/// tolerance cannot be met within `max_depth` recursion levels. The error payload
/// includes the error estimate, the interval midpoint, and the depth reached. Callers
/// handling pathological integrands should increase `max_depth` or switch to a
/// non-adaptive rule such as [`gauss_legendre_integrate`].
///
/// # Algorithm
///
/// Uses recursive bisection with Simpson's rule on each subinterval. At each level:
/// 1. Compute Simpson's rule on `[a, mid]` and `[mid, b]`
/// 2. Compare the composite estimate against the coarser estimate using Richardson
///    extrapolation: `error ≈ |total - whole| / 15`
/// 3. If the local error fits its share, retain the value and error estimate.
/// 4. Otherwise refine each half with half the local budget, until `max_depth`.
/// 5. Sum absolute error estimates across every terminal subinterval. Accept
///    only if that sum meets `tol`; otherwise return a convergence error.
///    This allows an isolated kink to use unused budget from smooth regions
///    without accepting an unconverged total integral.
///
/// # Complexity
///
/// - **Time**: O(2^max_depth) function evaluations in worst case; typically much fewer
/// - **Space**: O(max_depth) stack frames
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::adaptive_simpson;
///
/// // Integrate an oscillatory function with high precision
/// let integral = adaptive_simpson(|x| (10.0 * x).sin(), 0.0, std::f64::consts::PI, 1e-6, 100)
///     .expect("Integration succeeds");
/// assert!(integral.abs() < 0.01); // Should be close to 0
/// ```
pub fn adaptive_simpson<F2>(f: F2, a: f64, b: f64, tol: f64, max_depth: usize) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64 + Copy,
{
    if !a.is_finite() || !b.is_finite() || !tol.is_finite() || tol < 0.0 {
        return Err(InputError::Invalid.into());
    }
    #[allow(clippy::too_many_arguments)]
    fn adaptive_simpson_inner<F2>(
        f: F2,
        a: f64,
        b: f64,
        tol: f64,
        fa: f64,
        fb: f64,
        fc: f64,
        whole: f64,
        depth: usize,
        max_depth: usize,
    ) -> (f64, f64)
    where
        F2: Fn(f64) -> f64 + Copy,
    {
        let c = (a + b) / 2.0;

        let fd = f((a + c) / 2.0);
        let fe = f((c + b) / 2.0);

        // Use proper Simpson's rule for each sub-interval
        let h_left = (c - a) / 6.0; // (c-a)/6 for left Simpson interval
        let h_right = (b - c) / 6.0; // (b-c)/6 for right Simpson interval
        let left = h_left * (fa + 4.0 * fd + fc);
        let right = h_right * (fc + 4.0 * fe + fb);
        let total = left + right;

        let error_estimate = (total - whole).abs() / 15.0;

        if error_estimate <= tol || !error_estimate.is_finite() || depth >= max_depth {
            return (total, error_estimate);
        }

        let mid_tol = tol / 2.0;
        let (left_result, left_error) =
            adaptive_simpson_inner(f, a, c, mid_tol, fa, fc, fd, left, depth + 1, max_depth);
        let (right_result, right_error) =
            adaptive_simpson_inner(f, c, b, mid_tol, fc, fb, fe, right, depth + 1, max_depth);
        (left_result + right_result, left_error + right_error)
    }

    let c = (a + b) / 2.0;
    let h = (b - a) / 6.0;
    let fa = f(a);
    let fb = f(b);
    let fc = f(c);

    let whole = h * (fa + 4.0 * fc + fb);

    let (value, error_estimate) =
        adaptive_simpson_inner(f, a, b, tol, fa, fb, fc, whole, 0, max_depth);
    if value.is_finite() && error_estimate <= tol {
        Ok(value)
    } else {
        Err(InputError::SolverConvergenceFailed {
            iterations: max_depth,
            residual: error_estimate,
            last_x: c,
            reason: format!(
                "adaptive_simpson did not meet total tolerance {tol:.2e} at max_depth {max_depth} \
                 (interval [{a:.6e}, {b:.6e}], summed error estimate {error_estimate:.2e})"
            ),
        }
        .into())
    }
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
        _ => Err(InputError::Invalid.into()),
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
/// Returns [`InputError::Invalid`] when:
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
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::integration::gauss_legendre_integrate;
///
/// // Integrate x³ from -1 to 1 (exact answer: 0)
/// let integral = gauss_legendre_integrate(|x| x.powi(3), -1.0, 1.0, 4).expect("Valid order");
/// assert!(integral.abs() < 1e-10);
/// ```
pub fn gauss_legendre_integrate<F2>(f: F2, a: f64, b: f64, order: usize) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64,
{
    if !(a.is_finite() && b.is_finite()) {
        return Err(InputError::Invalid.into());
    }
    // Exact comparison: zero-width integration interval is a standard degenerate case.
    #[allow(clippy::float_cmp)]
    if a == b {
        return Ok(0.0);
    }
    let (xs, ws) = gl_nodes_weights(order)?;
    let half = 0.5 * (b - a);
    let mid = a + half;
    let mut acc = 0.0;
    for i in 0..xs.len() {
        let x = mid + half * xs[i];
        acc += ws[i] * f(x);
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
/// Returns [`InputError::Invalid`] when either bound is non-finite, the
/// interval is empty or reversed, `panels` is zero, or `order` is unsupported.
pub fn gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Result<Vec<(f64, f64)>, Error> {
    if panels == 0 || !(a.is_finite() && b.is_finite()) || b <= a {
        return Err(InputError::Invalid.into());
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
/// Returns [`InputError::Invalid`] when:
/// - `panels` is zero
/// - `order` is unsupported (see [`gauss_legendre_integrate`])
///
/// # Complexity
///
/// - **Time**: O(panels × order) function evaluations
/// - **Space**: O(1) auxiliary space
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
    if panels == 0 {
        return Err(InputError::Invalid.into());
    }
    let h = (b - a) / panels as f64;
    let mut sum = 0.0;
    for k in 0..panels {
        let ak = a + k as f64 * h;
        let bk = ak + h;
        sum += gauss_legendre_integrate(&f, ak, bk, order)?;
    }
    Ok(sum)
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
/// Returns [`InputError::Invalid`] for non-finite endpoints, a negative or
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
        return Err(InputError::Invalid.into());
    }
    gl_nodes_weights(order)?;

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
        let i1 = gauss_legendre_integrate(f, a, b, order)?;
        let mid = a + 0.5 * (b - a);
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
            last_x: a + 0.5 * (b - a),
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
    fn test_simpson_rule() {
        // Test Simpson's rule on a simple polynomial x² on [0, 1]
        // Exact integral = 1/3
        let f = |x: f64| x * x;
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
}
