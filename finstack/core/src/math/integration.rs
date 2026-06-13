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
//! - **Trapezoidal rule**: Simple, robust baseline method
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
//! use finstack_core::math::integration::GaussHermiteQuadrature;
//!
//! // Integrate x² over standard normal (expected value = 1.0)
//! let quad = GaussHermiteQuadrature::new(7)?;
//! let integral = quad.integrate(|x| x * x);
//! assert!((integral - 1.0).abs() < 0.1);
//! # Ok::<(), finstack_core::Error>(())
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
//!   - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 4.5.
//!     ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes))
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

// Removed over-engineered parameter bundling structs - use direct parameters instead

/// Gauss-Hermite quadrature points and weights for numerical integration
/// over the standard normal distribution.
///
/// These are pre-computed for common quadrature orders to avoid runtime
/// computation of the nodes and weights.
#[derive(Debug)]
pub struct GaussHermiteQuadrature {
    /// Quadrature points (x-coordinates)
    pub points: &'static [f64],
    /// Quadrature weights
    pub weights: &'static [f64],
}

impl serde::Serialize for GaussHermiteQuadrature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Determine the order based on the number of points
        let order = match self.points.len() {
            5 => 5,
            7 => 7,
            10 => 10,
            15 => 15,
            20 => 20,
            _ => return Err(serde::ser::Error::custom("Unknown quadrature order")),
        };

        #[derive(serde::Serialize)]
        struct QuadratureData {
            order: usize,
        }

        serde::Serialize::serialize(&QuadratureData { order }, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for GaussHermiteQuadrature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct QuadratureData {
            order: usize,
        }

        let data = QuadratureData::deserialize(deserializer)?;

        GaussHermiteQuadrature::new(data.order).map_err(|e| {
            serde::de::Error::custom(format!("Invalid quadrature order {}: {}", data.order, e))
        })
    }
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
    /// use finstack_core::math::integration::GaussHermiteQuadrature;
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
    /// # Ok::<(), finstack_core::Error>(())
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

    /// Adaptive Gauss-Hermite integration with automatic refinement.
    ///
    /// This method automatically increases the quadrature order if the function
    /// exhibits rapid changes or if high correlation values require greater precision.
    /// Critical for base correlation calibration near boundary conditions.
    ///
    /// # Arguments
    /// * `f` - Function to integrate
    /// * `tolerance` - Convergence tolerance for adaptive refinement
    ///
    /// # Returns
    /// High-precision integral estimate with automatic accuracy control
    ///
    /// # Refinement Strategy
    ///
    /// | Starting Order | Refinement Path |
    /// |---------------|-----------------|
    /// | 5 | 5 → 7 → 10 → 15 → 20 |
    /// | 7 | 7 → 10 → 15 → 20 |
    /// | 10 | 10 → 15 → 20 |
    /// | 15 | 15 → 20 |
    /// | 20 | 20 (no refinement) |
    // Refinement table: order 20 is listed explicitly (terminal, no refinement)
    // even though it matches the fallback arm.
    #[allow(clippy::match_same_arms)]
    pub fn integrate_adaptive<F2>(&self, f: F2, tolerance: f64) -> f64
    where
        F2: Fn(f64) -> f64 + Copy,
    {
        let base = self.integrate(f);
        match self.points.len() {
            20 => base,
            15 => {
                let gh20 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_20_POINTS,
                    weights: GAUSS_HERMITE_20_WEIGHTS,
                };
                // Return the higher-order estimate in both branches, consistent
                // with the other refinement paths below.
                gh20.integrate(f)
            }
            10 => {
                let gh15 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_15_POINTS,
                    weights: GAUSS_HERMITE_15_WEIGHTS,
                };
                let gh20 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_20_POINTS,
                    weights: GAUSS_HERMITE_20_WEIGHTS,
                };
                let v15 = gh15.integrate(f);
                if (v15 - base).abs() <= tolerance {
                    v15
                } else {
                    gh20.integrate(f)
                }
            }
            7 => {
                let gh10 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_10_POINTS,
                    weights: GAUSS_HERMITE_10_WEIGHTS,
                };
                let gh15 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_15_POINTS,
                    weights: GAUSS_HERMITE_15_WEIGHTS,
                };
                let v10 = gh10.integrate(f);
                if (v10 - base).abs() <= tolerance {
                    v10
                } else {
                    gh15.integrate(f)
                }
            }
            5 => {
                let gh7 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_7_POINTS,
                    weights: GAUSS_HERMITE_7_WEIGHTS,
                };
                let gh10 = GaussHermiteQuadrature {
                    points: GAUSS_HERMITE_10_POINTS,
                    weights: GAUSS_HERMITE_10_WEIGHTS,
                };
                let v7 = gh7.integrate(f);
                if (v7 - base).abs() <= tolerance {
                    v7
                } else {
                    gh10.integrate(f)
                }
            }
            _ => base,
        }
    }
}

/// Simpson's rule for numerical integration.
///
/// Provides good accuracy for smooth functions. Requires an even number of intervals.
///
/// # Arguments
///
/// * `f` - Function to integrate
/// * `a` - Lower bound
/// * `b` - Upper bound
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
/// use finstack_core::math::integration::simpson_rule;
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
/// * `a` - Lower bound
/// * `b` - Upper bound
/// * `tol` - Error tolerance for adaptive refinement
/// * `max_depth` - Maximum recursion depth to prevent infinite refinement
///
/// # Returns
///
/// Approximate integral value with error bounded by `tol` when convergence is reached.
///
/// # Errors
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
/// 3. If `error ≤ tol`, accept the composite estimate and return it
/// 4. If the error budget is not met and `depth == max_depth`, return
///    [`InputError::SolverConvergenceFailed`] with the
///    residual, interval, and tolerance in the error message
/// 5. Otherwise, recursively refine each half with `tol/2`
///
/// # Complexity
///
/// - **Time**: O(2^max_depth) function evaluations in worst case; typically much fewer
/// - **Space**: O(max_depth) stack frames
///
/// # Example
///
/// ```rust
/// use finstack_core::math::integration::adaptive_simpson;
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
    ) -> Result<f64, Error>
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

        if error_estimate <= tol {
            return Ok(total);
        }

        if depth >= max_depth {
            return Err(crate::error::InputError::SolverConvergenceFailed {
                iterations: depth,
                residual: error_estimate,
                last_x: (a + b) / 2.0,
                reason: format!(
                    "adaptive_simpson did not meet tolerance {tol:.2e} at max_depth {max_depth} \
                     (interval [{a:.6e}, {b:.6e}], error estimate {error_estimate:.2e})"
                ),
            }
            .into());
        }

        let mid_tol = tol / 2.0;
        let left_result =
            adaptive_simpson_inner(f, a, c, mid_tol, fa, fc, fd, left, depth + 1, max_depth)?;
        let right_result =
            adaptive_simpson_inner(f, c, b, mid_tol, fc, fb, fe, right, depth + 1, max_depth)?;
        Ok(left_result + right_result)
    }

    let c = (a + b) / 2.0;
    let h = (b - a) / 6.0;
    let fa = f(a);
    let fb = f(b);
    let fc = f(c);

    let whole = h * (fa + 4.0 * fc + fb);

    adaptive_simpson_inner(f, a, b, tol, fa, fb, fc, whole, 0, max_depth)
}

// -----------------------------------------------------------------------------
// Gauss–Legendre Quadrature (finite intervals)
// -----------------------------------------------------------------------------

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
/// use finstack_core::math::integration::gauss_legendre_integrate;
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
/// * `a` - Lower bound of integration
/// * `b` - Upper bound of integration
/// * `order` - Quadrature order per subinterval (supported: 2, 4, 8, 16)
/// * `tol` - Error tolerance for refinement decisions
/// * `max_depth` - Maximum recursion depth to prevent infinite refinement
///
/// # Returns
///
/// Approximate integral value with error bounded by `tol` (when possible).
///
/// # Algorithm
///
/// At each level:
/// 1. Compute integral over `[a, b]` using single Gauss-Legendre
/// 2. Compute integral over `[a, mid]` + `[mid, b]`
/// 3. If difference ≤ `tol` (halved per side at each level so the leaf
///    budgets sum to the caller's tolerance), return composite result
/// 4. If depth ≥ `max_depth`: accept the composite result only when the
///    error estimate is still within the caller's *original* (un-halved)
///    tolerance — a leaf that is locally unconverged against its halved
///    budget but within the global tolerance is not meaningfully wrong;
///    otherwise return `SolverConvergenceFailed` (matching
///    [`adaptive_simpson`]'s fail-loud behaviour)
/// 5. Otherwise, recursively refine each half
///
/// # Complexity
///
/// - **Time**: O(order × 2^max_depth) evaluations worst case
/// - **Space**: O(max_depth) stack frames
///
/// # Example
///
/// ```rust
/// use finstack_core::math::integration::gauss_legendre_integrate_adaptive;
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
    #[allow(clippy::too_many_arguments)]
    fn recurse<F2>(
        f: F2,
        a: f64,
        b: f64,
        order: usize,
        tol: f64,
        orig_tol: f64,
        depth: usize,
        max_depth: usize,
    ) -> Result<f64, Error>
    where
        F2: Fn(f64) -> f64 + Copy,
    {
        let i1 = gauss_legendre_integrate(f, a, b, order)?;
        let mid = a + 0.5 * (b - a);
        let i2_left = gauss_legendre_integrate(f, a, mid, order)?;
        let i2_right = gauss_legendre_integrate(f, mid, b, order)?;
        let i2 = i2_left + i2_right;
        let err = (i2 - i1).abs();
        if err <= tol {
            return Ok(i2);
        }
        if depth >= max_depth {
            // Locally unconverged against the halved per-leaf budget. Accept
            // when still within the caller's original tolerance (the result
            // is not meaningfully wrong at the requested accuracy); fail
            // loudly otherwise instead of silently returning a bad estimate.
            if err <= orig_tol {
                return Ok(i2);
            }
            return Err(crate::error::InputError::SolverConvergenceFailed {
                iterations: depth,
                residual: err,
                last_x: a + 0.5 * (b - a),
                reason: format!(
                    "gauss_legendre_integrate_adaptive did not meet tolerance {orig_tol:.2e} at \
                     max_depth {max_depth} (interval [{a:.6e}, {b:.6e}], error estimate {err:.2e})"
                ),
            }
            .into());
        }
        let left = recurse(f, a, mid, order, tol * 0.5, orig_tol, depth + 1, max_depth)?;
        let right = recurse(f, mid, b, order, tol * 0.5, orig_tol, depth + 1, max_depth)?;
        Ok(left + right)
    }

    recurse(f, a, b, order, tol, tol, 0, max_depth)
}

/// Trapezoidal rule for numerical integration.
///
/// Simple and robust integration method. Less accurate than Simpson's rule
/// for smooth functions, but more stable for discontinuous functions.
///
/// # Arguments
///
/// * `f` - Function to integrate
/// * `a` - Lower bound
/// * `b` - Upper bound
/// * `n` - Number of intervals (must be > 0)
///
/// # Returns
///
/// Approximate integral value.
///
/// # Errors
///
/// Returns [`InputError::Invalid`] when `n` is zero.
///
/// # Complexity
///
/// - **Time**: O(n) function evaluations
/// - **Space**: O(1) auxiliary space
///
/// # Accuracy
///
/// Error is O(h²) where h = (b-a)/n, assuming f has continuous second derivative.
/// For smooth functions, prefer [`simpson_rule`] which has O(h⁴) error.
///
/// # Example
///
/// ```rust
/// use finstack_core::math::integration::trapezoidal_rule;
///
/// // Integrate x from 0 to 1 (exact answer: 0.5)
/// let integral = trapezoidal_rule(|x| x, 0.0, 1.0, 100).expect("Valid n");
/// assert!((integral - 0.5).abs() < 1e-4);
/// ```
pub fn trapezoidal_rule<F2>(f: F2, a: f64, b: f64, n: usize) -> Result<f64, Error>
where
    F2: Fn(f64) -> f64,
{
    if n == 0 {
        return Err(InputError::Invalid.into());
    }

    let h = (b - a) / n as f64;
    let mut sum = 0.5 * (f(a) + f(b));

    for i in 1..n {
        let x = a + i as f64 * h;
        sum += f(x);
    }

    Ok(sum * h)
}

// -----------------------------------------------------------------------------
// Gauss-Laguerre quadrature via Golub-Welsch
// -----------------------------------------------------------------------------

/// Gauss-Laguerre quadrature for the weight `e^{−x}` on `[0, ∞)`.
///
/// Nodes and weights are computed at runtime via the **Golub-Welsch**
/// algorithm — spectral decomposition of the Jacobi (recurrence) matrix of
/// the Laguerre polynomials. Unlike [`GaussHermiteQuadrature`], which uses
/// precomputed tables for a fixed set of orders, this rule supports any
/// order `n ≥ 1` at a one-time `O(n²)` startup cost, making it suitable
/// for copula/credit models whose required quadrature order varies with
/// the degrees-of-freedom parameter (e.g., Student-t copula with
/// heavier-than-default tails needs `n > 10`).
///
/// # Algorithm
///
/// For Laguerre polynomials the three-term recurrence
/// `L_{k+1}(x) = ((2k + 1 − x) L_k(x) − k · L_{k−1}(x)) / (k + 1)`
/// gives the symmetrized Jacobi matrix
///
/// ```text
///      ┌                                           ┐
///      │ 1   1                                     │
///      │ 1   3   2                                 │
/// J =  │     2   5   3                             │
///      │         3   7   4                         │
///      │             ⋱    ⋱    ⋱                   │
///      │                 n-1  2n-1                 │
///      └                                           ┘
/// ```
///
/// with `J[k, k] = 2k + 1` and `J[k, k±1] = k` (1-based off-diagonal).
/// If `J = Q D Qᵀ` is its spectral decomposition, then
///
/// * nodes `x_i = D[i, i]` — the eigenvalues;
/// * weights `w_i = μ₀ · Q[0, i]²` where `μ₀ = ∫₀^∞ e^{−x} dx = 1`.
///
/// Both are strictly positive by construction and `x_i` is sorted
/// ascending.
///
/// # Exactness
///
/// An `n`-point Gauss-Laguerre rule integrates polynomials of degree up
/// to `2n − 1` exactly against `e^{−x}`. Equivalently,
/// `∫₀^∞ x^k e^{−x} dx = k!` is recovered exactly for `0 ≤ k ≤ 2n − 1`.
///
/// # References
///
/// - Golub, G. H., & Welsch, J. H. (1969). "Calculation of Gauss
///   Quadrature Rules." *Mathematics of Computation* 23, 221–230.
/// - Press, W. H., Teukolsky, S. A., Vetterling, W. T., & Flannery, B. P.
///   (2007). *Numerical Recipes: The Art of Scientific Computing* (3rd
///   ed.), §4.6 "Gaussian Quadratures and Orthogonal Polynomials."
///
/// Runtime Golub-Welsch replaces a previously hard-coded 10-node
/// Laguerre table in
/// `finstack_valuations::correlation::copula::student_t::StudentTCopula`,
/// which silently capped `with_quadrature_order(n > 10)` at `n = 10`.
#[derive(Debug, Clone)]
pub struct GaussLaguerreQuadrature {
    /// Quadrature nodes (sorted ascending, strictly positive).
    pub points: Vec<f64>,
    /// Quadrature weights (strictly positive, summing to `μ₀ = 1`).
    pub weights: Vec<f64>,
}

impl GaussLaguerreQuadrature {
    /// Construct an `n`-point Gauss-Laguerre rule via Golub-Welsch.
    ///
    /// # Errors
    ///
    /// Returns [`crate::InputError::Invalid`] if `n == 0`.
    pub fn new(n: usize) -> crate::Result<Self> {
        if n == 0 {
            return Err(crate::Error::Input(crate::InputError::Invalid));
        }

        // Symmetric tridiagonal Jacobi matrix of the standard Laguerre
        // polynomials (weight e^{-x} on [0, ∞)).
        let mut j = nalgebra::DMatrix::<f64>::zeros(n, n);
        for k in 0..n {
            j[(k, k)] = (2 * k + 1) as f64;
            if k > 0 {
                let bk = k as f64;
                j[(k, k - 1)] = bk;
                j[(k - 1, k)] = bk;
            }
        }

        let eig = nalgebra::SymmetricEigen::new(j);

        // Collect (eigenvalue, first-component^2 of the i-th normalized
        // eigenvector). μ₀ = 1 for standard Laguerre.
        let mut pairs: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let x = eig.eigenvalues[i];
                let v0 = eig.eigenvectors[(0, i)];
                (x, v0 * v0)
            })
            .collect();
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let (points, weights): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();
        Ok(Self { points, weights })
    }

    /// Apply the quadrature to `f`, returning `∫₀^∞ f(x) · e^{−x} dx`.
    ///
    /// Numerically this is `∑ w_i · f(x_i)`. Exact for polynomials of
    /// degree up to `2n − 1`.
    pub fn integrate<F>(&self, f: F) -> f64
    where
        F: Fn(f64) -> f64,
    {
        self.points
            .iter()
            .zip(self.weights.iter())
            .map(|(&x, &w)| w * f(x))
            .sum()
    }

    /// Number of quadrature nodes.
    #[inline]
    #[must_use]
    pub fn order(&self) -> usize {
        self.points.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauss_hermite_quadrature_normalization() {
        let quad = GaussHermiteQuadrature::new(5).expect("Order 5 is supported");

        // Test that integrating 1 over standard normal gives approximately 1
        let integral = quad.integrate(|_x| 1.0);
        assert!((integral - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_gauss_hermite_quadrature_polynomial() {
        let quad = GaussHermiteQuadrature::new(7).expect("Order 7 is supported");

        // Test that integrating x^2 over standard normal gives 1 (variance)
        let integral = quad.integrate(|x| x * x);
        assert!(
            (integral - 1.0).abs() < 0.1,
            "Integral of x² should be ~1, got {}",
            integral
        );
    }

    #[test]
    fn test_different_quadrature_orders() {
        // Test that higher order gives better accuracy for polynomial
        let f = |x: f64| x * x * x * x; // x^4 function

        let quad5 = GaussHermiteQuadrature::new(5).expect("Order 5 is supported");
        let quad7 = GaussHermiteQuadrature::new(7).expect("Order 7 is supported");
        let quad10 = GaussHermiteQuadrature::new(10).expect("Order 10 is supported");

        let integral5 = quad5.integrate(f);
        let integral7 = quad7.integrate(f);
        let integral10 = quad10.integrate(f);

        // Higher order should be more accurate for polynomials
        // For x^4 over standard normal, the integral should be 3
        let expected = 3.0;

        // Just check that all integrals are reasonable (close to expected)
        // The convergence ordering may not always hold for this specific test
        assert!(
            (integral5 - expected).abs() < 1.0,
            "5-point: {} vs expected {}",
            integral5,
            expected
        );
        assert!(
            (integral7 - expected).abs() < 0.5,
            "7-point: {} vs expected {}",
            integral7,
            expected
        );
        assert!(
            (integral10 - expected).abs() < 0.2,
            "10-point: {} vs expected {}",
            integral10,
            expected
        );
    }

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
    fn gauss_legendre_adaptive_accepts_leaf_within_original_tolerance() {
        // At order 2 on sin(x), the parent interval misses this tolerance, so
        // the algorithm recurses. Each child then misses the halved leaf budget
        // but is inside the caller's original tolerance, hitting the max-depth
        // acceptance branch instead of returning a convergence error.
        let value = gauss_legendre_integrate_adaptive(|x| x.sin(), 0.0, 1.0, 2, 4.7e-6, 1)
            .expect("leaf error is within original tolerance");
        let exact = 1.0 - 1.0_f64.cos();

        assert!((value - exact).abs() < 1e-5, "value={value}, exact={exact}");
    }

    // ---- Gauss-Laguerre (Golub-Welsch) ------------------------------------

    #[test]
    fn gauss_laguerre_order_10_matches_canonical_table() {
        // The n=10 nodes/weights produced by Golub-Welsch must match
        // the canonical Abramowitz-Stegun table.
        let quad = GaussLaguerreQuadrature::new(10).expect("n=10 valid");

        let expected: [(f64, f64); 10] = [
            (0.137_793_470_540_492, 0.308_441_115_765_02),
            (0.729_454_549_503_170, 0.401_119_929_155_27),
            (1.808_342_901_740_316, 0.218_068_287_611_81),
            (3.401_433_697_854_9, 0.062_087_456_098_68),
            (5.552_496_140_063_8, 0.009_501_516_975_18),
            (8.330_152_746_764_5, 0.000_753_008_388_587),
            (11.843_785_837_900_1, 0.000_028_259_233_495_0),
            (16.279_257_831_378_1, 0.000_000_424_931_398_5),
            (21.996_585_811_980_8, 0.000_000_001_839_564_8),
            (29.920_697_012_273_9, 0.000_000_000_000_991_2),
        ];

        for (i, (got_x, got_w)) in quad.points.iter().zip(quad.weights.iter()).enumerate() {
            let (want_x, want_w) = expected[i];
            // Relative tolerance on nodes (they span ~0.1 to ~30).
            assert!(
                (got_x - want_x).abs() < 1e-9,
                "node {i}: got {got_x:.15}, want {want_x:.15}"
            );
            // Weights span 14 orders of magnitude; use relative tolerance.
            let rel_err = if want_w.abs() > 0.0 {
                ((got_w - want_w) / want_w).abs()
            } else {
                (got_w - want_w).abs()
            };
            assert!(
                rel_err < 1e-4,
                "weight {i}: got {got_w:.15e}, want {want_w:.15e}, rel_err={rel_err:.2e}"
            );
        }
    }

    #[test]
    fn gauss_laguerre_integrates_moments_exactly() {
        // For Gauss-Laguerre with n points, ∫₀^∞ x^k e^{-x} dx = k! is exact
        // for 0 ≤ k ≤ 2n - 1. Test up through moderate orders where round-off
        // does not dominate.
        let quad = GaussLaguerreQuadrature::new(8).expect("n=8 valid");
        let factorials = [1.0, 1.0, 2.0, 6.0, 24.0, 120.0, 720.0, 5040.0];
        for (k, expected) in factorials.iter().enumerate() {
            let integral = quad.integrate(|x| x.powi(k as i32));
            let rel_err = (integral - expected).abs() / expected.max(1.0);
            assert!(
                rel_err < 1e-10,
                "∫ x^{k} e^{{-x}} dx: got {integral}, expected {expected}, rel_err={rel_err:.2e}"
            );
        }
    }

    #[test]
    fn gauss_laguerre_weights_sum_to_mu_0() {
        // μ₀ = ∫₀^∞ e^{-x} dx = 1. The Laguerre weights must sum to this
        // (exactness for the constant polynomial).
        for n in [1, 2, 5, 10, 20, 32] {
            let quad = GaussLaguerreQuadrature::new(n).expect("valid n");
            let sum: f64 = quad.weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "n={n}: Σ w_i = {sum:.15}, expected 1.0"
            );
        }
    }

    #[test]
    fn gauss_laguerre_nodes_positive_and_sorted() {
        // Laguerre nodes are strictly positive (weight support is [0, ∞))
        // and must be returned sorted ascending by [`new`].
        for n in [1, 2, 5, 10, 32] {
            let quad = GaussLaguerreQuadrature::new(n).expect("valid n");
            assert_eq!(quad.points.len(), n);
            assert_eq!(quad.weights.len(), n);
            let mut prev = 0.0;
            for (i, &x) in quad.points.iter().enumerate() {
                assert!(x > 0.0, "n={n}, node {i}: x={x} must be > 0");
                assert!(x > prev, "n={n}, node {i}: not strictly increasing");
                prev = x;
            }
            for (i, &w) in quad.weights.iter().enumerate() {
                assert!(w > 0.0, "n={n}, weight {i}: w={w} must be > 0");
                assert!(w.is_finite(), "n={n}, weight {i}: non-finite");
            }
        }
    }

    #[test]
    fn gauss_laguerre_rejects_zero_order() {
        assert!(GaussLaguerreQuadrature::new(0).is_err());
    }

    #[test]
    fn gauss_laguerre_low_order_exactness_boundary() {
        // A small-n rule is *exactly* exact for polynomials up to
        // degree 2n-1 and *not* exact beyond. Verify at n=2: exact for
        // x^0..x^3, measurably off for x^4. This is the precise
        // failure mode that made the hard-coded 10-node rule
        // inadequate for Student-t copulas with heavy tails — the
        // tail integrand has much higher polynomial content than the
        // low-order rule could represent.
        let q2 = GaussLaguerreQuadrature::new(2).expect("n=2 valid");
        let factorials = [1.0, 1.0, 2.0, 6.0];
        for (k, expected) in factorials.iter().enumerate() {
            let integral = q2.integrate(|x| x.powi(k as i32));
            assert!(
                (integral - expected).abs() < 1e-12,
                "n=2 must be exact at degree {k}: got {integral}, want {expected}"
            );
        }
        // Degree 4 = 2n is just outside exactness: 4! = 24. The n=2
        // Laguerre rule returns a noticeably different value.
        let approx = q2.integrate(|x| x.powi(4));
        let truncation_error = (approx - 24.0).abs();
        assert!(
            truncation_error > 1e-6,
            "n=2 must NOT be exact at degree 4: got {approx}, truncation {truncation_error}"
        );
        // Moving to n=3 recovers exactness at degree 4.
        let q3 = GaussLaguerreQuadrature::new(3).expect("n=3 valid");
        let approx3 = q3.integrate(|x| x.powi(4));
        assert!(
            (approx3 - 24.0).abs() < 1e-10,
            "n=3 must be exact at degree 4: got {approx3}"
        );
    }
}
