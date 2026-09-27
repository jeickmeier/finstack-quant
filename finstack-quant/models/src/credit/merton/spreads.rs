use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::math::norm_cdf;
use finstack_quant_core::{Error, Result};

use super::{MertonBarrierType, MertonModel};

/// Premium payments per year on a standard CDS contract (quarterly, ISDA).
const CDS_PREMIUM_PERIODS_PER_YEAR: f64 = 4.0;

/// Scale factor converting an ACT/365F year fraction into an ACT/360 premium
/// accrual. CDS premium legs accrue ACT/360 while survival and discount times
/// are measured ACT/365F, so the two axes differ by this ratio.
const ACT365F_TO_ACT360: f64 = 365.0 / 360.0;

/// Base date used for the throwaway survival curves built during CDS
/// calibration. The curves are only ever queried by year fraction, so the
/// anchor is immaterial; it exists because `HazardCurveBuilder` rejects the
/// 1970 sentinel to stop callers from accidentally anchoring real curves
/// there.
const CDS_CALIBRATION_ANCHOR: time::Date = time::macros::date!(2000 - 01 - 01);

/// ISDA-style CDS par spread implied by a survival curve.
///
/// Prices both legs on a uniform quarterly premium grid running to
/// `maturity`, using a flat continuously compounded discount rate:
///
/// - **Protection leg**: `(1 - R) * Σ DF(t_mid) * [S(t_{i-1}) - S(t_i)]`,
///   discounting each period's default mass at the period midpoint.
/// - **Premium leg (risky annuity)**: `Σ Δ * DF(t_i) * S(t_i)` plus the
///   standard half-period accrual-on-default term
///   `0.5 * Δ * DF(t_mid) * [S(t_{i-1}) - S(t_i)]`.
///
/// The par spread is the ratio, so the two legs balance at inception. This is
/// the O'Kane (2008) discretization of the ISDA Standard Model on a flat
/// curve; it deliberately does not model IMM roll dates, holiday calendars,
/// or settlement lags, because the structural model it serves is expressed
/// purely in year fractions.
///
/// # Arguments
///
/// * `hazard` - Survival curve to price against; its `recovery_rate()`
///   supplies the loss given default and its `sp(t)` the survival
///   probabilities, both keyed by year fraction from the curve base date
/// * `risk_free_rate` - Flat continuously compounded discount rate as a
///   decimal fraction
/// * `maturity` - Contract maturity in years; must be finite and strictly
///   positive
///
/// # Errors
///
/// Returns [`Error::Validation`] if `maturity` is not finite and positive, or
/// if the risky annuity collapses to zero (survival has already decayed to
/// nothing), which would make the par spread infinite.
fn par_spread_from_survival(
    hazard: &HazardCurve,
    risk_free_rate: f64,
    maturity: f64,
) -> Result<f64> {
    if !(maturity.is_finite() && maturity > 0.0) {
        return Err(Error::Validation(format!(
            "par_spread_from_survival: maturity must be > 0, got {maturity}"
        )));
    }
    let periods = (maturity * CDS_PREMIUM_PERIODS_PER_YEAR).round().max(1.0);
    let dt = maturity / periods;
    let accrual = dt * ACT365F_TO_ACT360;
    let lgd = 1.0 - hazard.recovery_rate();

    let mut protection = 0.0;
    let mut annuity = 0.0;
    let mut prev_survival = 1.0;
    for i in 1..=(periods as usize) {
        let t = i as f64 * dt;
        let survival = hazard.sp(t);
        let default_mass = (prev_survival - survival).max(0.0);
        let df_end = (-risk_free_rate * t).exp();
        let df_mid = (-risk_free_rate * (t - 0.5 * dt)).exp();

        protection += df_mid * default_mass;
        annuity += accrual * df_end * survival;
        annuity += 0.5 * accrual * df_mid * default_mass;

        prev_survival = survival;
    }

    if !(annuity.is_finite() && annuity > 0.0) {
        return Err(Error::Validation(format!(
            "par_spread_from_survival: risky annuity must be > 0, got {annuity}; \
             survival has decayed to zero over the contract life"
        )));
    }
    Ok(lgd * protection / annuity)
}

impl MertonModel {
    /// Zero-coupon bond credit spread with an exogenous recovery rate.
    ///
    /// s = -ln(1 - PD * (1 - R)) / T
    ///
    /// # Convention
    ///
    /// This is the **continuously compounded zero-coupon spread** of a risky
    /// discount bond whose recovery `R` is a fixed fraction of face value
    /// **paid at maturity**: `price = e^{-rT}(1 - PD·LGD)`, hence the formula
    /// above. It is deliberately not:
    ///
    /// - Merton's *endogenous* debt spread, where recovery is the firm's own
    ///   terminal asset value — see [`Self::debt_spread`], which can differ
    ///   by a factor of two or more for the same model;
    /// - a CDS par spread, which has a premium leg, accrual on default, and
    ///   discounting of the protection payment at the default time — see
    ///   [`Self::cds_par_spread`]. The two agree only to first order in PD
    ///   and diverge by roughly 7% at a 30% cumulative default probability.
    ///
    /// With a first-passage barrier the underlying PD refers to a default
    /// that can occur at any time before `T`, while this formula still
    /// assumes recovery is paid at `T`; that understates the present value of
    /// recovery. Use [`Self::cds_par_spread`] when default timing matters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `horizon <= 0` (the spread is
    /// undefined at zero horizon) or `recovery` is outside `[0, 1]` (an
    /// out-of-range recovery can make `1 - PD·LGD` non-positive, yielding
    /// NaN).
    ///
    /// # Arguments
    ///
    /// * `horizon` - Bond maturity T in years from the valuation date; must
    ///   be finite and strictly positive
    /// * `recovery` - Recovery rate as a decimal fraction of face value
    ///   (`0.40` is the senior-unsecured market convention), assumed paid at
    ///   maturity; must lie in `[0, 1]`
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

    /// Merton (1974) endogenous credit spread on the firm's zero-coupon debt.
    ///
    /// s = -ln(D / (B * e^{-rT})) / T
    ///
    /// where `D` is the model value of the firm's debt claim. Recovery is
    /// **endogenous**: debt holders receive `min(V_T, B)`, so the recovery
    /// rate is the firm's own terminal asset value rather than an assumed
    /// constant. Because equity and debt exhaust the firm,
    /// `D = V·e^{-qT} - E` where `E` is the equity call value, which is what
    /// this method evaluates (Poisson-mixed under `JumpDiffusion`).
    ///
    /// This is the "risk structure of interest rates" of Merton (1974) and is
    /// the model-consistent spread. It is typically well below
    /// [`Self::implied_spread`] with a 40% exogenous recovery, because a firm
    /// that defaults in the Merton model usually retains substantial asset
    /// value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `horizon <= 0`, if the barrier type
    /// is not `Terminal` (the Black-Cox and CreditGrades debt claims pay
    /// recovery at the first-passage time and are not valued here; use
    /// [`Self::cds_par_spread`]), or if the implied debt value is
    /// non-positive, which makes the log undefined.
    ///
    /// # Arguments
    ///
    /// * `horizon` - Maturity T of the firm's debt in years from the
    ///   valuation date; must be finite and strictly positive
    pub fn debt_spread(&self, horizon: f64) -> Result<f64> {
        if !(horizon.is_finite() && horizon > 0.0) {
            return Err(Error::Validation(format!(
                "debt_spread: horizon must be > 0, got {horizon}"
            )));
        }
        if !matches!(self.barrier_type, MertonBarrierType::Terminal) {
            return Err(Error::Validation(
                "debt_spread: the endogenous Merton (1974) debt spread is defined for \
                 MertonBarrierType::Terminal only; a first-passage claim pays recovery at the \
                 hitting time. Use cds_par_spread or implied_spread instead."
                    .to_string(),
            ));
        }
        let equity = self.terminal_equity_value(horizon);
        let debt_value = self.asset_value * (-self.payout_rate * horizon).exp() - equity;
        let risk_free_value = self.debt_barrier * (-self.risk_free_rate * horizon).exp();
        if !(debt_value.is_finite() && debt_value > 0.0) {
            return Err(Error::Validation(format!(
                "debt_spread: implied debt value must be > 0, got {debt_value}"
            )));
        }
        Ok(-(debt_value / risk_free_value).ln() / horizon)
    }

    /// Value of the equity claim `E[max(V_T - B, 0)]` discounted at `r`,
    /// under the terminal-barrier Poisson-mixture terminal distribution.
    ///
    /// Reduces to Black-Scholes for `GeometricBrownian` and to the Merton
    /// (1976) option series for `JumpDiffusion`.
    fn terminal_equity_value(&self, horizon: f64) -> f64 {
        let log_moneyness = (self.asset_value / self.debt_barrier).ln();
        let discount = (-self.risk_free_rate * horizon).exp();
        self.terminal_log_components(self.log_drift(self.risk_free_rate), horizon)
            .into_iter()
            .map(|(weight, mean, variance)| {
                let std_dev = variance.sqrt();
                let d2 = (log_moneyness + mean) / std_dev;
                let d1 = d2 + std_dev;
                let forward = self.asset_value * (mean + 0.5 * variance).exp();
                weight
                    * discount
                    * forward.mul_add(norm_cdf(d1), -(self.debt_barrier * norm_cdf(d2)))
            })
            .sum()
    }

    /// ISDA-style CDS par spread implied by the model's survival curve.
    ///
    /// The model's risk-neutral survival probabilities are exported to a
    /// [`HazardCurve`] on the quarterly premium grid — the same object
    /// [`Self::to_hazard_curve`] hands to downstream pricers — and both CDS
    /// legs are priced against it on a quarterly premium grid: a protection
    /// leg discounting each period's default mass at the period midpoint, and
    /// a risky annuity carrying the standard half-period accrual on default.
    /// Because the curve is built through the standard
    /// bootstrap, a model whose survival curve cannot produce a usable hazard
    /// curve fails here rather than silently returning a spread no pricer
    /// could reproduce.
    ///
    /// Prefer this over [`Self::implied_spread`] whenever the target is a
    /// quoted CDS level: the zero-coupon formula omits the premium leg,
    /// accrual on default, and discounting, and understates the par spread by
    /// roughly 7% at a 30% cumulative default probability.
    ///
    /// # Arguments
    ///
    /// * `maturity` - CDS maturity in years from the valuation date; must be
    ///   finite and strictly positive
    /// * `recovery` - Recovery rate as a decimal fraction of notional
    ///   (`0.40` is the senior-unsecured market convention); must lie in
    ///   `[0, 1]`, and for `CreditGrades` dynamics must equal the model's own
    ///   `mean_recovery`
    ///
    /// # Returns
    ///
    /// Par spread as a decimal fraction per annum (multiply by 10,000 for
    /// basis points).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `maturity` is not positive, if
    /// `recovery` is outside `[0, 1]` or contradicts the model's
    /// `mean_recovery`, if the implied survival curve is non-monotonic, or if
    /// the risky annuity collapses to zero. Propagates
    /// [`HazardCurve`] builder errors, including the hazard-rate ceiling for
    /// a firm whose implied default probability is effectively 1.
    pub fn cds_par_spread(&self, maturity: f64, recovery: f64) -> Result<f64> {
        if !(maturity.is_finite() && maturity > 0.0) {
            return Err(Error::Validation(format!(
                "cds_par_spread: maturity must be > 0, got {maturity}"
            )));
        }
        let periods = (maturity * CDS_PREMIUM_PERIODS_PER_YEAR).round().max(1.0);
        let tenors: Vec<f64> = (1..=(periods as usize))
            .map(|i| i as f64 * maturity / periods)
            .collect();
        let hazard = self.to_hazard_curve(
            "MERTON-CDS-PAR",
            CDS_CALIBRATION_ANCHOR,
            &tenors,
            recovery,
            DayCount::Act365F,
        )?;
        par_spread_from_survival(&hazard, self.risk_free_rate, maturity)
    }
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::math::norm_cdf;

    use super::super::{AssetDynamics, MertonBarrierType, MertonModel};

    #[test]
    fn implied_spread_positive_for_risky_firm() {
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).unwrap();
        let spread = m.implied_spread(5.0, 0.40).expect("spread");
        assert!(spread > 0.0, "Spread should be positive");
        assert!(spread < 0.20, "Spread should be reasonable, got {spread}");
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

    #[test]
    fn cds_par_spread_exceeds_zero_coupon_spread() {
        // The zero-coupon formula ignores the premium leg, accrual on
        // default, and discounting of the protection payment. For a name
        // with a material cumulative PD the true par spread is visibly
        // higher, which is why calibration must not use implied_spread.
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let zero_coupon = m.implied_spread(5.0, 0.40).expect("zc");
        let par = m.cds_par_spread(5.0, 0.40).expect("par");
        assert!(
            par > zero_coupon * 1.02,
            "par spread {par} should exceed zero-coupon spread {zero_coupon} by >2%"
        );
        assert!(
            par < zero_coupon * 1.30,
            "par spread {par} should stay within 30% of zero-coupon spread {zero_coupon}"
        );
    }

    #[test]
    fn cds_par_spread_increases_with_leverage() {
        let low = MertonModel::new(100.0, 0.25, 60.0, 0.04).expect("valid");
        let high = MertonModel::new(100.0, 0.25, 90.0, 0.04).expect("valid");
        assert!(
            high.cds_par_spread(5.0, 0.40).expect("high")
                > low.cds_par_spread(5.0, 0.40).expect("low")
        );
    }

    #[test]
    fn cds_par_spread_scales_with_loss_given_default() {
        // Halving LGD roughly halves the par spread: the protection leg is
        // linear in LGD and the annuity does not depend on it.
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        let lgd_60 = m.cds_par_spread(5.0, 0.40).expect("R=40%");
        let lgd_30 = m.cds_par_spread(5.0, 0.70).expect("R=70%");
        let ratio = lgd_30 / lgd_60;
        assert!(
            (ratio - 0.5).abs() < 1e-12,
            "spread should scale linearly in LGD, got ratio {ratio}"
        );
    }

    // Endogenous debt spread

    #[test]
    fn debt_spread_matches_black_scholes_put_valuation() {
        // D = B e^{-rT} - Put(V, B), so the endogenous spread must agree with
        // an independently computed Black-Scholes put.
        let (v, sigma, b, r, t) = (100.0_f64, 0.25_f64, 80.0_f64, 0.04_f64, 5.0_f64);
        let m = MertonModel::new(v, sigma, b, r).expect("valid");

        let sqrt_t = t.sqrt();
        let d1 = ((v / b).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * sqrt_t);
        let d2 = d1 - sigma * sqrt_t;
        let put = b * (-r * t).exp() * norm_cdf(-d2) - v * norm_cdf(-d1);
        let risk_free_value = b * (-r * t).exp();
        let expected = -((risk_free_value - put) / risk_free_value).ln() / t;

        assert!((m.debt_spread(t).expect("spread") - expected).abs() < 1e-12);
    }

    #[test]
    fn debt_spread_below_exogenous_forty_percent_recovery_spread() {
        // Endogenous recovery in the Merton model is the firm's own terminal
        // asset value, which for a moderately levered firm is far above 40%.
        let m = MertonModel::new(100.0, 0.25, 80.0, 0.04).expect("valid");
        assert!(m.debt_spread(5.0).expect("endogenous") < m.implied_spread(5.0, 0.40).expect("zc"));
    }

    #[test]
    fn debt_spread_rejects_first_passage_barrier() {
        let m = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            80.0,
            0.04,
            0.0,
            MertonBarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid");
        assert!(m.debt_spread(5.0).is_err());
    }
}
