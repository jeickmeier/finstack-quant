//! Heston (1993) stochastic volatility model.
//!
//! Implements the Heston model for European option pricing and global
//! calibration to market-implied volatilities. Uses the Gil-Pelaez / P1-P2
//! Fourier inversion with the "Little Heston Trap" formulation from
//! Albrecher et al. (2007) for numerical stability.
//!
//! # Mathematical Foundation
//!
//! The Heston model describes the joint dynamics of an asset price and its
//! instantaneous variance:
//!
//! ```text
//! dS = (r - q) S dt + √v S dW₁
//! dv = κ(θ - v) dt + σ√v dW₂
//! E[dW₁ dW₂] = ρ dt
//!
//! where:
//!   S = asset price
//!   v = instantaneous variance
//!   κ = mean reversion speed of variance
//!   θ = long-run variance level
//!   σ = volatility of variance (vol-of-vol)
//!   ρ = correlation between asset and variance processes
//! ```
//!
//! # Parameters
//!
//! | Parameter | Symbol | Range | Market Role |
//! |-----------|--------|-------|-------------|
//! | v0 | v₀ | > 0 | Initial variance |
//! | kappa | κ | > 0 | Mean reversion speed |
//! | theta | θ | > 0 | Long-run variance |
//! | sigma | σ | > 0 | Vol-of-vol (smile curvature) |
//! | rho | ρ | (-1, 1) | Skew direction |
//!
//! # Feller Condition
//!
//! The condition 2κθ > σ² ensures the variance process remains strictly
//! positive. When violated, the process can hit zero, potentially causing
//! numerical instability. The constructor warns but does not reject.
//!
//! # References
//!
//! - Heston, S. L. (1993). "A Closed-Form Solution for Options with Stochastic
//!   Volatility with Applications to Bond and Currency Options."
//!   *Review of Financial Studies*, 6(2), 327-343.
//! - Albrecher, H., Mayer, P., Schoutens, W., & Tistaert, J. (2007).
//!   "The Little Heston Trap." *Wilmott Magazine*, January 2007.
//! - Gatheral, J. (2006). *The Volatility Surface: A Practitioner's Guide*.
//!   Wiley Finance.
//! - Kahl, C., & Jäckel, P. (2005). "Not-so-complex logarithms in the Heston
//!   model." *Wilmott Magazine*, September 2005. (Characteristic-function
//!   tail decay rate used for the quadrature truncation bound.)

use num_complex::Complex64;
use std::f64::consts::PI;

const HESTON_G_DENOM_EPS: f64 = 1e-8;
const HESTON_EXPONENT_REAL_LIMIT: f64 = 700.0;

/// Target log-magnitude decay of the characteristic function at the
/// truncation point of the Gil-Pelaez integrals: `ln(1e12) ≈ 27.63`,
/// i.e. truncate where `|ψ(φ)|` has decayed below ~1e-12.
const HESTON_TAIL_LOG_TARGET: f64 = 27.631_021_115_928_547;

/// Width (in φ-space) of each composite Gauss-Legendre panel used for the
/// Gil-Pelaez inversion. With 16-node panels this keeps the node density at
/// the level historically used for the `[0, 50]` strip (128 nodes), which
/// resolves integrand oscillation `e^{-iφ ln(S/K)}` out to deep wings
/// (|ln(S/K)| ≈ 2 gives ~2 oscillation periods per panel).
const HESTON_QUAD_PANEL_WIDTH: f64 = 6.25;

/// Bounds on the number of composite quadrature panels per integral.
const HESTON_QUAD_MIN_PANELS: usize = 8;
const HESTON_QUAD_MAX_PANELS: usize = 320;

/// Number of composite Gauss-Legendre panels for a Gil-Pelaez integral over
/// `[lower, upper]`, keeping panel width at most [`HESTON_QUAD_PANEL_WIDTH`].
fn quadrature_panels(lower: f64, upper: f64) -> usize {
    let span = (upper - lower).max(0.0);
    let panels = (span / HESTON_QUAD_PANEL_WIDTH).ceil() as usize;
    panels.clamp(HESTON_QUAD_MIN_PANELS, HESTON_QUAD_MAX_PANELS)
}

/// Heston stochastic volatility model parameters.
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_quant_core::math::volatility::heston::HestonParams;
///
/// let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).unwrap();
/// assert!(params.satisfies_feller_condition());
///
/// let call = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
/// assert!(call > 0.0 && call < 100.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawHestonParams")]
pub struct HestonParams {
    /// Initial variance (v₀ > 0).
    pub v0: f64,
    /// Mean reversion speed (κ > 0).
    pub kappa: f64,
    /// Long-run variance (θ > 0).
    pub theta: f64,
    /// Vol-of-vol (σ > 0).
    pub sigma: f64,
    /// Correlation between spot and variance (-1 < ρ < 1).
    pub rho: f64,
}

/// Raw deserialization state of [`HestonParams`].
///
/// Mirrors the serialized field layout exactly so the wire format is
/// unchanged; conversion runs [`HestonParams::new`] validation and rejects
/// unknown fields.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHestonParams {
    /// Initial variance.
    v0: f64,
    /// Mean reversion speed.
    kappa: f64,
    /// Long-run variance.
    theta: f64,
    /// Vol-of-vol.
    sigma: f64,
    /// Spot-variance correlation.
    rho: f64,
}

impl TryFrom<RawHestonParams> for HestonParams {
    type Error = crate::Error;

    fn try_from(raw: RawHestonParams) -> crate::Result<Self> {
        Self::new(raw.v0, raw.kappa, raw.theta, raw.sigma, raw.rho)
    }
}

/// Log-space spot/strike and discounting inputs shared by Gil–Pelaez \(P_j\) integration.
#[derive(Clone, Copy)]
struct HestonPjCoords {
    /// Natural log of spot (\(\ln S\)).
    x: f64,
    /// Natural log of strike (\(\ln K\)).
    ln_k: f64,
    /// Risk-free rate (continuous).
    r: f64,
    /// Dividend yield (continuous).
    q: f64,
    /// Time to expiry in years.
    t: f64,
}

struct HestonStripCache {
    panel_half_width: f64,
    order: usize,
    grid: Vec<(f64, f64)>,
    psi1_over_iphi: Vec<Complex64>,
    psi2_over_iphi: Vec<Complex64>,
}

impl HestonStripCache {
    fn new(params: &HestonParams, coords: HestonPjCoords, upper_limit: f64) -> Option<Self> {
        let order = 16;
        let (grid, panel_half_width) = composite_gauss_legendre_grid(
            1e-8,
            upper_limit,
            order,
            quadrature_panels(1e-8, upper_limit),
        )?;
        let i = Complex64::i();
        let mut psi1_over_iphi = Vec::with_capacity(grid.len());
        let mut psi2_over_iphi = Vec::with_capacity(grid.len());

        for (phi, _) in &grid {
            let denom = i * *phi;
            let psi1 = params.char_func_j(1, *phi, coords.x, coords.r, coords.q, coords.t);
            let psi2 = params.char_func_j(2, *phi, coords.x, coords.r, coords.q, coords.t);
            psi1_over_iphi.push(if psi1.is_finite() {
                psi1 / denom
            } else {
                Complex64::new(0.0, 0.0)
            });
            psi2_over_iphi.push(if psi2.is_finite() {
                psi2 / denom
            } else {
                Complex64::new(0.0, 0.0)
            });
        }

        Some(Self {
            panel_half_width,
            order,
            grid,
            psi1_over_iphi,
            psi2_over_iphi,
        })
    }

    fn probability(&self, log_strike: f64, cached_values: &[Complex64]) -> f64 {
        let i = Complex64::i();
        let mut integral = 0.0;

        for (grid_chunk, cached_chunk) in self
            .grid
            .chunks(self.order)
            .zip(cached_values.chunks(self.order))
        {
            let mut panel_sum = 0.0;
            for ((phi, weight), cached) in grid_chunk.iter().zip(cached_chunk.iter()) {
                let exp_term = (-i * *phi * log_strike).exp();
                let value = (exp_term * *cached).re;
                if value.is_finite() {
                    panel_sum += *weight * value;
                }
            }
            integral += panel_sum * self.panel_half_width;
        }

        (0.5 + integral / PI).clamp(0.0, 1.0)
    }
}

fn gl_nodes_weights(order: usize) -> Option<(&'static [f64], &'static [f64])> {
    match order {
        2 => Some((
            &[-0.577_350_269_189_625_7, 0.577_350_269_189_625_7],
            &[1.0, 1.0],
        )),
        4 => Some((
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
        8 => Some((
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
        16 => Some((
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
        _ => None,
    }
}

fn composite_gauss_legendre_grid(
    a: f64,
    b: f64,
    order: usize,
    panels: usize,
) -> Option<(Vec<(f64, f64)>, f64)> {
    if panels == 0 || !(a.is_finite() && b.is_finite()) || b <= a {
        return None;
    }

    let (xs, ws) = gl_nodes_weights(order)?;
    let h = (b - a) / panels as f64;
    let mut grid = Vec::with_capacity(xs.len() * panels);
    let panel_half_width = 0.5 * h;

    for panel_idx in 0..panels {
        let panel_start = a + panel_idx as f64 * h;
        let panel_end = panel_start + h;
        let half = 0.5 * (panel_end - panel_start);
        let mid = panel_start + half;

        for (x, w) in xs.iter().zip(ws.iter()) {
            grid.push((mid + half * x, *w));
        }
    }

    Some((grid, panel_half_width))
}

impl HestonParams {
    /// Construct validated Heston parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `v0 <= 0` or non-finite
    /// - `kappa <= 0` or non-finite
    /// - `theta <= 0` or non-finite
    /// - `sigma <= 0` or non-finite
    /// - `rho` not in `(-1, 1)` or non-finite
    ///
    /// # Feller Condition
    ///
    /// If 2κθ ≤ σ², a warning is emitted (but the parameters are still accepted).
    pub fn new(v0: f64, kappa: f64, theta: f64, sigma: f64, rho: f64) -> crate::Result<Self> {
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Heston v0 (initial variance) must be positive, got {v0}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Heston kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Heston theta (long-run variance) must be positive, got {theta}"
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Heston sigma (vol-of-vol) must be positive, got {sigma}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(crate::Error::Validation(format!(
                "Heston rho (correlation) must be in (-1, 1), got {rho}"
            )));
        }

        let params = Self {
            v0,
            kappa,
            theta,
            sigma,
            rho,
        };

        if !params.satisfies_feller_condition() {
            tracing::warn!(
                v0 = v0,
                kappa = kappa,
                theta = theta,
                sigma = sigma,
                rho = rho,
                feller_lhs = 2.0 * kappa * theta,
                feller_rhs = sigma * sigma,
                "Heston Feller condition violated (2*kappa*theta <= sigma^2). \
                 Variance process can reach zero. This is acceptable for Fourier pricing \
                 but may cause issues in Monte Carlo simulation.",
            );
        }

        Ok(params)
    }

    /// Check whether the Feller condition (2κθ > σ²) is satisfied.
    ///
    /// When satisfied, the variance process is strictly positive almost surely.
    /// When violated, variance can reach zero, which causes numerical issues
    /// in Monte Carlo simulation (though Fourier pricing remains valid).
    #[must_use]
    pub fn satisfies_feller_condition(&self) -> bool {
        2.0 * self.kappa * self.theta > self.sigma * self.sigma
    }

    /// Return these parameters unchanged if the Feller condition holds,
    /// otherwise return an error.
    ///
    /// Use this when the parameters will be used downstream in Monte Carlo
    /// simulation where variance must stay strictly positive. Fourier-based
    /// pricing does not require the Feller condition.
    ///
    /// # Errors
    ///
    /// Returns an error if `2κθ ≤ σ²`.
    pub fn require_feller(self) -> crate::Result<Self> {
        if self.satisfies_feller_condition() {
            Ok(self)
        } else {
            Err(crate::Error::Validation(format!(
                "Heston Feller condition violated: 2*kappa*theta ({:.6}) <= sigma^2 ({:.6}). \
                 Variance process can reach zero, causing numerical issues in Monte Carlo. \
                 Use satisfies_feller_condition() to check, or omit require_feller() for \
                 Fourier-only pricing.",
                2.0 * self.kappa * self.theta,
                self.sigma * self.sigma,
            )))
        }
    }

    /// Price a European option using Fourier integration.
    ///
    /// Uses the Gil-Pelaez / P1-P2 formulation:
    /// ```text
    /// Call = S × exp(-qT) × P₁ - K × exp(-rT) × P₂
    /// Put  = Call - S × exp(-qT) + K × exp(-rT)   (put-call parity)
    /// ```
    ///
    /// where P₁ and P₂ are computed via numerical integration of the
    /// Heston characteristic function using composite Gauss-Legendre quadrature.
    ///
    /// # Arguments
    ///
    /// * `spot` - Current spot price
    /// * `strike` - Strike price
    /// * `r` - Risk-free rate (continuous compounding)
    /// * `q` - Dividend yield (continuous compounding)
    /// * `t` - Time to expiry in years
    /// * `is_call` - `true` for call, `false` for put
    ///
    /// # Returns
    ///
    /// Option price (non-negative).
    #[must_use]
    pub fn price_european(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        q: f64,
        t: f64,
        is_call: bool,
    ) -> f64 {
        if t <= 0.0 {
            if !spot.is_finite() || !strike.is_finite() {
                return f64::NAN;
            }
            return if is_call {
                (spot - strike).max(0.0)
            } else {
                (strike - spot).max(0.0)
            };
        }
        if !spot.is_finite()
            || !strike.is_finite()
            || !r.is_finite()
            || !q.is_finite()
            || !t.is_finite()
            || spot <= 0.0
            || strike <= 0.0
        {
            return f64::NAN;
        }

        // Degenerate case: very small vol-of-vol → use Black-Scholes with the
        // time-averaged deterministic variance (σ_v → 0 limit of CIR).
        if self.sigma < 1e-10 {
            return bs_call_fallback(
                spot,
                strike,
                r,
                q,
                t,
                self.deterministic_avg_vol(t),
                is_call,
            );
        }

        let (p1, p2) = self.compute_p1_p2(spot, strike, r, q, t);
        if !p1.is_finite() || !p2.is_finite() {
            return f64::NAN;
        }

        // Compute the put via parity from the UNclamped call, then clamp each
        // result once at the end. Clamping the call before applying parity
        // would shift the put by the clamped amount and break parity.
        let call = spot * (-q * t).exp() * p1 - strike * (-r * t).exp() * p2;

        if is_call {
            call.max(0.0)
        } else {
            // Put-call parity
            (call - spot * (-q * t).exp() + strike * (-r * t).exp()).max(0.0)
        }
    }

    /// Volatility of the σ_v → 0 deterministic-variance limit over `[0, t]`.
    ///
    /// When vol-of-vol vanishes, the CIR variance follows the deterministic
    /// path `v(s) = θ + (v₀ − θ)e^{−κs}`. The Black-Scholes-equivalent
    /// volatility is the square root of the time-averaged variance:
    ///
    /// ```text
    /// v̄ = θ + (v₀ − θ)(1 − e^{−κT}) / (κT),   σ = √v̄
    /// ```
    ///
    /// Using `√v₀` instead (the pre-fix behaviour) ignores mean reversion and
    /// is correct only when `v₀ = θ`.
    fn deterministic_avg_vol(&self, t: f64) -> f64 {
        let kt = self.kappa * t;
        let v_bar = if kt > 1e-12 {
            self.theta + (self.v0 - self.theta) * (1.0 - (-kt).exp()) / kt
        } else {
            self.v0
        };
        v_bar.max(0.0).sqrt()
    }

    /// Price a strip of European options sharing the same expiry and model inputs.
    ///
    /// Reuses the strike-independent part of the Fourier integrand across all
    /// strikes, reducing characteristic-function evaluations from O(strikes x grid)
    /// to O(grid).
    #[must_use]
    pub fn price_european_strip(
        &self,
        spot: f64,
        strikes: &[f64],
        r: f64,
        q: f64,
        t: f64,
        is_call: bool,
    ) -> Vec<f64> {
        if strikes.is_empty() {
            return Vec::new();
        }

        if t <= 0.0 {
            return strikes
                .iter()
                .map(|&strike| {
                    if !spot.is_finite() || !strike.is_finite() {
                        f64::NAN
                    } else if is_call {
                        (spot - strike).max(0.0)
                    } else {
                        (strike - spot).max(0.0)
                    }
                })
                .collect();
        }

        if !spot.is_finite()
            || !r.is_finite()
            || !q.is_finite()
            || !t.is_finite()
            || spot <= 0.0
            || strikes
                .iter()
                .any(|&strike| !strike.is_finite() || strike <= 0.0)
        {
            return strikes.iter().map(|_| f64::NAN).collect();
        }

        if self.sigma < 1e-10 {
            let vol = self.deterministic_avg_vol(t);
            return strikes
                .iter()
                .map(|&strike| bs_call_fallback(spot, strike, r, q, t, vol, is_call))
                .collect();
        }

        let coords = HestonPjCoords {
            x: spot.ln(),
            ln_k: strikes[0].ln(),
            r,
            q,
            t,
        };

        let upper_limit = self.integration_upper_limit(t);
        let Some(cache) = HestonStripCache::new(self, coords, upper_limit) else {
            return strikes
                .iter()
                .map(|&strike| self.price_european(spot, strike, r, q, t, is_call))
                .collect();
        };

        strikes
            .iter()
            .map(|&strike| {
                let log_strike = strike.ln();
                let p1 = cache.probability(log_strike, &cache.psi1_over_iphi);
                let p2 = cache.probability(log_strike, &cache.psi2_over_iphi);
                // Parity is applied to the unclamped call; clamp once at the end.
                let call = spot * (-q * t).exp() * p1 - strike * (-r * t).exp() * p2;

                if is_call {
                    call.max(0.0)
                } else {
                    (call - spot * (-q * t).exp() + strike * (-r * t).exp()).max(0.0)
                }
            })
            .collect()
    }

    /// Compute both P₁ and P₂ in a single Gauss-Legendre pass.
    ///
    /// Evaluates char_func_j for j=1 and j=2 at each quadrature point
    /// simultaneously, halving the number of integration passes compared
    /// to two separate `compute_pj` calls.
    fn compute_p1_p2(&self, spot: f64, strike: f64, r: f64, q: f64, t: f64) -> (f64, f64) {
        let coords = HestonPjCoords {
            x: spot.ln(),
            ln_k: strike.ln(),
            r,
            q,
            t,
        };

        let upper_limit = self.integration_upper_limit(t);
        self.compute_p1_p2_with_upper_limit(coords, upper_limit)
    }

    fn compute_p1_p2_with_upper_limit(
        &self,
        coords: HestonPjCoords,
        upper_limit: f64,
    ) -> (f64, f64) {
        let (i1, i2) = self
            .compute_p1_p2_interval_integral(coords, 1e-8, upper_limit)
            .unwrap_or((f64::NAN, f64::NAN));

        (
            if i1.is_finite() {
                (0.5 + i1 / PI).clamp(0.0, 1.0)
            } else {
                f64::NAN
            },
            if i2.is_finite() {
                (0.5 + i2 / PI).clamp(0.0, 1.0)
            } else {
                f64::NAN
            },
        )
    }

    /// Gauss-Legendre integration computing both P₁ and P₂ integrands at each
    /// quadrature point, sharing the `exp(-iφ ln K)/(iφ)` factor.
    fn compute_p1_p2_interval_integral(
        &self,
        coords: HestonPjCoords,
        lower: f64,
        upper: f64,
    ) -> Option<(f64, f64)> {
        if !(lower.is_finite() && upper.is_finite()) || upper <= lower {
            return None;
        }

        let integrand_pair = |phi: f64| -> (f64, f64) {
            (
                self.fourier_integrand(1, phi, coords),
                self.fourier_integrand(2, phi, coords),
            )
        };

        let panels = quadrature_panels(lower, upper);
        let order = 16_usize;
        let (xs, ws) = gl_nodes_weights(order)?;
        let h = (upper - lower) / panels as f64;
        let mut sum1 = 0.0_f64;
        let mut sum2 = 0.0_f64;

        for panel_idx in 0..panels {
            let panel_start = lower + panel_idx as f64 * h;
            let half = 0.5 * h;
            let mid = panel_start + half;

            for (x, w) in xs.iter().zip(ws.iter()) {
                let phi = mid + half * x;
                let (f1, f2) = integrand_pair(phi);
                let weight = half * w;
                sum1 += weight * f1;
                sum2 += weight * f2;
            }
        }

        Some((sum1, sum2))
    }

    /// Compute probability P_j via Fourier inversion (single-j variant for tests).
    ///
    /// P_j = 1/2 + (1/π) ∫₀^∞ Re[exp(-iφ ln K) ψ_j(φ) / (iφ)] dφ
    #[cfg(test)]
    fn compute_pj(&self, j: u8, spot: f64, strike: f64, r: f64, q: f64, t: f64) -> f64 {
        let coords = HestonPjCoords {
            x: spot.ln(),
            ln_k: strike.ln(),
            r,
            q,
            t,
        };

        let upper_limit = self.integration_upper_limit(t);
        self.compute_pj_with_upper_limit(j, coords, upper_limit)
    }

    #[cfg(test)]
    fn compute_pj_with_upper_limit(&self, j: u8, coords: HestonPjCoords, upper_limit: f64) -> f64 {
        let integral = self
            .compute_pj_interval_integral(j, coords, 1e-8, upper_limit)
            .unwrap_or(f64::NAN);

        if !integral.is_finite() {
            return f64::NAN;
        }

        (0.5 + integral / PI).clamp(0.0, 1.0)
    }

    #[cfg(test)]
    fn compute_pj_interval_integral(
        &self,
        j: u8,
        coords: HestonPjCoords,
        lower: f64,
        upper: f64,
    ) -> Option<f64> {
        if !(lower.is_finite() && upper.is_finite()) || upper <= lower {
            return None;
        }

        let integrand = |phi: f64| -> f64 { self.fourier_integrand(j, phi, coords) };

        crate::math::integration::gauss_legendre_integrate_composite(
            integrand,
            lower,
            upper,
            16,
            quadrature_panels(lower, upper),
        )
        .ok()
    }

    fn fourier_integrand(&self, j: u8, phi: f64, coords: HestonPjCoords) -> f64 {
        if phi.abs() < 1e-10 {
            return self.fourier_integrand_origin_limit(j, coords);
        }

        let i = Complex64::i();
        let psi = self.char_func_j(j, phi, coords.x, coords.r, coords.q, coords.t);
        if !psi.is_finite() {
            return 0.0;
        }
        let exp_term = (-i * phi * coords.ln_k).exp();
        let val = (exp_term * psi / (i * phi)).re;
        if val.is_finite() {
            val
        } else {
            0.0
        }
    }

    fn fourier_integrand_origin_limit(&self, j: u8, coords: HestonPjCoords) -> f64 {
        let h = 1.0e-5;
        let psi_plus = self.char_func_j(j, h, coords.x, coords.r, coords.q, coords.t);
        let psi_minus = self.char_func_j(j, -h, coords.x, coords.r, coords.q, coords.t);
        if !(psi_plus.is_finite() && psi_minus.is_finite()) {
            return 0.0;
        }
        let dpsi = (psi_plus - psi_minus) / (2.0 * h);
        let first_log_moment = (dpsi / Complex64::i()).re;
        let limit = first_log_moment - coords.ln_k;
        if limit.is_finite() {
            limit
        } else {
            0.0
        }
    }

    /// Upper integration limit for the Gil-Pelaez inversion.
    ///
    /// Chooses the truncation point `φ_max` so the characteristic function
    /// magnitude has decayed below ~1e-12, covering both tail regimes:
    ///
    /// 1. **Asymptotic (large `φ·t`)**: `|ψ(φ)| ≈ exp(−C∞ φ)` with
    ///    `C∞ = √(1−ρ²) (v₀ + κθT) / σ` (Kahl & Jäckel 2005, "Not-so-complex
    ///    logarithms in the Heston model", *Wilmott*, Sec. 5). This dominates
    ///    for long maturities and high vol-of-vol, where decay is far slower
    ///    than Gaussian.
    /// 2. **Pre-asymptotic (short-dated, `d·t ≲ 1`)**: the CF behaves
    ///    Black-Scholes-like, `|ψ(φ)| ≈ exp(−½ v̄ t φ²)`; using
    ///    `v̄ = min(v₀, θ)` is conservative.
    ///
    /// `φ_max` is the larger of the two bounds, clamped to `[50, 2000]`. The
    /// composite quadrature scales its panel count with the interval (see
    /// [`quadrature_panels`]) so node density — and therefore resolution of
    /// the `e^{-iφ ln K}` oscillation, including deep wings — is preserved
    /// regardless of the truncation point.
    fn integration_upper_limit(&self, t: f64) -> f64 {
        if self.sigma > 0.0 && t > 0.0 {
            let c_inf = (1.0 - self.rho * self.rho).sqrt()
                * (self.v0 + self.kappa * self.theta * t)
                / self.sigma;
            let exp_bound = if c_inf > 0.0 {
                HESTON_TAIL_LOG_TARGET / c_inf
            } else {
                f64::INFINITY
            };
            let v_min = self.v0.min(self.theta);
            let gauss_bound = if v_min > 0.0 {
                (2.0 * HESTON_TAIL_LOG_TARGET / (v_min * t)).sqrt()
            } else {
                f64::INFINITY
            };
            exp_bound.max(gauss_bound).clamp(50.0, 2_000.0)
        } else {
            100.0
        }
    }

    /// Characteristic function ψ_j(φ) for the Heston model.
    ///
    /// Uses the "Little Heston Trap" formulation (Albrecher et al. 2007)
    /// which places −d in the numerator of g, ensuring |g exp(−dT)| < 1
    /// and avoiding branch-cut discontinuities.
    fn char_func_j(&self, j: u8, phi: f64, x: f64, r: f64, q: f64, t: f64) -> Complex64 {
        let kappa = self.kappa;
        let theta = self.theta;
        let sigma = self.sigma;
        let rho = self.rho;
        let v0 = self.v0;

        let i = Complex64::i();
        let one = Complex64::new(1.0, 0.0);
        let zero = Complex64::new(0.0, 0.0);

        // For P₁: u = 0.5, b = κ − ρσ  (stock numeraire)
        // For P₂: u = −0.5, b = κ       (money market numeraire)
        let (u_j, b_j) = if j == 1 {
            (0.5, kappa - rho * sigma)
        } else {
            (-0.5, kappa)
        };

        let a = kappa * theta;
        let sigma_sq = sigma * sigma;

        // d = sqrt((ρσiφ − b)² − σ²(2u_j iφ − φ²))
        let rsi_phi = Complex64::new(0.0, rho * sigma * phi);
        let b = Complex64::new(b_j, 0.0);
        let d_sq = (rsi_phi - b).powi(2) - sigma_sq * (Complex64::new(-phi * phi, 2.0 * u_j * phi));
        let d = d_sq.sqrt();

        // Little Heston Trap: g = (b − ρσiφ − d)/(b − ρσiφ + d)
        let bm = b - rsi_phi;
        let g_denom = bm + d;
        let g_denom_limit = HESTON_G_DENOM_EPS * (1.0 + bm.norm() + d.norm());
        if !g_denom.is_finite() || g_denom.norm() <= g_denom_limit {
            return zero;
        }
        let g = (bm - d) / g_denom;
        if !g.is_finite() {
            return zero;
        }

        let exp_minus_dt = (-d * t).exp();
        if !exp_minus_dt.is_finite() {
            return zero;
        }

        // C = (r−q)iφT + (a/σ²)[(b−ρσiφ−d)T − 2 ln((1−g exp(−dT))/(1−g))]
        let c_val = i * phi * (r - q) * t
            + (a / sigma_sq)
                * ((bm - d) * t
                    - Complex64::new(2.0, 0.0) * ((one - g * exp_minus_dt) / (one - g)).ln());

        // D = (b−ρσiφ−d)/σ² × (1−exp(−dT))/(1−g exp(−dT))
        let d_val = ((bm - d) / sigma_sq) * (one - exp_minus_dt) / (one - g * exp_minus_dt);
        if !c_val.is_finite() || !d_val.is_finite() {
            return zero;
        }

        let exponent = c_val + d_val * v0 + i * phi * x;
        if !exponent.is_finite() || exponent.re > HESTON_EXPONENT_REAL_LIMIT {
            return zero;
        }

        // ψ_j(φ) = exp(C + D v₀ + iφx)
        let psi = exponent.exp();
        if psi.is_finite() {
            psi
        } else {
            zero
        }
    }
}

/// Calibration diagnostics returned alongside fitted parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HestonCalibrationResult {
    /// Calibrated Heston parameters.
    pub params: HestonParams,
    /// Root mean square error of volatility residuals (in vol units).
    pub rmse: f64,
    /// Number of solver iterations.
    pub iterations: usize,
    /// Whether the solver converged.
    pub converged: bool,
}

/// Calibrate Heston model parameters from market implied volatilities.
///
/// Fits the five Heston parameters (v₀, κ, θ, σ, ρ) by minimising
/// vega-weighted price differences across all expiry/strike pairs using
/// the Levenberg-Marquardt algorithm.
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `r` - Risk-free rate (continuous compounding)
/// * `q` - Dividend yield (continuous compounding)
/// * `expiries` - Expiry times for each slice (years)
/// * `strikes` - Strike prices per expiry: `strikes[i]` for `expiries[i]`
/// * `market_vols` - Market Black-76 implied vols per expiry: `market_vols[i]` for `expiries[i]`
///
/// # Returns
///
/// [`HestonCalibrationResult`] containing the fitted parameters and diagnostics.
///
/// # Algorithm
///
/// Uses an unconstrained parameterisation to map the bounded Heston
/// parameters to ℝ⁵:
///
/// | Heston | Unconstrained |
/// |--------|---------------|
/// | v₀ > 0 | x₀ = ln(v₀) |
/// | κ > 0 | x₁ = ln(κ) |
/// | θ > 0 | x₂ = ln(θ) |
/// | σ > 0 | x₃ = ln(σ) |
/// | ρ ∈ (−1,1) | x₄ = atanh(ρ) |
///
/// # Errors
///
/// Returns an error if:
/// - `expiries`, `strikes`, `market_vols` lengths are inconsistent
/// - Fewer than 5 data points (need at least 5 for 5 free parameters)
/// - Calibration fails to converge
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_quant_core::math::volatility::heston::{HestonParams, calibrate_heston};
///
/// let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).unwrap();
/// let spot = 100.0;
/// let r = 0.05;
/// let q = 0.0;
/// let expiries = [1.0];
/// let strikes_1y: Vec<f64> = vec![90.0, 95.0, 100.0, 105.0, 110.0];
/// let vols: Vec<f64> = strikes_1y.iter().map(|&k| {
///     // Generate synthetic vols by inverting Heston prices through BS
///     let price = params.price_european(spot, k, r, q, 1.0, true);
///     let fwd = spot * ((r - q) * 1.0).exp();
///     finstack_quant_core::math::volatility::implied_vol_black(price, fwd, k, 1.0, true)
///         .unwrap_or(0.2)
/// }).collect();
/// let strikes = [strikes_1y.as_slice()];
/// let market_vols = [vols.as_slice()];
///
/// let result = calibrate_heston(spot, r, q, &expiries, &strikes, &market_vols).unwrap();
/// assert!(result.rmse < 0.01);
/// ```
pub fn calibrate_heston(
    spot: f64,
    r: f64,
    q: f64,
    expiries: &[f64],
    strikes: &[&[f64]],
    market_vols: &[&[f64]],
) -> crate::Result<HestonCalibrationResult> {
    // ---- Validate inputs ----
    if expiries.len() != strikes.len() || expiries.len() != market_vols.len() {
        return Err(crate::Error::Validation(
            "expiries, strikes, and market_vols must have the same outer length".to_string(),
        ));
    }
    let mut n_total = 0usize;
    for (i, (&t, (ks, vs))) in expiries
        .iter()
        .zip(strikes.iter().zip(market_vols.iter()))
        .enumerate()
    {
        if ks.len() != vs.len() {
            return Err(crate::Error::Validation(format!(
                "strikes[{i}] and market_vols[{i}] must have the same length"
            )));
        }
        if t <= 0.0 {
            return Err(crate::Error::Validation(format!(
                "expiries[{i}] must be positive, got {t}"
            )));
        }
        n_total += ks.len();
    }
    if n_total < 5 {
        return Err(crate::Error::Validation(format!(
            "Need at least 5 data points for Heston calibration (5 free parameters), got {n_total}"
        )));
    }

    // ---- Flatten market data and pre-compute market prices + vegas ----
    let mut flat_market_price = Vec::with_capacity(n_total);
    let mut flat_vega = Vec::with_capacity(n_total);

    for (&t, (ks, vs)) in expiries.iter().zip(strikes.iter().zip(market_vols.iter())) {
        let fwd = spot * ((r - q) * t).exp();
        let df = (-r * t).exp();
        for (&k, &vol) in ks.iter().zip(vs.iter()) {
            let mkt_price = df * crate::math::volatility::black_call(fwd, k, vol, t);
            let vega = (df * crate::math::volatility::black_vega(fwd, k, vol, t)).max(1e-10);
            flat_market_price.push(mkt_price);
            flat_vega.push(vega);
        }
    }

    // ---- Initial guess ----
    // ATM vol → initial v0; reasonable defaults for other params
    let atm_vol = {
        let mut best = 0.2_f64;
        let mut best_dist = f64::MAX;
        for (&t, (ks, vs)) in expiries.iter().zip(strikes.iter().zip(market_vols.iter())) {
            let fwd = spot * ((r - q) * t).exp();
            for (&k, &v) in ks.iter().zip(vs.iter()) {
                let dist = ((k - fwd) / fwd).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best = v;
                }
            }
        }
        best
    };
    let v0_init = (atm_vol * atm_vol).max(1e-4);
    let kappa_init: f64 = 2.0;
    let theta_init = v0_init;
    let sigma_init: f64 = 0.3;
    let rho_init: f64 = -0.5;

    // Unconstrained parameterisation
    let x0 = [
        v0_init.ln(),
        kappa_init.ln(),
        theta_init.ln(),
        sigma_init.ln(),
        rho_init.clamp(-0.999, 0.999).atanh(),
    ];

    // ---- LM residual function ----
    // Residual_i = (model_price_i − market_price_i) / vega_i
    let residuals = |x: &[f64], resid: &mut [f64]| {
        let v0 = x[0].exp();
        let kappa = x[1].exp();
        let theta = x[2].exp();
        let sigma = x[3].exp();
        let rho = x[4].tanh();

        let params = HestonParams {
            v0,
            kappa,
            theta,
            sigma,
            rho,
        };

        let mut idx = 0usize;
        for (&t, ks) in expiries.iter().zip(strikes.iter()) {
            let model_prices = params.price_european_strip(spot, ks, r, q, t, true);
            for &model_price in &model_prices {
                let mkt_p = flat_market_price[idx];
                let vega = flat_vega[idx];
                if model_price.is_finite() {
                    resid[idx] = (model_price - mkt_p) / vega;
                } else {
                    resid[idx] = 1.0; // penalty
                }
                idx += 1;
            }
        }
    };

    // ---- Solve ----
    let solver = crate::math::solver_multi::LevenbergMarquardtSolver::new()
        .with_tolerance(1e-10)
        .with_max_iterations(300);

    let solution = solver.solve_system_with_dim_stats(residuals, &x0, n_total)?;

    let v0 = solution.params[0].exp();
    let kappa = solution.params[1].exp();
    let theta = solution.params[2].exp();
    let sigma = solution.params[3].exp();
    let rho = solution.params[4].tanh();

    // ---- Compute RMSE in vol space ----
    let fitted = HestonParams {
        v0,
        kappa,
        theta,
        sigma,
        rho,
    };
    let mut sse = 0.0;
    for (&t, (ks, vs)) in expiries.iter().zip(strikes.iter().zip(market_vols.iter())) {
        let fwd = spot * ((r - q) * t).exp();
        let df = (-r * t).exp();
        for (&k, &mv) in ks.iter().zip(vs.iter()) {
            let model_price = fitted.price_european(spot, k, r, q, t, true);
            let model_vol =
                crate::math::volatility::implied_vol_black(model_price / df, fwd, k, t, true)
                    .unwrap_or(mv);
            sse += (model_vol - mv) * (model_vol - mv);
        }
    }
    let rmse = (sse / n_total as f64).sqrt();

    let converged = matches!(
        solution.stats.termination_reason,
        crate::math::solver_multi::LmTerminationReason::ConvergedResidualNorm
            | crate::math::solver_multi::LmTerminationReason::ConvergedRelativeReduction
            | crate::math::solver_multi::LmTerminationReason::ConvergedGradient
            | crate::math::solver_multi::LmTerminationReason::StepTooSmall
    );

    // Validate recovered parameters
    let params = HestonParams::new(v0, kappa, theta, sigma, rho)?;

    Ok(HestonCalibrationResult {
        params,
        rmse,
        iterations: solution.stats.iterations,
        converged,
    })
}

/// Black-Scholes fallback for degenerate Heston (σ_v ≈ 0).
fn bs_call_fallback(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    t: f64,
    vol: f64,
    is_call: bool,
) -> f64 {
    use crate::math::special_functions::norm_cdf;

    if vol <= 0.0 || t <= 0.0 {
        return if is_call {
            (spot * (-q * t).exp() - strike * (-r * t).exp()).max(0.0)
        } else {
            (strike * (-r * t).exp() - spot * (-q * t).exp()).max(0.0)
        };
    }

    let sqrt_t = t.sqrt();
    // d1/d2 intentionally inline: In finstack_quant_core, cannot import from valuations
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * vol * vol) * t) / (vol * sqrt_t);
    let d2 = d1 - vol * sqrt_t;

    let call = spot * (-q * t).exp() * norm_cdf(d1) - strike * (-r * t).exp() * norm_cdf(d2);

    if is_call {
        call.max(0.0)
    } else {
        (call - spot * (-q * t).exp() + strike * (-r * t).exp()).max(0.0)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heston_params_validation() {
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).is_ok());
        assert!(HestonParams::new(0.0, 2.0, 0.04, 0.3, -0.5).is_err()); // v0 = 0
        assert!(HestonParams::new(-0.01, 2.0, 0.04, 0.3, -0.5).is_err()); // v0 < 0
        assert!(HestonParams::new(0.04, 0.0, 0.04, 0.3, -0.5).is_err()); // kappa = 0
        assert!(HestonParams::new(0.04, 2.0, 0.0, 0.3, -0.5).is_err()); // theta = 0
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.0, -0.5).is_err()); // sigma = 0
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, -1.0).is_err()); // rho = -1
        assert!(HestonParams::new(0.04, 2.0, 0.04, 0.3, 1.0).is_err()); // rho = 1
    }

    #[test]
    fn feller_condition() {
        // Satisfies: 2*2*0.04 = 0.16 > 0.09 = 0.3²
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        assert!(p.satisfies_feller_condition());

        // Violates: 2*0.5*0.04 = 0.04 < 0.25 = 0.5²
        let p2 = HestonParams::new(0.04, 0.5, 0.04, 0.5, -0.5).expect("valid");
        assert!(!p2.satisfies_feller_condition());
    }

    #[test]
    fn require_feller_accepts_only_strict_feller_parameters() {
        let ok = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        assert_eq!(ok.require_feller().expect("satisfies Feller"), ok);

        let violates = HestonParams::new(0.04, 0.5, 0.04, 0.5, -0.5).expect("valid");
        let err = violates
            .require_feller()
            .expect_err("violates Feller condition");
        assert!(
            err.to_string().contains("Feller condition violated"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn call_price_positive_and_bounded() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let call = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        assert!(call > 0.0, "Call should be positive, got {call}");
        assert!(call < 100.0, "Call should be < spot, got {call}");
    }

    #[test]
    fn put_call_parity() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7).expect("valid");
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.02;
        let t = 1.0;

        let call = p.price_european(s, k, r, q, t, true);
        let put = p.price_european(s, k, r, q, t, false);

        let lhs = call - put;
        let rhs = s * (-q * t).exp() - k * (-r * t).exp();

        assert!(
            (lhs - rhs).abs() < 0.01,
            "Put-call parity: C−P = {lhs:.4}, S·e^{{-qT}} − K·e^{{-rT}} = {rhs:.4}"
        );
    }

    #[test]
    fn moneyness_ordering() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let itm = p.price_european(100.0, 90.0, 0.05, 0.0, 1.0, true);
        let atm = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let otm = p.price_european(100.0, 110.0, 0.05, 0.0, 1.0, true);

        assert!(itm > atm, "ITM > ATM: {itm:.4} vs {atm:.4}");
        assert!(atm > otm, "ATM > OTM: {atm:.4} vs {otm:.4}");
    }

    #[test]
    fn black_scholes_limit() {
        let vol = 0.2;
        let var = vol * vol;
        // sigma_v → 0: Heston degenerates to Black-Scholes
        let p = HestonParams::new(var, 2.0, var, 1e-12, 0.0).expect("valid");
        let heston = p.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let bs = bs_call_fallback(100.0, 100.0, 0.05, 0.0, 1.0, vol, true);

        assert!(
            (heston - bs).abs() < 0.01,
            "Heston → BS limit: Heston={heston:.4}, BS={bs:.4}"
        );
    }

    #[test]
    fn sigma_v_zero_fallback_uses_time_averaged_variance() {
        // v0 ≠ θ: the σ_v → 0 limit is the time-averaged deterministic CIR
        // variance, NOT v0. v̄ = θ + (v0−θ)(1−e^{−κT})/(κT).
        let v0 = 0.01;
        let theta = 0.09;
        let kappa = 2.0;
        let t = 1.0;
        let p = HestonParams::new(v0, kappa, theta, 1e-12, 0.0).expect("valid");

        // Closed form
        let v_bar = theta + (v0 - theta) * (1.0 - (-kappa * t).exp()) / (kappa * t);
        assert!(
            (v_bar - 0.0554134).abs() < 1e-6,
            "expected v̄ ≈ 0.0554, got {v_bar:.6}"
        );

        // Brute-force average of the deterministic variance path
        // v(s) = θ + (v0−θ)e^{−κs} via fine trapezoidal integration.
        let n = 100_000;
        let h = t / n as f64;
        let v = |s: f64| theta + (v0 - theta) * (-kappa * s).exp();
        let mut integral = 0.0;
        for j in 0..n {
            integral += 0.5 * (v(j as f64 * h) + v((j + 1) as f64 * h)) * h;
        }
        let v_bar_numeric = integral / t;
        assert!(
            (v_bar - v_bar_numeric).abs() < 1e-9,
            "closed form v̄={v_bar:.10} vs brute force {v_bar_numeric:.10}"
        );

        // The fallback price must match Black-Scholes at σ = √v̄ (≈ 23.5%),
        // not at √v0 = 10%.
        let heston = p.price_european(100.0, 100.0, 0.05, 0.0, t, true);
        let bs_avg = bs_call_fallback(100.0, 100.0, 0.05, 0.0, t, v_bar.sqrt(), true);
        let bs_v0 = bs_call_fallback(100.0, 100.0, 0.05, 0.0, t, v0.sqrt(), true);
        assert!(
            (heston - bs_avg).abs() < 1e-10,
            "fallback should use √v̄: heston={heston:.6}, bs(√v̄)={bs_avg:.6}"
        );
        assert!(
            (heston - bs_v0).abs() > 1.0,
            "fallback must not collapse to √v0: heston={heston:.6}, bs(√v0)={bs_v0:.6}"
        );
    }

    #[test]
    fn put_call_parity_survives_clamping_in_extreme_region() {
        // Deep-OTM call / deep-ITM put region (~5-8 stddevs OTM) where the
        // raw call is ≈ 0 and quadrature noise can push it slightly negative.
        // The put is derived from the UNclamped call, so parity holds up to
        // the clamped noise rather than drifting by the full clamped amount.
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.0;
        let t: f64 = 0.5;

        for &strike in &[220.0_f64, 260.0, 300.0] {
            let call = p.price_european(spot, strike, r, q, t, true);
            let put = p.price_european(spot, strike, r, q, t, false);
            // call clamps to ~0 here; the put is derived from the UNclamped
            // call, so the parity residual is bounded by the clamped
            // quadrature noise on the raw call (~1e-7 in this region).
            let parity_residual = call - put - (spot * (-q * t).exp() - strike * (-r * t).exp());
            assert!(
                parity_residual.abs() < 1e-5,
                "K={strike}: parity residual {parity_residual:.2e} too large \
                 (call={call:.10}, put={put:.10})"
            );
            let intrinsic_fwd = strike * (-r * t).exp() - spot * (-q * t).exp();
            assert!(
                (put - intrinsic_fwd).abs() < 1e-5,
                "K={strike}: deep ITM put should be ≈ forward intrinsic: \
                 put={put:.8}, intrinsic={intrinsic_fwd:.8}"
            );
        }
    }

    #[test]
    fn heston_params_serde_validates_on_deserialize() {
        // Valid JSON round-trips.
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let json = serde_json::to_string(&p).expect("serialize");
        let back: HestonParams = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(p, back);

        // Out-of-range rho rejected.
        let bad = r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma":0.3,"rho":1.5}"#;
        assert!(serde_json::from_str::<HestonParams>(bad).is_err());

        // Unknown field rejected.
        let unknown = r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma":0.3,"rho":-0.5,"extra":1.0}"#;
        assert!(serde_json::from_str::<HestonParams>(unknown).is_err());
    }

    #[test]
    fn expired_option() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let itm_call = p.price_european(100.0, 90.0, 0.05, 0.0, 0.0, true);
        assert!((itm_call - 10.0).abs() < 1e-10, "Expired ITM call");

        let otm_call = p.price_european(100.0, 110.0, 0.05, 0.0, 0.0, true);
        assert!(otm_call.abs() < 1e-10, "Expired OTM call");

        let itm_put = p.price_european(100.0, 110.0, 0.05, 0.0, 0.0, false);
        assert!((itm_put - 10.0).abs() < 1e-10, "Expired ITM put");
    }

    #[test]
    fn invalid_inputs_return_nan() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let price = p.price_european(100.0, 0.0, 0.05, 0.0, 1.0, true);
        assert!(price.is_nan());
    }

    #[test]
    fn heston_upper_bound_captures_short_dated_tail() {
        // Short-dated/low-variance parameters where the CF decays
        // Black-Scholes-like (exp(-v t phi^2 / 2)); the pre-fix sigma-based
        // heuristic truncated at phi=500 and left material tail mass.
        let p = HestonParams::new(0.01, 3.0, 0.01, 0.02, -0.5).expect("valid");
        let spot: f64 = 100.0;
        let strike: f64 = 100.0;
        let r: f64 = 0.01;
        let q: f64 = 0.0;
        let t: f64 = 0.005;
        let coords = HestonPjCoords {
            x: spot.ln(),
            ln_k: strike.ln(),
            r,
            q,
            t,
        };
        let upper = p.integration_upper_limit(t);

        let at_heuristic = p.compute_pj(1, spot, strike, r, q, t);
        let extended = p.compute_pj_with_upper_limit(1, coords, 2.0 * upper);
        let truncated = p.compute_pj_with_upper_limit(1, coords, 0.5 * upper);

        assert!(
            (truncated - extended).abs() > 1e-6,
            "test case should exercise a materially non-zero tail: upper={upper}, truncated={truncated}, extended={extended}"
        );
        assert!(
            (at_heuristic - extended).abs() < 1e-8,
            "heuristic upper limit should capture the integrand tail: upper={upper}, at_heuristic={at_heuristic}, extended={extended}"
        );
    }

    #[test]
    fn heston_characteristic_function_handles_extreme_inputs() {
        let p = HestonParams::new(0.04, 0.1, 0.04, 1.0, 0.9).expect("valid");
        let psi = p.char_func_j(1, 0.0, 100.0_f64.ln(), 0.05, 0.0, 1.0);
        assert!(
            psi.is_finite(),
            "characteristic function should stay finite"
        );
    }

    #[test]
    fn heston_fourier_integrand_origin_uses_finite_limit() {
        let p = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let coords = HestonPjCoords {
            x: 100.0_f64.ln(),
            ln_k: 100.0_f64.ln(),
            r: 0.05,
            q: 0.01,
            t: 1.0,
        };

        let origin = p.fourier_integrand_origin_limit(1, coords);
        let nearby = p.fourier_integrand(1, 1.0e-6, coords);

        assert!(origin.is_finite());
        assert!(
            origin.abs() > 1.0e-6,
            "origin limit should not be silently zero"
        );
        assert!(
            (origin - nearby).abs() < 1.0e-3,
            "origin={origin}, nearby={nearby}"
        );
    }

    #[test]
    fn calibrate_heston_round_trip() {
        // Generate synthetic market data from known Heston params
        let true_params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let spot = 100.0;
        let r = 0.05;
        let q = 0.0;

        let expiries = [0.5, 1.0];
        let strikes_1: Vec<f64> = vec![90.0, 95.0, 100.0, 105.0, 110.0];
        let strikes_2: Vec<f64> = vec![85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0];

        // Compute synthetic implied vols via Heston price → BS inversion
        let make_vols = |ks: &[f64], t: f64| -> Vec<f64> {
            let fwd = spot * ((r - q) * t).exp();
            let df = (-r * t).exp();
            ks.iter()
                .map(|&k| {
                    let price = true_params.price_european(spot, k, r, q, t, true);
                    crate::math::volatility::implied_vol_black(price / df, fwd, k, t, true)
                        .unwrap_or(0.2)
                })
                .collect()
        };

        let vols_1 = make_vols(&strikes_1, 0.5);
        let vols_2 = make_vols(&strikes_2, 1.0);

        let strikes: Vec<&[f64]> = vec![&strikes_1, &strikes_2];
        let market_vols: Vec<&[f64]> = vec![&vols_1, &vols_2];

        let result = calibrate_heston(spot, r, q, &expiries, &strikes, &market_vols)
            .expect("should succeed");

        // RMSE should be small (prices are exact from the model)
        assert!(
            result.rmse < 0.02,
            "RMSE should be small: {:.4}",
            result.rmse
        );
        // Recovered parameters should be in the right ballpark
        assert!(
            (result.params.v0 - true_params.v0).abs() < 0.02,
            "v0 mismatch: {:.4} vs {:.4}",
            result.params.v0,
            true_params.v0
        );
        assert!(
            result.params.rho < 0.0,
            "rho should be negative: {:.4}",
            result.params.rho
        );
    }

    #[test]
    fn calibrate_heston_rejects_insufficient_data() {
        let strikes_only = [100.0, 105.0];
        let vols_only = [0.2, 0.21];
        let result = calibrate_heston(
            100.0,
            0.05,
            0.0,
            &[1.0],
            &[strikes_only.as_slice()],
            &[vols_only.as_slice()],
        );
        assert!(result.is_err(), "Should reject < 5 data points");
    }

    #[test]
    fn calibrate_heston_round_trip_with_nonzero_rates_and_dividends() {
        let true_params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let spot = 100.0;
        let r = 0.05;
        let q = 0.02;

        let expiries = [0.5, 1.0];
        let strikes_1: Vec<f64> = vec![90.0, 95.0, 100.0, 105.0, 110.0];
        let strikes_2: Vec<f64> = vec![85.0, 90.0, 95.0, 100.0, 105.0, 110.0, 115.0];

        let make_vols = |ks: &[f64], t: f64| -> Vec<f64> {
            let fwd = spot * ((r - q) * t).exp();
            let df = (-r * t).exp();
            ks.iter()
                .map(|&k| {
                    let discounted_price = true_params.price_european(spot, k, r, q, t, true);
                    crate::math::volatility::implied_vol_black(
                        discounted_price / df,
                        fwd,
                        k,
                        t,
                        true,
                    )
                    .unwrap_or(0.2)
                })
                .collect()
        };

        let vols_1 = make_vols(&strikes_1, 0.5);
        let vols_2 = make_vols(&strikes_2, 1.0);

        let strikes: Vec<&[f64]> = vec![&strikes_1, &strikes_2];
        let market_vols: Vec<&[f64]> = vec![&vols_1, &vols_2];

        let result = calibrate_heston(spot, r, q, &expiries, &strikes, &market_vols)
            .expect("should succeed");

        assert!(
            result.rmse < 0.02,
            "RMSE should stay small: {:.4}",
            result.rmse
        );
        assert!(
            (result.params.v0 - true_params.v0).abs() < 0.02,
            "v0 mismatch: {:.4} vs {:.4}",
            result.params.v0,
            true_params.v0
        );
        assert!(
            (result.params.theta - true_params.theta).abs() < 0.02,
            "theta mismatch: {:.4} vs {:.4}",
            result.params.theta,
            true_params.theta
        );
    }

    #[test]
    fn price_european_strip_matches_single_strike_prices() {
        let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.5).expect("valid");
        let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];

        let strip_prices = params.price_european_strip(100.0, &strikes, 0.05, 0.02, 1.0, true);

        assert_eq!(strip_prices.len(), strikes.len());
        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = params.price_european(100.0, strike, 0.05, 0.02, 1.0, true);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-5,
                "strip price {} should match single-strike price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }

    #[test]
    fn price_european_strip_put_call_parity_holds_per_strike() {
        let params = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7).expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.02;
        let t: f64 = 1.0;
        let strikes = [85.0, 95.0, 100.0, 105.0, 115.0];

        let calls = params.price_european_strip(spot, &strikes, r, q, t, true);
        let puts = params.price_european_strip(spot, &strikes, r, q, t, false);

        for ((&strike, &call), &put) in strikes.iter().zip(calls.iter()).zip(puts.iter()) {
            let parity = call - put - (spot * (-q * t).exp() - strike * (-r * t).exp());
            assert!(
                parity.abs() < 1e-12,
                "put-call parity should hold for K={strike}: residual={parity}"
            );
        }
    }

    #[test]
    fn price_european_strip_matches_single_strike_for_short_dated_params() {
        let params = HestonParams::new(0.01, 3.0, 0.01, 0.02, -0.5).expect("valid");
        let spot = 100.0;
        let r = 0.01;
        let q = 0.0;
        let t = 0.005;
        let strikes = [95.0, 100.0, 105.0];

        let strip_prices = params.price_european_strip(spot, &strikes, r, q, t, true);

        for (idx, &strike) in strikes.iter().enumerate() {
            let single_price = params.price_european(spot, strike, r, q, t, true);
            assert!(
                (strip_prices[idx] - single_price).abs() < 1e-5,
                "strip price {} should match refined single-strike price {} for K={}",
                strip_prices[idx],
                single_price,
                strike
            );
        }
    }
}
