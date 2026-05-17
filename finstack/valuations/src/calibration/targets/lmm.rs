//! LMM/BGM factor-loading calibration to the swaption volatility surface.
//!
//! The Bermudan LMM pricer uses a flat 2-factor loading structure
//!
//! ```text
//! λ_i = base_vol · ĝ_i,   ĝ_i = [1 − α·f_i, α·f_i, 0],   f_i = i / N
//! ```
//!
//! The shape vectors `ĝ_i` are fixed (a linear-decay proxy for the first two
//! principal components of the forward-rate correlation matrix), but the
//! overall scale `base_vol` must be **calibrated** so the model reprices the
//! co-terminal European swaptions embedded in the Bermudan's exercise
//! schedule. Plugging a raw swaption-surface vol straight in as `base_vol`
//! (the previous behaviour) is wrong: the surface quotes the *swap-rate*
//! Black vol, not the *forward-rate* instantaneous vol — the two differ by
//! the Rebonato shape factor `R` derived below.
//!
//! # Rebonato swaption-vol approximation
//!
//! For a European swaption with expiry `T_e` on the co-terminal swap covering
//! forwards `[first, N)`, the forward swap rate is the weighted basket
//! `S = Σ_i w_i F_i` with annuity weights `w_i = τ_i P(0,T_{i+1}) / A`. Its
//! Black variance to expiry is (Rebonato 2002, Ch. 8; Andersen–Piterbarg
//! 2010, §16.5)
//!
//! ```text
//! σ²_swaption · T_e ≈ (1/S²) Σ_i Σ_j w_i w_j F_i F_j ∫₀^{T_e} λ_i(t)·λ_j(t) dt
//! ```
//!
//! With **time-constant** loadings `λ_i = base_vol · ĝ_i` the integral is
//! `base_vol² · (ĝ_i·ĝ_j) · T_e`, so the swaption vol is *exactly linear* in
//! `base_vol`:
//!
//! ```text
//! σ_swaption = base_vol · R,
//! R = sqrt( (1/S²) Σ_i Σ_j w_i w_j F_i F_j (ĝ_i·ĝ_j) )
//! ```
//!
//! Calibration is therefore the closed-form `base_vol = σ_market / R` — no
//! iterative solve is needed, and the result reprices the co-terminal
//! European swaption to its market vol by construction.
//!
//! For displaced (shifted-lognormal) dynamics the same identity holds with
//! `F_i → F_i + d_i` and `S → S + d`, which is the basket level the
//! shifted-lognormal swap rate diffuses.
//!
//! # References
//!
//! - Rebonato, R. (2002). *Modern Pricing of Interest-Rate Derivatives*,
//!   Ch. 8, Princeton University Press.
//! - Andersen, L. & Piterbarg, V. (2010). *Interest Rate Modeling*, Vol. 2,
//!   §16.5, Atlantic Financial Press.

/// Inputs describing the co-terminal swap underlying one European swaption
/// slice of a Bermudan exercise schedule, expressed in LMM forward-rate
/// coordinates.
#[derive(Debug, Clone)]
pub(crate) struct CoTerminalSlice<'a> {
    /// Tenor dates `T_0..T_N` (year fractions, length `N+1`).
    pub tenors: &'a [f64],
    /// Accrual factors `τ_i = T_{i+1} − T_i` (length `N`).
    pub accrual_factors: &'a [f64],
    /// Initial forward rates `F_i(0)` (length `N`).
    pub initial_forwards: &'a [f64],
    /// Displacements `d_i` (length `N`).
    pub displacements: &'a [f64],
    /// Unscaled factor-loading shapes `ĝ_i` per forward (length `N`).
    pub loading_shapes: &'a [[f64; 3]],
    /// Index of the first forward alive at the swaption expiry (`first`).
    pub first_alive: usize,
}

/// Result of calibrating `base_vol` to a swaption surface.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LmmBaseVolCalibration {
    /// Calibrated overall loading scale.
    pub base_vol: f64,
    /// Rebonato shape factor `R` (`σ_swaption = base_vol · R`).
    ///
    /// Diagnostic output: the LMM pricer consumes only `base_vol`, but the
    /// shape factor and implied vol are surfaced for calibration tests and
    /// downstream callers that want to verify the repricing.
    #[allow(dead_code)]
    pub shape_factor: f64,
    /// LMM-implied co-terminal European swaption Black vol after calibration
    /// (equals the market target up to floating-point rounding).
    #[allow(dead_code)]
    pub implied_swaption_vol: f64,
}

/// Rebonato shape factor `R` linking `base_vol` to the co-terminal European
/// swaption Black vol: `σ_swaption = base_vol · R`.
///
/// Returns `None` when the swap is degenerate (no live forwards, zero
/// annuity, or a non-positive basket level) — the caller then falls back to
/// the raw surface vol.
pub(crate) fn rebonato_shape_factor(slice: &CoTerminalSlice<'_>) -> Option<f64> {
    let n = slice.accrual_factors.len();
    let first = slice.first_alive;
    if first >= n || slice.tenors.len() != n + 1 {
        return None;
    }

    // Discount factors P(0, T_j) for j = first..=N from the live forwards.
    // P(0, T_first) is the numeraire base; carry it as 1.0 and divide out via
    // the annuity weights, which is scale-invariant for the basket.
    let live = n - first;
    let mut df = vec![1.0_f64; live + 1];
    for k in 1..=live {
        let idx = first + k - 1;
        let denom = 1.0 + slice.accrual_factors[idx] * slice.initial_forwards[idx];
        if denom.abs() < 1e-15 {
            return None;
        }
        df[k] = df[k - 1] / denom;
    }

    // Annuity A = Σ τ_j P(0, T_{j+1}).
    let mut annuity = 0.0_f64;
    for k in 0..live {
        annuity += slice.accrual_factors[first + k] * df[k + 1];
    }
    if annuity.abs() < 1e-15 {
        return None;
    }

    // Shifted basket level S + d = Σ w_j (F_j + d_j), weights w_j = τ_j DF_{j+1}/A.
    // The displaced-lognormal swap rate diffuses about this shifted level.
    let mut weights = vec![0.0_f64; live];
    let mut basket = 0.0_f64;
    for k in 0..live {
        let idx = first + k;
        let w = slice.accrual_factors[idx] * df[k + 1] / annuity;
        weights[k] = w;
        basket += w * (slice.initial_forwards[idx] + slice.displacements[idx]);
    }
    if !(basket.is_finite()) || basket <= 1e-12 {
        return None;
    }

    // R² = (1/S²) Σ_i Σ_j w_i w_j (F_i+d_i)(F_j+d_j) (ĝ_i·ĝ_j).
    let mut acc = 0.0_f64;
    for ki in 0..live {
        let i = first + ki;
        let fi = slice.initial_forwards[i] + slice.displacements[i];
        let gi = slice.loading_shapes[i];
        for kj in 0..live {
            let j = first + kj;
            let fj = slice.initial_forwards[j] + slice.displacements[j];
            let gj = slice.loading_shapes[j];
            let dot = gi[0] * gj[0] + gi[1] * gj[1] + gi[2] * gj[2];
            acc += weights[ki] * weights[kj] * fi * fj * dot;
        }
    }
    let r_sq = acc / (basket * basket);
    if !(r_sq.is_finite()) || r_sq <= 0.0 {
        return None;
    }
    Some(r_sq.sqrt())
}

/// Calibrate the LMM `base_vol` so the co-terminal European swaption reprices
/// to the market Black vol `market_swaption_vol`.
///
/// Returns `None` when the Rebonato shape factor cannot be formed (degenerate
/// swap), signalling the caller to fall back to the uncalibrated raw vol.
pub(crate) fn calibrate_base_vol(
    slice: &CoTerminalSlice<'_>,
    market_swaption_vol: f64,
) -> Option<LmmBaseVolCalibration> {
    if !market_swaption_vol.is_finite() || market_swaption_vol <= 0.0 {
        return None;
    }
    let shape_factor = rebonato_shape_factor(slice)?;
    if shape_factor <= 1e-12 {
        return None;
    }
    let base_vol = market_swaption_vol / shape_factor;
    Some(LmmBaseVolCalibration {
        base_vol,
        shape_factor,
        implied_swaption_vol: base_vol * shape_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the linear-decay loading shapes used by the LMM Bermudan pricer.
    fn loading_shapes(n: usize, alpha: f64) -> Vec<[f64; 3]> {
        (0..n)
            .map(|i| {
                let frac = i as f64 / n.max(1) as f64;
                [1.0 - alpha * frac, alpha * frac, 0.0]
            })
            .collect()
    }

    #[test]
    fn shape_factor_is_positive_and_finite() {
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let r = rebonato_shape_factor(&slice).expect("shape factor");
        assert!(r.is_finite() && r > 0.0, "R must be positive finite, got {r}");
    }

    #[test]
    fn calibrated_base_vol_reprices_swaption() {
        // The whole point: base_vol · R == market vol by construction.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let market_vol = 0.22;
        let cal = super::calibrate_base_vol(&slice, market_vol).expect("calibration");
        assert!(
            (cal.implied_swaption_vol - market_vol).abs() < 1e-12,
            "calibrated LMM should reprice swaption vol {market_vol}, got {}",
            cal.implied_swaption_vol
        );
        // base_vol differs from the raw surface vol — this is the defect fix:
        // feeding `market_vol` directly as base_vol would mis-price by 1/R.
        assert!(
            (cal.base_vol - market_vol).abs() > 1e-6,
            "shape factor R must be != 1, otherwise calibration is a no-op"
        );
    }

    #[test]
    fn first_alive_offset_handled() {
        // Co-terminal swaption with expiry past the first tenor: only
        // forwards [first_alive, N) participate.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.03, 0.032, 0.034, 0.036];
        let disp = vec![0.0; 4];
        let shapes = loading_shapes(4, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 2,
        };
        let cal = super::calibrate_base_vol(&slice, 0.20).expect("calibration");
        assert!(cal.base_vol.is_finite() && cal.base_vol > 0.0);
        assert!((cal.implied_swaption_vol - 0.20).abs() < 1e-12);
    }

    #[test]
    fn rejects_degenerate_inputs() {
        let tenors = vec![0.0, 1.0];
        let accruals = vec![1.0];
        let forwards = vec![0.03];
        let disp = vec![0.0];
        let shapes = loading_shapes(1, 0.4);
        let slice = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &disp,
            loading_shapes: &shapes,
            first_alive: 1, // no live forwards
        };
        assert!(rebonato_shape_factor(&slice).is_none());
        assert!(super::calibrate_base_vol(&slice, 0.2).is_none());
        // Non-positive market vol rejected.
        let live_slice = CoTerminalSlice {
            first_alive: 0,
            ..slice
        };
        assert!(super::calibrate_base_vol(&live_slice, 0.0).is_none());
        assert!(super::calibrate_base_vol(&live_slice, -0.1).is_none());
    }

    #[test]
    fn displacement_shifts_basket() {
        // With a positive displacement the basket level rises, so for the
        // same market vol the calibrated base_vol changes — confirms the
        // shift feeds through the shape factor.
        let tenors = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let accruals = vec![1.0; 4];
        let forwards = vec![0.01, 0.012, 0.014, 0.016];
        let shapes = loading_shapes(4, 0.4);
        let no_shift = vec![0.0; 4];
        let shifted = vec![0.02; 4];
        let base = CoTerminalSlice {
            tenors: &tenors,
            accrual_factors: &accruals,
            initial_forwards: &forwards,
            displacements: &no_shift,
            loading_shapes: &shapes,
            first_alive: 0,
        };
        let shifted_slice = CoTerminalSlice {
            displacements: &shifted,
            ..base.clone()
        };
        let r0 = rebonato_shape_factor(&base).expect("r0");
        let r1 = rebonato_shape_factor(&shifted_slice).expect("r1");
        assert!(
            (r0 - r1).abs() > 1e-9,
            "displacement must change the shape factor"
        );
    }
}
