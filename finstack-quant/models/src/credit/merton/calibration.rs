use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::{Error, InputError, Result};

use super::{AssetDynamics, BarrierType, MertonModel};

impl MertonModel {
    /// Minimum equity value **as a fraction of firm value** for which the
    /// equity-vol relation `sigma_E = N(d1) * exp(-qT) * sigma_V * V / E` is
    /// numerically well posed. Below this, `E` is treated as effectively zero
    /// (the firm is economically in default) and the division is rejected.
    ///
    /// The threshold is relative rather than absolute so that it behaves the
    /// same for a firm reporting in units and one reporting in billions.
    const MIN_EQUITY_FRACTION: f64 = 1.0e-10;

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
    /// # Dynamics
    ///
    /// Diffusion-only. `JumpDiffusion` is rejected because the returned
    /// `sigma_E` is the delta-scaled *diffusive* equity volatility, which is
    /// not the observable equity volatility once jumps contribute variance;
    /// pairing them would silently corrupt a KMV inversion.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Time horizon T in years from the valuation date over
    ///   which equity is treated as a call on the firm's assets; must be
    ///   finite and strictly positive
    ///
    /// # Returns
    ///
    /// A tuple `(equity_value, equity_vol)`, the first in the same currency
    /// as `asset_value` and the second an annualized decimal fraction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `horizon` is not positive or the
    /// model uses `JumpDiffusion` dynamics, and [`InputError::Invalid`] if
    /// the implied equity value or `N(d1)` is below the well-posed floor (the
    /// firm is economically in default).
    pub fn try_implied_equity(&self, horizon: f64) -> Result<(f64, f64)> {
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "try_implied_equity: horizon must be > 0, got {horizon}"
            )));
        }
        if matches!(self.dynamics, AssetDynamics::JumpDiffusion { .. }) {
            return Err(Error::Validation(
                "try_implied_equity: the Black-Scholes equity-vol inversion is \
                 diffusion-only; under JumpDiffusion the delta-scaled volatility is \
                 the diffusive component alone and would misstate observed equity \
                 volatility. Use GeometricBrownian dynamics for equity calibration."
                    .to_string(),
            ));
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

        if !equity.is_finite() || equity <= Self::MIN_EQUITY_FRACTION * v || nd1 <= Self::MIN_ND1 {
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
    /// * `equity_value` - Observed market capitalization E, strictly positive
    ///   and in the same currency as `total_debt`
    /// * `equity_vol` - Observed annualized equity volatility `sigma_E` as a
    ///   decimal fraction (`0.35` is 35%); must be non-negative
    /// * `total_debt` - Face value of debt B acting as the option strike, in
    ///   the same currency as `equity_value` and strictly positive. Use
    ///   [`Self::kmv_default_point`] for the Moody's KMV default-point
    ///   convention
    /// * `risk_free_rate` - Continuously compounded risk-free rate r as a
    ///   decimal fraction
    /// * `payout_rate` - Continuous dividend / payout yield q on assets as a
    ///   decimal fraction
    /// * `maturity` - Debt horizon T in years, conventionally 1.0 in KMV
    ///   practice; must be strictly positive
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] or [`InputError::NegativeValue`]
    /// for out-of-domain inputs, [`InputError::Invalid`] when equity is a
    /// negligible fraction of firm value or `N(d1)` collapses to zero
    /// (deep-out-of-the-money, economically in default), and
    /// [`InputError::SolverConvergenceFailed`] if the fixed point does not
    /// settle within 100 iterations.
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
        // convergence test. Reject it up front with a descriptive error. The
        // test is relative to the initial firm-value estimate so it scales
        // with the reporting units.
        if equity_value <= Self::MIN_EQUITY_FRACTION * (equity_value + total_debt) {
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

        let mut v = e + b;
        let mut sigma_v = sigma_e * e / v;

        let max_iter = 100;
        let tol = 1e-8;

        for _ in 0..max_iter {
            let v_prev = v;
            let sigma_v_prev = sigma_v;

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

            // E = V*exp(-qT)*N(d1) - B*exp(-rT)*N(d2)
            v = (e + b * (-r * t).exp() * nd2) / (exp_neg_qt * nd1);
            sigma_v = sigma_e * e / (nd1 * exp_neg_qt * v);

            // Both unknowns must settle. Testing V alone can exit while
            // sigma_V is still moving, because V is far less sensitive to
            // sigma_V than the reverse near the fixed point.
            let v_converged = ((v - v_prev) / v_prev).abs() < tol;
            let sigma_converged = ((sigma_v - sigma_v_prev) / sigma_v_prev).abs() < tol;
            if v_converged && sigma_converged {
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

    /// Lowest asset volatility considered when calibrating to a CDS spread.
    const CDS_CALIBRATION_MIN_VOL: f64 = 0.01;

    /// Highest asset volatility considered when calibrating to a CDS spread.
    const CDS_CALIBRATION_MAX_VOL: f64 = 2.0;

    /// Number of scan points used to locate sign changes of the CDS
    /// calibration objective before handing an interval to Brent.
    const CDS_CALIBRATION_SCAN_POINTS: usize = 128;

    /// CDS spread calibration: find the asset volatility that reproduces a
    /// quoted CDS par spread.
    ///
    /// The objective is the model's [`cds_par_spread`](Self::cds_par_spread),
    /// i.e. an ISDA-style par spread built from the model's survival curve
    /// with a premium leg, accrual on default, and discounting — not the
    /// zero-coupon approximation of [`implied_spread`](Self::implied_spread),
    /// which understates a quoted spread by several percent at distressed
    /// levels. The calibrated model has `BarrierType::Terminal` with
    /// `AssetDynamics::GeometricBrownian` and the supplied `payout_rate`.
    ///
    /// # Multiple solutions
    ///
    /// The par spread is **not** monotonic in asset volatility. For a firm
    /// whose risk-neutral forward asset value is below the barrier, raising
    /// volatility first *lowers* the default probability (more upside to
    /// recover) before raising it, so a quoted spread can be consistent with
    /// two different volatilities. Rather than returning whichever root a
    /// bracketing solver happens to land on, the objective is scanned across
    /// `[0.01, 2.0]`; a unique sign change is refined with Brent's method,
    /// and zero or several sign changes produce a descriptive error naming
    /// the attainable spread range or the competing intervals.
    ///
    /// To use first-passage barriers after calibration, construct a new
    /// model via [`new_with_dynamics`](Self::new_with_dynamics) using the
    /// calibrated `asset_vol`.
    ///
    /// # Arguments
    ///
    /// * `cds_spread_bp` - Quoted CDS par spread in basis points (`150.0` is
    ///   150 bp); must be finite and strictly positive
    /// * `recovery` - Recovery rate as a decimal fraction of notional
    ///   (`0.40` is the senior-unsecured market convention); must lie in
    ///   `[0, 1]` and be strictly below 1, since a full-recovery contract has
    ///   a zero spread at every volatility
    /// * `total_debt` - Face value of debt B acting as the default barrier,
    ///   in the same currency as `asset_value` and strictly positive
    /// * `risk_free_rate` - Continuously compounded discount rate r as a
    ///   decimal fraction, used for both legs
    /// * `maturity` - CDS maturity T in years (`5.0` for the benchmark
    ///   point); must be strictly positive
    /// * `asset_value` - Assumed initial firm asset value V, held fixed
    ///   during the solve; must be strictly positive
    /// * `payout_rate` - Continuous dividend / payout yield q on assets as a
    ///   decimal fraction (pass `0.0` for a firm with no asset payout)
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] for non-positive
    /// `total_debt`, `maturity`, or `asset_value`, and [`Error::Validation`]
    /// if `recovery` or `cds_spread_bp` is out of range, if no volatility in
    /// `[0.01, 2.0]` reproduces the quote, or if the quote is consistent with
    /// more than one volatility. Propagates solver failures from Brent's
    /// method.
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
        if !(0.0..1.0).contains(&recovery) {
            return Err(Error::Validation(format!(
                "from_cds_spread: recovery must be in [0, 1), got {recovery}; a \
                 full-recovery contract has a zero spread at every volatility"
            )));
        }
        if !(cds_spread_bp.is_finite() && cds_spread_bp > 0.0) {
            return Err(Error::Validation(format!(
                "from_cds_spread: cds_spread_bp must be finite and > 0, got {cds_spread_bp}"
            )));
        }

        let target_spread = cds_spread_bp / 10_000.0;
        let objective = |sigma: f64| -> Result<f64> {
            let trial = Self::new_with_dynamics(
                asset_value,
                sigma,
                total_debt,
                risk_free_rate,
                payout_rate,
                BarrierType::Terminal,
                AssetDynamics::GeometricBrownian,
            )?;
            Ok(trial.cds_par_spread(maturity, recovery)? - target_spread)
        };

        let sigma_v = Self::solve_unique_root(&objective, cds_spread_bp, maturity)?;

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

    /// Locate the single volatility in `[CDS_CALIBRATION_MIN_VOL,
    /// CDS_CALIBRATION_MAX_VOL]` where `objective` changes sign, refusing to
    /// guess when the root is not unique.
    ///
    /// Some volatilities have no par spread at all: at the low end of the
    /// range a terminal-barrier firm's default probability is a vanishing
    /// number that *falls* with horizon (the drift outruns the diffusion), so
    /// it is not a survival curve and the hazard bootstrap rejects it. Those
    /// points are skipped rather than aborting the scan — they carry no
    /// credit risk and so cannot match a positive quote — and the count is
    /// reported if the scan ends up finding nothing.
    fn solve_unique_root(
        objective: &dyn Fn(f64) -> Result<f64>,
        cds_spread_bp: f64,
        maturity: f64,
    ) -> Result<f64> {
        let lo = Self::CDS_CALIBRATION_MIN_VOL;
        let hi = Self::CDS_CALIBRATION_MAX_VOL;
        let steps = Self::CDS_CALIBRATION_SCAN_POINTS;
        let step = (hi - lo) / steps as f64;

        let mut brackets: Vec<(f64, f64)> = Vec::new();
        let mut previous: Option<(f64, f64)> = None;
        let mut span: Option<(f64, f64)> = None;
        let mut skipped = 0usize;
        for i in 0..=steps {
            let sigma = lo + i as f64 * step;
            let Ok(value) = objective(sigma) else {
                skipped += 1;
                previous = None;
                continue;
            };
            span = Some(match span {
                Some((min, max)) => (min.min(value), max.max(value)),
                None => (value, value),
            });
            if value == 0.0 {
                return Ok(sigma);
            }
            if let Some((prev_sigma, prev_value)) = previous {
                if (prev_value < 0.0) != (value < 0.0) {
                    brackets.push((prev_sigma, sigma));
                }
            }
            previous = Some((sigma, value));
        }

        let Some((min_spread, max_spread)) = span else {
            return Err(Error::Validation(format!(
                "from_cds_spread: no asset volatility in [{lo}, {hi}] produces a \
                 usable survival curve at {maturity}y, so a {cds_spread_bp} bp quote \
                 cannot be matched; check the assumed asset value, debt barrier, and \
                 maturity."
            )));
        };

        match brackets.len() {
            1 => {
                let (bracket_lo, bracket_hi) = brackets[0];
                BrentSolver::new()
                    .tolerance(1e-10)
                    .bracket_bounds(bracket_lo, bracket_hi)
                    .solve(
                        |sigma| objective(sigma).unwrap_or(f64::NAN),
                        0.5 * (bracket_lo + bracket_hi),
                    )
            }
            0 => Err(Error::Validation(format!(
                "from_cds_spread: no asset volatility in [{lo}, {hi}] reproduces a \
                 {cds_spread_bp} bp spread at {maturity}y ({skipped} of {} scanned \
                 volatilities carry no credit risk at all). Attainable model spreads \
                 span {:.2} to {:.2} bp over that range; check the assumed asset \
                 value, debt barrier, and recovery.",
                steps + 1,
                (min_spread + cds_spread_bp / 10_000.0) * 10_000.0,
                (max_spread + cds_spread_bp / 10_000.0) * 10_000.0,
            ))),
            n => Err(Error::Validation(format!(
                "from_cds_spread: a {cds_spread_bp} bp spread at {maturity}y is \
                 consistent with {n} distinct asset volatilities (the par spread is \
                 non-monotonic in volatility for a firm below its barrier). \
                 Candidate brackets: {brackets:?}. Pin down the volatility from \
                 equity data instead, via from_equity."
            ))),
        }
    }

    /// Calibrate the debt barrier to match a target cumulative default
    /// probability over the given maturity.
    ///
    /// Uses terminal-barrier (classic Merton) `PD = N(-DD)` and Brent's
    /// method to find the barrier B such that `default_probability(maturity)
    /// == target_pd`. PD is strictly increasing in the barrier, so the root
    /// is unique.
    ///
    /// # Measure
    ///
    /// `target_pd` is interpreted under the **risk-neutral** measure, since
    /// the drift is `risk_free_rate - payout_rate - sigma^2/2`. To calibrate
    /// to a physical PD (a rating-implied or EDF-style default rate), pass
    /// the firm's expected physical asset return as `risk_free_rate`; the
    /// resulting barrier then reproduces that PD through
    /// [`default_probability_with_drift`](Self::default_probability_with_drift).
    ///
    /// # Arguments
    ///
    /// * `asset_value` - Current firm asset value V in the issuer's
    ///   reporting currency; must be strictly positive
    /// * `asset_vol` - Annualized asset volatility `sigma_V` as a decimal
    ///   fraction; must be strictly positive, since a zero-volatility firm
    ///   has a degenerate step-function PD that cannot hit an interior target
    /// * `risk_free_rate` - Continuously compounded risk-free rate r as a
    ///   decimal fraction
    /// * `payout_rate` - Continuous dividend / payout yield q on assets as a
    ///   decimal fraction. Omitting it (passing `0.0`) shifts the calibrated
    ///   barrier whenever the model is later evaluated with a non-zero payout
    /// * `target_pd` - Target cumulative default probability over `maturity`
    ///   as a decimal fraction (`0.01` is 1%); must lie in `(0, 1)`
    /// * `maturity` - Time horizon T in years; must be strictly positive
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] if `asset_value`,
    /// `asset_vol`, or `maturity` is non-positive, [`InputError::Invalid`] if
    /// `target_pd` is outside `(0, 1)`, and a solver error if no barrier in
    /// `[0.001 V, 0.999 V]` attains the target.
    pub fn from_target_pd(
        asset_value: f64,
        asset_vol: f64,
        risk_free_rate: f64,
        payout_rate: f64,
        target_pd: f64,
        maturity: f64,
    ) -> Result<Self> {
        if asset_value <= 0.0 || asset_vol <= 0.0 || maturity <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        if !(0.0..1.0).contains(&target_pd) || target_pd <= 0.0 {
            return Err(InputError::Invalid.into());
        }

        let solver = BrentSolver::new()
            .tolerance(1e-10)
            .bracket_bounds(0.001 * asset_value, 0.999 * asset_value);

        let barrier = solver.solve(
            |b| {
                let sigma = asset_vol;
                let mu = risk_free_rate - payout_rate - 0.5 * sigma * sigma;
                let sqrt_t = maturity.sqrt();
                let dd = ((asset_value / b).ln() + mu * maturity) / (sigma * sqrt_t);
                norm_cdf(-dd) - target_pd
            },
            0.5 * asset_value,
        )?;

        Self::new_with_dynamics(
            asset_value,
            asset_vol,
            barrier,
            risk_free_rate,
            payout_rate,
            BarrierType::Terminal,
            AssetDynamics::GeometricBrownian,
        )
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
    /// # Drift
    ///
    /// The CreditGrades asset process is driftless by construction, so the
    /// resulting model's default probabilities do **not** respond to
    /// `risk_free_rate`; the rate is retained only so the model can be used
    /// for discounting elsewhere. The payout rate is fixed at zero for the
    /// same reason.
    ///
    /// # Arguments
    ///
    /// * `equity_value` - Observed market capitalization E, strictly positive
    ///   and in the same currency as `total_debt`
    /// * `equity_vol` - Observed annualized equity volatility `sigma_E` as a
    ///   decimal fraction; must be non-negative
    /// * `total_debt` - Face value of debt, strictly positive; the barrier is
    ///   `total_debt * mean_recovery`
    /// * `risk_free_rate` - Continuously compounded risk-free rate r as a
    ///   decimal fraction. Stored on the model but not used by the
    ///   CreditGrades survival function
    /// * `barrier_uncertainty` - Log-barrier volatility `λ` (lognormal std dev
    ///   of the default barrier; Finger et al. 2002), a non-negative decimal.
    ///   `0.30` is the value calibrated in the original paper
    /// * `mean_recovery` - Mean recovery rate on debt at default as a decimal
    ///   fraction in `(0, 1]`; sets both the barrier level and the asset
    ///   value `V_0 = E + D * mean_recovery`
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NonPositiveValue`] or [`InputError::NegativeValue`]
    /// for out-of-domain equity and debt inputs, and [`Error::Validation`] if
    /// `mean_recovery` is outside `[0, 1]` or `barrier_uncertainty` is
    /// negative or non-finite.
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
}

#[cfg(test)]
mod tests {
    use super::super::{AssetDynamics, BarrierType, MertonModel};

    #[test]
    fn from_cds_spread_rejects_out_of_range_recovery() {
        assert!(MertonModel::from_cds_spread(150.0, -0.1, 80.0, 0.04, 5.0, 100.0, 0.0).is_err());
        assert!(MertonModel::from_cds_spread(150.0, 1.5, 80.0, 0.04, 5.0, 100.0, 0.0).is_err());
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
        let spread_bp = m.cds_par_spread(5.0, 0.40).expect("spread") * 10_000.0;
        let m2 = MertonModel::from_cds_spread(spread_bp, 0.40, 80.0, 0.04, 5.0, 100.0, 0.0)
            .expect("cds cal");
        assert!(
            (m2.asset_vol() - 0.25).abs() < 1e-6,
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
        let spread_bp = m_known.cds_par_spread(5.0, 0.40).expect("spread") * 10_000.0;

        let m_cal = MertonModel::from_cds_spread(spread_bp, 0.40, 80.0, 0.04, 5.0, 100.0, q)
            .expect("cds cal");

        assert!(
            (m_cal.asset_vol() - 0.25).abs() < 1e-6,
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
    fn from_cds_spread_scans_past_volatilities_with_no_survival_curve() {
        // At the bottom of the search range a firm at 1% asset vol and a 5%
        // drift has a default probability that shrinks with horizon, so the
        // hazard bootstrap refuses it. Those points carry no credit risk and
        // must be skipped, not treated as a calibration failure.
        let unusable = (0..=MertonModel::CDS_CALIBRATION_SCAN_POINTS).any(|i| {
            let step = (MertonModel::CDS_CALIBRATION_MAX_VOL
                - MertonModel::CDS_CALIBRATION_MIN_VOL)
                / MertonModel::CDS_CALIBRATION_SCAN_POINTS as f64;
            let sigma = MertonModel::CDS_CALIBRATION_MIN_VOL + i as f64 * step;
            MertonModel::new(100.0, sigma, 80.0, 0.05)
                .expect("valid")
                .cds_par_spread(5.0, 0.40)
                .is_err()
        });
        assert!(
            unusable,
            "the low-vol end of the scan should contain volatilities with no usable \
                 survival curve, otherwise this test no longer exercises the skip path"
        );

        let calibrated = MertonModel::from_cds_spread(200.0, 0.40, 80.0, 0.05, 5.0, 100.0, 0.0)
            .expect("cds cal");
        assert!(
            (calibrated.cds_par_spread(5.0, 0.40).expect("spread") - 0.02).abs() < 1e-8,
            "calibrated model should reprice the 200 bp quote, got {}",
            calibrated.cds_par_spread(5.0, 0.40).expect("spread")
        );
    }

    #[test]
    fn from_cds_spread_rejects_unattainable_quote() {
        // A 10,000 bp quote on a modestly levered firm is out of reach for
        // any volatility in the search range.
        let err = MertonModel::from_cds_spread(10_000.0, 0.40, 40.0, 0.04, 5.0, 100.0, 0.0)
            .expect_err("unattainable");
        let message = err.to_string();
        assert!(
            message.contains("no asset volatility"),
            "expected an attainability error, got: {message}"
        );
    }

    // Non-zero payout rate tests

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

    // Near-zero equity guards (W-10)

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

    // Non-convergence test

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

    // from_target_pd calibration tests

    #[test]
    fn from_target_pd_roundtrips() {
        let target_pd = 0.05; // 5% cumulative PD over 5 years
        let m = MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, target_pd, 5.0).expect("cal");
        let actual_pd = m.default_probability(5.0);
        assert!(
            (actual_pd - target_pd).abs() < 1e-6,
            "PD should match target: got {actual_pd}, want {target_pd}"
        );
    }

    #[test]
    fn from_target_pd_higher_pd_higher_barrier() {
        let m_low = MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, 0.01, 5.0).expect("low");
        let m_high = MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, 0.10, 5.0).expect("high");
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
        let m_bb = MertonModel::from_target_pd(200.0, 0.20, 0.045, 0.0, bb_pd, 5.0).expect("BB");
        assert!(
            (m_bb.default_probability(5.0) - bb_pd).abs() < 1e-6,
            "BB PD mismatch"
        );

        // B: annual PD ~200bp → 5Y cumulative ~9.5%
        let b_pd = 1.0 - (-0.0200_f64 * 5.0).exp();
        let m_b = MertonModel::from_target_pd(140.0, 0.30, 0.045, 0.0, b_pd, 5.0).expect("B");
        assert!(
            (m_b.default_probability(5.0) - b_pd).abs() < 1e-6,
            "B PD mismatch"
        );

        // CCC: annual PD ~400bp → 5Y cumulative ~18.1%
        let ccc_pd = 1.0 - (-0.0400_f64 * 5.0).exp();
        let m_ccc = MertonModel::from_target_pd(115.0, 0.40, 0.045, 0.0, ccc_pd, 5.0).expect("CCC");
        assert!(
            (m_ccc.default_probability(5.0) - ccc_pd).abs() < 1e-6,
            "CCC PD mismatch"
        );

        assert!(m_bb.debt_barrier() < 200.0);
        assert!(m_b.debt_barrier() < 140.0);
        assert!(m_ccc.debt_barrier() < 115.0);
    }

    #[test]
    fn from_target_pd_rejects_invalid_inputs() {
        assert!(MertonModel::from_target_pd(0.0, 0.25, 0.04, 0.0, 0.05, 5.0).is_err());
        assert!(MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, 1.0, 5.0).is_err());
        assert!(MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, -0.01, 5.0).is_err());
        // Zero asset volatility gives a degenerate step-function PD.
        assert!(MertonModel::from_target_pd(200.0, 0.0, 0.04, 0.0, 0.05, 5.0).is_err());
        // A zero target PD is unattainable for any interior barrier.
        assert!(MertonModel::from_target_pd(200.0, 0.25, 0.04, 0.0, 0.0, 5.0).is_err());
    }

    // CreditGrades cross-checks

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

    #[test]
    fn try_implied_equity_rejects_jump_diffusion() {
        let model = MertonModel::new_with_dynamics(
            100.0,
            0.20,
            80.0,
            0.05,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.30,
                jump_vol: 0.15,
            },
        )
        .expect("valid");
        assert!(model.try_implied_equity(1.0).is_err());
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
}
