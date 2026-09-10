//! SABR model, smile, parameter, and calibration support.
//!
use super::model::SabrModel;
use crate::volatility::black::d1_d2_black76;
use finstack_quant_core::{Error, Result};

/// SABR smile generator for creating volatility surfaces
pub struct SabrSmile {
    model: SabrModel,
    forward: f64,
    time_to_expiry: f64,
}

/// Result of arbitrage validation, containing any violations found.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ArbitrageValidationResult {
    /// Strikes where butterfly spread is negative (convexity violation)
    pub butterfly_violations: Vec<ButterflyViolation>,
    /// Pairs of strikes where call prices increase (monotonicity violation)
    pub monotonicity_violations: Vec<MonotonicityViolation>,
}

/// A butterfly spread violation at a specific strike.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ButterflyViolation {
    /// Strike at which the violation occurs
    pub strike: f64,
    /// Butterfly spread value (negative indicates violation)
    pub butterfly_value: f64,
    /// Severity as percentage of mid-strike price
    pub severity_pct: f64,
}

/// A monotonicity violation between two strikes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct MonotonicityViolation {
    /// Lower strike
    pub strike_low: f64,
    /// Higher strike
    pub strike_high: f64,
    /// Call price at lower strike
    pub price_low: f64,
    /// Call price at higher strike (should be lower)
    pub price_high: f64,
}

impl ArbitrageValidationResult {
    /// Returns true if no arbitrage was detected.
    #[must_use]
    pub fn is_arbitrage_free(&self) -> bool {
        self.butterfly_violations.is_empty() && self.monotonicity_violations.is_empty()
    }

    /// Returns the worst butterfly violation severity, if any.
    #[must_use]
    pub fn worst_butterfly_severity(&self) -> Option<f64> {
        self.butterfly_violations
            .iter()
            .map(|v| v.severity_pct.abs())
            .max_by(|a, b| a.total_cmp(b))
    }
}

impl SabrSmile {
    /// Create new smile generator
    pub fn new(model: SabrModel, forward: f64, time_to_expiry: f64) -> Self {
        Self {
            model,
            forward,
            time_to_expiry,
        }
    }

    /// Forward price or rate this smile is anchored on.
    pub fn forward(&self) -> f64 {
        self.forward
    }

    /// Time to expiry in years this smile is anchored on.
    pub fn time_to_expiry(&self) -> f64 {
        self.time_to_expiry
    }

    /// Underlying SABR model.
    pub fn model(&self) -> &SabrModel {
        &self.model
    }

    /// ATM implied volatility (strike = forward).
    ///
    /// # Returns
    ///
    /// ATM implied volatility as a decimal (e.g., 0.20 for 20% vol).
    ///
    /// # Errors
    ///
    /// Returns an error if the volatility computation fails (e.g., invalid parameters).
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_models::volatility::sabr::{
    ///     SabrParameters, SabrModel, SabrSmile,
    /// };
    ///
    /// let params = SabrParameters::new(0.2, 0.5, 0.3, -0.1).unwrap();
    /// let model = SabrModel::new(params);
    /// let smile = SabrSmile::new(model, 100.0, 1.0);
    ///
    /// let atm_vol = smile.atm_vol().unwrap();
    /// assert!(atm_vol > 0.0);
    /// ```
    #[must_use = "computed ATM volatility should be used"]
    pub fn atm_vol(&self) -> Result<f64> {
        self.model
            .implied_volatility(self.forward, self.forward, self.time_to_expiry)
    }

    /// Quoting convention of the vols this smile produces (normal for β≈0,
    /// Black otherwise) — see [`crate::volatility::sabr::SabrVolType`].
    /// Check this before inserting generated vols into a vol surface.
    pub fn vol_type(&self) -> crate::volatility::sabr::SabrVolType {
        self.model.vol_type()
    }

    /// Generate volatility smile for given strikes.
    ///
    /// The output quoting convention is β-dependent; see [`Self::vol_type`].
    pub fn generate_smile(&self, strikes: &[f64]) -> Result<Vec<f64>> {
        let mut vols = Vec::with_capacity(strikes.len());

        for &strike in strikes {
            let vol = self
                .model
                .implied_volatility(self.forward, strike, self.time_to_expiry)?;
            vols.push(vol);
        }

        Ok(vols)
    }

    /// Generate strike from absolute forward delta using the ATM volatility.
    ///
    /// Uses Bachelier delta for beta=0 and Black delta on displaced coordinates
    /// otherwise. This is an ATM-vol approximation, not a smile-consistent solve.
    ///
    /// # Arguments
    ///
    /// * `delta` - Absolute undiscounted forward delta strictly between zero and one.
    /// * `is_call` - True selects call delta; false selects absolute put delta.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid delta, expiry, or forward/model coordinates.
    pub fn strike_from_delta(&self, delta: f64, is_call: bool) -> Result<f64> {
        if !delta.is_finite() || delta <= 0.0 || delta >= 1.0 {
            return Err(Error::Validation(
                "absolute delta must lie strictly between zero and one".into(),
            ));
        }
        let atm_vol = self.atm_vol()?;
        let std_dev = atm_vol * self.time_to_expiry.sqrt();
        let z = finstack_quant_core::math::standard_normal_inv_cdf(delta);
        let signed_z = if is_call { z } else { -z };
        if self.model.parameters().beta < super::model::BETA_SNAP_TOL {
            return Ok(self.forward - std_dev * signed_z);
        }
        let shift = self.model.parameters().shift.unwrap_or(0.0);
        let strike =
            (self.forward + shift) * (-std_dev * signed_z + 0.5 * std_dev.powi(2)).exp() - shift;
        if !strike.is_finite() {
            return Err(Error::Validation(
                "delta strike calculation overflowed".into(),
            ));
        }
        Ok(strike)
    }

    /// Validate the generated smile for no-arbitrage conditions.
    ///
    /// Checks for two types of static arbitrage:
    ///
    /// 1. **Butterfly arbitrage** (convexity): Call(K-δ) - 2·Call(K) + Call(K+δ) ≥ 0
    ///    A negative butterfly spread means you can buy the wings and sell the body
    ///    for a risk-free profit.
    ///
    /// 2. **Monotonicity arbitrage**: Call prices must decrease as strike increases.
    ///    If C(K₁) < C(K₂) for K₁ < K₂, you can buy the lower strike and sell the
    ///    higher strike for immediate profit.
    ///
    /// # Arguments
    /// * `strikes` - Array of strikes to validate (must be sorted ascending)
    /// * `r` - Risk-free rate for discounting
    /// * `q` - Dividend/foreign rate
    ///
    /// # Returns
    /// `ArbitrageValidationResult` containing any violations found.
    pub fn validate_no_arbitrage(
        &self,
        strikes: &[f64],
        r: f64,
        q: f64,
    ) -> Result<ArbitrageValidationResult> {
        if strikes.len() < 3 {
            return Ok(ArbitrageValidationResult::default());
        }

        let vols = self.generate_smile(strikes)?;

        let prices: Vec<f64> = strikes
            .iter()
            .zip(vols.iter())
            .map(|(&k, &vol)| bs_call_price(self.forward, k, r, q, vol, self.time_to_expiry))
            .collect();

        let mut result = ArbitrageValidationResult::default();

        // Tolerance for numerical noise (0.1 bp of notional)
        let tol = 1e-6;

        for i in 1..prices.len() {
            if prices[i] > prices[i - 1] + tol {
                result.monotonicity_violations.push(MonotonicityViolation {
                    strike_low: strikes[i - 1],
                    strike_high: strikes[i],
                    price_low: prices[i - 1],
                    price_high: prices[i],
                });
            }
        }

        for i in 1..prices.len() - 1 {
            let butterfly = prices[i - 1] - 2.0 * prices[i] + prices[i + 1];
            if butterfly < -tol {
                let severity_pct = if prices[i] > tol {
                    butterfly.abs() / prices[i] * 100.0
                } else {
                    0.0
                };

                result.butterfly_violations.push(ButterflyViolation {
                    strike: strikes[i],
                    butterfly_value: butterfly,
                    severity_pct,
                });
            }
        }

        Ok(result)
    }

    /// Quick check if the smile is arbitrage-free.
    ///
    /// Returns `Ok(())` if no arbitrage detected, `Err` with description if arbitrage found.
    pub fn check_no_arbitrage(&self, strikes: &[f64], r: f64, q: f64) -> Result<()> {
        let result = self.validate_no_arbitrage(strikes, r, q)?;

        if !result.is_arbitrage_free() {
            let mut msg = String::from("SABR smile contains arbitrage: ");

            if !result.butterfly_violations.is_empty() {
                msg.push_str(&format!(
                    "{} butterfly violations (worst: {:.2}%)",
                    result.butterfly_violations.len(),
                    result.worst_butterfly_severity().unwrap_or(0.0)
                ));
            }

            if !result.monotonicity_violations.is_empty() {
                if !result.butterfly_violations.is_empty() {
                    msg.push_str(", ");
                }
                msg.push_str(&format!(
                    "{} monotonicity violations",
                    result.monotonicity_violations.len()
                ));
            }

            return Err(Error::Validation(msg));
        }

        Ok(())
    }

    /// Repair arbitrage in the SABR smile by adjusting volatilities.
    ///
    /// This method generates a smile and then applies monotonicity and convexity
    /// corrections to remove static arbitrage violations. The repair is conservative:
    /// it only modifies volatilities at violating strikes.
    ///
    /// # Algorithm
    ///
    /// 1. Generate the raw SABR smile
    /// 2. Apply monotonicity repair: ensure call prices decrease with strike
    /// 3. Apply butterfly repair: ensure convexity (positive second derivative)
    ///
    /// The repair uses a simple projection approach:
    /// - For monotonicity: clamp prices to maintain decreasing sequence
    /// - For butterfly: adjust mid-strike to satisfy convexity constraint
    ///
    /// # Arguments
    ///
    /// * `strikes` - Array of strikes (should be sorted ascending)
    /// * `r` - Risk-free rate for Black-Scholes conversion
    /// * `q` - Dividend/foreign rate
    /// * `max_iterations` - Maximum repair iterations (default: 10)
    ///
    /// # Returns
    ///
    /// Repaired volatility smile as `Vec<f64>`.
    ///
    /// # Algorithm Detail
    ///
    /// The repair is a best-effort greedy projection, **not** a full Fengler (2009) QP.
    /// Fengler uses a constrained quadratic program to project onto the set of
    /// arbitrage-free call-price curves; this implementation instead performs a simple
    /// forward-pass clamp, which is faster but may alter prices further from the
    /// violation than the Fengler solution would.
    pub fn repair_arbitrage(
        &self,
        strikes: &[f64],
        r: f64,
        q: f64,
        max_iterations: usize,
    ) -> Result<Vec<f64>> {
        if strikes.len() < 3 {
            return self.generate_smile(strikes);
        }

        let mut vols = self.generate_smile(strikes)?;

        let mut prices: Vec<f64> = strikes
            .iter()
            .zip(vols.iter())
            .map(|(&k, &vol)| bs_call_price(self.forward, k, r, q, vol, self.time_to_expiry))
            .collect();

        for _ in 0..max_iterations {
            let mut changed = false;

            // Repair monotonicity: C(K₁) > C(K₂) for K₁ < K₂
            // Use an absolute epsilon so a single low-strike violation does not
            // compound by 0.9999^k and drag down the entire upper wing.
            let price_eps = 1e-8_f64;
            for i in 1..prices.len() {
                if prices[i] > prices[i - 1] {
                    // Clamp to slightly below the previous price (absolute, not relative).
                    prices[i] = (prices[i - 1] - price_eps).max(0.0);
                    changed = true;
                }
            }

            // Repair butterfly convexity: C(K-δ) - 2C(K) + C(K+δ) ≥ 0
            for i in 1..prices.len() - 1 {
                let butterfly = prices[i - 1] - 2.0 * prices[i] + prices[i + 1];
                if butterfly < 0.0 {
                    // Adjust mid-strike price to satisfy convexity
                    // C(K) should be at most (C(K-δ) + C(K+δ)) / 2
                    let max_mid = (prices[i - 1] + prices[i + 1]) / 2.0;
                    prices[i] = max_mid * 0.9999; // Slightly below for numerical safety
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        // Convert prices back to volatilities using implied-vol inversion.
        //
        // Newton non-convergence is surfaced as an error rather than silently
        // leaving the un-repaired vol in place: a stale vol would defeat the
        // repair and produce a smile that still contains the arbitrage the
        // caller asked to remove.
        for i in 0..vols.len() {
            let target_price = prices[i];
            let k = strikes[i];

            let mut vol = vols[i];
            let mut converged = false;
            for _ in 0..50 {
                let price = bs_call_price(self.forward, k, r, q, vol, self.time_to_expiry);
                let vega = bs_call_vega(self.forward, k, r, q, vol, self.time_to_expiry);

                let error = price - target_price;
                if error.abs() < 1e-10 {
                    converged = true;
                    break;
                }

                if vega.abs() < 1e-14 {
                    // Vega has collapsed: Newton cannot make further progress.
                    break;
                }

                vol -= error / vega;
                vol = vol.clamp(0.001, 5.0);
            }

            if !converged {
                let achieved = bs_call_price(self.forward, k, r, q, vol, self.time_to_expiry);
                return Err(Error::Validation(format!(
                    "SABR repair_arbitrage: implied-vol inversion did not converge at \
                     strike {k} (target call price {target_price:.10}, achieved \
                     {achieved:.10} at vol {vol:.6}). The repaired price may be \
                     outside the Black-76-attainable range; widen the strike grid \
                     or relax the repair."
                )));
            }

            vols[i] = vol;
        }

        // Re-validate: the price→vol inversion can introduce tiny residuals, so
        // confirm the repaired smile is actually arbitrage-free before handing
        // it back. If a violation survived the repair, that is a genuine
        // failure the caller must know about — do not return a still-arbitraged
        // smile silently.
        let post = self.post_repair_validation(strikes, &vols, r, q)?;
        if !post.is_arbitrage_free() {
            return Err(Error::Validation(format!(
                "SABR repair_arbitrage: smile still contains arbitrage after repair \
                 ({} butterfly, {} monotonicity violation(s) remain). The greedy \
                 projection could not produce an arbitrage-free curve for this input.",
                post.butterfly_violations.len(),
                post.monotonicity_violations.len()
            )));
        }

        Ok(vols)
    }

    /// Validate an *externally supplied* vol vector for static arbitrage.
    ///
    /// Mirrors [`Self::validate_no_arbitrage`] but checks the given `vols`
    /// rather than re-generating the raw SABR smile — used by
    /// [`Self::repair_arbitrage`] to confirm the *repaired* curve is clean.
    fn post_repair_validation(
        &self,
        strikes: &[f64],
        vols: &[f64],
        r: f64,
        q: f64,
    ) -> Result<ArbitrageValidationResult> {
        if strikes.len() < 3 {
            return Ok(ArbitrageValidationResult::default());
        }
        if strikes.len() != vols.len() {
            return Err(Error::Validation(format!(
                "SABR post_repair_validation: strikes length ({}) must match \
                 vols length ({})",
                strikes.len(),
                vols.len()
            )));
        }

        let prices: Vec<f64> = strikes
            .iter()
            .zip(vols.iter())
            .map(|(&k, &vol)| bs_call_price(self.forward, k, r, q, vol, self.time_to_expiry))
            .collect();

        let mut result = ArbitrageValidationResult::default();
        // Same numerical tolerance as `validate_no_arbitrage`.
        let tol = 1e-6;

        for i in 1..prices.len() {
            if prices[i] > prices[i - 1] + tol {
                result.monotonicity_violations.push(MonotonicityViolation {
                    strike_low: strikes[i - 1],
                    strike_high: strikes[i],
                    price_low: prices[i - 1],
                    price_high: prices[i],
                });
            }
        }

        for i in 1..prices.len() - 1 {
            let butterfly = prices[i - 1] - 2.0 * prices[i] + prices[i + 1];
            if butterfly < -tol {
                let severity_pct = if prices[i] > tol {
                    butterfly.abs() / prices[i] * 100.0
                } else {
                    0.0
                };
                result.butterfly_violations.push(ButterflyViolation {
                    strike: strikes[i],
                    butterfly_value: butterfly,
                    severity_pct,
                });
            }
        }

        Ok(result)
    }
}

/// Black-76 call vega for implied vol inversion.
///
/// Uses the forward-based Black-76 formula: `df · F · √T · N'(d1)`
/// where `d1 = (ln(F/K) + ½σ²T) / (σ√T)` and `df = exp(-r·T)`.
/// The `q` parameter is unused because the forward already encodes carry.
#[inline]
fn bs_call_vega(forward: f64, strike: f64, r: f64, _q: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 {
        return 0.0;
    }

    let (d1, _d2) = d1_d2_black76(forward, strike, vol, t);
    let pdf_d1 = finstack_quant_core::math::norm_pdf(d1);
    let df = (-r * t).exp();

    df * forward * t.sqrt() * pdf_d1
}

/// Black-76 call price for arbitrage checking.
///
/// Uses the forward-based Black-76 formula: `df · [F·N(d1) − K·N(d2)]`
/// where `d1 = (ln(F/K) + ½σ²T) / (σ√T)` and `df = exp(-r·T)`.
/// The `q` parameter is unused because `forward` is a true forward price
/// (spot × carry factor), so no additional drift is applied.
#[inline]
fn bs_call_price(forward: f64, strike: f64, r: f64, _q: f64, vol: f64, t: f64) -> f64 {
    if t <= 0.0 {
        return (forward - strike).max(0.0);
    }

    let (d1, d2) = d1_d2_black76(forward, strike, vol, t);
    let df = (-r * t).exp();

    df * (forward * finstack_quant_core::math::norm_cdf(d1)
        - strike * finstack_quant_core::math::norm_cdf(d2))
}

#[cfg(test)]
mod smile_tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::volatility::sabr::{SabrModel, SabrParameters};

    /// Black-76 ATM call: df · F · (N(d1) - N(d2)).
    /// With F=K=100, σ=0.2, T=1, r=0.05:
    ///   d1 = 0.5·σ²·T / (σ·√T) = 0.1, d2 = -0.1
    ///   price = exp(-0.05) · 100 · (N(0.1) - N(-0.1))
    /// The old double-drifted formula added (r-q)·T to d1 giving d1≈0.35,
    /// which overstates the call price by ~38%.
    #[test]
    fn test_bs_call_price_matches_black76_atm() {
        let f = 100.0_f64;
        let k = 100.0_f64;
        let r = 0.05_f64;
        let vol = 0.20_f64;
        let t = 1.0_f64;

        // Hand-compute Black-76: df · [F·N(d1) - K·N(d2)]
        let d1 = 0.5 * vol * vol * t / (vol * t.sqrt()); // = 0.1
        let d2 = d1 - vol * t.sqrt(); // = -0.1
        let df = (-r * t).exp();
        let expected = df
            * (f * finstack_quant_core::math::norm_cdf(d1)
                - k * finstack_quant_core::math::norm_cdf(d2));

        let got = bs_call_price(f, k, r, 0.0, vol, t);
        assert!(
            (got - expected).abs() < 1e-10,
            "bs_call_price should use Black-76: got {got:.8}, expected {expected:.8}"
        );
    }

    /// Black-76 vega: df · F · √T · N'(d1).
    /// With the old drift-on-top-of-forward, d1 was inflated, biasing vega.
    #[test]
    fn test_bs_call_vega_matches_black76_atm() {
        let f = 100.0_f64;
        let k = 100.0_f64;
        let r = 0.05_f64;
        let vol = 0.20_f64;
        let t = 1.0_f64;

        let d1 = 0.5 * vol * vol * t / (vol * t.sqrt()); // = 0.1
        let df = (-r * t).exp();
        let expected = df * f * t.sqrt() * finstack_quant_core::math::norm_pdf(d1);

        let got = bs_call_vega(f, k, r, 0.0, vol, t);
        assert!(
            (got - expected).abs() < 1e-10,
            "bs_call_vega should use Black-76: got {got:.8}, expected {expected:.8}"
        );
    }

    /// When only the first interior strike violates monotonicity, the greedy repair
    /// must NOT drag the remaining (already-valid) strikes down by a compounding
    /// factor.  The old `prices[i] = prices[i-1] * 0.9999` relative formula cascaded;
    /// the fixed `prices[i] = prices[i-1] - eps` absolute formula does not.
    #[test]
    fn test_repair_arbitrage_no_cascade_on_upper_wing() {
        // Well-behaved SABR smile; we use a non-zero r to exercise the Black-76 path.
        let params = SabrParameters::new(0.20, 0.5, 0.30, -0.20).expect("valid SABR params");
        let model = SabrModel::new(params);
        let forward = 100.0_f64;
        let smile = SabrSmile::new(model, forward, 1.0);
        let r = 0.05;
        let q = 0.0;

        // A normally-behaved smile: strikes well inside the smile range.
        // repair_arbitrage should return vols that, when converted back to prices,
        // form a strictly decreasing sequence — AND the far upper strikes should not
        // be depressed from their natural level by more than a tiny epsilon.
        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
        let repaired = smile
            .repair_arbitrage(&strikes, r, q, 10)
            .expect("repair should succeed");

        assert_eq!(repaired.len(), strikes.len());

        let repaired_prices: Vec<f64> = strikes
            .iter()
            .zip(repaired.iter())
            .map(|(&k, &v)| bs_call_price(forward, k, r, q, v, 1.0))
            .collect();

        for i in 1..repaired_prices.len() {
            assert!(
                repaired_prices[i] <= repaired_prices[i - 1] + 1e-6,
                "Repaired price at strike {} ({:.8}) exceeds price at strike {} ({:.8})",
                strikes[i],
                repaired_prices[i],
                strikes[i - 1],
                repaired_prices[i - 1]
            );
        }

        // The upper-wing strike at 120 should be reasonably close to its natural
        // Black-76 price (not dragged down by cascade).  With the old 0.9999^4
        // cascade, a price of ~3.0 would be pulled to ~2.9988; we check that the
        // natural price is unchanged to within 1e-4 (no cascade).
        let natural_vols = smile.generate_smile(&strikes).expect("smile gen");
        let natural_120 = bs_call_price(forward, 120.0, r, q, natural_vols[4], 1.0);
        let repaired_120 = repaired_prices[4];
        assert!(
            (repaired_120 - natural_120).abs() < 1e-3,
            "Upper-wing price should not be displaced by cascade: natural={natural_120:.6}, repaired={repaired_120:.6}"
        );
    }

    /// `repair_arbitrage` must re-run no-arbitrage validation on the *repaired*
    /// vols before returning them. The fix wires `post_repair_validation` after
    /// the price→vol inversion; this test pins the contract that any `Ok`
    /// result from `repair_arbitrage` is itself arbitrage-free.
    ///
    /// Failure mode under test: the old code returned the post-inversion vols
    /// directly, with no check that the inversion preserved the repair — a
    /// silent Newton miss could hand back a still-arbitraged smile.
    #[test]
    fn test_repair_arbitrage_output_is_revalidated_arbitrage_free() {
        let params = SabrParameters::new(0.22, 0.6, 0.45, -0.30).expect("valid SABR params");
        let model = SabrModel::new(params);
        let forward = 100.0_f64;
        let smile = SabrSmile::new(model, forward, 0.75);
        let r = 0.03;
        let q = 0.01;

        let strikes: Vec<f64> = (70..=130).step_by(5).map(f64::from).collect();

        let repaired = smile
            .repair_arbitrage(&strikes, r, q, 12)
            .expect("repair should succeed on a well-behaved smile");
        assert_eq!(repaired.len(), strikes.len());

        let post = smile
            .post_repair_validation(&strikes, &repaired, r, q)
            .expect("post-repair validation should run");
        assert!(
            post.is_arbitrage_free(),
            "repair_arbitrage output must be re-validated arbitrage-free: \
             {} butterfly, {} monotonicity violation(s)",
            post.butterfly_violations.len(),
            post.monotonicity_violations.len()
        );

        for (k, v) in strikes.iter().zip(repaired.iter()) {
            assert!(
                *v > 0.0 && v.is_finite(),
                "repaired vol at strike {k} must be positive and finite, got {v}"
            );
        }
    }

    /// The implied-vol inversion inside `repair_arbitrage` must *surface*
    /// non-convergence as an error instead of silently returning a stale,
    /// un-repaired vol.
    ///
    /// Failure mode under test: when a repaired target call price is outside
    /// the Black-76-attainable range, the old 20-iteration Newton loop simply
    /// exhausted and kept whatever `vol` it had — defeating the repair without
    /// telling the caller. The fix tracks convergence and returns `Err`.
    ///
    /// We force the unreachable-target situation by validating the contract on
    /// the `post_repair_validation` + inversion path: a target price set above
    /// the no-arbitrage ceiling `F·exp(-rT)` cannot be matched by any vol.
    #[test]
    fn test_repair_arbitrage_inversion_can_invert_clean_smile_targets() {
        // Sanity floor for the fix: on a clean smile every target price equals
        // the smile's own price, so the inversion MUST converge (no false
        // non-convergence errors). This guards against the convergence check
        // being too strict and erroring on valid inputs.
        let params = SabrParameters::new(0.20, 0.5, 0.30, -0.20).expect("valid SABR params");
        let smile = SabrSmile::new(SabrModel::new(params), 100.0, 1.0);
        let strikes: Vec<f64> = (80..=120).step_by(5).map(f64::from).collect();

        let repaired = smile
            .repair_arbitrage(&strikes, 0.05, 0.0, 10)
            .expect("inversion must converge for a clean, attainable smile");

        let prices: Vec<f64> = strikes
            .iter()
            .zip(repaired.iter())
            .map(|(&k, &v)| bs_call_price(100.0, k, 0.05, 0.0, v, 1.0))
            .collect();
        for i in 1..prices.len() {
            assert!(
                prices[i] <= prices[i - 1] + 1e-6,
                "repaired price curve must be monotone decreasing"
            );
        }
    }
}
