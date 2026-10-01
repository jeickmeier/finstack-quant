use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::{Error, Result};

use super::{AssetDynamics, MertonBarrierType, MertonModel};

/// Approximate CreditGrades survival probability (Finger et al. 2002).
///
/// `barrier_uncertainty` is the log-barrier volatility `λ` — the standard
/// deviation of the natural log of the default barrier, *not* a generic
/// uncertainty scalar. It enters the time-scaled variance as
/// `a_t² = σ²t + λ²` and shifts the effective leverage by `exp(λ²)`.
///
/// The CreditGrades asset process is **driftless** by construction, so
/// `risk_free_rate` and `payout_rate` play no part here.
///
/// `barrier_uncertainty` is guaranteed non-negative by
/// [`AssetDynamics::validate`], which every construction path runs.
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
    let lambda = barrier_uncertainty;
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

impl MertonModel {
    /// Diffusion log-drift of `ln(V_t)` for a caller-supplied total asset
    /// return.
    ///
    /// `total_return` is `r` under the risk-neutral measure and the expected
    /// physical asset return `mu` under the real-world measure. Under
    /// jump-diffusion the Poisson compensator `-lambda * kappa` is subtracted
    /// so that `E[V_T] = V_0 * exp((total_return - q) * T)` in both measures.
    #[inline]
    pub(super) fn log_drift(&self, total_return: f64) -> f64 {
        let sigma = self.asset_vol;
        let base = total_return - self.payout_rate - 0.5 * sigma * sigma;
        match self.dynamics {
            AssetDynamics::JumpDiffusion {
                jump_intensity,
                jump_mean,
                jump_vol,
            } => {
                let kappa = (jump_mean + 0.5 * jump_vol * jump_vol).exp() - 1.0;
                base - jump_intensity * kappa
            }
            AssetDynamics::GeometricBrownian | AssetDynamics::CreditGrades { .. } => base,
        }
    }

    /// Poisson-mixture decomposition of `ln(V_T / V_0)` at the terminal date.
    ///
    /// Returns `(weight, mean, variance)` triples such that, conditional on
    /// component `n`, `ln(V_T/V_0) ~ Normal(mean_n, variance_n)`. Geometric
    /// Brownian motion yields the single component
    /// `(1, mu*T, sigma^2*T)`; Merton (1976) jump-diffusion yields the
    /// Poisson mixture `w_n = e^{-lambda T}(lambda T)^n / n!` with
    /// `mean_n = mu*T + n*mu_J` and `variance_n = sigma^2*T + n*sigma_J^2`.
    ///
    /// The series is truncated once the remaining Poisson mass is
    /// negligible, so the weights sum to 1 within `f64` precision.
    pub(super) fn terminal_log_components(
        &self,
        log_drift: f64,
        horizon: f64,
    ) -> Vec<(f64, f64, f64)> {
        let sigma = self.asset_vol;
        let diffusion_var = sigma * sigma * horizon;
        let AssetDynamics::JumpDiffusion {
            jump_intensity,
            jump_mean,
            jump_vol,
        } = self.dynamics
        else {
            return vec![(1.0, log_drift * horizon, diffusion_var)];
        };

        let lambda_t = jump_intensity * horizon;
        // Poisson tail beyond mean + 10 standard deviations carries less than
        // 1e-15 of the mass, which is below the accuracy of `norm_cdf`.
        let n_max = (lambda_t + 10.0 * lambda_t.sqrt()).ceil().max(20.0) as usize;
        let mut components = Vec::with_capacity(n_max + 1);
        let mut weight = (-lambda_t).exp();
        for n in 0..=n_max {
            if n > 0 {
                weight *= lambda_t / n as f64;
            }
            components.push((
                weight,
                log_drift.mul_add(horizon, n as f64 * jump_mean),
                jump_vol.mul_add(jump_vol * n as f64, diffusion_var),
            ));
        }
        components
    }

    /// Terminal-barrier default probability for a given diffusion log-drift.
    fn terminal_pd(&self, log_drift: f64, horizon: f64) -> f64 {
        let log_moneyness = (self.asset_value / self.debt_barrier).ln();
        self.terminal_log_components(log_drift, horizon)
            .into_iter()
            .map(|(weight, mean, variance)| {
                weight * norm_cdf(-(log_moneyness + mean) / variance.sqrt())
            })
            .sum()
    }

    /// Black-Cox first-passage default probability for a given diffusion
    /// log-drift and barrier growth rate.
    fn first_passage_pd(&self, log_drift: f64, barrier_growth_rate: f64, horizon: f64) -> f64 {
        let sigma = self.asset_vol;
        let sigma_sqrt_t = sigma * horizon.sqrt();

        // Reduce to a flat barrier at 0: the distance process
        // X_t = ln(V_t / (B e^{g t})) is BM with drift nu = mu - g started at
        // x0 = ln(V/B). The growing barrier shifts the *drift*, not the
        // starting distance.
        let nu = log_drift - barrier_growth_rate;
        let x0 = (self.asset_value / self.debt_barrier).ln();

        let d_plus = (x0 + nu * horizon) / sigma_sqrt_t;
        let d_minus = (x0 - nu * horizon) / sigma_sqrt_t;

        // Black-Cox reflection term `exp(-2*nu*x0/sigma^2) * N(-d_minus)`.
        // The exponential factor overflows to `+inf` for a large
        // `|exponent|` (e.g. a strongly negative drift, or a low vol with a
        // high rate). When `N(-d_minus)` simultaneously underflows to `0` the
        // naive product is `inf * 0 = NaN`, which would survive the final
        // `clamp(0, 1)`.
        //
        // The Gaussian tail `N(-d_minus)` decays as `exp(-d_minus^2/2)`,
        // which dominates the (at most exponential-in-`d_minus`) power
        // factor, so the term tends to `0` whenever `N(-d_minus)` does.
        // Guard that case, then evaluate the surviving product in log-space
        // so a genuinely large term overflows cleanly to `+inf` (and clamps
        // to `1`) instead of producing a `NaN`.
        let exponent = -2.0 * nu / (sigma * sigma);
        let nd_minus = norm_cdf(-d_minus);
        let reflection_term = if nd_minus <= 0.0 {
            0.0
        } else {
            (exponent * x0 + nd_minus.ln()).exp()
        };

        let pd = norm_cdf(-d_plus) + reflection_term;
        if pd.is_nan() {
            return 0.0;
        }
        pd.clamp(0.0, 1.0)
    }

    /// Default probability for a given diffusion log-drift, dispatching on
    /// the barrier type. Not valid for `CreditGrades` dynamics, which are
    /// driftless and carry their own survival function.
    fn pd_from_log_drift(&self, log_drift: f64, horizon: f64) -> f64 {
        if horizon <= 0.0 {
            return 0.0;
        }
        match self.barrier_type {
            MertonBarrierType::Terminal => self.terminal_pd(log_drift, horizon),
            MertonBarrierType::FirstPassage {
                barrier_growth_rate,
            } => self.first_passage_pd(log_drift, barrier_growth_rate, horizon),
        }
    }

    /// Risk-neutral distance-to-default over the given horizon.
    ///
    /// DD = (ln(V/B) + (r - q - sigma^2/2) * T) / (sigma * sqrt(T))
    ///
    /// A higher DD indicates a lower probability of default.
    /// Returns `f64::INFINITY` when `horizon <= 0` (no time = infinite DD,
    /// yielding PD = N(-∞) = 0).
    ///
    /// # Measure
    ///
    /// This is the **risk-neutral (Q-measure)** `d2`, driven by the risk-free
    /// rate. It is *not* the Moody's KMV distance-to-default, which uses the
    /// firm's expected physical asset return and a default point of
    /// short-term debt plus half of long-term debt. Use
    /// [`Self::distance_to_default_with_drift`] together with
    /// [`Self::kmv_default_point`] for the KMV/EDF quantity.
    ///
    /// # Dynamics
    ///
    /// DD is a pure-diffusion statistic. Under `JumpDiffusion` the drift
    /// includes the Poisson compensator, so `N(-DD)` is the zero-jump term of
    /// the default probability rather than the default probability itself;
    /// call [`Self::default_probability`] for that. Under `CreditGrades` the
    /// survival law is driftless and does not factor through DD at all, so
    /// `N(-DD)` again differs from [`Self::default_probability`].
    ///
    /// # Arguments
    ///
    /// * `horizon` - Time horizon T in years from the valuation date; a
    ///   non-positive horizon returns `+∞`
    #[inline]
    pub fn distance_to_default(&self, horizon: f64) -> f64 {
        if horizon <= 0.0 {
            return f64::INFINITY;
        }
        let sqrt_t = horizon.sqrt();
        ((self.asset_value / self.debt_barrier).ln()
            + self.log_drift(self.risk_free_rate) * horizon)
            / (self.asset_vol * sqrt_t)
    }

    /// Physical-measure (Moody's KMV) distance-to-default over the given
    /// horizon.
    ///
    /// DD = (ln(V/B) + (mu - q - sigma^2/2) * T) / (sigma * sqrt(T))
    ///
    /// This is the KMV/EDF construction: the risk-free rate is replaced by
    /// the firm's expected physical asset return, so the result measures how
    /// many standard deviations of one-year asset return separate the firm
    /// from its default point under the real-world measure. Pair it with
    /// [`Self::kmv_default_point`] to reproduce the KMV default point
    /// convention when building the model.
    ///
    /// # Arguments
    ///
    /// * `asset_drift` - Expected **physical** total return on the firm's
    ///   assets `mu`, continuously compounded and expressed as a decimal
    ///   fraction (`0.09` is 9% per annum). This replaces the risk-free rate
    ///   in the drift; the model's `payout_rate` is still subtracted.
    /// * `horizon` - Time horizon T in years from the valuation date; a
    ///   non-positive horizon returns `+∞`
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `asset_drift` is not finite, or if
    /// the model uses `CreditGrades` dynamics, whose survival function is
    /// driftless by construction and therefore has no physical-measure
    /// counterpart.
    pub fn distance_to_default_with_drift(&self, asset_drift: f64, horizon: f64) -> Result<f64> {
        self.check_drift_supported(asset_drift, "distance_to_default_with_drift")?;
        if horizon <= 0.0 {
            return Ok(f64::INFINITY);
        }
        let sqrt_t = horizon.sqrt();
        Ok(
            ((self.asset_value / self.debt_barrier).ln() + self.log_drift(asset_drift) * horizon)
                / (self.asset_vol * sqrt_t),
        )
    }

    /// Reject drift-parameterised queries the configured dynamics cannot
    /// answer.
    fn check_drift_supported(&self, asset_drift: f64, method: &str) -> Result<()> {
        if !asset_drift.is_finite() {
            return Err(Error::Validation(format!(
                "{method}: asset_drift must be finite, got {asset_drift}"
            )));
        }
        if matches!(self.dynamics, AssetDynamics::CreditGrades { .. }) {
            return Err(Error::Validation(format!(
                "{method}: CreditGrades dynamics are driftless by construction \
                 (Finger et al. 2002), so no physical-measure distance-to-default \
                 or EDF is defined. Use GeometricBrownian dynamics for KMV/EDF work."
            )));
        }
        Ok(())
    }

    /// Risk-neutral default probability over the given horizon.
    ///
    /// # Measure
    ///
    /// This is the **risk-neutral (Q-measure)** default probability: the
    /// drift is the risk-free `r − q − σ²/2`, not the firm's real-world
    /// asset drift. It is the right quantity for pricing and credit-spread
    /// work ([`Self::implied_spread`], [`Self::cds_par_spread`],
    /// [`Self::to_hazard_curve`]), but it materially **overstates** the
    /// physical/real-world PD (EDF) whenever the market price of asset risk
    /// is positive — often by several times for a healthy firm. For
    /// expected-loss, capital, or rating analytics use
    /// [`Self::default_probability_with_drift`], which substitutes the
    /// physical asset drift.
    ///
    /// - **Terminal barrier, `GeometricBrownian`**: PD = N(-DD) (Merton 1974).
    /// - **Terminal barrier, `JumpDiffusion`**: the Merton (1976) Poisson
    ///   mixture. Conditional on `n` jumps the terminal log-asset value is
    ///   Gaussian with mean shifted by `n * mu_J` and variance inflated by
    ///   `n * sigma_J^2`, so
    ///
    ///   PD = Σ_n e^{-λT}(λT)^n/n! · N(-(ln(V/B) + (r-q-λκ-σ²/2)T + n·mu_J) / sqrt(σ²T + n·sigma_J²))
    ///
    ///   with `κ = exp(mu_J + sigma_J²/2) - 1`. `JumpDiffusion` is rejected
    ///   with a first-passage barrier at construction time.
    /// - **First-passage barrier**: Black-Cox (1976) closed-form with
    ///   exponentially growing barrier `B(t) = B * exp(g * t)`. The distance
    ///   process `X_t = ln(V_t / B(t))` is Brownian motion with drift
    ///   `nu = mu - g` (where `mu = r - q - sigma^2/2`) started at
    ///   `x0 = ln(V/B)`, and the first-passage probability to 0 is
    ///
    ///   PD = N(-(x0 + nu*T) / (sigma*sqrt(T)))
    ///      + exp(-2*nu*x0 / sigma^2) * N((-x0 + nu*T) / (sigma*sqrt(T)))
    ///
    ///   The first-passage result is clamped to `[0, 1]` to absorb
    ///   floating-point overshoot when the drift is strongly negative.
    /// - **`CreditGrades`**: the Finger et al. (2002) approximate survival
    ///   function, which is driftless and therefore ignores `r` and `q`.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Time horizon T in years from the valuation date; a
    ///   non-positive horizon returns 0
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
        self.pd_from_log_drift(self.log_drift(self.risk_free_rate), horizon)
    }

    /// Risk-neutral default probabilities over a grid of horizons.
    ///
    /// Evaluates [`Self::default_probability`] at each horizon, in input
    /// order, so hosts build a PD term structure with one call.
    ///
    /// # Arguments
    ///
    /// * `horizons` - Time horizons in years from the valuation date; a
    ///   non-positive horizon maps to 0. An empty slice returns an empty
    ///   vector.
    pub fn default_probabilities(&self, horizons: &[f64]) -> Vec<f64> {
        horizons
            .iter()
            .map(|horizon| self.default_probability(*horizon))
            .collect()
    }

    /// Physical-measure default probability (theoretical EDF) over the given
    /// horizon.
    ///
    /// Identical dispatch to [`Self::default_probability`], with the firm's
    /// expected physical asset return substituted for the risk-free rate.
    /// This is the "theoretical EDF" of the Moody's KMV framework: the
    /// real-world probability that the asset value ends below (or, with a
    /// first-passage barrier, ever touches) the default point. Moody's
    /// published EDF applies a further proprietary empirical mapping from
    /// distance-to-default to observed default frequency, which is not
    /// reproduced here.
    ///
    /// # Arguments
    ///
    /// * `asset_drift` - Expected **physical** total return on the firm's
    ///   assets `mu`, continuously compounded and expressed as a decimal
    ///   fraction (`0.09` is 9% per annum). Replaces the risk-free rate in
    ///   the drift; the model's `payout_rate` is still subtracted, and under
    ///   `JumpDiffusion` the Poisson compensator still applies.
    /// * `horizon` - Time horizon T in years from the valuation date; a
    ///   non-positive horizon returns 0
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `asset_drift` is not finite, or if
    /// the model uses `CreditGrades` dynamics, whose survival function is
    /// driftless by construction.
    pub fn default_probability_with_drift(&self, asset_drift: f64, horizon: f64) -> Result<f64> {
        self.check_drift_supported(asset_drift, "default_probability_with_drift")?;
        Ok(self.pd_from_log_drift(self.log_drift(asset_drift), horizon))
    }

    /// Moody's KMV default point: short-term debt plus half of long-term
    /// debt.
    ///
    /// The KMV framework does not use total liabilities as the default
    /// barrier. Empirically firms default when asset value falls to roughly
    /// current liabilities plus half of long-term liabilities, because
    /// long-dated debt does not have to be repaid immediately. Feed the
    /// result in as `debt_barrier` when building a model for KMV/EDF work.
    ///
    /// # Arguments
    ///
    /// * `short_term_debt` - Book value of debt and other liabilities due
    ///   within one year, in the issuer's reporting currency. Must be finite
    ///   and non-negative.
    /// * `long_term_debt` - Book value of debt maturing beyond one year, in
    ///   the same currency. Must be finite and non-negative; exactly half of
    ///   it enters the default point.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if either input is non-finite or
    /// negative, or if the resulting default point is zero (a firm with no
    /// liabilities has no default point).
    pub fn kmv_default_point(short_term_debt: f64, long_term_debt: f64) -> Result<f64> {
        if !(short_term_debt.is_finite() && short_term_debt >= 0.0) {
            return Err(Error::Validation(format!(
                "kmv_default_point: short_term_debt must be finite and >= 0, \
                 got {short_term_debt}"
            )));
        }
        if !(long_term_debt.is_finite() && long_term_debt >= 0.0) {
            return Err(Error::Validation(format!(
                "kmv_default_point: long_term_debt must be finite and >= 0, \
                 got {long_term_debt}"
            )));
        }
        let default_point = 0.5f64.mul_add(long_term_debt, short_term_debt);
        if default_point <= 0.0 {
            return Err(Error::Validation(
                "kmv_default_point: short_term_debt + 0.5 * long_term_debt must be > 0; \
                 a firm with no liabilities has no default point"
                    .to_string(),
            ));
        }
        Ok(default_point)
    }
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::math::norm_cdf;

    use super::super::{AssetDynamics, MertonBarrierType, MertonModel};

    #[test]
    fn default_probabilities_match_the_scalar_method_in_order() {
        let m = MertonModel::new(100.0, 0.20, 80.0, 0.05).unwrap();
        let horizons = [5.0, 0.0, 1.0];
        let grid = m.default_probabilities(&horizons);
        let scalar: Vec<f64> = horizons.iter().map(|h| m.default_probability(*h)).collect();
        assert_eq!(grid, scalar);
        assert!(m.default_probabilities(&[]).is_empty());
    }

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
            MertonBarrierType::FirstPassage {
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

    // Extreme parameter edge cases

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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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

    // Jump-diffusion default probability

    fn jump_diffusion_model(jump_mean: f64) -> MertonModel {
        MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            MertonBarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean,
                jump_vol: 0.15,
            },
        )
        .expect("valid")
    }

    #[test]
    fn jump_diffusion_pd_matches_monte_carlo() {
        use finstack_quant_core::math::random::Pcg64Rng;
        let m = jump_diffusion_model(-0.30);
        let analytic = m.default_probability(2.0);

        let mut rng = Pcg64Rng::new(7);
        let paths = m
            .simulate_paths(200_000, 200, 2.0, &mut rng, true)
            .expect("paths");
        let defaults = paths
            .iter_paths()
            .filter(|p| *p.last().expect("non-empty") < m.debt_barrier())
            .count();
        let empirical = defaults as f64 / paths.num_paths as f64;

        assert!(
            (analytic - empirical).abs() < 0.005,
            "Merton-1976 mixture PD {analytic} should match simulated terminal PD {empirical}"
        );
    }

    #[test]
    fn jump_diffusion_pd_exceeds_diffusion_only_pd() {
        // Downward jumps add left-tail mass that a pure diffusion of the same
        // sigma cannot produce.
        let jd = jump_diffusion_model(-0.30);
        let gbm = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        assert!(
            jd.default_probability(2.0) > gbm.default_probability(2.0) * 1.5,
            "jump PD {} should materially exceed GBM PD {}",
            jd.default_probability(2.0),
            gbm.default_probability(2.0)
        );
    }

    #[test]
    fn jump_diffusion_pd_collapses_to_gbm_without_jumps() {
        let jd = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            MertonBarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.0,
                jump_mean: -0.30,
                jump_vol: 0.15,
            },
        )
        .expect("valid");
        let gbm = MertonModel::new(100.0, 0.20, 80.0, 0.05).expect("valid");
        assert!((jd.default_probability(3.0) - gbm.default_probability(3.0)).abs() < 1e-12);
    }

    // Physical measure (Moody's KMV / EDF)

    #[test]
    fn physical_dd_matches_risk_neutral_dd_at_the_risk_free_rate() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let physical = m
            .distance_to_default_with_drift(0.04, 3.0)
            .expect("supported");
        assert!((physical - m.distance_to_default(3.0)).abs() < 1e-12);
    }

    #[test]
    fn physical_pd_below_risk_neutral_pd_for_a_positive_risk_premium() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let edf = m
            .default_probability_with_drift(0.11, 5.0)
            .expect("supported");
        assert!(
            edf < m.default_probability(5.0),
            "EDF {edf} must be below the risk-neutral PD {}",
            m.default_probability(5.0)
        );
        assert!(edf > 0.0);
    }

    #[test]
    fn physical_measure_rejects_credit_grades() {
        let m = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.04, 0.30, 0.40).expect("cg");
        assert!(m.distance_to_default_with_drift(0.09, 1.0).is_err());
        assert!(m.default_probability_with_drift(0.09, 1.0).is_err());
    }

    #[test]
    fn kmv_default_point_is_short_term_plus_half_long_term_debt() {
        let dp = MertonModel::kmv_default_point(40.0, 120.0).expect("valid");
        assert!((dp - 100.0).abs() < 1e-12);
        assert!(MertonModel::kmv_default_point(-1.0, 10.0).is_err());
        assert!(MertonModel::kmv_default_point(0.0, 0.0).is_err());
    }

    #[test]
    fn kmv_edf_workflow_produces_a_plausible_one_year_default_rate() {
        // Textbook KMV setup: equity inversion for (V, sigma_V), the
        // short-term-plus-half-long-term default point, then a physical drift.
        let default_point = MertonModel::kmv_default_point(30.0, 60.0).expect("valid");
        let m = MertonModel::from_equity(120.0, 0.40, default_point, 0.04, 0.0, 1.0)
            .expect("kmv inversion");
        let dd = m
            .distance_to_default_with_drift(0.09, 1.0)
            .expect("supported");
        let edf = m
            .default_probability_with_drift(0.09, 1.0)
            .expect("supported");
        assert!(dd > 0.0, "a solvent firm should have positive DD, got {dd}");
        assert!((edf - norm_cdf(-dd)).abs() < 1e-12);
    }

    #[test]
    fn credit_grades_survival_ignores_the_risk_free_rate() {
        // The CreditGrades process is driftless, so the rate must not move PD.
        let low = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.00, 0.30, 0.40).expect("cg");
        let high = MertonModel::credit_grades(25.0, 0.50, 80.0, 0.20, 0.30, 0.40).expect("cg");
        assert!((low.default_probability(5.0) - high.default_probability(5.0)).abs() < 1e-15);
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
            MertonBarrierType::FirstPassage {
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
}
