//! Merton structural credit model with distance-to-default and default probability.
//!
//! Implements the Merton (1974) model and its Black-Cox (1976) first-passage
//! extension for estimating firm default probability from balance-sheet data.
//!
//! # References
//!
//! - Merton, R. C. (1974). "On the Pricing of Corporate Debt: The Risk
//!   Structure of Interest Rates." *Journal of Finance*, 29(2), 449-470.
//! - Black, F. & Cox, J. C. (1976). "Valuing Corporate Securities: Some
//!   Effects of Bond Indenture Provisions." *Journal of Finance*, 31(2), 351-367.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_valuations::models::credit::MertonModel;
//!
//! let model = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
//! let dd = model.distance_to_default(1.0);
//! let pd = model.default_probability(1.0);
//! let spread = model.implied_spread(5.0, 0.40).unwrap();
//! ```

use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::{Error, InputError, Result};

use finstack_quant_core::math::random::{poisson_inverse_cdf, RandomNumberGenerator};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Asset dynamics specification for the Merton model.
///
/// Controls the stochastic process assumed for the firm's asset value.
#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum AssetDynamics {
    /// Standard geometric Brownian motion (lognormal diffusion).
    GeometricBrownian,
    /// Jump-diffusion process (Merton 1976) with Poisson jumps.
    JumpDiffusion {
        /// Poisson jump arrival intensity (jumps per year).
        jump_intensity: f64,
        /// Mean log-jump size.
        jump_mean: f64,
        /// Volatility of log-jump size.
        jump_vol: f64,
    },
    /// CreditGrades model extension with stochastic-barrier survival adjustment.
    ///
    /// `default_probability()` applies the standard approximate CreditGrades
    /// survival function using the log-barrier volatility parameter.
    CreditGrades {
        /// Log-normal barrier volatility `λ`: the standard deviation of the
        /// natural log of the default barrier (Finger et al. 2002,
        /// "CreditGrades Technical Document"). Despite the field name, this
        /// is *not* a generic uncertainty scalar — it is the lognormal
        /// dispersion of the global recovery rate, entering the survival
        /// formula as `a_t² = σ²t + λ²` and the barrier shift `exp(λ²)`.
        barrier_uncertainty: f64,
        /// Mean recovery rate at default.
        mean_recovery: f64,
    },
}

/// Barrier monitoring type for default determination.
#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum BarrierType {
    /// Default only assessed at maturity (classic Merton).
    Terminal,
    /// Continuous barrier monitoring (Black-Cox extension).
    FirstPassage {
        /// Growth rate of the default barrier over time.
        barrier_growth_rate: f64,
    },
}

/// Approximate CreditGrades survival probability (Finger et al. 2002).
///
/// `barrier_uncertainty` is the log-barrier volatility `λ` — the standard
/// deviation of the natural log of the default barrier, *not* a generic
/// uncertainty scalar. It enters the time-scaled variance as
/// `a_t² = σ²t + λ²` and shifts the effective leverage by `exp(λ²)`.
fn credit_grades_default_probability(
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    barrier_uncertainty: f64,
    horizon: f64,
) -> f64 {
    if horizon <= 0.0 {
        return 0.0;
    }

    // `lambda` is the log-barrier volatility (lognormal barrier std dev).
    let lambda = barrier_uncertainty.max(0.0);
    let a_t = (asset_vol.mul_add(asset_vol, lambda * lambda / horizon) * horizon).sqrt();
    if a_t <= 0.0 {
        return if asset_value <= debt_barrier {
            1.0
        } else {
            0.0
        };
    }

    let d = (asset_value / debt_barrier) * (lambda * lambda).exp();
    let ln_d = d.ln();
    let survival = norm_cdf(-0.5 * a_t + ln_d / a_t) - d * norm_cdf(-0.5 * a_t - ln_d / a_t);
    (1.0 - survival).clamp(0.0, 1.0)
}

/// Merton structural credit model.
///
/// Models a firm's equity as a call option on its assets, where default
/// occurs when asset value falls below the debt barrier.
///
/// # Fields
///
/// - `asset_value` (V_0): Current market value of the firm's assets.
/// - `asset_vol` (sigma_V): Annualized volatility of asset returns.
/// - `debt_barrier` (B): Face value of debt / default point.
/// - `risk_free_rate` (r): Continuous risk-free rate.
/// - `payout_rate` (q): Continuous dividend / payout yield on assets.
/// - `barrier_type`: Terminal or first-passage barrier monitoring.
/// - `dynamics`: Asset return dynamics specification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct MertonModel {
    asset_value: f64,
    asset_vol: f64,
    debt_barrier: f64,
    risk_free_rate: f64,
    payout_rate: f64,
    barrier_type: BarrierType,
    dynamics: AssetDynamics,
}

impl MertonModel {
    /// Create a new Merton model with GBM dynamics and terminal barrier.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current asset value V_0 (must be > 0)
    /// * `asset_vol` - Asset volatility sigma_V (must be > 0)
    /// * `debt_barrier` - Debt face value B (must be > 0)
    /// * `risk_free_rate` - Risk-free rate r
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] if `asset_value`, `asset_vol`,
    /// or `debt_barrier` are non-positive.
    pub fn new(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
    ) -> Result<Self> {
        Self::new_with_dynamics(
            asset_value,
            asset_vol,
            debt_barrier,
            risk_free_rate,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
    }

    /// Create a new Merton model with full parameterisation.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current asset value V_0 (must be > 0)
    /// * `asset_vol` - Asset volatility sigma_V (must be > 0)
    /// * `debt_barrier` - Debt face value B (must be > 0)
    /// * `risk_free_rate` - Risk-free rate r
    /// * `payout_rate` - Dividend / payout yield q
    /// * `barrier_type` - Terminal or first-passage
    /// * `dynamics` - Asset return dynamics
    ///
    /// # `V <= B` (in-default state)
    ///
    /// `asset_value <= debt_barrier` is intentionally accepted: it represents
    /// a firm at or through its default point (distressed names). Pricing
    /// then degenerates consistently — first-passage paths default
    /// immediately, terminal-barrier default probabilities approach 1, and
    /// the CreditGrades survival formula returns PD = 1 in the
    /// zero-variance limit. Callers wanting a strictly solvent firm should
    /// validate `asset_value > debt_barrier` themselves.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] if `asset_value`, `asset_vol`,
    /// or `debt_barrier` are non-positive.
    pub fn new_with_dynamics(
        asset_value: f64,
        asset_vol: f64,
        debt_barrier: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        barrier_type: BarrierType,
        dynamics: AssetDynamics,
    ) -> Result<Self> {
        if asset_value <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if asset_vol <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if debt_barrier <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        Ok(Self {
            asset_value,
            asset_vol,
            debt_barrier,
            risk_free_rate,
            payout_rate,
            barrier_type,
            dynamics,
        })
    }

    /// Distance-to-default over the given horizon.
    ///
    /// DD = (ln(V/B) + (r - q - sigma^2/2) * T) / (sigma * sqrt(T))
    ///
    /// A higher DD indicates a lower probability of default.
    /// Returns `f64::INFINITY` when `horizon <= 0` (no time = infinite DD,
    /// yielding PD = N(-∞) = 0).
    #[inline]
    pub fn distance_to_default(&self, horizon: f64) -> f64 {
        if horizon <= 0.0 {
            return f64::INFINITY;
        }
        let sigma = self.asset_vol;
        let mu = self.risk_free_rate - self.payout_rate - 0.5 * sigma * sigma;
        let sqrt_t = horizon.sqrt();
        ((self.asset_value / self.debt_barrier).ln() + mu * horizon) / (sigma * sqrt_t)
    }

    /// Default probability over the given horizon.
    ///
    /// # Measure
    ///
    /// This is the **risk-neutral (Q-measure)** default probability: the
    /// drift is the risk-free `r − q − σ²/2`, not the firm's real-world
    /// asset drift. It is the right quantity for pricing and credit-spread
    /// work ([`Self::implied_spread`], [`Self::to_hazard_curve`]), but it
    /// materially **overstates** the physical/real-world PD (EDF) whenever
    /// the market price of asset risk is positive — often by several times
    /// for a healthy firm. Do not feed it into expected-loss, capital, or
    /// rating analytics that require a physical PD; those need the
    /// real-world drift substituted for `r`.
    ///
    /// - **Terminal barrier**: PD = N(-DD) (Merton 1974).
    /// - **First-passage barrier**: Black-Cox (1976) closed-form with
    ///   exponentially growing barrier `B(t) = B * exp(g * t)`. The distance
    ///   process `X_t = ln(V_t / B(t))` is Brownian motion with drift
    ///   `nu = mu - g` (where `mu = r - q - sigma^2/2`) started at
    ///   `x0 = ln(V/B)`, and the first-passage probability to 0 is
    ///   :
    ///
    ///   PD = N(-(x0 + nu*T) / (sigma*sqrt(T)))
    ///      + exp(-2*nu*x0 / sigma^2) * N((-x0 + nu*T) / (sigma*sqrt(T)))
    ///
    /// The first-passage result is clamped to `[0, 1]` to absorb
    /// floating-point overshoot when the drift is strongly negative.
    pub fn default_probability(&self, horizon: f64) -> f64 {
        if let AssetDynamics::CreditGrades {
            barrier_uncertainty,
            ..
        } = self.dynamics
        {
            return credit_grades_default_probability(
                self.asset_value,
                self.asset_vol,
                self.debt_barrier,
                barrier_uncertainty,
                horizon,
            );
        }

        match self.barrier_type {
            BarrierType::Terminal => {
                let dd = self.distance_to_default(horizon);
                norm_cdf(-dd)
            }
            BarrierType::FirstPassage {
                barrier_growth_rate,
            } => {
                let sigma = self.asset_vol;
                let mu = self.risk_free_rate - self.payout_rate - 0.5 * sigma * sigma;
                let sqrt_t = horizon.sqrt();
                let sigma_sqrt_t = sigma * sqrt_t;

                // Reduce to a flat barrier at 0 : the
                // distance process X_t = ln(V_t / (B e^{g t})) is BM with
                // drift nu = mu - g started at x0 = ln(V/B). The growing
                // barrier shifts the *drift*, not the starting distance.
                let nu = mu - barrier_growth_rate;
                let x0 = (self.asset_value / self.debt_barrier).ln();

                let d_plus = (x0 + nu * horizon) / sigma_sqrt_t;
                let d_minus = (x0 - nu * horizon) / sigma_sqrt_t;

                // Black-Cox reflection term `exp(-2*nu*x0/sigma^2) * N(-d_minus)`.
                // The exponential factor overflows to `+inf` for a large
                // `|exponent|` (e.g. a strongly negative drift, or a low
                // vol with a high rate). When `N(-d_minus)` simultaneously
                // underflows to `0` the naive product is `inf * 0 = NaN`,
                // which would survive the final `clamp(0, 1)`.
                //
                // The Gaussian tail `N(-d_minus)` decays as `exp(-d_minus^2/2)`,
                // which dominates the (at most exponential-in-`d_minus`) power
                // factor, so the term tends to `0` whenever `N(-d_minus)` does.
                // Guard that case, then evaluate the surviving product in
                // log-space so a genuinely large term overflows cleanly to
                // `+inf` (and clamps to `1`) instead of producing a `NaN`.
                let exponent = -2.0 * nu / (sigma * sigma);
                let nd_minus = norm_cdf(-d_minus);
                let reflection_term = if nd_minus <= 0.0 {
                    0.0
                } else {
                    let log_term = exponent * x0 + nd_minus.ln();
                    log_term.exp()
                };

                let pd = norm_cdf(-d_plus) + reflection_term;
                if pd.is_nan() {
                    return 0.0;
                }
                pd.clamp(0.0, 1.0)
            }
        }
    }

    /// Implied credit spread from default probability and recovery rate.
    ///
    /// s = -ln(1 - PD * (1 - R)) / T
    ///
    /// where PD is the default probability over horizon T and R is the
    /// assumed recovery rate (fraction of face value recovered at default).
    ///
    /// # Errors
    ///
    /// Returns a validation error if `horizon <= 0` (the spread is undefined
    /// at zero horizon) or `recovery` is outside `[0, 1]` (an out-of-range
    /// recovery can make `1 - PD·LGD` non-positive, yielding NaN).
    #[inline]
    pub fn implied_spread(&self, horizon: f64, recovery: f64) -> Result<f64> {
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "implied_spread: horizon must be > 0, got {horizon}"
            )));
        }
        if !(0.0..=1.0).contains(&recovery) {
            return Err(Error::Validation(format!(
                "implied_spread: recovery must be in [0, 1], got {recovery}"
            )));
        }
        let pd = self.default_probability(horizon);
        let lgd = 1.0 - recovery;
        Ok(-(1.0 - pd * lgd).ln() / horizon)
    }

    // -----------------------------------------------------------------------
    // Calibration methods
    // -----------------------------------------------------------------------

    /// Minimum equity value (in asset-value units) for which the equity-vol
    /// relation `sigma_E = N(d1) * exp(-qT) * sigma_V * V / E` is numerically
    /// well posed. Below this, `E` is treated as effectively zero (the firm is
    /// economically in default) and the division is rejected.
    const MIN_EQUITY: f64 = 1.0e-8;

    /// Minimum `N(d1)` for which the KMV / equity-vol inversion is well posed.
    /// Below this, the equity is deep-out-of-the-money on the firm's assets
    /// and the inversion is numerically unstable.
    const MIN_ND1: f64 = 1.0e-12;

    /// Compute implied equity value and equity volatility from the structural
    /// model, rejecting numerically degenerate (near-default) configurations.
    ///
    /// Uses the Black-Scholes call option formula where equity is a call on
    /// the firm's assets with strike equal to the debt barrier, accounting
    /// for continuous payout rate q (Hull, 9th ed., Chapter 17):
    ///
    /// - d1 = (ln(V/B) + (r - q + sigma^2/2) * T) / (sigma * sqrt(T))
    /// - d2 = d1 - sigma * sqrt(T)
    /// - E = V * exp(-q*T) * N(d1) - B * exp(-r*T) * N(d2)
    /// - sigma_E = N(d1) * exp(-q*T) * sigma_V * V / E
    ///
    /// For a deeply distressed firm `E -> 0+`, so `sigma_E` would diverge to
    /// `+inf`. This method rejects such inputs up front with a descriptive
    /// error instead of returning `inf`/`NaN`.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Time horizon T in years (must be > 0)
    ///
    /// # Returns
    ///
    /// A tuple `(equity_value, equity_vol)`.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Invalid`] if the implied equity value or `N(d1)`
    /// is below the well-posed floor (the firm is economically in default).
    pub fn try_implied_equity(&self, horizon: f64) -> Result<(f64, f64)> {
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "try_implied_equity: horizon must be > 0, got {horizon}"
            )));
        }
        let v = self.asset_value;
        let sigma = self.asset_vol;
        let b = self.debt_barrier;
        let r = self.risk_free_rate;
        let q = self.payout_rate;
        let sqrt_t = horizon.sqrt();

        let d1 = ((v / b).ln() + (r - q + 0.5 * sigma * sigma) * horizon) / (sigma * sqrt_t);
        let d2 = d1 - sigma * sqrt_t;

        let nd1 = norm_cdf(d1);
        let nd2 = norm_cdf(d2);

        let exp_neg_qt = (-q * horizon).exp();
        let equity = v * exp_neg_qt * nd1 - b * (-r * horizon).exp() * nd2;

        if !equity.is_finite() || equity <= Self::MIN_EQUITY || nd1 <= Self::MIN_ND1 {
            return Err(InputError::Invalid.into());
        }

        let equity_vol = nd1 * exp_neg_qt * sigma * v / equity;
        Ok((equity, equity_vol))
    }

    /// KMV calibration: recover asset value and asset volatility from observed
    /// equity value and equity volatility.
    ///
    /// Solves the 2x2 nonlinear system iteratively (fixed-point iteration),
    /// including the continuous payout rate q:
    ///
    /// - E = V * exp(-q*T) * N(d1) - B * exp(-r*T) * N(d2)
    /// - sigma_E * E = N(d1) * exp(-q*T) * sigma_V * V
    ///
    /// Convergence is typically fast (10-20 iterations).
    ///
    /// # Arguments
    ///
    /// * `equity_value` - Observed market equity value E
    /// * `equity_vol` - Observed equity volatility sigma_E
    /// * `total_debt` - Face value of debt B
    /// * `risk_free_rate` - Risk-free rate r
    /// * `payout_rate` - Continuous dividend / payout yield q
    /// * `maturity` - Time to maturity T in years
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are invalid (including a near-zero
    /// `equity_value`, or a deep-out-of-the-money configuration where
    /// `N(d1)` collapses to zero) or if iteration fails to converge.
    pub fn from_equity(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        maturity: f64,
    ) -> Result<Self> {
        if equity_value <= 0.0 || total_debt <= 0.0 || maturity <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if equity_vol < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        // A near-zero equity value makes the KMV volatility inversion
        // `sigma_V = sigma_E * E / (N(d1) * exp(-qT) * V)` ill-conditioned and
        // can drive intermediate iterates to inf/NaN, silently defeating the
        // convergence test. Reject it up front with a descriptive error.
        if equity_value <= Self::MIN_EQUITY {
            return Err(InputError::Invalid.into());
        }

        let e = equity_value;
        let sigma_e = equity_vol;
        let b = total_debt;
        let r = risk_free_rate;
        let q = payout_rate;
        let t = maturity;
        let sqrt_t = t.sqrt();
        let exp_neg_qt = (-q * t).exp();

        // Initial guesses
        let mut v = e + b;
        let mut sigma_v = sigma_e * e / v;

        let max_iter = 100;
        let tol = 1e-8;

        for _ in 0..max_iter {
            let v_prev = v;

            let d1 = ((v / b).ln() + (r - q + 0.5 * sigma_v * sigma_v) * t) / (sigma_v * sqrt_t);
            let d2 = d1 - sigma_v * sqrt_t;

            let nd1 = norm_cdf(d1);
            let nd2 = norm_cdf(d2);

            // Deep-OTM: N(d1) -> 0 makes the V / sigma_V updates blow up to
            // inf/NaN, which makes the relative-change test silently never
            // fire. Reject explicitly rather than burning all iterations.
            if nd1 <= Self::MIN_ND1 {
                return Err(InputError::Invalid.into());
            }

            // Update V from the call pricing equation: E = V*exp(-qT)*N(d1) - B*exp(-rT)*N(d2)
            v = (e + b * (-r * t).exp() * nd2) / (exp_neg_qt * nd1);
            // Update sigma_V from the volatility relation
            sigma_v = sigma_e * e / (nd1 * exp_neg_qt * v);

            if ((v - v_prev) / v_prev).abs() < tol {
                return Self::new_with_dynamics(
                    v,
                    sigma_v,
                    b,
                    r,
                    q,
                    BarrierType::Terminal,
                    AssetDynamics::GeometricBrownian,
                );
            }
        }

        Err(InputError::SolverConvergenceFailed {
            iterations: max_iter,
            residual: {
                let d1 =
                    ((v / b).ln() + (r - q + 0.5 * sigma_v * sigma_v) * t) / (sigma_v * sqrt_t);
                let nd1 = norm_cdf(d1);
                let nd2 = norm_cdf(d1 - sigma_v * sqrt_t);
                (v * exp_neg_qt * nd1 - b * (-r * t).exp() * nd2 - e).abs()
            },
            last_x: v,
            reason: "KMV fixed-point iteration did not converge".to_string(),
        }
        .into())
    }

    /// CDS spread calibration: find asset volatility that matches a target
    /// CDS spread.
    ///
    /// Uses Brent's method to solve for sigma_V such that the model's
    /// implied spread equals the target CDS spread. The calibration uses
    /// the terminal-barrier (classic Merton) default probability formula
    /// `PD = N(-DD)`, with the distance-to-default drift
    /// `mu = r - q - sigma^2/2` (consistent with
    /// [`distance_to_default`](Self::distance_to_default) and
    /// [`try_implied_equity`](Self::try_implied_equity)). The resulting model has
    /// `BarrierType::Terminal` with `AssetDynamics::GeometricBrownian` and
    /// the supplied `payout_rate`.
    ///
    /// To use first-passage barriers after calibration, construct a new
    /// model via [`new_with_dynamics`](Self::new_with_dynamics) using the
    /// calibrated `asset_vol`.
    ///
    /// # Arguments
    ///
    /// * `cds_spread_bp` - Target CDS spread in basis points
    /// * `recovery` - Recovery rate (fraction)
    /// * `total_debt` - Face value of debt B
    /// * `risk_free_rate` - Risk-free rate r
    /// * `maturity` - Time to maturity T in years
    /// * `asset_value` - Assumed initial asset value V
    /// * `payout_rate` - Continuous dividend / payout yield q (pass `0.0`
    ///   for a firm with no asset payout)
    ///
    /// # Errors
    ///
    /// Returns an error if the solver fails to find a solution or inputs
    /// are invalid.
    pub fn from_cds_spread(
        cds_spread_bp: f64,
        recovery: f64,
        total_debt: f64,
        risk_free_rate: f64,
        maturity: f64,
        asset_value: f64,
        payout_rate: f64,
    ) -> Result<Self> {
        if total_debt <= 0.0 || maturity <= 0.0 || asset_value <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if !(0.0..=1.0).contains(&recovery) {
            return Err(Error::Validation(format!(
                "from_cds_spread: recovery must be in [0, 1], got {recovery}"
            )));
        }

        let target_spread = cds_spread_bp / 10_000.0;

        let solver = BrentSolver::new().tolerance(1e-8).bracket_bounds(0.01, 2.0);

        let sigma_v = solver.solve(
            |sigma| {
                // Build a temporary model with this sigma_v to compute implied spread.
                // We use the inner formula directly to avoid the Result from new().
                let v = asset_value;
                let b = total_debt;
                let r = risk_free_rate;
                let q = payout_rate;
                let sig = sigma;
                // Drift includes the payout term, matching distance_to_default.
                let mu = r - q - 0.5 * sig * sig;
                let sqrt_t = maturity.sqrt();
                let dd = ((v / b).ln() + mu * maturity) / (sig * sqrt_t);
                let pd = norm_cdf(-dd);
                let lgd = 1.0 - recovery;
                let spread = -(1.0 - pd * lgd).ln() / maturity;
                spread - target_spread
            },
            0.20, // initial guess
        )?;

        Self::new_with_dynamics(
            asset_value,
            sigma_v,
            total_debt,
            risk_free_rate,
            payout_rate,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
    }

    /// Calibrate the debt barrier to match a target cumulative default
    /// probability over the given maturity.
    ///
    /// Uses terminal-barrier (classic Merton) `PD = N(-DD)` and Brent's
    /// method to find the barrier B such that `default_probability(maturity)
    /// == target_pd`.
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current asset value V
    /// * `asset_vol` - Asset volatility sigma_V
    /// * `risk_free_rate` - Risk-free rate r
    /// * `target_pd` - Target cumulative default probability (e.g. 0.01
    ///   for 1%)
    /// * `maturity` - Time horizon T in years
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are invalid or the solver fails.
    pub fn from_target_pd(
        asset_value: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        target_pd: f64,
        maturity: f64,
    ) -> Result<Self> {
        if asset_value <= 0.0 || maturity <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if asset_vol < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        if !(0.0..1.0).contains(&target_pd) {
            return Err(InputError::Invalid.into());
        }

        let solver = BrentSolver::new()
            .tolerance(1e-10)
            .bracket_bounds(0.001 * asset_value, 0.999 * asset_value);

        let barrier = solver.solve(
            |b| {
                let sigma = asset_vol;
                let mu = risk_free_rate - 0.5 * sigma * sigma;
                let sqrt_t = maturity.sqrt();
                let dd = ((asset_value / b).ln() + mu * maturity) / (sigma * sqrt_t);
                norm_cdf(-dd) - target_pd
            },
            0.5 * asset_value,
        )?;

        Self::new(asset_value, asset_vol, barrier, risk_free_rate)
    }

    /// CreditGrades model construction from equity observables (simplified).
    ///
    /// Derives asset value and asset volatility from equity data and
    /// constructs a model with `CreditGrades` dynamics and `FirstPassage`
    /// barrier. This is a simplified version of the CreditGrades model
    /// (Finger et al. 2002) that uses:
    ///
    /// - Asset value: `V_0 = E + D * R_mean`
    /// - Asset volatility: `sigma_V = sigma_E * E / V_0`
    /// - Barrier: `B = D * R_mean` (deterministic)
    ///
    /// The `barrier_uncertainty` parameter is the log-barrier volatility `λ`
    /// (the lognormal standard deviation of the default barrier) and feeds the
    /// `CreditGrades` survival function via `a_t² = σ²t + λ²`.
    ///
    /// # Arguments
    ///
    /// * `equity_value` - Observed market equity value E
    /// * `equity_vol` - Observed equity volatility sigma_E
    /// * `total_debt` - Face value of debt
    /// * `risk_free_rate` - Risk-free rate r
    /// * `barrier_uncertainty` - Log-barrier volatility `λ` (lognormal std dev
    ///   of the default barrier; Finger et al. 2002)
    /// * `mean_recovery` - Mean recovery rate at default
    ///
    /// # Errors
    ///
    /// Returns an error if inputs are invalid.
    pub fn credit_grades(
        equity_value: f64,
        equity_vol: f64,
        total_debt: f64,
        risk_free_rate: f64,
        barrier_uncertainty: f64,
        mean_recovery: f64,
    ) -> Result<Self> {
        if equity_value <= 0.0 || total_debt <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if equity_vol < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        if !(0.0..=1.0).contains(&mean_recovery) {
            return Err(Error::Validation(format!(
                "credit_grades: mean_recovery must be in [0, 1], got {mean_recovery}"
            )));
        }
        if !(barrier_uncertainty.is_finite() && barrier_uncertainty >= 0.0) {
            return Err(Error::Validation(format!(
                "credit_grades: barrier_uncertainty (log-barrier vol λ) must be \
                 finite and >= 0, got {barrier_uncertainty}"
            )));
        }

        // Asset value = equity + debt * mean_recovery
        let v0 = equity_value + total_debt * mean_recovery;
        // Asset vol from leverage relation
        let sigma_v = equity_vol * equity_value / v0;
        // Barrier = debt * mean_recovery
        let barrier = total_debt * mean_recovery;

        Self::new_with_dynamics(
            v0,
            sigma_v,
            barrier,
            risk_free_rate,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::CreditGrades {
                barrier_uncertainty,
                mean_recovery,
            },
        )
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Current asset value V_0.
    #[inline]
    pub fn asset_value(&self) -> f64 {
        self.asset_value
    }

    /// Asset volatility sigma_V.
    #[inline]
    pub fn asset_vol(&self) -> f64 {
        self.asset_vol
    }

    /// Debt barrier B.
    #[inline]
    pub fn debt_barrier(&self) -> f64 {
        self.debt_barrier
    }

    /// Risk-free rate r.
    #[inline]
    pub fn risk_free_rate(&self) -> f64 {
        self.risk_free_rate
    }

    /// Payout rate q (dividend yield).
    #[inline]
    pub fn payout_rate(&self) -> f64 {
        self.payout_rate
    }

    /// Barrier monitoring type.
    #[inline]
    pub fn barrier_type(&self) -> &BarrierType {
        &self.barrier_type
    }

    /// Asset dynamics specification.
    #[inline]
    pub fn dynamics(&self) -> &AssetDynamics {
        &self.dynamics
    }

    // -----------------------------------------------------------------------
    // Hazard curve generation
    // -----------------------------------------------------------------------

    /// Generate a [`HazardCurve`] compatible with existing pricing engines.
    ///
    /// Converts structural model default probabilities to piecewise-constant
    /// hazard rates at the specified tenor grid.
    ///
    /// # Algorithm
    ///
    /// 1. Compute survival probability S(t) = 1 - PD(t) at each tenor.
    /// 2. Back out piecewise-constant hazard rates between consecutive tenors:
    ///    - λ_0 = -ln(S(t_0)) / t_0
    ///    - λ_i = -ln(S(t_{i+1}) / S(t_i)) / (t_{i+1} - t_i) for i >= 1
    /// 3. Build via `HazardCurve::builder`.
    ///
    /// # Arguments
    ///
    /// * `id` - Curve identifier
    /// * `base_date` - Valuation date for the curve
    /// * `tenors` - Tenor grid in years (must be non-empty, positive; need not be sorted)
    /// * `recovery` - Recovery rate assumption (e.g. 0.40)
    ///
    /// # Errors
    ///
    /// Returns an error if `tenors` is empty, contains non-positive values,
    /// if the implied survival curve is non-monotonic, or if the `HazardCurve`
    /// builder fails.
    pub fn to_hazard_curve(
        &self,
        id: &str,
        base_date: time::Date,
        tenors: &[f64],
        recovery: f64,
    ) -> Result<HazardCurve> {
        if tenors.is_empty() {
            return Err(InputError::TooFewPoints.into());
        }

        // Sort tenors and validate positivity
        let mut sorted_tenors: Vec<f64> = tenors.to_vec();
        sorted_tenors.sort_by(|a, b| a.total_cmp(b));

        if sorted_tenors[0] <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }

        const EPSILON: f64 = 1e-15;

        // Compute survival probabilities, clamped to [epsilon, 1.0]
        let survivals: Vec<f64> = sorted_tenors
            .iter()
            .map(|&t| {
                let pd = self.default_probability(t);
                (1.0 - pd).clamp(EPSILON, 1.0)
            })
            .collect();

        // Build knot points: (tenor, hazard_rate)
        let mut knots: Vec<(f64, f64)> = Vec::with_capacity(sorted_tenors.len());

        // First point: λ_0 = -ln(S(t_0)) / t_0
        let lambda_0 = -survivals[0].ln() / sorted_tenors[0];
        knots.push((sorted_tenors[0], lambda_0));

        // Subsequent points: λ_i = -ln(S(t_{i+1}) / S(t_i)) / (t_{i+1} - t_i)
        for i in 1..sorted_tenors.len() {
            if survivals[i] > survivals[i - 1] {
                return Err(Error::Validation(format!(
                    "Merton hazard bootstrap produced non-monotonic survival: \
                     S({:.6}y)={:.12} > S({:.6}y)={:.12}",
                    sorted_tenors[i],
                    survivals[i],
                    sorted_tenors[i - 1],
                    survivals[i - 1]
                )));
            }
            let dt = sorted_tenors[i] - sorted_tenors[i - 1];
            // Duplicate/non-increasing tenors give dt == 0; the equal survivals
            // pass the monotonic check above, so guard here to avoid emitting a
            // NaN hazard knot (-ln(1)/0 = 0/0) into the curve.
            if dt <= 0.0 {
                return Err(Error::Validation(format!(
                    "Merton hazard bootstrap requires strictly increasing tenors; \
                     got duplicate or non-increasing tenor {:.6}y",
                    sorted_tenors[i]
                )));
            }
            let lambda_i = -(survivals[i] / survivals[i - 1]).ln() / dt;
            knots.push((sorted_tenors[i], lambda_i));
        }

        HazardCurve::builder(id)
            .base_date(base_date)
            .knots(knots)
            .recovery_rate(recovery)
            .build()
    }
}

// ---------------------------------------------------------------------------
// Monte Carlo path simulation (feature-gated)
// ---------------------------------------------------------------------------

/// Results from Monte Carlo path simulation.
#[derive(Debug, Clone)]
pub struct SimulatedPaths {
    /// Time grid from 0 to T.
    pub times: Vec<f64>,
    /// Asset values in row-major order: `path_idx * (num_steps + 1) + time_idx`.
    pub asset_values: Vec<f64>,
    /// Number of paths simulated.
    pub num_paths: usize,
    /// Number of time steps.
    pub num_steps: usize,
}

impl SimulatedPaths {
    /// Number of stored values per path, including the initial value.
    #[must_use]
    pub fn values_per_path(&self) -> usize {
        self.num_steps + 1
    }

    /// Return one asset value by path and time-grid index.
    #[must_use]
    pub fn get(&self, path_idx: usize, time_idx: usize) -> Option<f64> {
        if path_idx >= self.num_paths || time_idx > self.num_steps {
            return None;
        }
        self.asset_values
            .get(path_idx * self.values_per_path() + time_idx)
            .copied()
    }

    /// Return the contiguous row for one path.
    #[must_use]
    pub fn path(&self, path_idx: usize) -> Option<&[f64]> {
        if path_idx >= self.num_paths {
            return None;
        }
        let start = path_idx * self.values_per_path();
        let end = start + self.values_per_path();
        self.asset_values.get(start..end)
    }

    /// Iterate over path rows.
    pub fn iter_paths(&self) -> impl Iterator<Item = &[f64]> {
        self.asset_values.chunks_exact(self.values_per_path())
    }

    /// Materialize nested path storage for callers that need the old shape.
    #[must_use]
    pub fn to_nested(&self) -> Vec<Vec<f64>> {
        self.iter_paths().map(<[f64]>::to_vec).collect()
    }
}

struct StepJumpData {
    base_count: usize,
    anti_count: usize,
    jump_normals: Vec<f64>,
}

impl MertonModel {
    /// Simulate asset value paths using Monte Carlo.
    ///
    /// Supports GBM and jump-diffusion dynamics. Optionally uses antithetic
    /// variates to reduce variance.
    ///
    /// `CreditGrades` dynamics simulate the *asset value* as plain GBM: the
    /// CreditGrades stochastic barrier only enters the analytic
    /// default-probability formulas, not the simulated paths. Callers needing
    /// pathwise CreditGrades default times must layer the barrier on top.
    ///
    /// # Arguments
    ///
    /// * `num_paths` - Number of paths to simulate
    /// * `num_steps` - Number of time steps per path (must be >= 1)
    /// * `horizon` - Time horizon T in years (must be > 0)
    /// * `rng` - Random number generator (trait object)
    /// * `antithetic` - If true, use antithetic variates for variance reduction
    ///
    /// # Returns
    ///
    /// [`SimulatedPaths`] containing the time grid and all simulated asset paths.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `num_steps == 0` (the time grid would be
    /// degenerate with `dt = inf`) or `horizon <= 0`.
    pub fn simulate_paths(
        &self,
        num_paths: usize,
        num_steps: usize,
        horizon: f64,
        rng: &mut dyn RandomNumberGenerator,
        antithetic: bool,
    ) -> Result<SimulatedPaths> {
        if num_steps == 0 {
            return Err(Error::Validation(
                "simulate_paths: num_steps must be >= 1".into(),
            ));
        }
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "simulate_paths: horizon must be > 0, got {horizon}"
            )));
        }
        let dt = horizon / num_steps as f64;
        let sqrt_dt = dt.sqrt();

        // Build time grid: t = 0, dt, 2*dt, ..., T
        let times: Vec<f64> = (0..=num_steps).map(|i| i as f64 * dt).collect();

        let v0 = self.asset_value;
        let sigma = self.asset_vol;
        let r = self.risk_free_rate;
        let q = self.payout_rate;

        // Determine drift and whether we have jumps
        let (drift_per_step, jump_params) = match &self.dynamics {
            AssetDynamics::GeometricBrownian | AssetDynamics::CreditGrades { .. } => {
                let drift = (r - q - 0.5 * sigma * sigma) * dt;
                (drift, None)
            }
            AssetDynamics::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            } => {
                // kappa = E[e^J] - 1 where J ~ N(mu_J, sigma_J^2)
                let kappa = (jump_mean + 0.5 * jump_vol * jump_vol).exp() - 1.0;
                // Compensated drift to keep E[V(T)] = V0 * e^{(r-q)T}
                let drift = (r - q - jump_intensity * kappa - 0.5 * sigma * sigma) * dt;
                (drift, Some((*jump_intensity, *jump_mean, *jump_vol)))
            }
        };

        let diffusion = sigma * sqrt_dt;

        // Determine how many base paths to generate
        let (n_base, gen_antithetic) = if antithetic {
            // For num_paths requested: generate ceil(num_paths/2) base paths
            // and their mirrors. Total = 2 * n_base.
            // If num_paths is odd, we generate one extra base path without mirror
            // to hit exactly num_paths.
            let n_base = num_paths.div_ceil(2);
            (n_base, true)
        } else {
            (num_paths, false)
        };

        let values_per_path = num_steps + 1;
        let mut all_paths: Vec<f64> = Vec::with_capacity(num_paths * values_per_path);
        let mut normals = vec![0.0; num_steps];

        for _ in 0..n_base {
            // Generate normals for this base path
            normals.iter_mut().for_each(|z| *z = rng.normal(0.0, 1.0));

            // Generate jump data if needed
            let jump_data: Option<Vec<StepJumpData>> = jump_params.map(|(lambda, _, _)| {
                let lambda_dt = lambda * dt;
                (0..num_steps)
                    .map(|_| {
                        let u = rng.uniform();
                        let base_count = poisson_inverse_cdf(lambda_dt, u);
                        let anti_count =
                            poisson_inverse_cdf(lambda_dt, (1.0 - u).min(1.0 - f64::EPSILON));
                        let max_count = base_count.max(anti_count);
                        let jump_normals: Vec<f64> =
                            (0..max_count).map(|_| rng.normal(0.0, 1.0)).collect();
                        StepJumpData {
                            base_count,
                            anti_count,
                            jump_normals,
                        }
                    })
                    .collect()
            });

            // Build the base path
            all_paths.push(v0);
            let mut v = v0;

            for step in 0..num_steps {
                let z = normals[step];
                v *= (drift_per_step + diffusion * z).exp();

                // Apply jumps if present
                if let (Some(ref jd), Some((_, mu_j, sigma_j))) = (&jump_data, jump_params) {
                    let jump_step = &jd[step];
                    for &jz in jump_step.jump_normals.iter().take(jump_step.base_count) {
                        // Jump multiplier e^J with J ~ N(mu_J, sigma_J^2): mean
                        // E[e^J] = exp(mu_J + sigma_J^2/2), matching the kappa
                        // compensator above. No Ito correction belongs on the
                        // jump itself .
                        v *= (mu_j + sigma_j * jz).exp();
                    }
                }

                all_paths.push(v);
            }

            // Build the antithetic (mirror) path if requested
            if gen_antithetic && all_paths.len() / values_per_path < num_paths {
                all_paths.push(v0);
                let mut v_anti = v0;

                for step in 0..num_steps {
                    let z = -normals[step]; // Negated normal
                    v_anti *= (drift_per_step + diffusion * z).exp();

                    if let (Some(ref jd), Some((_, mu_j, sigma_j))) = (&jump_data, jump_params) {
                        let jump_step = &jd[step];
                        for &jz in jump_step.jump_normals.iter().take(jump_step.anti_count) {
                            let jz = -jz;
                            // Same e^J law as the base path .
                            v_anti *= (mu_j + sigma_j * jz).exp();
                        }
                    }

                    all_paths.push(v_anti);
                }
            }
        }

        // Trim to exact num_paths in case antithetic generated one extra
        all_paths.truncate(num_paths * values_per_path);

        Ok(SimulatedPaths {
            times,
            asset_values: all_paths,
            num_paths,
            num_steps,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dd_textbook_values() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
        let dd = m.distance_to_default(1.0);
        // DD = (ln(100/80) + (0.05 - 0 - 0.02)*1) / (0.2*1) = (0.22314 + 0.03) / 0.2 = 1.2657
        assert!((dd - 1.2657).abs() < 0.01, "DD={dd}");
    }

    #[test]
    fn pd_textbook_values() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
        let pd = m.default_probability(1.0);
        // PD = N(-1.2657) ~ 0.1028
        assert!((pd - 0.1028).abs() < 0.01, "PD={pd}");
    }

    #[test]
    fn zero_vol_means_no_default_when_solvent() {
        let m = MertonModel::new(100.0, 1e-10, 80.0, 0.05).unwrap();
        let pd = m.default_probability(1.0);
        assert!(pd < 1e-6, "Zero vol, solvent -> PD~0, got {pd}");
    }

    #[test]
    fn pd_increases_with_vol() {
        let m_low = MertonModel::new(100.0, 0.10, 80.0, 0.05).unwrap();
        let m_high = MertonModel::new(100.0, 0.40, 80.0, 0.05).unwrap();
        assert!(m_high.default_probability(1.0) > m_low.default_probability(1.0));
    }

    #[test]
    fn pd_increases_with_leverage() {
        let m_low = MertonModel::new(100.0, 0.20, 60.0, 0.05).unwrap();
        let m_high = MertonModel::new(100.0, 0.20, 95.0, 0.05).unwrap();
        assert!(m_high.default_probability(1.0) > m_low.default_probability(1.0));
    }

    #[test]
    fn first_passage_pd_higher_than_terminal() {
        let m_term = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
        let m_fp = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.05,
            },
            AssetDynamics::GeometricBrownian,
        )
        .unwrap();
        assert!(
            m_fp.default_probability(5.0) > m_term.default_probability(5.0),
            "First-passage PD should be higher than terminal PD"
        );
    }

    #[test]
    fn implied_spread_positive_for_risky_firm() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).unwrap();
        let spread = m.implied_spread(5.0, 0.40).expect("spread");
        assert!(spread > 0.0, "Spread should be positive");
        assert!(spread < 0.20, "Spread should be reasonable, got {spread}");
    }

    #[test]
    fn new_rejects_invalid_inputs() {
        assert!(MertonModel::new(0.0, 0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(-1.0, 0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(100.0, -0.20, 80.0, 0.05).is_err());
        assert!(MertonModel::new(100.0, 0.20, 0.0, 0.05).is_err());
    }

    #[test]
    fn implied_spread_rejects_invalid_inputs() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).unwrap();
        assert!(m.implied_spread(0.0, 0.40).is_err(), "horizon = 0");
        assert!(m.implied_spread(-1.0, 0.40).is_err(), "horizon < 0");
        assert!(m.implied_spread(5.0, -0.1).is_err(), "recovery < 0");
        assert!(m.implied_spread(5.0, 1.1).is_err(), "recovery > 1");
    }

    #[test]
    fn from_cds_spread_rejects_out_of_range_recovery() {
        assert!(MertonModel::from_cds_spread(150.0, -0.1, 80.0, 0.04, 5.0, 100.0, 0.0).is_err());
        assert!(MertonModel::from_cds_spread(150.0, 1.5, 80.0, 0.04, 5.0, 100.0, 0.0).is_err());
    }

    #[test]
    fn simulate_paths_rejects_degenerate_grid() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).unwrap();
        let mut rng = finstack_quant_core::math::random::Pcg64Rng::new(42);
        assert!(m.simulate_paths(10, 0, 5.0, &mut rng, false).is_err());
        assert!(m.simulate_paths(10, 60, 0.0, &mut rng, false).is_err());
    }

    #[test]
    fn implied_equity_from_known_asset() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let (equity, equity_vol) = m.try_implied_equity(1.0).expect("healthy firm");
        // E should be V*N(d1) - B*e^(-rT)*N(d2)
        assert!(equity > 0.0, "Equity should be positive, got {equity}");
        assert!(
            equity_vol > 0.0,
            "Equity vol should be positive, got {equity_vol}"
        );
        // With V=100, B=80, sigma=0.20, r=0.05, T=1:
        // d1 = (ln(1.25) + (0.05 + 0.02)*1) / 0.2 = (0.2231 + 0.07) / 0.2 = 1.4657
        // d2 = 1.4657 - 0.2 = 1.2657
        // E = 100*N(1.4657) - 80*e^(-0.05)*N(1.2657) ~ 100*0.9286 - 76.10*0.8972 ~ 24.59
        assert!((equity - 24.59).abs() < 1.0, "Equity={equity}");
    }

    #[test]
    fn from_equity_recovers_known_values() {
        let m_known = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let (equity, equity_vol) = m_known.try_implied_equity(1.0).expect("healthy firm");
        let m_calibrated = MertonModel::from_equity(equity, equity_vol, 80.0, 0.05, 0.0, 1.0)
            .expect("calibration");
        assert!(
            (m_calibrated.asset_value() - 100.0).abs() < 0.5,
            "Asset value should recover: got {}",
            m_calibrated.asset_value()
        );
        assert!(
            (m_calibrated.asset_vol() - 0.20).abs() < 0.01,
            "Asset vol should recover: got {}",
            m_calibrated.asset_vol()
        );
    }

    #[test]
    fn from_cds_spread_roundtrips() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let spread = m.implied_spread(5.0, 0.40).expect("spread");
        let spread_bp = spread * 10_000.0;
        let m2 = MertonModel::from_cds_spread(spread_bp, 0.40, 80.0, 0.04, 5.0, 100.0, 0.0)
            .expect("cds cal");
        assert!(
            (m2.asset_vol() - 0.25).abs() < 0.02,
            "Asset vol should recover: got {}",
            m2.asset_vol()
        );
    }

    #[test]
    fn from_cds_spread_roundtrips_with_payout_rate() {
        // Calibrate a firm with a real asset payout q > 0. The spread is
        // produced by a model carrying that payout; from_cds_spread must
        // thread the same q into its calibration drift so the recovered
        // sigma_V matches. Dropping q biases sigma_V.
        let q = 0.04;
        let m_known = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.04,
            q,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        let spread = m_known.implied_spread(5.0, 0.40).expect("spread");
        let spread_bp = spread * 10_000.0;

        let m_cal = MertonModel::from_cds_spread(spread_bp, 0.40, 80.0, 0.04, 5.0, 100.0, q)
            .expect("cds cal");

        assert!(
            (m_cal.asset_vol() - 0.25).abs() < 0.01,
            "Asset vol should recover with q={q}: got {}",
            m_cal.asset_vol()
        );
        assert!(
            (m_cal.payout_rate() - q).abs() < 1e-12,
            "Payout rate should be preserved: got {}",
            m_cal.payout_rate()
        );
    }

    #[test]
    fn to_hazard_curve_survival_matches_pd() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let hc = m
            .to_hazard_curve("TEST", base, &[1.0, 3.0, 5.0, 7.0, 10.0], 0.40)
            .expect("hc");
        // Survival at 5Y should match 1 - PD(5)
        let sp5 = hc.sp(5.0);
        let pd5 = m.default_probability(5.0);
        assert!(
            (sp5 - (1.0 - pd5)).abs() < 0.02,
            "sp5={sp5}, 1-pd5={}",
            1.0 - pd5
        );
    }

    #[test]
    fn to_hazard_curve_hazard_rates_positive() {
        let m = MertonModel::new(100.0, 0.30, 80.0, 0.04).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let hc = m
            .to_hazard_curve("TEST2", base, &[1.0, 3.0, 5.0], 0.40)
            .expect("hc");
        // All hazard rates should be positive for a risky firm
        for t in [0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
            let hr = hc.hazard_rate(t);
            assert!(
                hr > 0.0,
                "Hazard rate at t={t} should be positive, got {hr}"
            );
        }
    }

    #[test]
    fn to_hazard_curve_riskier_firm_higher_hazard() {
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let m_safe = MertonModel::new(100.0, 0.15, 50.0, 0.04).expect("valid");
        let m_risky = MertonModel::new(100.0, 0.30, 85.0, 0.04).expect("valid");
        let hc_safe = m_safe
            .to_hazard_curve("SAFE", base, &[1.0, 5.0, 10.0], 0.40)
            .expect("hc");
        let hc_risky = m_risky
            .to_hazard_curve("RISKY", base, &[1.0, 5.0, 10.0], 0.40)
            .expect("hc");
        assert!(
            hc_risky.hazard_rate(3.0) > hc_safe.hazard_rate(3.0),
            "Riskier firm should have higher hazard rate"
        );
    }

    #[test]
    fn to_hazard_curve_rejects_non_monotonic_survival() {
        // V < B with positive drift can make terminal PD fall with horizon,
        // which implies increasing survival and a negative hazard segment.
        let m = MertonModel::new(98.0, 0.30, 100.0, 0.10).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");

        let survival_1y = 1.0 - m.default_probability(1.0);
        let survival_5y = 1.0 - m.default_probability(5.0);
        assert!(
            survival_5y > survival_1y,
            "fixture must have increasing survival: 1Y={survival_1y}, 5Y={survival_5y}"
        );

        assert!(m.to_hazard_curve("BAD", base, &[1.0, 5.0], 0.40).is_err());
    }

    #[test]
    fn to_hazard_curve_rejects_duplicate_tenors() {
        // Duplicate tenors (after sorting) give dt == 0 with equal survivals,
        // which previously slipped past the monotonic-survival check and emitted
        // a NaN hazard knot (-ln(1)/0). It must now be rejected.
        let m = MertonModel::new(120.0, 0.25, 100.0, 0.05).expect("valid");
        let base = time::Date::from_calendar_date(2026, time::Month::March, 1).expect("valid date");
        let result = m.to_hazard_curve("DUP", base, &[1.0, 5.0, 5.0], 0.40);
        assert!(
            result.is_err(),
            "duplicate tenors must be rejected, got {result:?}"
        );
    }

    #[test]
    fn credit_grades_produces_valid_model() {
        let m = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 0.40).expect("cg");
        assert!(m.asset_value() > 0.0);
        assert!(m.asset_vol() > 0.0);
        assert!(matches!(m.dynamics(), AssetDynamics::CreditGrades { .. }));
        assert!(matches!(m.barrier_type(), BarrierType::FirstPassage { .. }));
        let pd = m.default_probability(5.0);
        assert!(pd > 0.0 && pd < 1.0, "PD should be in (0,1), got {pd}");
    }

    // -----------------------------------------------------------------------
    // Non-zero payout rate tests
    // -----------------------------------------------------------------------

    #[test]
    fn implied_equity_with_payout_rate() {
        // With q > 0, equity should be lower than the q=0 case because
        // the asset leaks value via dividends.
        let m_no_q = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let m_with_q = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.03,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");

        let (eq_no_q, _) = m_no_q.try_implied_equity(1.0).expect("healthy firm");
        let (eq_with_q, _) = m_with_q.try_implied_equity(1.0).expect("healthy firm");

        assert!(
            eq_with_q < eq_no_q,
            "Equity with payout should be lower: q=0 -> {eq_no_q}, q=0.03 -> {eq_with_q}"
        );
    }

    #[test]
    fn from_equity_roundtrips_with_payout_rate() {
        let m_known = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.02,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        let (equity, equity_vol) = m_known.try_implied_equity(1.0).expect("healthy firm");

        let m_cal = MertonModel::from_equity(equity, equity_vol, 80.0, 0.05, 0.02, 1.0)
            .expect("calibration");

        assert!(
            (m_cal.asset_value() - 100.0).abs() < 0.5,
            "Asset value should recover with q=0.02: got {}",
            m_cal.asset_value()
        );
        assert!(
            (m_cal.asset_vol() - 0.20).abs() < 0.01,
            "Asset vol should recover with q=0.02: got {}",
            m_cal.asset_vol()
        );
        assert!(
            (m_cal.payout_rate() - 0.02).abs() < 1e-10,
            "Payout rate should be preserved: got {}",
            m_cal.payout_rate()
        );
    }

    // -----------------------------------------------------------------------
    // Near-zero equity guards (W-10)
    // -----------------------------------------------------------------------

    #[test]
    fn implied_equity_rejects_near_zero_equity() {
        // A deeply distressed firm: V far below B with low vol drives the
        // call-option equity value to ~0, so the equity-vol division would
        // otherwise blow up to inf/NaN.
        let m = MertonModel::new(1.0, 0.05, 1.0e9, 0.05).expect("valid");
        let res = m.try_implied_equity(1.0);
        assert!(
            res.is_err(),
            "near-zero implied equity should be rejected, got {res:?}"
        );
    }

    #[test]
    fn implied_equity_ok_for_healthy_firm() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let (equity, equity_vol) = m.try_implied_equity(1.0).expect("healthy firm");
        assert!(equity.is_finite() && equity > 0.0);
        assert!(equity_vol.is_finite() && equity_vol > 0.0);
    }

    #[test]
    fn from_equity_rejects_near_zero_equity() {
        // A near-zero equity input must be rejected up front with a
        // descriptive `Invalid` error, not silently churned through the
        // fixed-point loop until `SolverConvergenceFailed` (which would
        // burn all 100 iterations and report a misleading reason).
        let res = MertonModel::from_equity(1.0e-12, 0.30, 80.0, 0.05, 0.0, 1.0);
        let err = res.expect_err("near-zero equity input should be rejected");
        let msg = err.to_string();
        assert!(
            !msg.contains("did not converge"),
            "should be an up-front input rejection, not a convergence failure: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Non-convergence test
    // -----------------------------------------------------------------------

    #[test]
    fn from_equity_rejects_invalid_inputs() {
        assert!(
            MertonModel::from_equity(0.0, 0.30, 80.0, 0.05, 0.0, 1.0).is_err(),
            "Zero equity should be rejected"
        );
        assert!(
            MertonModel::from_equity(25.0, -0.30, 80.0, 0.05, 0.0, 1.0).is_err(),
            "Negative vol should be rejected"
        );
        assert!(
            MertonModel::from_equity(25.0, 0.30, 0.0, 0.05, 0.0, 1.0).is_err(),
            "Zero debt should be rejected"
        );
        assert!(
            MertonModel::from_equity(25.0, 0.30, 80.0, 0.05, 0.0, 0.0).is_err(),
            "Zero maturity should be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // Extreme parameter edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn high_vol_pd_approaches_one() {
        // With very high vol and high leverage, PD should approach 1.
        let m = MertonModel::new(100.0, 2.0, 99.0, 0.01).expect("valid");
        let pd = m.default_probability(10.0);
        assert!(
            pd > 0.5,
            "High vol + high leverage PD should be > 0.5, got {pd}"
        );
    }

    #[test]
    fn very_low_leverage_pd_near_zero() {
        // V >> B should give near-zero PD.
        let m = MertonModel::new(1000.0, 0.20, 10.0, 0.05).expect("valid");
        let pd = m.default_probability(1.0);
        assert!(pd < 1e-6, "Very low leverage PD should be ~0, got {pd}");
    }

    #[test]
    fn first_passage_pd_bounded() {
        let m = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.05,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.02,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");

        for &t in &[0.5, 1.0, 5.0, 10.0, 30.0] {
            let pd = m.default_probability(t);
            assert!((0.0..=1.0).contains(&pd), "PD({t}) = {pd} out of [0, 1]");
        }
    }

    #[test]
    fn first_passage_pd_finite_under_overflowing_power_term() {
        // P0-2: the Black-Cox first-passage term `(V/H)^(-2*mu/sigma^2) * N(d-)`
        // can overflow. With a large positive risk-neutral drift (low vol,
        // very high rate) the exponent `-2*mu/sigma^2` is a large negative
        // number; for a firm trading below its barrier `(V/H)^exponent`
        // overflows to `+inf` while `N(d-)` underflows to `0`, so the naive
        // product is `inf * 0 = NaN` which survives `clamp(0, 1)`.
        let m = MertonModel::new_with_dynamics(
            50.0,
            0.05,
            100.0,
            3.0,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        let pd = m.default_probability(1.0);
        assert!(
            pd.is_finite(),
            "first-passage PD must be finite (not NaN from inf*0), got {pd}"
        );
        assert!(
            (0.0..=1.0).contains(&pd),
            "first-passage PD must lie in [0, 1], got {pd}"
        );
    }

    #[test]
    fn first_passage_pd_finite_under_large_negative_drift() {
        // The mirror case: a strongly negative drift (very high vol) drives
        // a large positive exponent; for a firm above its barrier the power
        // term overflows. PD must still be a finite, clamped value.
        let m = MertonModel::new_with_dynamics(
            1.0e6,
            0.05,
            1.0,
            0.0,
            5.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        for &t in &[0.5, 1.0, 5.0, 30.0] {
            let pd = m.default_probability(t);
            assert!(
                pd.is_finite() && (0.0..=1.0).contains(&pd),
                "first-passage PD({t}) must be finite in [0, 1], got {pd}"
            );
        }
    }

    #[test]
    fn first_passage_higher_barrier_growth_higher_pd() {
        // Higher barrier growth rate should produce higher PD at any given horizon,
        // since the default barrier rises faster.
        let m_low = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.05,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        let m_high = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.05,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.05,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");

        let pd_low = m_low.default_probability(5.0);
        let pd_high = m_high.default_probability(5.0);
        assert!(
            pd_high > pd_low,
            "Higher barrier growth should increase PD: g=0 -> {pd_low}, g=0.05 -> {pd_high}"
        );
    }

    /// Black-Cox (1976) growing-barrier regression .
    ///
    /// With V=120, B=80, sigma=0.25, r=0.06, q=0.01, g=0.03, T=5:
    ///   mu = 0.06 - 0.01 - 0.03125 = 0.01875, nu = mu - g = -0.01125,
    ///   x0 = ln(1.5) = 0.4054651,
    ///   d_plus  = (x0 + nu*T)/(sigma*sqrt(T)) =  0.624694,
    ///   d_minus = (x0 - nu*T)/(sigma*sqrt(T)) =  0.825941,
    ///   PD = N(-0.624694) + e^{0.1459674} * N(-0.825941) = 0.502635.
    #[test]
    fn first_passage_growing_barrier_matches_black_cox_1976() {
        let m = MertonModel::new_with_dynamics(
            120.0,
            0.25,
            80.0,
            0.06,
            0.01,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.03,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");

        let pd = m.default_probability(5.0);
        let expected = 0.502635;
        assert!(
            (pd - expected).abs() < 1e-3,
            "growing-barrier Black-Cox PD should be ~{expected}, got {pd}"
        );
    }

    /// A growing barrier B*e^{g t} only shifts the drift of the distance
    /// process: PD(V, B, sigma, r, q, g) == PD(V, B, sigma, r - g, q, 0)
    /// exactly. The pre-M4 formula violated this invariance for g != 0.
    #[test]
    fn first_passage_growing_barrier_equals_drift_shifted_flat_barrier() {
        let (v, sigma, b, r, q, g) = (120.0, 0.25, 80.0, 0.06, 0.01, 0.03);
        let m_growing = MertonModel::new_with_dynamics(
            v,
            sigma,
            b,
            r,
            q,
            BarrierType::FirstPassage {
                barrier_growth_rate: g,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        let m_shifted = MertonModel::new_with_dynamics(
            v,
            sigma,
            b,
            r - g,
            q,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");

        for horizon in [0.5, 1.0, 3.0, 5.0, 10.0] {
            let pd_g = m_growing.default_probability(horizon);
            let pd_s = m_shifted.default_probability(horizon);
            assert!(
                (pd_g - pd_s).abs() < 1e-12,
                "growing-barrier PD must equal drift-shifted flat-barrier PD at T={horizon}: \
                 {pd_g} vs {pd_s}"
            );
        }
    }

    #[test]
    fn implied_spread_monotonic_in_leverage() {
        let low_lev = MertonModel::new(100.0, 0.25, 40.0, 0.04).expect("valid");
        let mid_lev = MertonModel::new(100.0, 0.25, 70.0, 0.04).expect("valid");
        let high_lev = MertonModel::new(100.0, 0.25, 95.0, 0.04).expect("valid");

        let s_low = low_lev.implied_spread(5.0, 0.40).expect("spread");
        let s_mid = mid_lev.implied_spread(5.0, 0.40).expect("spread");
        let s_high = high_lev.implied_spread(5.0, 0.40).expect("spread");

        assert!(
            s_low < s_mid && s_mid < s_high,
            "Spread should increase with leverage: {s_low} < {s_mid} < {s_high}"
        );
    }

    // -----------------------------------------------------------------------
    // from_target_pd calibration tests
    // -----------------------------------------------------------------------

    #[test]
    fn from_target_pd_roundtrips() {
        let target_pd = 0.05; // 5% cumulative PD over 5 years
        let m = MertonModel::from_target_pd(200.0, 0.25, 0.04, target_pd, 5.0).expect("cal");
        let actual_pd = m.default_probability(5.0);
        assert!(
            (actual_pd - target_pd).abs() < 1e-6,
            "PD should match target: got {actual_pd}, want {target_pd}"
        );
    }

    #[test]
    fn from_target_pd_higher_pd_higher_barrier() {
        let m_low = MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.01, 5.0).expect("low");
        let m_high = MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.10, 5.0).expect("high");
        assert!(
            m_high.debt_barrier() > m_low.debt_barrier(),
            "Higher PD target should need higher barrier: low={}, high={}",
            m_low.debt_barrier(),
            m_high.debt_barrier()
        );
    }

    #[test]
    fn from_target_pd_realistic_credit_grades() {
        // BB: annual PD ~20bp → 5Y cumulative ~1.0%
        let bb_pd = 1.0 - (-0.0020_f64 * 5.0).exp();
        let m_bb = MertonModel::from_target_pd(200.0, 0.20, 0.045, bb_pd, 5.0).expect("BB");
        assert!(
            (m_bb.default_probability(5.0) - bb_pd).abs() < 1e-6,
            "BB PD mismatch"
        );

        // B: annual PD ~200bp → 5Y cumulative ~9.5%
        let b_pd = 1.0 - (-0.0200_f64 * 5.0).exp();
        let m_b = MertonModel::from_target_pd(140.0, 0.30, 0.045, b_pd, 5.0).expect("B");
        assert!(
            (m_b.default_probability(5.0) - b_pd).abs() < 1e-6,
            "B PD mismatch"
        );

        // CCC: annual PD ~400bp → 5Y cumulative ~18.1%
        let ccc_pd = 1.0 - (-0.0400_f64 * 5.0).exp();
        let m_ccc = MertonModel::from_target_pd(115.0, 0.40, 0.045, ccc_pd, 5.0).expect("CCC");
        assert!(
            (m_ccc.default_probability(5.0) - ccc_pd).abs() < 1e-6,
            "CCC PD mismatch"
        );

        // All calibrated barriers should be below asset value
        assert!(m_bb.debt_barrier() < 200.0);
        assert!(m_b.debt_barrier() < 140.0);
        assert!(m_ccc.debt_barrier() < 115.0);
    }

    #[test]
    fn from_target_pd_rejects_invalid_inputs() {
        assert!(MertonModel::from_target_pd(0.0, 0.25, 0.04, 0.05, 5.0).is_err());
        assert!(MertonModel::from_target_pd(200.0, 0.25, 0.04, 1.0, 5.0).is_err());
        assert!(MertonModel::from_target_pd(200.0, 0.25, 0.04, -0.01, 5.0).is_err());
    }

    // -----------------------------------------------------------------------
    // CreditGrades cross-checks
    // -----------------------------------------------------------------------

    #[test]
    fn credit_grades_asset_value_matches_formula() {
        // V_0 = E + D * R_mean
        let e = 25.0;
        let d = 80.0;
        let r_mean = 0.40;
        let m = MertonModel::credit_grades(e, 0.50, d, 0.04, 0.30, r_mean).expect("cg");
        let expected_v = e + d * r_mean;
        assert!(
            (m.asset_value() - expected_v).abs() < 1e-10,
            "V = E + D*R_mean = {expected_v}, got {}",
            m.asset_value()
        );
    }

    #[test]
    fn credit_grades_barrier_matches_formula() {
        // Barrier = D * R_mean
        let d = 80.0;
        let r_mean = 0.40;
        let m = MertonModel::credit_grades(25.0, 0.50, d, 0.04, 0.30, r_mean).expect("cg");
        let expected_barrier = d * r_mean;
        assert!(
            (m.debt_barrier() - expected_barrier).abs() < 1e-10,
            "Barrier = D*R_mean = {expected_barrier}, got {}",
            m.debt_barrier()
        );
    }

    #[test]
    fn credit_grades_asset_vol_matches_formula() {
        // sigma_V = sigma_E * E / V_0
        let e = 25.0;
        let sigma_e = 0.50;
        let d = 80.0;
        let r_mean = 0.40;
        let m = MertonModel::credit_grades(e, sigma_e, d, 0.04, 0.30, r_mean).expect("cg");
        let v0 = e + d * r_mean;
        let expected_sigma_v = sigma_e * e / v0;
        assert!(
            (m.asset_vol() - expected_sigma_v).abs() < 1e-10,
            "sigma_V = sigma_E * E / V_0 = {expected_sigma_v}, got {}",
            m.asset_vol()
        );
    }

    #[test]
    fn credit_grades_higher_equity_vol_higher_pd() {
        let m_low = MertonModel::credit_grades(25.0, 0.30, 80.0, 0.04, 0.30, 0.40).expect("cg");
        let m_high = MertonModel::credit_grades(25.0, 0.70, 80.0, 0.04, 0.30, 0.40).expect("cg");
        assert!(
            m_high.default_probability(5.0) > m_low.default_probability(5.0),
            "Higher equity vol should increase CG PD"
        );
    }

    /// `try_implied_equity` must reject a non-positive horizon explicitly
    /// (matching `implied_spread` and `simulate_paths`) rather than letting
    /// `horizon = 0` silently return intrinsic value with a meaningless
    /// "equity vol".
    #[test]
    fn try_implied_equity_rejects_non_positive_horizon() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        assert!(m.try_implied_equity(0.0).is_err(), "horizon=0 must error");
        assert!(
            m.try_implied_equity(-1.0).is_err(),
            "negative horizon must error"
        );
        assert!(
            m.try_implied_equity(f64::NAN).is_err(),
            "NaN horizon must error"
        );
    }

    /// The CreditGrades constructor must reject out-of-domain inputs instead
    /// of silently building a nonsensical model: `mean_recovery > 1` yields a
    /// barrier above face debt, and a negative `barrier_uncertainty` was
    /// previously clamped to 0 at compute time, silently discarding it.
    #[test]
    fn credit_grades_rejects_out_of_range_recovery_and_negative_lambda() {
        assert!(
            MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 1.5).is_err(),
            "mean_recovery > 1 must be rejected"
        );
        assert!(
            MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, -0.1).is_err(),
            "negative mean_recovery must be rejected"
        );
        assert!(
            MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, -0.30, 0.40).is_err(),
            "negative barrier_uncertainty must be rejected"
        );
    }

    /// Golden pin for the CreditGrades survival formula (Finger et al. 2002).
    ///
    /// Every other CreditGrades test asserts only structural properties
    /// (monotonicity, sensitivity), so a subtle regression — e.g. dropping
    /// the `exp(λ²)` leverage factor or the `λ²` term in `a_t²` — would pass
    /// them all. Reference values computed with an independent Python
    /// implementation of the Technical Document survival function
    /// `P = Φ(−A/2 + ln d/A) − d·Φ(−A/2 − ln d/A)` with `A² = σ_V²t + λ²`,
    /// `d = (V₀/B)·e^{λ²}`, and the constructor mapping `V₀ = E + D·R̄`,
    /// `σ_V = σ_E·E/V₀`, `B = D·R̄`:
    ///   E=25, σ_E=0.50, D=80, λ=0.30, R̄=0.40
    ///   → PD(1y) = 0.10002066946020438, PD(5y) = 0.3349985804601995.
    #[test]
    fn credit_grades_pd_matches_independent_finger_reference() {
        let m = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 0.40).expect("cg");
        let pd_1y = m.default_probability(1.0);
        let pd_5y = m.default_probability(5.0);
        // Tolerance 1e-9: the reference uses Python's erf-based normal CDF,
        // which differs from this crate's norm_cdf by O(1e-11) in the tails.
        // A structural regression (e.g. losing exp(λ²)) moves PD by O(1e-2).
        assert!(
            (pd_1y - 0.100_020_669_460_204_38).abs() < 1e-9,
            "CreditGrades 1y PD {pd_1y:.15} != independent reference 0.100020669460204"
        );
        assert!(
            (pd_5y - 0.334_998_580_460_199_5).abs() < 1e-9,
            "CreditGrades 5y PD {pd_5y:.15} != independent reference 0.334998580460200"
        );
    }

    #[test]
    fn credit_grades_barrier_uncertainty_affects_pd() {
        let low_lambda =
            MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.10, 0.40).expect("cg");
        let high_lambda =
            MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.60, 0.40).expect("cg");
        assert_ne!(
            low_lambda.default_probability(5.0),
            high_lambda.default_probability(5.0),
            "CreditGrades barrier uncertainty must feed the survival function"
        );
    }

    // -----------------------------------------------------------------------
    // Monte Carlo path simulation tests
    // -----------------------------------------------------------------------

    #[test]
    fn simulate_paths_deterministic_with_seed() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);
        let paths1 = m
            .simulate_paths(10, 60, 5.0, &mut rng1, false)
            .expect("paths");
        let paths2 = m
            .simulate_paths(10, 60, 5.0, &mut rng2, false)
            .expect("paths");
        assert_eq!(
            paths1.path(0),
            paths2.path(0),
            "Same seed should give same paths"
        );
    }

    #[test]
    fn simulate_paths_gbm_mean_converges() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(50_000, 60, 5.0, &mut rng, true)
            .expect("paths");
        let mean_terminal: f64 = paths
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>()
            / paths.num_paths as f64;
        let expected = 100.0 * (0.05_f64 * 5.0).exp();
        let rel_error = (mean_terminal - expected).abs() / expected;
        assert!(
            rel_error < 0.02,
            "Mean terminal should converge to E[V(T)] = V\u{2080}\u{00d7}e^(rT) = {expected}, got {mean_terminal}, rel_err={rel_error}"
        );
    }

    /// Jump-diffusion martingale check : the compensated
    /// drift uses kappa = E[e^J] - 1 = exp(mu_J + sigma_J^2/2) - 1, so the
    /// simulated jump multiplier must be e^J with J ~ N(mu_J, sigma_J^2)
    /// (mean exp(mu_J + sigma_J^2/2)). With that pairing,
    /// E[V(T)] = V0 * e^{(r-q)T} holds exactly.
    #[test]
    fn simulate_paths_jump_diffusion_mean_converges() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.05,
                jump_vol: 0.10,
            },
        )
        .expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(50_000, 60, 5.0, &mut rng, true)
            .expect("paths");
        let mean_terminal: f64 = paths
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>()
            / paths.num_paths as f64;
        let expected = 100.0 * (0.05_f64 * 5.0).exp();
        let rel_error = (mean_terminal - expected).abs() / expected;
        assert!(
            rel_error < 0.02,
            "JD mean terminal should converge to E[V(T)] = V0*e^(rT) = {expected}, \
             got {mean_terminal}, rel_err={rel_error}"
        );
    }

    #[test]
    fn simulate_paths_correct_dimensions() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let mut rng = Pcg64Rng::new(42);
        let paths = m
            .simulate_paths(100, 60, 5.0, &mut rng, false)
            .expect("paths");
        assert_eq!(paths.num_paths, 100);
        assert_eq!(paths.num_steps, 60);
        assert_eq!(paths.times.len(), 61); // includes t=0
        assert_eq!(paths.asset_values.len(), 100 * 61);
        assert_eq!(paths.values_per_path(), 61);
        assert!(
            (paths.times[0] - 0.0).abs() < 1e-10,
            "First time should be 0"
        );
        assert!(
            (paths.times[60] - 5.0).abs() < 1e-10,
            "Last time should be horizon"
        );
        assert!(
            (paths.get(0, 0).expect("path value") - 100.0).abs() < 1e-10,
            "Should start at V\u{2080}"
        );
    }

    #[test]
    fn jump_diffusion_produces_different_paths() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m_gbm = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        let m_jd = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.05,
                jump_vol: 0.10,
            },
        )
        .expect("valid");
        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);
        let paths_gbm = m_gbm
            .simulate_paths(100, 60, 5.0, &mut rng1, false)
            .expect("paths");
        let paths_jd = m_jd
            .simulate_paths(100, 60, 5.0, &mut rng2, false)
            .expect("paths");
        // JD paths should differ from GBM (different drift compensation + jumps)
        let gbm_terminal: f64 = paths_gbm
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>();
        let jd_terminal: f64 = paths_jd
            .iter_paths()
            .map(|p| *p.last().expect("non-empty"))
            .sum::<f64>();
        assert!(
            (gbm_terminal - jd_terminal).abs() > 1.0,
            "JD should produce different terminal values"
        );
    }

    /// Regression test for the Black-Cox first-passage reflection-term sign.
    ///
    /// When the risk-neutral log-drift mu = r - q - 0.5*sigma^2 is exactly 0,
    /// d_plus == d_minus == d = ln(V/H) / (sigma*sqrt(T)), so the correct
    /// Black-Cox formula collapses to:
    ///
    ///   PD = N(-d) + (V/H)^0 * N(-d) = 2*N(-d)
    ///
    /// With the original buggy code (N(+d_minus) instead of N(-d_minus)) the
    /// reflection term becomes N(d) = 1 - N(-d), yielding PD = 1.0 — certain
    /// default for a healthy, well-capitalised firm.
    #[test]
    fn first_passage_zero_drift_reflection_sign() {
        // Parameters chosen so mu = r - q - 0.5*sigma^2 = 0 exactly.
        // r = 0.5 * sigma^2, q = 0.0, sigma = 0.2  =>  r = 0.02.
        let sigma: f64 = 0.2;
        let r = 0.5 * sigma * sigma; // = 0.02
        let t = 3.0_f64;
        let v = 120.0_f64;
        let b = 100.0_f64;

        let model = MertonModel::new_with_dynamics(
            v,
            sigma,
            b,
            r,
            0.0, // payout_rate = 0
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid model");

        let pd = model.default_probability(t);

        // When mu = 0, H = B (no barrier growth), d = ln(V/B) / (sigma*sqrt(T)).
        let d = (v / b).ln() / (sigma * t.sqrt());
        let expected = 2.0 * norm_cdf(-d);

        assert!(
            (pd - expected).abs() < 1e-10,
            "Black-Cox zero-drift PD should be 2*N(-d) = {expected:.10}, got {pd:.10}"
        );
        assert!(
            pd < 1.0,
            "Healthy firm (V > B) with zero drift must have PD < 1.0, got {pd}"
        );
    }

    #[test]
    fn first_passage_nonzero_drift_reflection_sign() {
        // Parameters chosen so mu = r - q - 0.5*sigma^2 != 0.
        // r = 0.05, q = 0.01, sigma = 0.20  =>  mu = 0.05 - 0.01 - 0.02 = 0.02 (positive drift).
        let sigma: f64 = 0.20;
        let r = 0.05_f64;
        let q = 0.01_f64;
        let t = 3.0_f64;
        let v = 120.0_f64;
        let b = 100.0_f64;

        let model = MertonModel::new_with_dynamics(
            v,
            sigma,
            b,
            r,
            q,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid model");

        let pd = model.default_probability(t);

        // Closed-form reference (mu != 0, barrier_growth_rate = 0 => H = B).
        let mu = r - q - 0.5 * sigma * sigma; // = 0.02
        let sqrt_t = t.sqrt();
        let sigma_sqrt_t = sigma * sqrt_t;
        let log_v_h = (v / b).ln();
        let d_plus = (log_v_h + mu * t) / sigma_sqrt_t;
        let d_minus = (log_v_h - mu * t) / sigma_sqrt_t;
        let exponent = -2.0 * mu / (sigma * sigma);
        let expected = norm_cdf(-d_plus) + (v / b).powf(exponent) * norm_cdf(-d_minus);

        assert!(
            (pd - expected).abs() < 1e-10,
            "Black-Cox non-zero-drift PD should be {expected:.10}, got {pd:.10}"
        );
        assert!(
            pd < 1.0,
            "Healthy firm (V > B) with positive drift must have PD < 1.0, got {pd}"
        );
    }
}
