//! Rough Heston stochastic volatility model via Fourier pricing.
//!
//! Implements the rough Heston model of El Euch & Rosenbaum (2019) for
//! European option pricing using the characteristic function obtained by
//! solving a fractional Riccati equation with the implicit fractional Adams-Moulton
//! method, using step refinement to check numerical convergence.
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
//!   Heston models." *Mathematical Finance*, 29(1), 3–38. `docs/REFERENCES.md#el-euch-rosenbaum-2019`
//! - Diethelm, K., Ford, N. J. & Freed, A. D. (2004). "Detailed error analysis
//!   for a fractional Adams method." *Numerical Algorithms*, 36(1), 31–52.
//! - Lewis, A. L. (2000). *Option Valuation under Stochastic Volatility*.
//!   Finance Press.
//! - Gatheral, J., Jaisson, T. & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949. `docs/REFERENCES.md#gatheral-jaisson-rosenbaum-2018`

use num_complex::Complex64;
use std::f64::consts::PI;

use finstack_quant_core::math::special_functions::ln_gamma;

/// Maximum real part of exponents to avoid overflow in `exp()`.
const EXPONENT_REAL_LIMIT: f64 = 700.0;

/// Solves the fractional Riccati ODE with implicit fractional Adams-Moulton steps.
///
/// Product integration on a uniform time grid gives a quadratic equation for
/// each new value. Solving that equation, rather than applying one explicit
/// predictor correction, prevents the mean-reversion stiffness from causing
/// the divergent trajectories of the explicit scheme. The fractional ODE for D(u, t):
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
    /// Product-integration weights for prior values, indexed by lag.
    history_weights: Vec<f64>,
    /// Special product-integration weight on the initial value at each step.
    initial_weights: Vec<f64>,
    /// Coefficient of the implicit endpoint contribution.
    endpoint_weight: f64,
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
        let endpoint_weight = step_size.powf(alpha) / ln_gamma(alpha + 2.0).exp();
        let history_weights = (0..num_steps)
            .map(|lag| {
                let lag = lag as f64;
                ((lag + 2.0).powf(alpha + 1.0) + lag.powf(alpha + 1.0)
                    - 2.0 * (lag + 1.0).powf(alpha + 1.0))
                    * endpoint_weight
            })
            .collect();
        let initial_weights = (0..num_steps)
            .map(|step| {
                let step = step as f64;
                (step.powf(alpha + 1.0) - (step - alpha) * (step + 1.0).powf(alpha))
                    * endpoint_weight
            })
            .collect();
        Self {
            alpha,
            num_steps,
            step_size,
            history_weights,
            initial_weights,
            endpoint_weight,
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
    ///
    /// # Arguments
    ///
    /// * `u` - U used by the algorithm, subject to the enclosing type invariants and documented units.
    /// * `kappa` - Mean-reversion speed of the stochastic volatility or short-rate factor
    /// * `sigma` - Diffusion volatility of the process in decimal annual units
    /// * `rho` - Instantaneous correlation between Brownian drivers, in `[-1, 1]`
    pub fn solve_d(&self, u: Complex64, kappa: f64, sigma: f64, rho: f64) -> Vec<Complex64> {
        let n = self.num_steps;

        // Riccati coefficients: F(D) = a + b*D + c*D^2
        // El Euch & Rosenbaum (2019): F(u, x) = −½u(u + i) + (iuρσ − κ)x + ½σ²x²
        //   = −½(u² + iu) + (iuρσ − κ)x + ½σ²x²
        // The constant term satisfies a(−i) = 0, which makes D(−i, ·) ≡ 0 and
        // enforces the martingale condition ψ(−i) = 1 .
        let iu = Complex64::i() * u;
        let a = -0.5 * (u * u + iu);
        let b = iu * rho * sigma - kappa;
        let c = Complex64::new(0.5 * sigma * sigma, 0.0);

        let f = |d: Complex64| -> Complex64 { a + b * d + c * d * d };

        let mut d = vec![Complex64::new(0.0, 0.0); n + 1];
        let mut f_vals = vec![Complex64::new(0.0, 0.0); n + 1];
        f_vals[0] = f(d[0]); // f(D(0)) = f(0) = a

        // The endpoint solves x = history + weight * (a + b*x + c*x²).
        // Re(-b) is positive on the pricing contour. The principal square
        // root selects the solution continuous from weight=0. Rationalizing
        // the quadratic formula avoids cancellation as vol-of-vol tends to 0.
        let minus_linear = Complex64::new(1.0, 0.0) - b * self.endpoint_weight;
        let quadratic = c * self.endpoint_weight;
        for step in 0..n {
            let mut history = f_vals[0] * self.initial_weights[step];
            for (j, f_j) in f_vals[1..=step].iter().enumerate() {
                history += f_j * self.history_weights[step - j - 1];
            }
            let constant = history + a * self.endpoint_weight;
            let discriminant = minus_linear * minus_linear - 4.0 * quadratic * constant;
            d[step + 1] = 2.0 * constant / (minus_linear + discriminant.sqrt());
            f_vals[step + 1] = f(d[step + 1]);
            if !d[step + 1].is_finite() || !f_vals[step + 1].is_finite() {
                d[step + 1..].fill(Complex64::new(f64::NAN, f64::NAN));
                break;
            }
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
    /// correct only at α = 1 (classical Heston); see .
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
/// use finstack_quant_models::volatility::rough_heston::RoughHestonFourierParams;
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
    /// `finstack_quant_models::monte_carlo::process::rough_heston` allows ρ ∈ \[−1, 1\]
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
    type Error = finstack_quant_core::Error;

    fn try_from(raw: RawRoughHestonFourierParams) -> finstack_quant_core::Result<Self> {
        Self::new(raw.v0, raw.kappa, raw.theta, raw.sigma, raw.rho, raw.hurst)
    }
}

/// Initial resolution used before checking convergence by step doubling.
const INITIAL_RICCATI_STEPS: usize = 200;
/// Maximum resolution; unresolved calculations return NaN rather than a price.
const MAX_RICCATI_STEPS: usize = 1600;
/// Single-frequency queries can afford one more refinement than an entire
/// pricing integral. At u=10 the ordinary H=0.1 case needs 3,200 steps to
/// meet the same absolute characteristic-function tolerance.
const MAX_CHARACTERISTIC_STEPS: usize = 3200;
/// Absolute tolerance after normalizing the premium by its spot/strike scale.
const PRICE_SCALE_TOLERANCE: f64 = 1e-6;
/// Absolute tolerance for characteristic-function convergence.
const CHARACTERISTIC_TOLERANCE: f64 = 1e-6;

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
    ) -> finstack_quant_core::Result<Self> {
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RoughHeston v0 (initial variance) must be positive, got {v0}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RoughHeston kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RoughHeston theta (long-run variance) must be positive, got {theta}"
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RoughHeston sigma (vol-of-vol) must be positive, got {sigma}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RoughHeston rho (correlation) must be in (-1, 1), got {rho}"
            )));
        }
        if hurst <= 0.0 || hurst >= 0.5 || !hurst.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
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
    /// `D(T)` (El Euch & Rosenbaum 2019, Thm 4.1; .
    ///
    /// # Arguments
    ///
    /// * `u` - Fourier frequency (complex)
    /// * `r` - Risk-free rate
    /// * `q` - Dividend yield
    /// * `t` - Finite non-negative time to expiry in years.
    ///
    /// # Returns
    ///
    /// The characteristic function, or complex NaN when inputs are invalid,
    /// any solver value is non-finite, or step refinement does not converge
    /// within 3,200 steps at absolute tolerance `1e-6`.
    pub fn char_func(&self, u: Complex64, r: f64, q: f64, t: f64) -> Complex64 {
        if !u.is_finite() || !r.is_finite() || !q.is_finite() || !t.is_finite() || t < 0.0 {
            return Complex64::new(f64::NAN, f64::NAN);
        }
        if t == 0.0 {
            return Complex64::new(1.0, 0.0);
        }
        let drift = Complex64::i() * u * (r - q) * t;
        let mut previous: Option<Complex64> = None;
        let mut steps = INITIAL_RICCATI_STEPS;
        while steps <= MAX_CHARACTERISTIC_STEPS {
            let solver = FractionalRiccatiSolver::new(self.hurst, t, steps);
            let value = self
                .demeaned_char_func(&solver, u)
                .map(|value| value * drift.exp());
            if let (Some(current), Some(prior)) = (value, previous) {
                if (current - prior).norm() <= CHARACTERISTIC_TOLERANCE {
                    return current;
                }
            }
            previous = value;
            steps *= 2;
        }
        Complex64::new(f64::NAN, f64::NAN)
    }

    /// Evaluate the demeaned characteristic function without concealing solver failure.
    fn demeaned_char_func(
        &self,
        solver: &FractionalRiccatiSolver,
        u: Complex64,
    ) -> Option<Complex64> {
        let trajectory = solver.solve_d(u, self.kappa, self.sigma, self.rho);
        let exponent = solver.solve_c(&trajectory, self.kappa, self.theta)
            + solver.fractional_integral_d(&trajectory) * self.v0;
        if !exponent.is_finite() || exponent.re > EXPONENT_REAL_LIMIT {
            return None;
        }
        let value = exponent.exp();
        value.is_finite().then_some(value)
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
    /// Lewis contour Im(w) = −½ . The drift terms cancel
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
    /// Option price after convergence of the fractional time discretization.
    /// Returns NaN for invalid inputs, failed Fourier nodes, unresolved time
    /// refinement, or material violations of the call-price bounds. Prices
    /// are not clamped: tiny quadrature noise remains visible. Time refinement
    /// uses a tolerance of `1e-6` times the larger discounted spot/strike and
    /// at most 1,600 steps; Fourier integration uses the fixed documented grid.
    ///
    /// # References
    ///
    /// - Lewis, A. L. (2001). "A Simple Option Formula for General Jump-Diffusion
    ///   and Other Exponential Lévy Processes." `docs/REFERENCES.md#merton-1976-jump`
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

        let spot_pv = spot * (-q * t).exp();
        let strike_pv = strike * (-r * t).exp();
        if !spot_pv.is_finite() || !strike_pv.is_finite() || !x.is_finite() {
            return f64::NAN;
        }
        let price_tolerance = PRICE_SCALE_TOLERANCE * spot_pv.max(strike_pv);
        let lower_bound = (spot_pv - strike_pv).max(0.0);
        let mut previous: Option<f64> = None;
        let mut steps = INITIAL_RICCATI_STEPS;
        while steps <= MAX_RICCATI_STEPS {
            let current = self
                .lewis_integral(t, x, steps)
                .map(|integral| spot_pv - strike_pv * integral / PI);
            if let (Some(call), Some(prior)) = (current, previous) {
                if (call - prior).abs() <= price_tolerance
                    && call >= lower_bound - price_tolerance
                    && call <= spot_pv + price_tolerance
                {
                    return if is_call {
                        call
                    } else {
                        call - spot_pv + strike_pv
                    };
                }
            }
            previous = current;
            steps *= 2;
        }
        // A failed node, lack of convergence, or a material no-arbitrage
        // violation is a pricing failure, never a zero Fourier contribution.
        f64::NAN
    }

    /// Integrate the Lewis contour at one time resolution, propagating failed nodes.
    fn lewis_integral(&self, t: f64, x: f64, steps: usize) -> Option<f64> {
        let solver = FractionalRiccatiSolver::new(self.hurst, t, steps);
        // The Lewis denominator has poles at ±i/2. Resolve the origin on
        // short panels: a uniform 12.5-wide first panel leaves a persistent
        // premium bias even when the Riccati time grid has converged.
        let mut grid =
            finstack_quant_core::math::gauss_legendre_grid(1e-8, 1.0, GL_ORDER, 1).ok()?;
        grid.extend(finstack_quant_core::math::gauss_legendre_grid(1.0, 5.0, GL_ORDER, 1).ok()?);
        grid.extend(
            finstack_quant_core::math::gauss_legendre_grid(
                5.0,
                DEFAULT_UPPER_LIMIT,
                GL_ORDER,
                GL_PANELS,
            )
            .ok()?,
        );
        let mut integral = 0.0;
        for (frequency, weight) in grid {
            let u = Complex64::new(frequency, -0.5);
            let characteristic = self.demeaned_char_func(&solver, u)?;
            let phase = Complex64::new(0.5 * x, frequency * x).exp();
            let value = (phase * characteristic).re / (frequency * frequency + 0.25);
            if !value.is_finite() {
                return None;
            }
            integral += weight * value;
        }
        integral.is_finite().then_some(integral)
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
        crate::volatility::implied_vol_black(undiscounted, forward, strike, t, is_call).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_riccati_is_finite_across_stiff_lewis_contour() {
        let solver = FractionalRiccatiSolver::new(0.1, 10.0, 200);
        for frequency in [0.1, 1.0, 10.0, 50.0, 100.0, 200.0] {
            let trajectory = solver.solve_d(Complex64::new(frequency, -0.5), 10.0, 0.3, -0.7);
            assert!(trajectory.iter().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn stiff_rough_heston_price_matches_refined_native_solver() {
        let params = RoughHestonFourierParams::new(0.04, 10.0, 0.04, 0.3, -0.7, 0.1)
            .expect("valid stiff parameters");
        let price = params.price_european(100.0, 100.0, 0.0, 0.0, 10.0, true);
        let refined = 100.0
            - 100.0
                * params
                    .lewis_integral(10.0, 0.0, 1600)
                    .expect("all refined Fourier nodes converge")
                / PI;
        assert!(price > 20.0 && price < 30.0, "stiff ATM call: {price}");
        assert!(
            (price - refined).abs() < 1e-4,
            "price={price}, refined={refined}"
        );
    }

    #[test]
    fn deterministic_variance_limit_matches_gaussian_characteristic_and_black_price() {
        let params = RoughHestonFourierParams::new(0.04, 10.0, 0.04, 1e-8, 0.0, 0.1)
            .expect("valid deterministic limit");
        let frequency = Complex64::new(1.0, 0.0);
        let actual = params.char_func(frequency, 0.0, 0.0, 10.0);
        let gaussian =
            (-0.5 * 0.04 * 10.0 * (frequency * frequency + Complex64::i() * frequency)).exp();
        assert!(
            (actual - gaussian).norm() < 2e-6,
            "actual={actual}, gaussian={gaussian}"
        );
        let price = params.price_european(100.0, 100.0, 0.0, 0.0, 10.0, true);
        let black = crate::closed_form::black_call(100.0, 100.0, 0.2, 10.0);
        assert!((price - black).abs() < 1e-4, "rough={price}, Black={black}");
    }

    #[test]
    fn failed_rough_heston_nodes_remain_visible() {
        let params = RoughHestonFourierParams {
            sigma: f64::INFINITY,
            ..RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1)
                .expect("valid base parameters")
        };
        assert!(params
            .price_european(100.0, 100.0, 0.0, 0.0, 1.0, true)
            .is_nan());
        assert!(!params
            .char_func(Complex64::new(1.0, 0.0), 0.0, 0.0, 1.0)
            .is_finite());
        assert!(params
            .implied_vol(100.0, 100.0, 0.0, 0.0, 1.0, true)
            .is_none());
    }

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

    /// Regression: the Riccati constant term must satisfy a(−i) = 0, so
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
    fn char_func_real_frequency_converges_to_refined_solution() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1)
            .expect("valid ordinary parameters");
        let frequency = Complex64::new(10.0, 0.0);
        let actual = params.char_func(frequency, 0.05, 0.0, 1.0);
        let solver = FractionalRiccatiSolver::new(0.1, 1.0, 6400);
        let refined = params
            .demeaned_char_func(&solver, frequency)
            .expect("refined characteristic function")
            * (Complex64::i() * frequency * 0.05).exp();
        assert!(actual.is_finite());
        assert!(
            (actual - refined).norm() < CHARACTERISTIC_TOLERANCE,
            "actual={actual}, refined={refined}"
        );
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

    /// Regression: φ(−i, T) = E[S_T/S_0] = e^{(r−q)T} (martingale
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

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn call_price_positive_and_bounded() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let call = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);
        assert!(call > 0.0, "Call should be positive, got {call}");
        assert!(call < 100.0, "Call should be < spot, got {call}");
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn put_price_positive_and_bounded() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let put = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, false);
        assert!(put > 0.0, "Put should be positive, got {put}");
        assert!(put < 100.0, "Put should be < strike, got {put}");
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
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
    #[ignore = "slow: covered by mise rust-test-slow"]
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

    // Black-Scholes limit golden values (σ → 0, B1/M7 regression)

    /// With vanishing vol-of-vol and v₀ = θ the variance is deterministic at
    /// θ, so the model degenerates to Black-Scholes at vol √θ. This is a
    /// genuine external reference (no circularity): the pre-fix pricer
    /// returned 34.22 / 12.66 / 0.00 against the true 24.59 / 10.45 / 3.25.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
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

        // Bound both absolute and relative error in the deterministic-variance
        // limit. The pre-fix pricer was off by 10-40% here; the implicit time
        // solver and origin-resolved Fourier grid also remove its later bias.
        for &strike in &[80.0, 100.0, 120.0] {
            let rough = params.price_european(spot, strike, r, q, t, true);
            let bs = df * crate::closed_form::black_call(forward, strike, vol, t);
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
    #[ignore = "slow: covered by mise rust-test-slow"]
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
            let bs = df * crate::closed_form::black_put(forward, strike, vol, t);
            let abs_err = (rough - bs).abs();
            let rel = abs_err / bs;
            assert!(
                abs_err < 1e-2 && rel < 2e-2,
                "σ→0 rough Heston put (K={strike}) must match Black-Scholes: \
                 rough={rough:.6}, bs={bs:.6}, abs={abs_err:.2e}, rel={rel:.2e}"
            );
        }
    }

    // Convergence to standard Heston at H → 0.5

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
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
        let standard = crate::volatility::heston::HestonParams::new(v0, kappa, theta, sigma, rho)
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

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
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
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn price_increases_with_time() {
        let params = RoughHestonFourierParams::new(0.04, 2.0, 0.04, 0.3, -0.7, 0.1).expect("valid");
        let short_t = params.price_european(100.0, 100.0, 0.05, 0.0, 0.25, true);
        let long_t = params.price_european(100.0, 100.0, 0.05, 0.0, 1.0, true);

        assert!(
            long_t > short_t,
            "Longer maturity should have higher ATM call: short={short_t:.4}, long={long_t:.4}"
        );
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
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
    #[ignore = "slow: covered by mise rust-test-slow"]
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

        let vol = iv.expect("checked above");
        let forward = spot * ((r - q) * t).exp();
        let repriced = (-r * t).exp() * crate::closed_form::black_call(forward, strike, vol, t);

        assert!(
            (repriced - price).abs() < 0.01,
            "Round-trip: original={price:.6}, repriced={repriced:.6}"
        );
    }
}
