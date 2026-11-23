//! Gauss-Hermite quadrature nodes, weights, and integration.

use super::adaptive::{DIRECT_SUM_LIMIT, quadrature_failure};
use crate::Error;
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
    /// let integral = quad.integrate(|x| Ok(x * x))?;
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

    /// Expectation of `f(Z)` for a standard normal `Z`.
    ///
    /// The stored nodes and weights follow the physicists' Gauss-Hermite rule
    /// for `∫ e^{-z^2} g(z) dz`. This method converts that convention to a
    /// standard-normal expectation by evaluating `f(√2 z)` and dividing the
    /// weighted sum by `√π`. The normalized weights sum to one, so the result
    /// is a weighted average of the samples: samples too large for the
    /// unnormalized sum are averaged with normalized weights instead, and a
    /// representable expectation is not lost to overflow.
    ///
    /// # Arguments
    ///
    /// * `f` - Fallible integrand taking a standard normal variate, evaluated
    ///   once per node in increasing coordinate order. It must describe one
    ///   deterministic function for the whole call. The first error it returns
    ///   stops the rule and is returned unchanged.
    ///
    /// # Returns
    ///
    /// The finite approximation of `∫ f(x) φ(x) dx` over the real line, where
    /// `φ` is the standard normal density.
    ///
    /// # Errors
    ///
    /// Returns the callback's own error unchanged. Returns
    /// [`InputError::SolverConvergenceFailed`](crate::InputError::SolverConvergenceFailed) (a computation error) when a
    /// sample is NaN or infinite, when a nonzero weighted sample underflows to
    /// zero, or when the weighted sum is not representable; `iterations` is
    /// the number of callbacks made and `last_x` the coordinate concerned.
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_core::math::integration::GaussHermiteQuadrature;
    ///
    /// // E[Z²] = 1 for a standard normal Z.
    /// let quad = GaussHermiteQuadrature::new(7)?;
    /// let second_moment = quad.integrate(|x| Ok(x * x))?;
    /// assert!((second_moment - 1.0).abs() < 1e-12);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn integrate<F>(&self, mut f: F) -> Result<f64, Error>
    where
        F: FnMut(f64) -> Result<f64, Error>,
    {
        const RULE: &str = "GaussHermiteQuadrature::integrate";
        const NOT_REPRESENTABLE: &str = "weighted sum is not representable";
        let sqrt_2 = std::f64::consts::SQRT_2;
        let sqrt_pi = std::f64::consts::PI.sqrt();
        let order = self.points.len();
        let mut samples = [0.0_f64; MAX_HERMITE_ORDER];
        let mut direct = true;
        for (i, &z) in self.points.iter().enumerate() {
            let x = sqrt_2 * z;
            let sample = f(x)?;
            if !sample.is_finite() {
                return Err(quadrature_failure(
                    RULE,
                    i + 1,
                    f64::NAN,
                    x,
                    "integrand was not finite",
                ));
            }
            direct &= sample.abs() <= DIRECT_SUM_LIMIT;
            samples[i] = sample;
        }
        let mut acc = NeumaierAccumulator::new();
        for (i, &sample) in samples.iter().enumerate().take(order) {
            let weight = if direct {
                self.weights[i]
            } else {
                self.weights[i] / sqrt_pi
            };
            let term = weight * sample;
            let x = sqrt_2 * self.points[i];
            if sample != 0.0 && term == 0.0 {
                return Err(quadrature_failure(
                    RULE,
                    order,
                    f64::NAN,
                    x,
                    "a nonzero weighted sample underflowed to zero",
                ));
            }
            acc.add(term)
                .map_err(|_| quadrature_failure(RULE, order, f64::NAN, x, NOT_REPRESENTABLE))?;
        }
        let total = acc
            .get_total()
            .map_err(|_| quadrature_failure(RULE, order, f64::NAN, 0.0, NOT_REPRESENTABLE))?;
        let value = if direct { total / sqrt_pi } else { total };
        if !value.is_finite() || (total != 0.0 && value == 0.0) {
            return Err(quadrature_failure(
                RULE,
                order,
                f64::NAN,
                0.0,
                NOT_REPRESENTABLE,
            ));
        }
        Ok(value)
    }
}

/// Largest supported Gauss-Hermite order.
pub(super) const MAX_HERMITE_ORDER: usize = 20;
