use super::swaption::{hw1f_swaption_price_inner, Hw1fSwaptionPriceInput};
use super::*;

// HW1F mean-reversion bounds.
pub(super) const KAPPA_MIN: f64 = 0.001;
pub(super) const KAPPA_MAX: f64 = 1.0;

// Short-rate volatility bounds for the log-space LM solve.
pub(super) const SIGMA_MIN: f64 = 1e-5;
pub(super) const SIGMA_MAX: f64 = 2.0;

// Relative margin for detecting parameters pinned near a box bound.
pub(super) const AT_BOUND_REL_TOL: f64 = 1e-6;

/// Reject calibrated HW1F parameters pinned near their box bounds.
pub(super) fn reject_at_bound_params(
    kappa: f64,
    sigma: f64,
    context: &str,
) -> finstack_quant_core::Result<()> {
    let near_bound = |v: f64, lo: f64, hi: f64| -> Option<&'static str> {
        if v <= lo * (1.0 + AT_BOUND_REL_TOL) {
            Some("lower")
        } else if v >= hi * (1.0 - AT_BOUND_REL_TOL) {
            Some("upper")
        } else {
            None
        }
    };
    if let Some(side) = near_bound(kappa, KAPPA_MIN, KAPPA_MAX) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: calibrated κ = {kappa:.6} is pinned at the {side} bound of \
             [{KAPPA_MIN}, {KAPPA_MAX}]. The optimizer wanted to leave the feasible \
             region — the mean-reversion speed is not identified by this quote set. \
             Review the swaption grid (expiry/tenor spread) or supply a bounded \
             `initial_guess`."
        )));
    }
    if let Some(side) = near_bound(sigma, SIGMA_MIN, SIGMA_MAX) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: calibrated σ = {sigma:.6} is pinned at the {side} bound of \
             [{SIGMA_MIN}, {SIGMA_MAX}]. The optimizer wanted to leave the feasible \
             region — review the quoted vols (a zero/extreme vol input is the usual \
             cause) or supply a bounded `initial_guess`."
        )));
    }
    Ok(())
}

/// Vega floor: 1 bp of annuity-year. Protects against division by a
/// near-zero vega at extreme expiries or zero quoted vol.
///
/// The vega used here is evaluated once at the *market* quote and used to
/// scale `(price_model − price_mkt)` into an approximate vol-error
/// residual. That linearisation is a first-order Taylor approximation
/// valid only near the solution; see the residual computation in
/// `HullWhiteSwaptionTarget::calculate_residuals` for the full
/// approximation-regime discussion (W-38).
pub(super) const SWAPTION_VEGA_FLOOR: f64 = 1e-8;

/// Validate a quote-level vega against [`SWAPTION_VEGA_FLOOR`] and reject
/// degenerate quotes.
///
/// Why this exists: when `actual_vega` is below the floor (deep OTM short
/// expiry, stale quote, near-zero quoted vol), the LM residual scaling
/// `(price_error) / vega` explodes — with `floor = 1e-8` the scaling factor
/// is `1e8` — so the quote dominates the Gauss-Newton step while LM reports
/// a clean termination. Silently substituting the floor produced a
/// distorted fit with no error surfaced, so a below-floor vega is now a
/// hard validation error: the caller must drop or repair the offending
/// quote before calibrating.
///
/// Returns the validated vega unchanged, or an error naming the quote when
/// the vega is non-finite or below `floor`.
pub(super) fn require_quote_vega(
    actual_vega: f64,
    floor: f64,
    quote_label: &str,
) -> finstack_quant_core::Result<f64> {
    if !actual_vega.is_finite() || actual_vega < floor {
        return Err(finstack_quant_core::Error::Validation(format!(
            "HW1F calibration: quote {quote_label} has vega {actual_vega:.3e} below the \
             {floor:.3e} floor; its 1/vega residual scaling would dominate the LM \
             objective. Drop the quote (deep OTM short expiry, stale, or near-zero \
             vol) or repair its inputs before calibrating."
        )));
    }
    Ok(actual_vega)
}

/// Number of deterministic multi-start restarts used for HW1F calibration.
pub(super) const HW_NUM_RESTARTS: usize = 5;
/// Halton perturbation scale (50%) applied to each parameter on restart.
pub(super) const HW_PERTURB_SCALE: f64 = 0.5;
/// Validation tolerance reported on the HW1F calibration report.
pub(super) const HW_VALIDATION_TOLERANCE: f64 = 1e-6;

/// Pre-computed market data for one swaption quote, captured once before
/// LM iteration so that the residual loop is a pure numeric computation.
///
/// `schedule` is the contractual fixed-leg schedule. When `None` the
/// calibrator uses the legacy constant-`tenor/n_periods` schedule preserved
/// for the float-only public API and existing tests.
pub(super) struct PreparedSwaption {
    pub(super) market_price: f64,
    pub(super) fwd_swap_rate: f64,
    pub(super) vega: f64,
    pub(super) schedule: Option<SwaptionSchedule>,
}

/// `GlobalSolveTarget` impl carrying everything HW1F swaption calibration
/// needs to evaluate residuals. The borrowed `df` keeps the target zero-
/// allocation per residual call; the pre-computed market data avoids re-
/// pricing from quotes inside the LM hot loop.
pub(super) struct HullWhiteSwaptionTarget<'a> {
    pub(super) df: &'a dyn Fn(f64) -> f64,
    pub(super) ppy: usize,
    pub(super) initial_x0: [f64; 2],
    pub(super) prepared: Vec<PreparedSwaption>,
}

impl<'a> GlobalSolveTarget for HullWhiteSwaptionTarget<'a> {
    type Quote = SwaptionQuote;
    type Curve = HullWhiteCalibrationParams;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> finstack_quant_core::Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        // HW1F has 2 scalar parameters (lnκ, lnσ); we use a dummy 2-point
        // time grid to satisfy the framework's knot-oriented API. Values
        // must be strictly positive to clear `validate_global_inputs`,
        // so we use `[1.0, 2.0]`. The target ignores `times` entirely
        // in `build_curve_from_params`.
        Ok((vec![1.0, 2.0], self.initial_x0.to_vec(), quotes.to_vec()))
    }

    fn build_curve_from_params(
        &self,
        _times: &[f64],
        params: &[f64],
    ) -> finstack_quant_core::Result<Self::Curve> {
        // Used by `build_curve_final_from_params` (default delegation).
        // For solver iterations we override to skip validation; here we
        // accept anything finite-positive and leave the κ-bounds check
        // to the wrapper post-solve so a transient out-of-bounds final
        // step does not mask a successful calibration.
        let kappa = params[0].exp();
        let sigma = params[1].exp();
        Ok(HullWhiteCalibrationParams { kappa, sigma })
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> finstack_quant_core::Result<()> {
        for (idx, q) in quotes.iter().enumerate() {
            let pre = &self.prepared[idx];
            let model_price = hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
                kappa: curve.kappa,
                sigma: curve.sigma,
                df: self.df,
                t0: q.expiry,
                tenor: q.tenor,
                swap_rate: pre.fwd_swap_rate,
                periods_per_year: self.ppy,
                schedule: pre.schedule.as_ref(),
            });
            if !model_price.is_finite() {
                // Signal infeasibility to the LM solver instead of injecting a
                // magic sentinel as a real residual: a hard-coded literal here
                // would flow into the Gauss-Newton step as `literal / vega` and
                // can dominate or poison the objective. Returning `Err` lets the
                // global solver substitute a properly bounded penalty pattern
                // (see `solver::global::fill_penalty`).
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Hull-White swaption model produced a non-finite price \
                     ({model_price:?}) for quote {}Yx{}Y (κ={:.6e}, σ={:.6e}); \
                     residual is infeasible",
                    q.expiry, q.tenor, curve.kappa, curve.sigma
                )));
            }
            // Vega-weighted price residual: `(price_model − price_mkt)/vega`
            // is, by a first-order Taylor expansion of price in vol, the
            // approximation `σ_model − σ_market`, so all quotes enter the
            // objective on a common implied-vol scale (Gilli–Maringer–
            // Schumann §13.4).
            //
            // APPROXIMATION REGIME (W-38): this linearisation is accurate
            // only NEAR the solution, where `price_model ≈ price_mkt` and
            // the vega evaluated at the *market* quote is a good proxy for
            // the local price/vol slope. Far from the solution — during LM
            // exploration or multi-start restarts — the true price/vol
            // map is nonlinear and the fixed market vega mis-scales the
            // residual, so the LM objective is a distorted (but still
            // descent-compatible) surface rather than a true vol-error
            // objective. Andersen–Piterbarg (*Interest Rate Modeling*,
            // Vol. III) instead iterate implied-vol residuals directly.
            // The vega-scaled form is retained here because it avoids a
            // per-iteration implied-vol inversion and converges to the
            // same minimiser once the iterates enter the valid regime.
            residuals[idx] = (model_price - pre.market_price) / pre.vega;
        }
        Ok(())
    }

    fn residual_key(&self, quote: &Self::Quote, idx: usize) -> String {
        format!("{idx}:{}Yx{}Y", quote.expiry, quote.tenor)
    }

    /// Log-space lower bounds `[ln(KAPPA_MIN), ln(SIGMA_MIN)]`.
    ///
    /// Enforced during the solve so κ cannot approach 0⁺ — at which point
    /// `B(t,T) = (1 − e^{−κτ})/κ` and the integrated-variance factor blow up.
    /// Previously `KAPPA_MAX` was only checked post-solve and there was no
    /// lower κ bound active during iteration.
    fn lower_bounds(&self) -> Option<Vec<f64>> {
        Some(vec![KAPPA_MIN.ln(), SIGMA_MIN.ln()])
    }

    /// Log-space upper bounds `[ln(KAPPA_MAX), ln(SIGMA_MAX)]`.
    fn upper_bounds(&self) -> Option<Vec<f64>> {
        Some(vec![KAPPA_MAX.ln(), SIGMA_MAX.ln()])
    }
}

/// Pre-computed market data for one cap/floor quote.
pub(super) struct PreparedCapFloor {
    pub(super) market_price: f64,
    pub(super) vega: f64,
}

/// `GlobalSolveTarget` impl for HW1F cap/floor calibration. Used only on
/// the two-parameter path (κ, σ). The fixed-κ path stays on the existing
/// 1D Brent solver because a single scalar root-find does not benefit
/// from the LM machinery.
pub(super) struct HullWhiteCapFloorTarget<'a> {
    pub(super) discount_df: &'a dyn Fn(f64) -> f64,
    pub(super) forward_df: &'a dyn Fn(f64) -> f64,
    pub(super) frequency: SwapFrequency,
    pub(super) initial_x0: [f64; 2],
    pub(super) prepared: Vec<PreparedCapFloor>,
}

impl<'a> GlobalSolveTarget for HullWhiteCapFloorTarget<'a> {
    type Quote = CapFloorQuote;
    type Curve = HullWhiteCalibrationParams;

    fn build_time_grid_and_guesses(
        &self,
        quotes: &[Self::Quote],
    ) -> finstack_quant_core::Result<(Vec<f64>, Vec<f64>, Vec<Self::Quote>)> {
        Ok((vec![1.0, 2.0], self.initial_x0.to_vec(), quotes.to_vec()))
    }

    fn build_curve_from_params(
        &self,
        _times: &[f64],
        params: &[f64],
    ) -> finstack_quant_core::Result<Self::Curve> {
        let kappa = params[0].exp();
        let sigma = params[1].exp();
        Ok(HullWhiteCalibrationParams { kappa, sigma })
    }

    fn calculate_residuals(
        &self,
        curve: &Self::Curve,
        quotes: &[Self::Quote],
        residuals: &mut [f64],
    ) -> finstack_quant_core::Result<()> {
        for (idx, quote) in quotes.iter().enumerate() {
            let pre = &self.prepared[idx];
            let spec = CapFloorPriceSpec::from_quote(quote, self.frequency);
            let model_price = hw1f_cap_floor_price(
                curve.kappa,
                curve.sigma,
                self.discount_df,
                self.forward_df,
                spec,
            );
            if !model_price.is_finite() {
                // Signal infeasibility to the LM solver instead of injecting a
                // magic sentinel as a real residual: a hard-coded literal here
                // would flow into the Gauss-Newton step as `literal / vega` and
                // can dominate or poison the objective. Returning `Err` lets the
                // global solver substitute a properly bounded penalty pattern
                // (see `solver::global::fill_penalty`).
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Hull-White {} model produced a non-finite price \
                     ({model_price:?}) for quote {}Y strike {:.6} \
                     (κ={:.6e}, σ={:.6e}); residual is infeasible",
                    if quote.is_cap { "cap" } else { "floor" },
                    quote.maturity,
                    quote.strike,
                    curve.kappa,
                    curve.sigma
                )));
            }
            residuals[idx] = (model_price - pre.market_price) / pre.vega;
        }
        Ok(())
    }

    fn residual_key(&self, quote: &Self::Quote, idx: usize) -> String {
        format!(
            "{idx}:{}Y_{}_{:.6}",
            quote.maturity,
            if quote.is_cap { "cap" } else { "floor" },
            quote.strike
        )
    }

    /// Log-space lower bounds `[ln(KAPPA_MIN), ln(SIGMA_MIN)]`.
    ///
    /// Enforced during the solve so κ cannot approach 0⁺ — at which point
    /// `B(t,T) = (1 − e^{−κτ})/κ` and the integrated-variance factor blow up.
    fn lower_bounds(&self) -> Option<Vec<f64>> {
        Some(vec![KAPPA_MIN.ln(), SIGMA_MIN.ln()])
    }

    /// Log-space upper bounds `[ln(KAPPA_MAX), ln(SIGMA_MAX)]`.
    fn upper_bounds(&self) -> Option<Vec<f64>> {
        Some(vec![KAPPA_MAX.ln(), SIGMA_MAX.ln()])
    }
}
