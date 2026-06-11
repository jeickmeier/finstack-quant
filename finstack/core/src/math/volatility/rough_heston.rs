//! Rough Heston stochastic volatility model via Fourier pricing.
//!
//! Implements the rough Heston model of El Euch & Rosenbaum (2019) for
//! European option pricing using the characteristic function obtained by
//! solving a fractional Riccati equation with the Adams predictor-corrector
//! method of Diethelm, Ford & Freed (2004).
//!
//! # Mathematical Foundation
//!
//! The rough Heston model replaces the classical Heston mean-reversion SDE
//! for instantaneous variance with a fractional Volterra equation:
//!
//! ```text
//! dS(t) = (r − q) S(t) dt + √v(t) S(t) dW₁(t)
//!
//! v(t) = v₀ + (1/Γ(α)) ∫₀ᵗ (t−s)^{α−1} κ(θ − v(s)) ds
//!        + (1/Γ(α)) ∫₀ᵗ (t−s)^{α−1} σ √v(s) dW₂(s)
//!
//! where α = H + 0.5 and H ∈ (0, 0.5) is the Hurst exponent.
//! ```
//!
//! The characteristic function φ(u, T) = E[e^{iu ln(S_T/S_0)}] is:
//!
//! ```text
//! φ(u, T) = exp(iu(r−q)T + C(u,T) + I^{1−α}D(u,T) · v₀)
//! ```
//!
//! where D(u, t) solves the fractional Riccati ODE, C(u, T) integrates
//! κθ · D over the trajectory, and I^{1−α} is the Riemann-Liouville
//! fractional integral of order 1−α (El Euch & Rosenbaum 2019, Thm 4.1).
//!
//! European option prices are computed via the Lewis (2000) single-integral
//! formula:
//!
//! ```text
//! Call = S e^{−qT} − (K e^{−rT} / π) ∫₀^∞ Re[e^{i(u−i/2)x} ψ(u − i/2) / (u² + 1/4)] du
//! ```
//!
//! where x = ln(F/K), F = S e^{(r−q)T}, and ψ is the characteristic function
//! of the demeaned log return ln(S_T/F).
//!
//! # References
//!
//! - El Euch, O. & Rosenbaum, M. (2019). "The characteristic function of rough
//!   Heston models." *Mathematical Finance*, 29(1), 3–38.
//! - Diethelm, K., Ford, N. J. & Freed, A. D. (2004). "Detailed error analysis
//!   for a fractional Adams method." *Numerical Algorithms*, 36(1), 31–52.
//! - Lewis, A. L. (2000). *Option Valuation under Stochastic Volatility*.
//!   Finance Press.
//! - Gatheral, J., Jaisson, T. & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949.

use num_complex::Complex64;
use std::f64::consts::PI;

use crate::math::special_functions::ln_gamma;

/// Maximum real part of exponents to avoid overflow in `exp()`.
const EXPONENT_REAL_LIMIT: f64 = 700.0;

// ---------------------------------------------------------------------------
// FractionalRiccatiSolver
// ---------------------------------------------------------------------------

/// Solves the fractional Riccati ODE via the Adams predictor-corrector method.
///
/// Uses the fractional Adams-Bashforth-Moulton scheme (Diethelm et al. 2004)
/// on a uniform time grid. The fractional ODE for D(u, t):
///
/// ```text
/// D^α_t D(t) = F(D(t))
/// F(x) = −½(u² + iu) + (iuρσ − κ)x + ½σ²x²
/// ```
///
/// is reformulated as a Volterra integral equation and solved with product
/// integration weights.
pub struct FractionalRiccatiSolver {
    /// Fractional index α = H + 0.5.
    alpha: f64,
    /// Number of time discretization steps.
    num_steps: usize,
    /// Uniform step size h = T / num_steps.
    step_size: f64,
}

impl FractionalRiccatiSolver {
    /// Create a new solver for given Hurst exponent, maturity, and step count.
    ///
    /// # Arguments
    ///
    /// * `hurst` - Hurst exponent H ∈ (0, 0.5)
    /// * `maturity` - Time to expiry T > 0
    /// * `num_steps` - Number of time steps (more steps = higher accuracy)
    pub fn new(hurst: f64, maturity: f64, num_steps: usize) -> Self {
        let alpha = hurst + 0.5;
        let step_size = maturity / num_steps as f64;
        Self {
            alpha,
            num_steps,
            step_size,
        }
    }

    /// Solve D(u, t_j) for all time grid points j = 0, ..., num_steps.
    ///
    /// Returns a vector of length `num_steps + 1` with D(u, 0) = 0.
    ///
    /// The Riccati function is F(D) = a + b·D + c·D² where:
    /// - a = −½(u² + iu)
    /// - b = iuρσ − κ
    /// - c = ½σ²
    pub fn solve_d(&self, u: Complex64, kappa: f64, sigma: f64, rho: f64) -> Vec<Complex64> {
        let n = self.num_steps;
        let h = self.step_size;
        let alpha = self.alpha;

        // Riccati coefficients: F(D) = a + b*D + c*D^2
        // El Euch & Rosenbaum (2019): F(u, x) = −½u(u + i) + (iuρσ − κ)x + ½σ²x²
        //   = −½(u² + iu) + (iuρσ − κ)x + ½σ²x²
        // The constant term satisfies a(−i) = 0, which makes D(−i, ·) ≡ 0 and
        // enforces the martingale condition ψ(−i) = 1 (review finding B1).
        let iu = Complex64::i() * u;
        let a = -0.5 * (u * u + iu);
        let b = iu * rho * sigma - kappa;
        let c = Complex64::new(0.5 * sigma * sigma, 0.0);

        let f = |d: Complex64| -> Complex64 { a + b * d + c * d * d };

        let mut d = vec![Complex64::new(0.0, 0.0); n + 1];
        let mut f_vals = vec![Complex64::new(0.0, 0.0); n + 1];
        f_vals[0] = f(d[0]); // f(D(0)) = f(0) = a

        let h_alpha = h.powf(alpha);
        let gamma_alpha_p1 = ln_gamma(alpha + 1.0).exp(); // Γ(α+1)
        let gamma_alpha_p2 = ln_gamma(alpha + 2.0).exp(); // Γ(α+2)

        for step in 0..n {
            // Predictor (fractional Adams-Bashforth)
            // y^P_{n+1} = y_0 + (h^α / Γ(α+1)) * Σ_{j=0}^{n} b_{j} * f(y_j)
            // where b_j = (n+1-j)^α - (n-j)^α
            let mut predictor = Complex64::new(0.0, 0.0);
            for (j, f_j) in f_vals[..=step].iter().enumerate() {
                let b_weight =
                    ((step + 1 - j) as f64).powf(alpha) - ((step - j) as f64).powf(alpha);
                predictor += Complex64::new(b_weight, 0.0) * f_j;
            }
            let d_pred = predictor * h_alpha / gamma_alpha_p1;

            // Corrector (fractional Adams-Moulton, single iteration)
            // y_{n+1} = y_0 + (h^α / Γ(α+2)) * [Σ_{j=0}^{n} a_j * f(y_j) + f(y^P)]
            // Corrector weights a_{j,n+1}:
            //   j=0: n^{α+1} - (n-α)(n+1)^α
            //   1 ≤ j ≤ n: (n-j+2)^{α+1} + (n-j)^{α+1} - 2(n-j+1)^{α+1}
            //   j=n+1: 1 (the predictor term)
            let mut corrector = Complex64::new(0.0, 0.0);
            for (j, f_j) in f_vals[..=step].iter().enumerate() {
                let a_weight = if j == 0 {
                    (step as f64).powf(alpha + 1.0)
                        - ((step as f64) - alpha) * ((step + 1) as f64).powf(alpha)
                } else {
                    ((step - j + 2) as f64).powf(alpha + 1.0)
                        + ((step - j) as f64).powf(alpha + 1.0)
                        - 2.0 * ((step - j + 1) as f64).powf(alpha + 1.0)
                };
                corrector += Complex64::new(a_weight, 0.0) * f_j;
            }
            // Add the predictor contribution (j = step+1 term, weight = 1)
            corrector += f(d_pred);

            d[step + 1] = corrector * h_alpha / gamma_alpha_p2;
            f_vals[step + 1] = f(d[step + 1]);
        }

        d
    }

    /// Compute C(u, T) = κθ · ∫₀ᵀ D(u, s) ds via the trapezoidal rule.
    ///
    /// # Arguments
    ///
    /// * `d_trajectory` - D values at each grid point (from [`solve_d`](Self::solve_d))
    /// * `kappa` - Mean reversion speed
    /// * `theta` - Long-run variance
    pub fn solve_c(&self, d_trajectory: &[Complex64], kappa: f64, theta: f64) -> Complex64 {
        let h = self.step_size;
        let kappa_theta = Complex64::new(kappa * theta, 0.0);

        let mut integral = Complex64::new(0.0, 0.0);
        for j in 0..d_trajectory.len().saturating_sub(1) {
            integral += (d_trajectory[j] + d_trajectory[j + 1]) * 0.5 * h;
        }

        kappa_theta * integral
    }

    /// Compute the Riemann-Liouville fractional integral `I^{1−α}D(T)`:
    ///
    /// ```text
    /// I^{1−α}D(T) = (1/Γ(1−α)) ∫₀ᵀ (T−s)^{−α} D(s) ds
    /// ```
    ///
    /// This is the coefficient of v₀ in the rough Heston characteristic
    /// function (El Euch & Rosenbaum 2019, Thm 4.1). Using `D(T)` instead is
    /// correct only at α = 1 (classical Heston); see review finding M7.
    ///
    /// The integral is evaluated by product integration: `D` is taken
    /// piecewise-linear on the solver grid and the kernel moments
    /// `∫ τ^{−α} dτ` and `∫ τ^{1−α} dτ` are integrated exactly per segment,
    /// which handles the integrable endpoint singularity at s = T.
    ///
    /// # Arguments
    ///
    /// * `d_trajectory` - D values at each grid point (from [`solve_d`](Self::solve_d))
    pub fn fractional_integral_d(&self, d_trajectory: &[Complex64]) -> Complex64 {
        let n = d_trajectory.len().saturating_sub(1);
        if n == 0 {
            return Complex64::new(0.0, 0.0);
        }
        let h = self.step_size;
        let alpha = self.alpha;
        // 1−α ∈ (0, ½) for H ∈ (0, ½), so both exponents below are positive
        // and the kernel τ^{−α} is integrable.
        let e0 = 1.0 - alpha;
        let e1 = 2.0 - alpha;
        let inv_gamma = 1.0 / ln_gamma(e0).exp();

        // Segment j spans s ∈ [t_j, t_{j+1}]; substitute τ = T − s so that
        // τ ∈ [(m−1)h, mh] with m = n − j. With D piecewise-linear,
        //   ∫ τ^{−α} D(s) ds = D_j·(M0 − c1) + D_{j+1}·c1
        // where M0 = ∫ τ^{−α} dτ, M1 = ∫ τ^{1−α} dτ, c1 = m·M0 − M1/h.
        let mut integral = Complex64::new(0.0, 0.0);
        for (j, pair) in d_trajectory.windows(2).enumerate() {
            let m = (n - j) as f64;
            let hi = m * h;
            let lo = (m - 1.0) * h;
            let m0 = (hi.powf(e0) - lo.powf(e0)) / e0;
            let m1 = (hi.powf(e1) - lo.powf(e1)) / e1;
            let c1 = m * m0 - m1 / h;
            integral += pair[0] * (m0 - c1) + pair[1] * c1;
        }

        integral * inv_gamma
    }
}

// ---------------------------------------------------------------------------
// RoughHestonFourierParams
// ---------------------------------------------------------------------------

/// Rough Heston model parameters for Fourier-based European option pricing.
///
/// # Parameters
///
/// | Parameter | Symbol | Range | Market Role |
/// |-----------|--------|-------|-------------|
/// | v0 | v₀ | > 0 | Initial variance |
/// | kappa | κ | > 0 | Mean reversion speed |
/// | theta | θ | > 0 | Long-run variance |
/// | sigma | σ | > 0 | Vol-of-vol |
/// | rho | ρ | (−1, 1) | Spot-vol correlation |
/// | hurst | H | (0, 0.5) | Roughness (Hurst exponent) |
///
/// # Examples
///
/// ```rust
/// use finstack_core::math::volatility::rough_heston::RoughHestonFourierParams;
///
/// let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).unwrap();
/// let call = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
/// assert!(call > 0.0 && call < 100.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawRoughHestonFourierParams")]
pub struct RoughHestonFourierParams {
    /// Initial variance (v₀ > 0).
    pub v0: f64,
    /// Mean reversion speed (κ > 0).
    pub kappa: f64,
    /// Long-run variance (θ > 0).
    pub theta: f64,
    /// Vol-of-vol (σ > 0).
    pub sigma: f64,
    /// Spot-vol correlation (−1 < ρ < 1).
    ///
    /// Strict inequality is required for the Fourier pricer because ρ = ±1
    /// makes the correlation matrix singular, causing numerical instability
    /// in the characteristic function. The MC process `RoughHestonParams` in
    /// `finstack_monte_carlo::process::rough_heston` allows ρ ∈ \[−1, 1\]
    /// because QE-style schemes can handle degenerate correlation.
    pub rho: f64,
    /// Hurst exponent (0 < H < 0.5 for rough regime).
    pub hurst: f64,
}

/// Raw deserialization state of [`RoughHestonFourierParams`].
///
/// Mirrors the serialized field layout exactly so the wire format is
/// unchanged; conversion runs [`RoughHestonFourierParams::new`] validation
/// and rejects unknown fields.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoughHestonFourierParams {
    /// Initial variance.
    v0: f64,
    /// Mean reversion speed.
    kappa: f64,
    /// Long-run variance.
    theta: f64,
    /// Vol-of-vol.
    sigma: f64,
    /// Spot-vol correlation.
    rho: f64,
    /// Hurst exponent.
    hurst: f64,
}

impl TryFrom<RawRoughHestonFourierParams> for RoughHestonFourierParams {
    type Error = crate::Error;

    fn try_from(raw: RawRoughHestonFourierParams) -> crate::Result<Self> {
        Self::new(raw.v0, raw.kappa, raw.theta, raw.sigma, raw.rho, raw.hurst)
    }
}

/// Default number of time steps for the fractional Riccati solver.
const DEFAULT_RICCATI_STEPS: usize = 200;

/// Default upper integration limit for Fourier inversion.
const DEFAULT_UPPER_LIMIT: f64 = 200.0;

/// Number of Gauss-Legendre panels for Fourier integration.
const GL_PANELS: usize = 16;

/// Gauss-Legendre quadrature order per panel.
const GL_ORDER: usize = 16;

impl RoughHestonFourierParams {
    /// Construct validated rough Heston parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `v0 <= 0` or non-finite
    /// - `kappa <= 0` or non-finite
    /// - `theta <= 0` or non-finite
    /// - `sigma <= 0` or non-finite
    /// - `rho` not in `(-1, 1)` or non-finite
    /// - `hurst` not in `(0, 0.5)` or non-finite
    pub fn new(
        v0: f64,
        kappa: f64,
        theta: f64,
        sigma: f64,
        rho: f64,
        hurst: f64,
    ) -> crate::Result<Self> {
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston v0 (initial variance) must be positive, got {v0}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston theta (long-run variance) must be positive, got {theta}"
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston sigma (vol-of-vol) must be positive, got {sigma}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston rho (correlation) must be in (-1, 1), got {rho}"
            )));
        }
        if hurst <= 0.0 || hurst >= 0.5 || !hurst.is_finite() {
            return Err(crate::Error::Validation(format!(
                "RoughHeston hurst must be in (0, 0.5), got {hurst}"
            )));
        }

        Ok(Self {
            v0,
            kappa,
            theta,
            sigma,
            rho,
            hurst,
        })
    }

    /// Compute the risk-neutral characteristic function φ(u, T).
    ///
    /// Returns E[exp(iu · ln(S_T / S_0))] under the risk-neutral measure:
    ///
    /// ```text
    /// φ(u, T) = exp(iu(r−q)T + C(u,T) + I^{1−α}D(u,T) · v₀)
    /// ```
    ///
    /// The v₀ coefficient is the fractional integral `I^{1−α}D(T)`, not
    /// `D(T)` (El Euch & Rosenbaum 2019, Thm 4.1; review finding M7).
    ///
    /// # Arguments
    ///
    /// * `u` - Fourier frequency (complex)
    /// * `r` - Risk-free rate
    /// * `q` - Dividend yield
    /// * `t` - Time to expiry
    pub fn char_func(&self, u: Complex64, r: f64, q: f64, t: f64) -> Complex64 {
        let solver = FractionalRiccatiSolver::new(self.hurst, t, DEFAULT_RICCATI_STEPS);
        let d_traj = solver.solve_d(u, self.kappa, self.sigma, self.rho);
        let c_val = solver.solve_c(&d_traj, self.kappa, self.theta);
        let i_d_val = solver.fractional_integral_d(&d_traj);

        let exponent = Complex64::i() * u * (r - q) * t + c_val + i_d_val * self.v0;

        if !exponent.is_finite() || exponent.re > EXPONENT_REAL_LIMIT {
            return Complex64::new(0.0, 0.0);
        }

        let result = exponent.exp();
        if result.is_finite() {
            result
        } else {
            Complex64::new(0.0, 0.0)
        }
    }

    /// Price a European option using the Lewis (2000) single-integral formula.
    ///
    /// Uses the demeaned characteristic function ψ(u) of X = ln(S_T/F):
    ///
    /// ```text
    /// Call = S e^{−qT} − (K e^{−rT} / π) ∫₀^∞ Re[e^{iwx} ψ(w)] / (u²+¼) du
    /// ```
    ///
    /// where x = ln(F/K), F = S e^{(r−q)T}, w = u − i/2, and
    /// ψ(w) = exp(C(w) + I^{1−α}D(w)·v₀). The contour phase is e^{iwx}
    /// = e^{x/2}·e^{iux} — the e^{x/2} factor comes from evaluating on the
    /// Lewis contour Im(w) = −½ (review finding B1). The drift terms cancel
    /// analytically. Puts use put-call parity.
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
    /// Option price. The value is not clamped at zero: tiny negative values
    /// can arise from quadrature noise on far-OTM options, and materially
    /// negative values indicate a pricing failure that must stay visible.
    ///
    /// # References
    ///
    /// - Lewis, A. L. (2001). "A Simple Option Formula for General Jump-Diffusion
    ///   and Other Exponential Lévy Processes."
    /// - Cui, Y., Del Baño Rollin, S. & Germano, G. (2017). "Full and fast
    ///   calibration of the Heston stochastic volatility model." *European Journal
    ///   of Operational Research*, 263(2), 625–638.
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
        // Degenerate / invalid inputs
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

        let forward = spot * ((r - q) * t).exp();
        let x = (forward / strike).ln(); // log-forward-moneyness ln(F/K)

        let solver = FractionalRiccatiSolver::new(self.hurst, t, DEFAULT_RICCATI_STEPS);

        // Lewis integrand at quadrature point u:
        //   Re[e^{iwx} · ψ(w)] / (u² + 1/4),  w = u − i/2
        //
        // where ψ(w) = exp(C(w) + I^{1−α}D(w)·v₀). The phase i·w·x carries
        // the real contour factor e^{x/2} (B1); the v₀ coefficient is the
        // fractional integral I^{1−α}D, not D(T) (M7). The risk-neutral
        // drift cancels analytically when converting from φ (char func of
        // log-return) to ψ (char func of demeaned log-return).
        let integrand = |u_real: f64| -> f64 {
            let w = Complex64::new(u_real, -0.5);
            let d_traj = solver.solve_d(w, self.kappa, self.sigma, self.rho);
            let c_val = solver.solve_c(&d_traj, self.kappa, self.theta);
            let i_d_val = solver.fractional_integral_d(&d_traj);

            // i·w·x = i·(u − i/2)·x = x/2 + i·u·x
            let exponent = Complex64::new(0.5 * x, u_real * x) + c_val + i_d_val * self.v0;
            if !exponent.is_finite() || exponent.re > EXPONENT_REAL_LIMIT {
                return 0.0;
            }

            let denom = u_real * u_real + 0.25;
            let val = (exponent.exp() / denom).re;
            if val.is_finite() {
                val
            } else {
                0.0
            }
        };

        let integral = crate::math::integration::gauss_legendre_integrate_composite(
            integrand,
            1e-8,
            DEFAULT_UPPER_LIMIT,
            GL_ORDER,
            GL_PANELS,
        )
        .unwrap_or(0.0);

        // No `.max(0.0)` clamp: a materially negative value signals a pricer
        // defect and must stay visible rather than be silently truncated
        // (the old clamp returned exactly 0 for deep-OTM calls; B1). Small
        // negative quadrature noise is possible for far-OTM options.
        let call = spot * (-q * t).exp() - strike * (-r * t).exp() * integral / PI;

        if is_call {
            call
        } else {
            // Put-call parity: P = C - S·e^{-qT} + K·e^{-rT}
            call - spot * (-q * t).exp() + strike * (-r * t).exp()
        }
    }

    /// Extract the Black-76 implied volatility from the rough Heston price.
    ///
    /// Returns `None` if the price cannot be inverted (e.g., deep OTM with
    /// near-zero premium).
    ///
    /// # Arguments
    ///
    /// * `spot` - Current spot price
    /// * `strike` - Strike price
    /// * `r` - Risk-free rate
    /// * `q` - Dividend yield
    /// * `t` - Time to expiry
    /// * `is_call` - `true` for call, `false` for put
    pub fn implied_vol(
        &self,
        spot: f64,
        strike: f64,
        r: f64,
        q: f64,
        t: f64,
        is_call: bool,
    ) -> Option<f64> {
        let price = self.price_european(spot, strike, r, q, t, is_call);
        if !price.is_finite() || price <= 0.0 {
            return None;
        }
        let forward = spot * ((r - q) * t).exp();
        let df = (-r * t).exp();
        // implied_vol_black expects undiscounted price (forward measure)
        let undiscounted = price / df;
        crate::math::volatility::implied_vol_black(undiscounted, forward, strike, t, is_call).ok()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Parameter validation
    // -----------------------------------------------------------------------

    #[test]
    fn valid_params() {
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).is_ok());
    }

    #[test]
    fn rejects_invalid_v0() {
        assert!(RoughHestonFourierParams::new(0.0, 2.0, 0.04, 0.3, -0.7, 0.1).is_err());
        assert!(RoughHestonFourierParams::new(-0.01, 2.0, 0.04, 0.3, -0.7, 0.1).is_err());
        assert!(RoughHestonFourierParams::new(f64::NAN, 2.0, 0.04, 0.3, -0.7, 0.1).is_err());
    }

    #[test]
    fn rejects_invalid_kappa() {
        assert!(RoughHestonFourierParams::new(0.04, 0.0, 0.04, 0.3, -0.7, 0.1).is_err());
        assert!(RoughHestonFourierParams::new(0.04, -1.0, 0.04, 0.3, -0.7, 0.1).is_err());
    }

    #[test]
    fn rejects_invalid_theta() {
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.0, 0.3, -0.7, 0.1).is_err());
    }

    #[test]
    fn rejects_invalid_sigma() {
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.0, -0.7, 0.1).is_err());
    }

    #[test]
    fn rejects_invalid_rho() {
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -1.0, 0.1).is_err());
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, 1.0, 0.1).is_err());
    }

    #[test]
    fn rejects_invalid_hurst() {
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.0).is_err());
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.5).is_err());
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, -0.1).is_err());
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.6).is_err());
        assert!(RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, f64::NAN).is_err());
    }

    #[test]
    fn rough_heston_params_serde_validates_on_deserialize() {
        // Valid JSON round-trips.
        let p = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let json = serde_json::to_string(&p).expect("serialize");
        let back: RoughHestonFourierParams = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(p, back);

        // Out-of-range rho rejected.
        let bad_rho = r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma":0.3,"rho":1.5,"hurst":0.1}"#;
        assert!(serde_json::from_str::<RoughHestonFourierParams>(bad_rho).is_err());

        // Out-of-range hurst rejected (H must be in (0, 0.5)).
        let bad_hurst =
            r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma":0.3,"rho":-0.7,"hurst":0.7}"#;
        assert!(serde_json::from_str::<RoughHestonFourierParams>(bad_hurst).is_err());

        // Unknown field rejected.
        let unknown =
            r#"{"v0":0.04,"kappa":2.0,"theta":0.04,"sigma":0.3,"rho":-0.7,"hurst":0.1,"x":1}"#;
        assert!(serde_json::from_str::<RoughHestonFourierParams>(unknown).is_err());
    }

    // -----------------------------------------------------------------------
    // Fractional Riccati solver
    // -----------------------------------------------------------------------

    #[test]
    fn riccati_initial_condition() {
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, 100);
        let u = Complex64::new(1.0, 0.0);
        let d = solver.solve_d(u, 2.0, 0.3, -0.7);
        assert_eq!(d[0], Complex64::new(0.0, 0.0), "D(0) must be zero");
    }

    #[test]
    fn riccati_trajectory_length() {
        let n = 50;
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, n);
        let u = Complex64::new(1.0, 0.0);
        let d = solver.solve_d(u, 2.0, 0.3, -0.7);
        assert_eq!(d.len(), n + 1);
    }

    #[test]
    fn riccati_values_finite() {
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, 200);
        let u = Complex64::new(5.0, 0.0);
        let d = solver.solve_d(u, 2.0, 0.3, -0.7);
        for (j, val) in d.iter().enumerate() {
            assert!(val.is_finite(), "D[{j}] is not finite: {val}");
        }
    }

    #[test]
    fn riccati_c_zero_for_zero_d() {
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, 100);
        let d_zero = vec![Complex64::new(0.0, 0.0); 101];
        let c = solver.solve_c(&d_zero, 2.0, 0.04);
        assert!(c.norm() < 1e-15, "C should be zero for zero D trajectory");
    }

    /// B1 regression: the Riccati constant term must satisfy a(−i) = 0, so
    /// D(−i, ·) ≡ 0 along the whole trajectory. This is the martingale
    /// condition E[S_T/F] = 1; the old sign error gave a(−i) = 1.
    #[test]
    fn riccati_martingale_condition_d_at_minus_i_is_zero() {
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, 200);
        let d = solver.solve_d(Complex64::new(0.0, -1.0), 2.0, 0.3, -0.7);
        for (j, val) in d.iter().enumerate() {
            assert!(
                val.norm() < 1e-12,
                "martingale condition violated: D(−i)[{j}] = {val}"
            );
        }
    }

    /// `I^{1−α}D` reduces to plain trapezoidal-like integration sanity: for a
    /// constant trajectory D ≡ c, I^{1−α}D(T) = c·T^{1−α}/Γ(2−α) exactly.
    #[test]
    fn fractional_integral_constant_trajectory() {
        let hurst = 0.1;
        let t = 1.5;
        let solver = FractionalRiccatiSolver::new(hurst, t, 200);
        let c = Complex64::new(0.7, -0.3);
        let traj = vec![c; 201];

        let alpha = hurst + 0.5;
        let expected = c * t.powf(1.0 - alpha) / ln_gamma(2.0 - alpha).exp();
        let actual = solver.fractional_integral_d(&traj);
        assert!(
            (actual - expected).norm() < 1e-12,
            "I^{{1−α}} of a constant must be exact: got {actual}, expected {expected}"
        );
    }

    // -----------------------------------------------------------------------
    // Characteristic function
    // -----------------------------------------------------------------------

    #[test]
    fn char_func_at_zero() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let phi_0 = params.char_func(Complex64::new(0.0, 0.0), 0.05, 0.0, 1.0);
        // φ(0) = 1 for characteristic functions
        assert!(
            (phi_0.re - 1.0).abs() < 1e-6,
            "φ(0) should be ~1, got {phi_0}"
        );
        assert!(phi_0.im.abs() < 1e-6, "Im(φ(0)) should be ~0, got {phi_0}");
    }

    #[test]
    fn char_func_bounded() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        for u_re in [0.1, 1.0, 5.0, 10.0, 20.0] {
            let phi = params.char_func(Complex64::new(u_re, 0.0), 0.05, 0.0, 1.0);
            // Allow small numerical overshoot from the Adams scheme
            assert!(
                phi.norm() <= 1.0 + 1e-2,
                "|φ({u_re})| should be ≤ 1 (within tolerance), got {:.6}",
                phi.norm()
            );
        }
    }

    /// B1 regression: φ(−i, T) = E[S_T/S_0] = e^{(r−q)T} (martingale
    /// property of the discounted spot). With the correct Riccati constant
    /// term D(−i, ·) ≡ 0 so this holds essentially exactly.
    #[test]
    fn char_func_martingale_property() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let r = 0.05;
        let q = 0.02;
        let t = 1.0;
        let phi = params.char_func(Complex64::new(0.0, -1.0), r, q, t);
        let expected = ((r - q) * t).exp();
        assert!(
            (phi.re - expected).abs() < 1e-10 && phi.im.abs() < 1e-10,
            "φ(−i) should equal e^{{(r−q)T}} = {expected}, got {phi}"
        );
    }

    // -----------------------------------------------------------------------
    // European pricing
    // -----------------------------------------------------------------------

    #[test]
    fn call_price_positive_and_bounded() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let call = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        assert!(call > 0.0, "Call should be positive, got {call}");
        assert!(call < 100.0, "Call should be < spot, got {call}");
    }

    #[test]
    fn put_price_positive_and_bounded() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let put = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, false);
        assert!(put > 0.0, "Put should be positive, got {put}");
        assert!(put < 100.0, "Put should be < strike, got {put}");
    }

    #[test]
    fn put_call_parity() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let s = 100.0;
        let k = 100.0;
        let r = 0.05;
        let q = 0.02;
        let t = 1.0;

        let call = params.price_european(s, k, r, q, t, true);
        let put = params.price_european(s, k, r, q, t, false);

        let lhs = call - put;
        let rhs = s * (-q * t).exp() - k * (-r * t).exp();

        assert!(
            (lhs - rhs).abs() < 0.05,
            "Put-call parity violated: C−P = {lhs:.6}, Se^{{-qT}} − Ke^{{-rT}} = {rhs:.6}"
        );
    }

    #[test]
    fn moneyness_ordering() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let itm = params.price_european(100.0, 90.0, 0.05, 0.0, 1.0, true);
        let atm = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let otm = params.price_european(100.0, 110.0, 0.05, 0.0, 1.0, true);

        assert!(itm > atm, "ITM > ATM: {itm:.4} vs {atm:.4}");
        assert!(atm > otm, "ATM > OTM: {atm:.4} vs {otm:.4}");
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn prices_non_negative() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        for &k in &[80.0, 90.0, 100.0, 110.0, 120.0] {
            let call = params.price_european(100.0, k, 0.05, 0.0, 1.0, true);
            let put = params.price_european(100.0, k, 0.05, 0.0, 1.0, false);
            assert!(call >= 0.0, "Negative call for K={k}: {call}");
            assert!(put >= 0.0, "Negative put for K={k}: {put}");
        }
    }

    #[test]
    fn expired_option() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let itm_call = params.price_european(100.0, 90.0, 0.05, 0.0, 0.0, true);
        assert!(
            (itm_call - 10.0).abs() < 1e-10,
            "Expired ITM call: {itm_call}"
        );
        let otm_call = params.price_european(100.0, 110.0, 0.05, 0.0, 0.0, true);
        assert!(otm_call.abs() < 1e-10, "Expired OTM call: {otm_call}");
    }

    // -----------------------------------------------------------------------
    // Black-Scholes limit golden values (σ → 0, B1/M7 regression)
    // -----------------------------------------------------------------------

    /// With vanishing vol-of-vol and v₀ = θ the variance is deterministic at
    /// θ, so the model degenerates to Black-Scholes at vol √θ. This is a
    /// genuine external reference (no circularity): the pre-fix pricer
    /// returned 34.22 / 12.66 / 0.00 against the true 24.59 / 10.45 / 3.25.
    #[test]
    fn bs_limit_call_prices_across_moneyness() {
        let vol = 0.2;
        let params = RoughHestonFourierParams::new(vol * vol, 2.0, vol * vol, 1e-3, 0.0, 0.1)
            .expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.0;
        let t: f64 = 1.0;
        let forward = spot * ((r - q) * t).exp();
        let df = (-r * t).exp();

        // The 200-step fractional Adams scheme leaves ~0.005 absolute bias
        // (D(t) ~ t^α has a singular derivative at 0); bound both the
        // absolute error (~0.5 bp of spot) and the relative error. The
        // pre-fix pricer was off by 10-40% here.
        for &strike in &[80.0, 100.0, 120.0] {
            let rough = params.price_european(spot, strike, r, q, t, true);
            let bs = df * crate::math::volatility::black_call(forward, strike, vol, t);
            let abs_err = (rough - bs).abs();
            let rel = abs_err / bs;
            assert!(
                abs_err < 1e-2 && rel < 2e-2,
                "σ→0 rough Heston call (K={strike}) must match Black-Scholes: \
                 rough={rough:.6}, bs={bs:.6}, abs={abs_err:.2e}, rel={rel:.2e}"
            );
        }
    }

    /// Direct put pricing against the same Black-Scholes limit — guards the
    /// put leg independently rather than only through put-call parity.
    #[test]
    fn bs_limit_put_prices_across_moneyness() {
        let vol = 0.2;
        let params = RoughHestonFourierParams::new(vol * vol, 2.0, vol * vol, 1e-3, 0.0, 0.1)
            .expect("valid");
        let spot: f64 = 100.0;
        let r: f64 = 0.05;
        let q: f64 = 0.0;
        let t: f64 = 1.0;
        let forward = spot * ((r - q) * t).exp();
        let df = (-r * t).exp();

        for &strike in &[80.0, 100.0, 120.0] {
            let rough = params.price_european(spot, strike, r, q, t, false);
            let bs = df * crate::math::volatility::black_put(forward, strike, vol, t);
            let abs_err = (rough - bs).abs();
            let rel = abs_err / bs;
            assert!(
                abs_err < 1e-2 && rel < 2e-2,
                "σ→0 rough Heston put (K={strike}) must match Black-Scholes: \
                 rough={rough:.6}, bs={bs:.6}, abs={abs_err:.2e}, rel={rel:.2e}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Convergence to standard Heston at H → 0.5
    // -----------------------------------------------------------------------

    #[test]
    fn matches_standard_heston_near_h_half_across_moneyness() {
        // With H close to 0.5 (α → 1) the fractional Riccati reduces to the
        // classical Heston Riccati and I^{1−α}D → D(T), so rough Heston must
        // agree with the classical Heston pricer tightly — across moneyness,
        // not just ATM. The pre-fix suite used a 15% ATM-only tolerance that
        // could not see B1/M7.
        let v0 = 0.04;
        let kappa = 2.0;
        let theta = 0.04;
        let sigma = 0.3;
        let rho = -0.5;

        let rough =
            RoughHestonFourierParams::new(v0, kappa, theta, sigma, rho, 0.499).expect("valid");
        let standard =
            crate::math::volatility::heston::HestonParams::new(v0, kappa, theta, sigma, rho)
                .expect("valid");

        let spot = 100.0;
        let r = 0.05;
        let q = 0.0;
        let t = 1.0;

        for &strike in &[80.0, 90.0, 100.0, 110.0, 120.0] {
            let rough_price = rough.price_european(spot, strike, r, q, t, true);
            let heston_price = standard.price_european(spot, strike, r, q, t, true);
            let rel_diff = (rough_price - heston_price).abs() / heston_price;
            assert!(
                rel_diff < 5e-3,
                "Rough Heston (H≈0.5, K={strike}) must match standard Heston: \
                 rough={rough_price:.6}, heston={heston_price:.6}, rel_diff={rel_diff:.4e}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Price sensitivity
    // -----------------------------------------------------------------------

    #[test]
    fn price_increases_with_vol_of_vol() {
        // Higher sigma generally increases OTM option value
        let base = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.2, -0.7, 0.3).expect("valid");
        let high = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.6, -0.7, 0.3).expect("valid");

        let base_price = base.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        let high_price = high.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);

        // For OTM puts, vol-of-vol effect is clearly visible
        let base_otm = base.price_european(100.0, 80.0, 0.05, 0.0, 1.0, false);
        let high_otm = high.price_european(100.0, 80.0, 0.05, 0.0, 1.0, false);

        assert!(
            high_otm > base_otm,
            "Higher sigma should increase OTM put: base={base_otm:.6}, high={high_otm:.6}"
        );

        // ATM: both should be reasonable prices
        assert!(
            base_price > 0.0,
            "Base ATM call should be positive: {base_price}"
        );
        assert!(
            high_price > 0.0,
            "High ATM call should be positive: {high_price}"
        );
    }

    #[test]
    fn price_increases_with_time() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let short_t = params.price_european(100.0, 100.0, 0.05, 0.0, 0.25, true);
        let long_t = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);

        assert!(
            long_t > short_t,
            "Longer maturity should have higher ATM call: short={short_t:.4}, long={long_t:.4}"
        );
    }

    // -----------------------------------------------------------------------
    // Implied volatility
    // -----------------------------------------------------------------------

    #[test]
    fn implied_vol_produces_valid_result() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let iv = params.implied_vol(100.0, 100.0, 0.05, 0.0, 1.0, true);
        assert!(iv.is_some(), "Should produce valid implied vol");
        let vol = iv.expect("checked above");
        assert!(
            vol > 0.0 && vol < 2.0,
            "Implied vol should be reasonable: {vol}"
        );
    }

    #[test]
    fn implied_vol_round_trip() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let spot = 100.0;
        let strike = 100.0;
        let r = 0.05;
        let q = 0.0;
        let t = 1.0;

        let price = params.price_european(spot, strike, r, q, t, true);
        let iv = params.implied_vol(spot, strike, r, q, t, true);
        assert!(iv.is_some(), "Should produce valid implied vol");

        // Re-price using Black-76 with the implied vol
        let vol = iv.expect("checked above");
        let forward = spot * ((r - q) * t).exp();
        let repriced =
            (-r * t).exp() * crate::math::volatility::black_call(forward, strike, vol, t);

        assert!(
            (repriced - price).abs() < 0.01,
            "Round-trip: original={price:.6}, repriced={repriced:.6}"
        );
    }
}
