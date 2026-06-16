//! Hull-White one-factor model calibration to European swaptions.
//!
//! Calibrates the two Hull-White parameters (mean reversion κ and short rate
//! volatility σ) by minimising squared swaption price errors using the
//! Levenberg-Marquardt algorithm.
//!
//! # Mathematical Foundation
//!
//! The Hull-White one-factor model specifies the short rate dynamics:
//!
//! ```text
//! dr(t) = [θ(t) − κ r(t)] dt + σ dW(t)
//!
//! where:
//!   κ = mean reversion speed
//!   σ = short rate volatility
//!   θ(t) = time-dependent drift chosen to match the initial term structure
//! ```
//!
//! # Swaption Pricing
//!
//! European swaptions are priced analytically using the Jamshidian (1989)
//! decomposition, which expresses a coupon bond option as a portfolio of
//! zero-coupon bond options under the HW1F model.
//!
//! The zero-coupon bond option volatility is:
//!
//! ```text
//! σ_P(t, T, S) = B(T,S) × σ × √((1 − e^{−2κt}) / (2κ))
//!
//! where B(T,S) = (1/κ)(1 − e^{−κ(S−T)})
//! ```
//!
//! # References
//!
//! - Hull, J. & White, A. (1990). "Pricing Interest-Rate-Derivative Securities."
//!   *Review of Financial Studies*, 3(4), 573-592.
//! - Jamshidian, F. (1989). "An Exact Bond Option Formula."
//!   *Journal of Finance*, 44(1), 205-209.
//! - Brigo, D. & Mercurio, F. (2006). *Interest Rate Models — Theory and Practice*.
//!   Springer Finance (2nd ed.), Chapter 3.

use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};
use std::collections::BTreeMap;

use crate::calibration::config::CalibrationConfig;
use crate::calibration::solver::global::GlobalFitOptimizer;
use crate::calibration::solver::multi_start::MultiStartConfig;
use crate::calibration::solver::traits::GlobalSolveTarget;
use crate::calibration::CalibrationReport;
use crate::models::trees::HullWhiteTreeConfig;

/// Hull-White one-factor model parameters.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::calibration::hull_white::HullWhiteParams;
///
/// let params = HullWhiteParams::new(0.05, 0.01).unwrap();
/// assert!(params.kappa > 0.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HullWhiteParams {
    /// Mean reversion speed (κ > 0).
    pub kappa: f64,
    /// Short rate volatility (σ > 0).
    pub sigma: f64,
}

impl Default for HullWhiteParams {
    /// Returns generic default parameters for testing and initialization.
    ///
    /// These defaults (κ=3%, σ=1%) are not calibrated and should not be used
    /// for production pricing without an explicit calibration decision.
    fn default() -> Self {
        Self {
            kappa: 0.03,
            sigma: 0.01,
        }
    }
}

impl HullWhiteParams {
    /// Construct validated Hull-White parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if `kappa <= 0` or `sigma <= 0`.
    pub fn new(kappa: f64, sigma: f64) -> finstack_quant_core::Result<Self> {
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Hull-White kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Hull-White sigma (short rate volatility) must be positive, got {sigma}"
            )));
        }
        Ok(Self { kappa, sigma })
    }

    /// Returns true when these parameters are the generic uncalibrated defaults.
    #[must_use]
    pub fn is_uncalibrated_default(&self) -> bool {
        (self.kappa - 0.03).abs() < f64::EPSILON && (self.sigma - 0.01).abs() < f64::EPSILON
    }

    /// Create tree configuration with the specified number of steps.
    pub(crate) fn tree_config(&self, steps: usize) -> HullWhiteTreeConfig {
        // Defensive against future code paths that might bypass the
        // construction-time validation: a non-positive mean-reversion would
        // produce an exploding (mean-anti-reverting) tree.
        debug_assert!(
            self.kappa > 0.0,
            "Hull-White mean reversion kappa must be positive, got {}",
            self.kappa
        );
        HullWhiteTreeConfig::new(self.kappa, self.sigma, steps)
    }

    /// B function: B(t₁, t₂) = (1 − e^{−κ(t₂−t₁)}) / κ
    ///
    /// For small κ, uses the Taylor expansion B ≈ (t₂ − t₁) to avoid
    /// division by near-zero.
    #[must_use]
    pub fn b_function(&self, t1: f64, t2: f64) -> f64 {
        hw_b(self.kappa, t1, t2)
    }

    /// Zero-coupon bond option volatility under HW1F.
    ///
    /// ```text
    /// σ_P(t, T, S) = B(T,S) × σ × √((1 − e^{−2κ(T−t)}) / (2κ))
    /// ```
    ///
    /// # Arguments
    ///
    /// * `t` - Current time
    /// * `big_t` - Option expiry time (T)
    /// * `s` - Bond maturity time (S > T)
    #[must_use]
    pub fn bond_option_vol(&self, t: f64, big_t: f64, s: f64) -> f64 {
        hw_bond_vol(self.kappa, self.sigma, t, big_t, s)
    }
}

/// `MarketContext` scalar-store keys for swaption-calibrated HW1F parameters.
///
/// Returns the `(kappa_key, sigma_key)` pair under which the swaption
/// Hull-White calibration step writes its solved κ/σ into the
/// [`MarketContext`](finstack_quant_core::market_data::context::MarketContext)
/// scalar store. The calibration writer and any downstream reader (e.g. the
/// HW1F pricer parameter resolver) must obtain these keys here so the
/// convention has a single source of truth and cannot drift.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::calibration::hull_white::hw1f_scalar_keys;
///
/// let (kappa, sigma) = hw1f_scalar_keys("USD-OIS");
/// assert_eq!(kappa, "USD-OIS_HW1F_KAPPA");
/// assert_eq!(sigma, "USD-OIS_HW1F_SIGMA");
/// ```
#[must_use]
pub fn hw1f_scalar_keys(curve_id: &str) -> (String, String) {
    (
        format!("{curve_id}_HW1F_KAPPA"),
        format!("{curve_id}_HW1F_SIGMA"),
    )
}

/// `MarketContext` scalar-store keys for cap/floor-calibrated HW1F parameters.
///
/// Returns the `(kappa_key, sigma_key)` pair under which the cap/floor
/// Hull-White calibration step writes its solved κ/σ into the
/// [`MarketContext`](finstack_quant_core::market_data::context::MarketContext)
/// scalar store. As with [`hw1f_scalar_keys`], this is the single source of
/// truth shared by the calibration writer and downstream readers.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::calibration::hull_white::capfloor_hw1f_scalar_keys;
///
/// let (kappa, sigma) = capfloor_hw1f_scalar_keys("USD-OIS");
/// assert_eq!(kappa, "USD-OIS_CAPFLOOR_HW1F_KAPPA");
/// assert_eq!(sigma, "USD-OIS_CAPFLOOR_HW1F_SIGMA");
/// ```
#[must_use]
pub fn capfloor_hw1f_scalar_keys(curve_id: &str) -> (String, String) {
    (
        format!("{curve_id}_CAPFLOOR_HW1F_KAPPA"),
        format!("{curve_id}_CAPFLOOR_HW1F_SIGMA"),
    )
}

/// Market quote for a European swaption used in HW1F calibration.
///
/// Represents an **ATM** European swaption with its market volatility. The
/// quote carries no strike: the calibrator always prices the swaption at the
/// forward swap rate implied by the supplied curve, so off-ATM quotes cannot
/// be represented by this type.
///
/// Deserialization rejects unknown fields and applies the same validation as
/// [`SwaptionQuote::try_new`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "SwaptionQuoteRaw")]
pub struct SwaptionQuote {
    /// Swaption expiry in years (T₀).
    pub expiry: f64,
    /// Underlying swap tenor in years (e.g. 5.0 for a 5Y swap).
    pub tenor: f64,
    /// Market-quoted volatility.
    pub volatility: f64,
    /// `true` for normal (Bachelier) vol, `false` for lognormal (Black-76) vol.
    pub is_normal_vol: bool,
}

/// Wire shape for [`SwaptionQuote`]: rejects unknown fields, then routes
/// through [`SwaptionQuote::try_new`] for value validation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SwaptionQuoteRaw {
    expiry: f64,
    tenor: f64,
    volatility: f64,
    is_normal_vol: bool,
}

impl TryFrom<SwaptionQuoteRaw> for SwaptionQuote {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: SwaptionQuoteRaw) -> Result<Self, Self::Error> {
        Self::try_new(raw.expiry, raw.tenor, raw.volatility, raw.is_normal_vol)
    }
}

/// Market quote for an interest-rate cap/floor used in HW1F calibration.
///
/// The quote represents a flat volatility for a full cap/floor from today to
/// `maturity`, with caplet/floorlet periods generated from the calibration
/// frequency. Normal vols are represented in decimal rate units: `0.0088`
/// means 88bp normal volatility.
///
/// Quotes are interpreted as standard market cap/floor quotes: the
/// spot-start caplet (whose rate fixes at `t = 0` and therefore carries no
/// optionality) is excluded from both the market and model legs, and each
/// caplet's option expiry is its fixing date (period start), not its payment
/// date.
///
/// Deserialization rejects unknown fields and applies the same validation as
/// [`CapFloorQuote::try_new`].
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "CapFloorQuoteRaw")]
pub struct CapFloorQuote {
    /// Cap/floor maturity in years.
    pub maturity: f64,
    /// Strike rate as a decimal.
    pub strike: f64,
    /// Market-quoted volatility.
    pub volatility: f64,
    /// `true` for cap, `false` for floor.
    pub is_cap: bool,
    /// `true` for normal (Bachelier) vol. Lognormal cap/floor HW1F
    /// calibration is intentionally not accepted yet.
    pub is_normal_vol: bool,
}

/// Wire shape for [`CapFloorQuote`]: rejects unknown fields, then routes
/// through [`CapFloorQuote::try_new`] for value validation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CapFloorQuoteRaw {
    maturity: f64,
    strike: f64,
    volatility: f64,
    is_cap: bool,
    is_normal_vol: bool,
}

impl TryFrom<CapFloorQuoteRaw> for CapFloorQuote {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: CapFloorQuoteRaw) -> Result<Self, Self::Error> {
        Self::try_new(
            raw.maturity,
            raw.strike,
            raw.volatility,
            raw.is_cap,
            raw.is_normal_vol,
        )
    }
}

impl CapFloorQuote {
    /// Construct a validated cap/floor market quote.
    pub fn try_new(
        maturity: f64,
        strike: f64,
        volatility: f64,
        is_cap: bool,
        is_normal_vol: bool,
    ) -> finstack_quant_core::Result<Self> {
        validate_cap_floor_quote(maturity, strike, volatility, is_normal_vol)?;
        Ok(Self {
            maturity,
            strike,
            volatility,
            is_cap,
            is_normal_vol,
        })
    }
}

/// Configuration for cap/floor HW1F calibration.
#[derive(Debug, Clone, Copy, Default)]
pub struct CapFloorCalibrationConfig {
    /// Payment frequency used to decompose full caps/floors into caplets.
    pub frequency: SwapFrequency,
    /// Optional source mean reversion. Required when calibrating from a
    /// single cap/floor quote because one quote cannot identify both κ and σ.
    pub fixed_kappa: Option<f64>,
    /// Optional initial guess when solving both κ and σ.
    pub initial_guess: Option<HullWhiteParams>,
}

impl SwaptionQuote {
    /// Construct a validated swaption market quote.
    pub fn try_new(
        expiry: f64,
        tenor: f64,
        volatility: f64,
        is_normal_vol: bool,
    ) -> finstack_quant_core::Result<Self> {
        if !expiry.is_finite() || expiry <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption expiry must be positive, got {expiry}"
            )));
        }
        if !tenor.is_finite() || tenor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption tenor must be positive, got {tenor}"
            )));
        }
        if !volatility.is_finite() || volatility <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Swaption volatility must be positive, got {volatility}"
            )));
        }
        Ok(Self {
            expiry,
            tenor,
            volatility,
            is_normal_vol,
        })
    }
}

/// Number of coupon payments per year for the underlying swap in HW1F calibration.
///
/// USD swaps are semi-annual (2), EUR swaps are annual (1).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    Default,
)]
pub enum SwapFrequency {
    /// 1 payment per year (EUR, GBP standard).
    Annual,
    /// 2 payments per year (USD standard).
    #[default]
    SemiAnnual,
    /// 4 payments per year.
    Quarterly,
}

impl SwapFrequency {
    pub(crate) fn periods_per_year(self) -> usize {
        match self {
            Self::Annual => 1,
            Self::SemiAnnual => 2,
            Self::Quarterly => 4,
        }
    }
}

/// HW1F κ hard-bounds check. Mean-reversion must lie in [1e-3, 1.0].
///
/// **Lower bound (`1e-3`):** below this, the mean-reversion half-life
/// `ln(2)/κ` exceeds 693y. More practically, `B(t,T) = (1 − e^{−κ(T−t)})/κ`
/// grows nearly linearly with `(T−t)` — at κ=1e-3, `B(0, 30) ≈ 29.55` —
/// and the bond-option vol `σ_P ∝ B(T,S) · σ · √variance_factor` blows up
/// for long-dated, volatile calibrations. Concretely: at κ=1e-3, σ=0.01,
/// T=20, B(20,21) ≈ 1.0, the variance factor `(1 − e^{−2κT})/(2κ) ≈ 19.6`,
/// so `σ_P ≈ 1.0 × 0.01 × √19.6 ≈ 0.044` per unit notional, which Brent
/// resolves robustly. Below κ=1e-3 the integrated-variance-time floor
/// becomes O(T) rather than O(1/κ), and the Jamshidian d1/d2 lose
/// numerical stability in the put-pricing formula.
///
/// **Upper bound (`1.0`):** above this, the half-life drops below 8 months
/// and the short rate is essentially absorbed at its instantaneous level
/// over typical (1Y+) swaption expiries — HW1F effectively collapses to
/// a Vasicek with no meaningful term structure for bond options.
const KAPPA_MIN: f64 = 0.001;
const KAPPA_MAX: f64 = 1.0;

/// Short-rate volatility bounds enforced as native LM box constraints in
/// log-space.
///
/// **Lower bound (`1e-5`):** 0.1 bp of annualised short-rate volatility — far
/// below any economically meaningful HW1F calibration but strictly positive,
/// so the log-space parameter `ln σ` stays finite. A smaller σ would make the
/// model degenerate (deterministic short rate) and the vega-scaled residual
/// ill-conditioned.
///
/// **Upper bound (`2.0`):** 200% annualised short-rate volatility. No realistic
/// rates market calibration approaches this; the cap simply keeps LM iterates
/// (and multi-start perturbations) from wandering into a regime where the
/// Jamshidian decomposition loses numerical accuracy.
const SIGMA_MIN: f64 = 1e-5;
const SIGMA_MAX: f64 = 2.0;

/// Vega floor: 1 bp of annuity-year. Protects against division by a
/// near-zero vega at extreme expiries or zero quoted vol.
///
/// The vega used here is evaluated once at the *market* quote and used to
/// scale `(price_model − price_mkt)` into an approximate vol-error
/// residual. That linearisation is a first-order Taylor approximation
/// valid only near the solution; see the residual computation in
/// `HullWhiteSwaptionTarget::calculate_residuals` for the full
/// approximation-regime discussion (W-38).
const SWAPTION_VEGA_FLOOR: f64 = 1e-8;

/// Apply [`SWAPTION_VEGA_FLOOR`] (or the supplied floor) to a quote-level vega
/// and surface the substitution to the caller.
///
/// Why this exists: when `actual_vega` is below the floor (deep OTM short
/// expiry, stale quote, zero quoted vol), the LM residual scaling
/// `(price_error) / vega` is replaced by `(price_error) / floor`. With
/// `floor = 1e-8`, that scaling factor is `1e8`, so the quote can dominate
/// the Gauss-Newton step while LM reports a clean termination. The audit
/// recommendation (item 1) is to surface every floor hit so the analyst can
/// drop or down-weight the offending quote.
///
/// Returns the floored vega; pushes a per-quote diagnostic into `hits` when
/// the floor was applied. The caller is responsible for forwarding `hits`
/// into the `CalibrationReport` metadata.
fn floor_vega_and_record(
    actual_vega: f64,
    floor: f64,
    quote_label: &str,
    hits: &mut Vec<String>,
) -> f64 {
    if !actual_vega.is_finite() || actual_vega < floor {
        tracing::warn!(
            quote = quote_label,
            actual_vega = actual_vega,
            vega_floor = floor,
            "HW1F vega floor applied: residual scaling (1/vega) is capped; \
             this quote may dominate the LM objective. Review or drop the quote."
        );
        hits.push(format!(
            "{quote_label}: actual_vega={actual_vega:.3e} below floor {floor:.3e}"
        ));
        floor
    } else {
        actual_vega
    }
}

/// Number of deterministic multi-start restarts used for HW1F calibration.
const HW_NUM_RESTARTS: usize = 5;
/// Halton perturbation scale (50%) applied to each parameter on restart.
const HW_PERTURB_SCALE: f64 = 0.5;
/// Validation tolerance reported on the HW1F calibration report.
const HW_VALIDATION_TOLERANCE: f64 = 1e-6;

/// Pre-computed market data for one swaption quote, captured once before
/// LM iteration so that the residual loop is a pure numeric computation.
///
/// `accruals` is the per-period payment-leg year-fraction sequence. When
/// `None` the calibrator uses the legacy constant-`tenor/n_periods` schedule
/// (preserved for the float-only public API and existing tests). When `Some`,
/// the supplied year fractions are used directly — see
/// [`calibrate_hull_white_to_swaptions_with_schedules`] for the recipe used
/// to build them from real (date, day-count) market data.
struct PreparedSwaption {
    market_price: f64,
    fwd_swap_rate: f64,
    vega: f64,
    accruals: Option<Box<[f64]>>,
}

/// `GlobalSolveTarget` impl carrying everything HW1F swaption calibration
/// needs to evaluate residuals. The borrowed `df` keeps the target zero-
/// allocation per residual call; the pre-computed market data avoids re-
/// pricing from quotes inside the LM hot loop.
struct HullWhiteSwaptionTarget<'a> {
    df: &'a dyn Fn(f64) -> f64,
    ppy: usize,
    initial_x0: [f64; 2],
    prepared: Vec<PreparedSwaption>,
}

impl<'a> GlobalSolveTarget for HullWhiteSwaptionTarget<'a> {
    type Quote = SwaptionQuote;
    type Curve = HullWhiteParams;

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
        Ok(HullWhiteParams { kappa, sigma })
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
                accruals: pre.accruals.as_deref(),
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

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        format!("{}Yx{}Y", quote.expiry, quote.tenor)
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
struct PreparedCapFloor {
    market_price: f64,
    vega: f64,
}

/// `GlobalSolveTarget` impl for HW1F cap/floor calibration. Used only on
/// the two-parameter path (κ, σ). The fixed-κ path stays on the existing
/// 1D Brent solver because a single scalar root-find does not benefit
/// from the LM machinery.
struct HullWhiteCapFloorTarget<'a> {
    discount_df: &'a dyn Fn(f64) -> f64,
    forward_df: &'a dyn Fn(f64) -> f64,
    frequency: SwapFrequency,
    initial_x0: [f64; 2],
    prepared: Vec<PreparedCapFloor>,
}

impl<'a> GlobalSolveTarget for HullWhiteCapFloorTarget<'a> {
    type Quote = CapFloorQuote;
    type Curve = HullWhiteParams;

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
        Ok(HullWhiteParams { kappa, sigma })
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

    fn residual_key(&self, quote: &Self::Quote, _idx: usize) -> String {
        format!(
            "{}Y_{}_{:.6}",
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

/// Calibrate Hull-White 1-factor parameters to European swaption market data.
///
/// Fits κ (mean reversion) and σ (short rate volatility) by minimising
/// squared differences between model and market swaption prices.
///
/// # Arguments
///
/// * `df` - Discount factor function: `df(t)` returns P(0, t). Must satisfy `df(0) ≈ 1`.
/// * `quotes` - Swaption market data.
/// * `frequency` - Coupon frequency of the underlying swap (e.g., semi-annual for USD,
///   annual for EUR). This materially affects the annuity factor and forward swap rate.
/// * `initial_guess` - Optional seed for (κ, σ). Pass `None` to use built-in defaults.
///
/// # Returns
///
/// Calibrated [`HullWhiteParams`] and a [`CalibrationReport`] with residual diagnostics.
///
/// # Algorithm
///
/// 1. For each swaption quote, compute the market price from the quoted vol.
/// 2. Model prices are computed analytically via the Jamshidian (1989) decomposition.
/// 3. The Levenberg-Marquardt solver minimises the sum of squared price errors,
///    routed through `GlobalFitOptimizer` so HW1F shares the same numeric
///    plumbing (multi-start, diagnostics, error reporting) as curve calibration.
/// 4. Uses the unconstrained parameterisation: `(ln κ, ln σ)`.
///
/// # Residual scaling (ATM assumption)
///
/// Each per-quote residual is `(price_model − price_mkt) / vega`, where
/// `vega` is the *ATM* Bachelier / Black-76 vega evaluated via
/// `swaption_atm_vega` (strike = forward swap rate). This linearisation
/// converges to the right minimiser when the calibration set is at-the-money
/// or close to it: at ATM the strike-vol slope is small and the ATM-vega
/// is a good proxy for the true `dPrice/dVol`. For materially off-ATM
/// quotes (deep ITM/OTM swaptions), the ATM-vega proxy under- or over-scales
/// the residual depending on the smile, and the LM objective is then a
/// *distorted* (but still descent-compatible) surface. If you need to
/// calibrate to a smile, weight or down-weight off-ATM quotes externally,
/// or invest in true implied-vol-error iteration as in Andersen-Piterbarg
/// (*Interest Rate Modeling* Vol III §3.3). A `vega_floor_hits` count is
/// reported in the result metadata; investigate any non-zero count before
/// trusting the calibrated `(κ, σ)`.
///
/// # Errors
///
/// Returns an error if:
/// - Fewer than 2 quotes are provided (2 free parameters)
/// - Calibration fails to converge
/// - Discount function returns invalid values
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::calibration::hull_white::{
///     calibrate_hull_white_to_swaptions, SwaptionQuote, SwapFrequency,
/// };
///
/// let quotes = vec![
///     SwaptionQuote { expiry: 1.0, tenor: 5.0, volatility: 0.005, is_normal_vol: true },
///     SwaptionQuote { expiry: 5.0, tenor: 5.0, volatility: 0.006, is_normal_vol: true },
///     SwaptionQuote { expiry: 10.0, tenor: 5.0, volatility: 0.005, is_normal_vol: true },
/// ];
///
/// // Flat 3% discount curve, semi-annual USD convention
/// let df = |t: f64| (-0.03 * t).exp();
/// let (params, report) = calibrate_hull_white_to_swaptions(
///     &df, &quotes, SwapFrequency::SemiAnnual, None,
/// ).unwrap();
/// assert!(report.success);
/// ```
pub fn calibrate_hull_white_to_swaptions(
    df: &dyn Fn(f64) -> f64,
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    initial_guess: Option<HullWhiteParams>,
) -> finstack_quant_core::Result<(HullWhiteParams, CalibrationReport)> {
    calibrate_hull_white_to_swaptions_core(df, quotes, frequency, None, initial_guess, None)
}

fn calibrate_hull_white_to_swaptions_core(
    df: &dyn Fn(f64) -> f64,
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    schedules: Option<&[Vec<f64>]>,
    initial_guess: Option<HullWhiteParams>,
    schedule_source: Option<&'static str>,
) -> finstack_quant_core::Result<(HullWhiteParams, CalibrationReport)> {
    if quotes.len() < 2 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Need at least 2 swaption quotes for HW1F calibration (2 free parameters), got {}",
            quotes.len()
        )));
    }
    if let Some(schedules) = schedules {
        if schedules.len() != quotes.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "schedules.len() ({}) must match quotes.len() ({})",
                schedules.len(),
                quotes.len()
            )));
        }
    }
    for (i, q) in quotes.iter().enumerate() {
        if q.expiry <= 0.0 || q.tenor <= 0.0 || q.volatility <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Invalid swaption quote at index {i}: expiry={}, tenor={}, vol={}",
                q.expiry, q.tenor, q.volatility
            )));
        }
    }

    let n_quotes = quotes.len();
    let ppy = frequency.periods_per_year();

    // Pre-compute market data once; the LM hot loop only does numeric ops.
    let mut prepared = Vec::with_capacity(n_quotes);
    let mut fwd_swap_rates = Vec::with_capacity(n_quotes);
    let mut vega_floor_hits: Vec<String> = Vec::new();
    let mut schedule_fallbacks: Vec<String> = Vec::new();
    for (idx, q) in quotes.iter().enumerate() {
        // Validate the per-quote schedule up-front with the same predicate
        // the pricer uses (`valid_swap_accruals`), so the metadata stamp
        // reflects what the calibration actually consumed. A malformed
        // schedule falls back to the synthetic constant-dt recipe — that
        // fallback is kept, but it is no longer silent: it is warned about
        // and stamped per quote in the report metadata.
        let n_periods = (q.tenor * ppy as f64).round().max(1.0) as usize;
        let accruals_slice =
            schedules.and_then(|s| valid_swap_accruals(Some(s[idx].as_slice()), n_periods));
        if schedules.is_some() && accruals_slice.is_none() {
            let label = format!("{}Yx{}Y", q.expiry, q.tenor);
            tracing::warn!(
                quote = label.as_str(),
                "HW1F swaption calibration: per-quote accrual schedule is malformed \
                 (wrong length or non-positive entries); falling back to the synthetic \
                 constant-dt schedule for this quote"
            );
            schedule_fallbacks.push(label);
        }
        let (annuity, fwd_rate) = if let Some(accruals) = accruals_slice {
            compute_swap_annuity_and_rate_inner(df, q.expiry, q.tenor, ppy, Some(accruals))
        } else {
            compute_swap_annuity_and_rate(df, q.expiry, q.tenor, ppy)
        };
        let market_price = compute_swaption_market_price(
            annuity,
            fwd_rate,
            q.expiry,
            q.volatility,
            q.is_normal_vol,
        );
        let raw_vega =
            swaption_atm_vega(annuity, fwd_rate, q.expiry, q.volatility, q.is_normal_vol);
        let label = format!("{}Yx{}Y", q.expiry, q.tenor);
        let vega =
            floor_vega_and_record(raw_vega, SWAPTION_VEGA_FLOOR, &label, &mut vega_floor_hits);
        prepared.push(PreparedSwaption {
            market_price,
            fwd_swap_rate: fwd_rate,
            vega,
            accruals: accruals_slice.map(|s| s.to_vec().into_boxed_slice()),
        });
        fwd_swap_rates.push(fwd_rate);
    }

    let (default_kappa_init, default_sigma_init) = infer_hw_initial_guess(quotes, &fwd_swap_rates);
    let kappa_init: f64 = initial_guess.map(|p| p.kappa).unwrap_or(default_kappa_init);
    let sigma_init: f64 = initial_guess.map(|p| p.sigma).unwrap_or(default_sigma_init);
    let x0 = [kappa_init.ln(), sigma_init.ln()];

    let target = HullWhiteSwaptionTarget {
        df,
        ppy,
        initial_x0: x0,
        prepared,
    };

    // Use solver tolerance 1e-12 (matches the prior hand-rolled LM
    // settings) and validation tolerance 1e-6 (the historical
    // accept/reject threshold for HW1F price residuals).
    let mut config = CalibrationConfig::default();
    config.solver = config.solver.with_tolerance(1e-12).with_max_iterations(300);

    let multi_start = MultiStartConfig {
        num_restarts: HW_NUM_RESTARTS,
        perturbation_scale: HW_PERTURB_SCALE,
    };

    let (params, mut report) = GlobalFitOptimizer::optimize_with_multi_start(
        &target,
        quotes,
        &config,
        Some(HW_VALIDATION_TOLERANCE),
        Some(&multi_start),
    )?;

    // Override the report type tag (stored in metadata["type"]) and add
    // HW-specific metadata. The framework reports a generic "global_fit"
    // type; HW consumers expect "hull_white_1f" for serialization stability.
    report = report
        .with_model_version(finstack_quant_core::versions::HULL_WHITE_1F)
        .with_metadata("type", "hull_white_1f".to_string())
        .with_metadata("kappa", format!("{:.6}", params.kappa))
        .with_metadata("sigma", format!("{:.6}", params.sigma))
        .with_metadata("initial_kappa", format!("{kappa_init:.6}"))
        .with_metadata("initial_sigma", format!("{sigma_init:.6}"))
        .with_metadata("multi_start_restarts", HW_NUM_RESTARTS.to_string())
        .with_metadata(
            "residual_weighting",
            "1/vega (vega-weighted price residual)".to_string(),
        )
        .with_metadata("swap_frequency", format!("{frequency:?}"))
        .with_metadata("vega_floor_hits", vega_floor_hits.len().to_string());
    if let Some(schedule_source) = schedule_source {
        // Stamp the actual per-basket source: if any quote fell back to the
        // synthetic schedule, the basket is "mixed" and the fallback quotes
        // are listed so the analyst can see exactly which quotes were not
        // priced on real day counts.
        let actual_source = if schedule_fallbacks.is_empty() {
            schedule_source.to_string()
        } else if schedule_fallbacks.len() == quotes.len() {
            "synthetic_constant_dt".to_string()
        } else {
            "mixed".to_string()
        };
        report = report.with_metadata("schedule_source", actual_source);
        if !schedule_fallbacks.is_empty() {
            report = report
                .with_metadata(
                    "schedule_fallback_count",
                    schedule_fallbacks.len().to_string(),
                )
                .with_metadata("schedule_fallback_quotes", schedule_fallbacks.join("; "));
        }
    }
    if !vega_floor_hits.is_empty() {
        report = report.with_metadata("vega_floor_hits_detail", vega_floor_hits.join("; "));
    }

    if !(KAPPA_MIN..=KAPPA_MAX).contains(&params.kappa) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Hull-White calibration produced κ = {:.6} outside the \
             bounded range [{KAPPA_MIN}, {KAPPA_MAX}]. This typically \
             indicates an under-weighted, over-damped, or under-specified \
             swaption grid; review the quotes or supply a bounded \
             `initial_guess`.",
            params.kappa
        )));
    }

    // Final validation of (κ, σ) > 0 — `HullWhiteParams::new` is the
    // canonical gate.
    let params = HullWhiteParams::new(params.kappa, params.sigma)?;
    Ok((params, report))
}

/// Calibrate HW1F to swaptions using *real* per-period accrual year fractions.
///
/// Functionally identical to [`calibrate_hull_white_to_swaptions`] but takes
/// per-quote accrual schedules so the synthetic constant-`dt` schedule is
/// replaced by genuine market day-counts (e.g. Act/360 USD SOFR, 30/360 EUR
/// EURIBOR). This brings calibrated `(κ, σ)` into tight parity with
/// vendor models (Bloomberg VCUB, QuantLib `Gaussian1dSwaptionEngine`) that
/// use real schedules.
///
/// # Arguments
///
/// * `schedules[i]` — per-period accrual year fractions for `quotes[i]`.
///   Must contain `(quotes[i].tenor * frequency.periods_per_year()).round()`
///   strictly-positive values; their sum must equal `quotes[i].tenor` to
///   within numerical precision. If any schedule is malformed, the calibrator
///   falls back to the constant-`dt` recipe for that quote, emits a
///   `tracing::warn!`, and stamps the report metadata: `schedule_source`
///   becomes `"mixed"` (or `"synthetic_constant_dt"` when every quote fell
///   back) and `schedule_fallback_quotes` lists the affected quotes.
///
/// # OIS-Specific Limitations
///
/// HW1F swaption calibration here treats every leg as a vanilla fixed-vs.-
/// IBOR swap. For OIS swaptions (compounded-in-arrears), the daily compounding
/// inside each accrual period is approximated by a single forward rate — the
/// HW1F r* equation does not capture the daily reset structure. This is
/// acceptable for ATM or near-ATM calibration (the loss is well below typical
/// market vol-of-vol noise) but is not appropriate for term-RFR-strict
/// calibration. The cap/floor path uses the analytical HW1F caplet vol
/// formula and is unaffected.
pub fn calibrate_hull_white_to_swaptions_with_schedules(
    df: &dyn Fn(f64) -> f64,
    quotes: &[SwaptionQuote],
    frequency: SwapFrequency,
    schedules: &[Vec<f64>],
    initial_guess: Option<HullWhiteParams>,
) -> finstack_quant_core::Result<(HullWhiteParams, CalibrationReport)> {
    calibrate_hull_white_to_swaptions_core(
        df,
        quotes,
        frequency,
        Some(schedules),
        initial_guess,
        Some("real_day_count"),
    )
}

/// Calibrate Hull-White 1-factor parameters to cap/floor market quotes.
///
/// Normal cap/floor quotes are first converted to Bachelier cap/floor prices
/// using the supplied discount and projection curves. The HW1F objective then
/// reprices the same cap/floor decomposition using HW1F-implied normal caplet
/// volatilities. A single quote requires `config.fixed_kappa`; otherwise the
/// two model parameters are underdetermined.
pub fn calibrate_hull_white_to_cap_floors(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    quotes: &[CapFloorQuote],
    config: CapFloorCalibrationConfig,
) -> finstack_quant_core::Result<(HullWhiteParams, CalibrationReport)> {
    if quotes.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "Need at least one cap/floor quote for HW1F calibration".to_string(),
        ));
    }
    if quotes.len() == 1 && config.fixed_kappa.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "One cap/floor quote cannot calibrate both HW1F kappa and sigma; provide fixed_kappa"
                .to_string(),
        ));
    }
    for (idx, quote) in quotes.iter().enumerate() {
        validate_cap_floor_quote(
            quote.maturity,
            quote.strike,
            quote.volatility,
            quote.is_normal_vol,
        )
        .map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "Invalid cap/floor quote at index {idx}: {err}"
            ))
        })?;
        // The spot-start caplet is excluded (it has no optionality), so a
        // quote must span at least two periods to contribute any caplet.
        let periods = (quote.maturity * config.frequency.periods_per_year() as f64)
            .round()
            .max(1.0) as usize;
        if periods < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor quote at index {idx} ({}Y at {:?} frequency) contains only the \
                 spot-start caplet, which is excluded from calibration; quote a longer maturity",
                quote.maturity, config.frequency
            )));
        }
    }

    let frequency = config.frequency;
    let market_prices: Vec<f64> = quotes
        .iter()
        .map(|quote| {
            bachelier_cap_floor_price(
                discount_df,
                forward_df,
                quote.maturity,
                quote.strike,
                quote.volatility,
                quote.is_cap,
                frequency,
            )
        })
        .collect();
    let mut vega_floor_hits: Vec<String> = Vec::new();
    let vegas: Vec<f64> = quotes
        .iter()
        .map(|quote| {
            let raw = cap_floor_bachelier_vega(
                discount_df,
                forward_df,
                quote.maturity,
                quote.strike,
                quote.volatility,
                frequency,
            );
            let label = format!(
                "{}Y_{}_{:.6}",
                quote.maturity,
                if quote.is_cap { "cap" } else { "floor" },
                quote.strike
            );
            floor_vega_and_record(raw, SWAPTION_VEGA_FLOOR, &label, &mut vega_floor_hits)
        })
        .collect();

    if let Some(fixed_kappa) = config.fixed_kappa {
        // Single-parameter (σ only) — keep the 1D path. The generic LM
        // machinery would add no value for a scalar minimisation.
        //
        // Guardrail parity with the two-parameter path: the fixed κ must
        // satisfy the same band the LM box constraints enforce, the σ search
        // spans up to SIGMA_MAX (not an arbitrary smaller cap), an at-bound
        // σ is rejected, and the report residuals are vega-scaled so the
        // validation tolerance is applied on the vol scale, matching the
        // two-parameter objective.
        let fixed = HullWhiteParams::new(fixed_kappa, 1e-4)?.kappa;
        if !(KAPPA_MIN..=KAPPA_MAX).contains(&fixed) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor HW1F fixed_kappa = {fixed:.6} outside the bounded range \
                 [{KAPPA_MIN}, {KAPPA_MAX}]"
            )));
        }
        let sigma = solve_cap_floor_sigma_for_fixed_kappa(
            fixed,
            discount_df,
            forward_df,
            quotes,
            &market_prices,
            frequency,
        )?;
        if sigma >= SIGMA_MAX * (1.0 - 1e-6) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Cap/floor HW1F sigma calibration hit the upper search bound \
                 ({sigma:.6} ≈ SIGMA_MAX = {SIGMA_MAX}); the quotes are inconsistent \
                 with the fixed kappa = {fixed:.6}"
            )));
        }
        let mut residuals = BTreeMap::new();
        for (idx, quote) in quotes.iter().enumerate() {
            let spec = CapFloorPriceSpec::from_quote(quote, frequency);
            let model_price = hw1f_cap_floor_price(fixed, sigma, discount_df, forward_df, spec);
            residuals.insert(
                format!(
                    "{}Y_{}_{:.6}",
                    quote.maturity,
                    if quote.is_cap { "cap" } else { "floor" },
                    quote.strike
                ),
                // Vega-scaled (vol-units) residual, matching the
                // two-parameter LM objective so HW_VALIDATION_TOLERANCE
                // means the same thing on both paths.
                (model_price - market_prices[idx]) / vegas[idx],
            );
        }
        let moneyness = cap_floor_moneyness_summary(quotes, forward_df, frequency);
        let report = enrich_cap_floor_report(
            CalibrationReport::for_type_with_tolerance(
                "hull_white_1f_cap_floor",
                residuals,
                1,
                HW_VALIDATION_TOLERANCE,
            ),
            fixed,
            sigma,
            quotes.len(),
            true,
            frequency,
            &vega_floor_hits,
            moneyness,
        );
        return Ok((HullWhiteParams::new(fixed, sigma)?, report));
    }

    // Two-parameter (κ, σ) path via GlobalFitOptimizer.
    let init = config.initial_guess.unwrap_or_default();
    let x0 = [init.kappa.ln(), init.sigma.ln()];

    let prepared: Vec<PreparedCapFloor> = market_prices
        .iter()
        .zip(vegas.iter())
        .map(|(&market_price, &vega)| PreparedCapFloor { market_price, vega })
        .collect();

    let target = HullWhiteCapFloorTarget {
        discount_df,
        forward_df,
        frequency,
        initial_x0: x0,
        prepared,
    };

    let mut config_lm = CalibrationConfig::default();
    config_lm.solver = config_lm
        .solver
        .with_tolerance(1e-12)
        .with_max_iterations(300);

    let multi_start = MultiStartConfig {
        num_restarts: HW_NUM_RESTARTS,
        perturbation_scale: HW_PERTURB_SCALE,
    };

    let (params, report) = GlobalFitOptimizer::optimize_with_multi_start(
        &target,
        quotes,
        &config_lm,
        Some(HW_VALIDATION_TOLERANCE),
        Some(&multi_start),
    )?;

    if !(KAPPA_MIN..=KAPPA_MAX).contains(&params.kappa) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Hull-White cap/floor calibration produced κ = {:.6} outside the bounded range [{KAPPA_MIN}, {KAPPA_MAX}]",
            params.kappa
        )));
    }

    let moneyness = cap_floor_moneyness_summary(quotes, forward_df, frequency);
    let report = enrich_cap_floor_report(
        report.with_metadata("type", "hull_white_1f_cap_floor".to_string()),
        params.kappa,
        params.sigma,
        quotes.len(),
        false,
        frequency,
        &vega_floor_hits,
        moneyness,
    );

    Ok((HullWhiteParams::new(params.kappa, params.sigma)?, report))
}

/// Apply cap/floor metadata shared by the fixed-kappa and two-parameter paths.
#[allow(clippy::too_many_arguments)]
fn enrich_cap_floor_report(
    report: CalibrationReport,
    kappa: f64,
    sigma: f64,
    quote_count: usize,
    fixed_kappa: bool,
    frequency: SwapFrequency,
    vega_floor_hits: &[String],
    moneyness: MoneynessSummary,
) -> CalibrationReport {
    let mut r = report
        .with_model_version(finstack_quant_core::versions::HULL_WHITE_1F)
        .with_metadata("kappa", format!("{kappa:.6}"))
        .with_metadata("sigma", format!("{sigma:.6}"))
        .with_metadata("quote_count", quote_count.to_string())
        .with_metadata("fixed_kappa", fixed_kappa.to_string())
        .with_metadata(
            "residual_weighting",
            "1/vega (vega-weighted price residual)".to_string(),
        )
        .with_metadata("calibration_family", "cap_floor_hw1f".to_string())
        .with_metadata("frequency", format!("{frequency:?}"))
        .with_metadata("vega_floor_hits", vega_floor_hits.len().to_string())
        // Audit P3a: off-ATM diagnostic. Vega-weighted residuals linearise
        // around the *ATM* vega, so quotes whose strikes are far from the
        // per-caplet forward rate sit outside the regime where the
        // linearisation is accurate. Report both the max and mean
        // |strike − fwd| / fwd across all caplets so an analyst can spot
        // when the calibration was driven by deep-OTM/ITM quotes (the LM
        // objective is still descent-compatible but its scaling is
        // distorted; see the HW1F module-level docstring).
        .with_metadata("max_moneyness_distance", format!("{:.6}", moneyness.max))
        .with_metadata("mean_moneyness_distance", format!("{:.6}", moneyness.mean));
    if !vega_floor_hits.is_empty() {
        r = r.with_metadata("vega_floor_hits_detail", vega_floor_hits.join("; "));
    }
    r
}

/// Aggregate off-ATM diagnostic: `|strike − caplet_forward| / caplet_forward`
/// summarised across every caplet of every cap/floor quote in the basket.
/// Returned zero for an empty basket or when forwards are non-positive.
#[derive(Clone, Copy, Debug, Default)]
struct MoneynessSummary {
    max: f64,
    mean: f64,
}

fn cap_floor_moneyness_summary(
    quotes: &[CapFloorQuote],
    forward_df: &dyn Fn(f64) -> f64,
    frequency: SwapFrequency,
) -> MoneynessSummary {
    let mut max_dist = 0.0_f64;
    let mut sum_dist = 0.0_f64;
    let mut count = 0_usize;
    for quote in quotes {
        for (t_start, t_end, _accrual) in cap_floor_periods(quote.maturity, frequency) {
            let fwd = forward_rate_from_df(forward_df, t_start, t_end);
            if !fwd.is_finite() || fwd.abs() < 1e-12 {
                continue;
            }
            let dist = ((quote.strike - fwd) / fwd).abs();
            if dist.is_finite() {
                max_dist = max_dist.max(dist);
                sum_dist += dist;
                count += 1;
            }
        }
    }
    if count == 0 {
        MoneynessSummary::default()
    } else {
        MoneynessSummary {
            max: max_dist,
            mean: sum_dist / count as f64,
        }
    }
}

/// ATM vega for a swaption expressed in the same volatility units as the
/// quote (Bachelier σ for normal vol, Black-76 σ for lognormal).
///
/// Used as the per-quote weight in the vega-weighted price residual; see
/// the module-level note in `calibrate_hull_white_to_swaptions`.
fn swaption_atm_vega(annuity: f64, fwd_rate: f64, expiry: f64, vol: f64, is_normal: bool) -> f64 {
    if is_normal {
        annuity
            * finstack_quant_core::math::volatility::bachelier_vega(fwd_rate, fwd_rate, vol, expiry)
    } else {
        annuity * finstack_quant_core::math::volatility::black_vega(fwd_rate, fwd_rate, vol, expiry)
    }
}

fn validate_cap_floor_quote(
    maturity: f64,
    strike: f64,
    volatility: f64,
    is_normal_vol: bool,
) -> finstack_quant_core::Result<()> {
    if !maturity.is_finite() || maturity <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor maturity must be positive, got {maturity}"
        )));
    }
    if !strike.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor strike must be finite, got {strike}"
        )));
    }
    if !volatility.is_finite() || volatility <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "cap/floor volatility must be positive, got {volatility}"
        )));
    }
    if !is_normal_vol {
        return Err(finstack_quant_core::Error::Validation(
            "cap/floor HW1F calibration currently requires normal/Bachelier vol quotes".to_string(),
        ));
    }
    Ok(())
}

/// Calibrate the HW1F volatility `sigma` against a basket of cap/floor quotes for a
/// fixed mean-reversion `kappa`.
///
/// # Item 7: minimise a residual norm, not a signed sum
///
/// A previous implementation root-found the **signed sum** `Σ (price_i − market_i)`
/// with a Brent solver. With more than one cap in the basket, opposite pricing errors
/// cancel in that sum: a `sigma` that overprices one cap and underprices another by the
/// same amount makes the signed sum zero, so Brent reports a "root" that is **not** a
/// least-squares fit — every individual cap can still be badly mispriced.
///
/// This implementation minimises the sum of squared residuals `Σ (price_i − market_i)²`
/// instead. Each cap/floor price is monotone in `sigma` (positive vega), so each squared
/// residual is unimodal in `sigma` and the SSE is unimodal — a golden-section search
/// over the plausible normal-vol range converges to the unique least-squares optimum.
fn solve_cap_floor_sigma_for_fixed_kappa(
    kappa: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    quotes: &[CapFloorQuote],
    market_prices: &[f64],
    frequency: SwapFrequency,
) -> finstack_quant_core::Result<f64> {
    // Sum of squared residuals across the whole basket. A non-finite price (pathological
    // sigma) is mapped to `+inf` so the minimiser steers away from it.
    let sse = |sigma: f64| -> f64 {
        let mut acc = 0.0_f64;
        for (quote, market_price) in quotes.iter().zip(market_prices.iter()) {
            let spec = CapFloorPriceSpec::from_quote(quote, frequency);
            let price = hw1f_cap_floor_price(kappa, sigma, discount_df, forward_df, spec);
            if !price.is_finite() {
                return f64::INFINITY;
            }
            let r = price - market_price;
            acc += r * r;
        }
        acc
    };

    // Plausible normal-vol search range for cap/floor sigma. The full
    // interval `[1e-8, SIGMA_MAX]` is split into three sub-brackets and each
    // is minimised independently. **Audit P2d**: a single golden-section
    // sweep assumes the SSE is unimodal in σ, which holds for a single quote
    // (each cap's price is monotone in σ) but **not** for multi-quote
    // baskets at different strikes where individual squared residuals can
    // bottom out at different σ values, creating local minima between
    // them. Multi-start with one bracket per decade catches that case at
    // negligible cost (the pricer runs ~200×3 = 600 times vs ~200×1).
    // The upper limit matches the two-parameter LM box constraint
    // (SIGMA_MAX) so both paths search the same σ domain.
    let brackets: [(f64, f64); 3] = [(1e-8, 5e-3), (5e-3, 5e-2), (5e-2, SIGMA_MAX)];

    // Reject the case where the objective is non-finite across the whole range — the
    // pricer cannot produce a usable fit and a silent bogus sigma must not be returned.
    let any_finite = brackets.iter().any(|&(lo, hi)| {
        sse(lo).is_finite() || sse(hi).is_finite() || sse(0.5 * (lo + hi)).is_finite()
    });
    if !any_finite {
        return Err(finstack_quant_core::Error::Validation(
            "Cap/floor HW1F sigma calibration objective is non-finite across the search range"
                .to_string(),
        ));
    }

    let mut best_sigma: Option<f64> = None;
    let mut best_sse = f64::INFINITY;
    for &(lo, hi) in &brackets {
        if let Some((sigma, sse_val)) = golden_section_min(&sse, lo, hi, 1e-12, 200) {
            if sse_val < best_sse {
                best_sse = sse_val;
                best_sigma = Some(sigma);
            }
        }
    }

    let sigma = best_sigma.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Cap/floor HW1F sigma calibration could not locate a finite minimum across any \
             search bracket"
                .to_string(),
        )
    })?;
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Cap/floor HW1F sigma calibration produced an invalid sigma: {sigma}"
        )));
    }
    Ok(sigma)
}

/// Golden-section minimisation of `f` on `[lo, hi]`. Returns the minimiser
/// `x` and `f(x)` after contracting the bracket below `x_tol`, capped at
/// `max_iters`. Returns `None` when the objective is non-finite at every
/// probe point in the bracket (the caller can then skip the bracket).
fn golden_section_min(
    f: &impl Fn(f64) -> f64,
    lo: f64,
    hi: f64,
    x_tol: f64,
    max_iters: usize,
) -> Option<(f64, f64)> {
    const INV_PHI: f64 = 0.618_033_988_749_894_8; // 1/φ
    let mut a = lo;
    let mut b = hi;
    let mut c = b - INV_PHI * (b - a);
    let mut d = a + INV_PHI * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    for _ in 0..max_iters {
        if (b - a).abs() <= x_tol {
            break;
        }
        if fc <= fd {
            b = d;
            d = c;
            fd = fc;
            c = b - INV_PHI * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + INV_PHI * (b - a);
            fd = f(d);
        }
    }
    let x = 0.5 * (a + b);
    let fx = f(x);
    if !x.is_finite() || x <= 0.0 || !fx.is_finite() {
        return None;
    }
    Some((x, fx))
}

/// Price a full cap/floor with a flat normal volatility quote.
pub(crate) fn bachelier_cap_floor_price(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    maturity: f64,
    strike: f64,
    normal_vol: f64,
    is_cap: bool,
    frequency: SwapFrequency,
) -> f64 {
    cap_floor_periods(maturity, frequency)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(forward_df, t_start, t_end);
            let df = discount_df(t_end);
            // Option expiry is the fixing time `t_start`, not the payment
            // time `t_end`: the caplet's rate is fixed at the period start
            // and accrues no vol afterwards.
            normal_caplet_price(forward, strike, normal_vol, t_start, accrual, df, is_cap)
        })
        .sum()
}

fn cap_floor_bachelier_vega(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    maturity: f64,
    strike: f64,
    normal_vol: f64,
    frequency: SwapFrequency,
) -> f64 {
    cap_floor_periods(maturity, frequency)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(forward_df, t_start, t_end);
            let df = discount_df(t_end);
            // Vol accrues only to the fixing time `t_start` (see
            // `bachelier_cap_floor_price`).
            normal_caplet_vega(forward, strike, normal_vol, t_start) * accrual * df
        })
        .sum()
}

/// Cap/floor shape used by HW1F pricing helpers.
#[derive(Clone, Copy)]
pub(crate) struct CapFloorPriceSpec {
    maturity: f64,
    strike: f64,
    is_cap: bool,
    frequency: SwapFrequency,
}

impl CapFloorPriceSpec {
    pub(crate) fn new(maturity: f64, strike: f64, is_cap: bool, frequency: SwapFrequency) -> Self {
        Self {
            maturity,
            strike,
            is_cap,
            frequency,
        }
    }

    fn from_quote(quote: &CapFloorQuote, frequency: SwapFrequency) -> Self {
        Self::new(quote.maturity, quote.strike, quote.is_cap, frequency)
    }
}

/// Price a full cap/floor exactly under HW1F by pricing each caplet as a
/// zero-coupon bond option.
///
/// A caplet fixing at `T`, paying `τ·max(L(T,S) − K, 0)` at `S`, equals
/// `(1 + τK)` zero-coupon bond **puts** with strike `X = 1/(1 + τK)` on
/// `P(T,S)`, expiring at `T`; a floorlet is the corresponding bond **call**
/// (Brigo–Mercurio §2.6 / Hull §31). The ZCB option is priced with the same
/// HW1F bond-option formula used by the Jamshidian swaption decomposition
/// ([`hw_bond_vol`]). This replaces the earlier mapping of HW bond vol to an
/// approximate forward-rate normal vol, which understated the caplet vol by
/// a `(1 + τF)` factor.
///
/// Dual-curve handling: the ZCB option is evaluated on the forward
/// (projection) curve and scaled by the deterministic discount/projection
/// basis `P_d(0,S)/P_f(0,S)`; for single-curve calibration the factor is 1
/// and the price is exact.
pub(crate) fn hw1f_cap_floor_price(
    kappa: f64,
    sigma: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> f64 {
    cap_floor_periods(spec.maturity, spec.frequency)
        .map(|(t_start, t_end, accrual)| {
            hw1f_caplet_price_zcb_option(
                kappa,
                sigma,
                discount_df,
                forward_df,
                t_start,
                t_end,
                accrual,
                spec.strike,
                spec.is_cap,
            )
        })
        .sum()
}

/// Exact HW1F caplet/floorlet price via the ZCB-option equivalence.
///
/// Returns NaN on pathological curve inputs (non-finite or non-positive
/// discount factors) so the calibration objective's non-finite-price error
/// contract keeps working.
#[allow(clippy::too_many_arguments)]
fn hw1f_caplet_price_zcb_option(
    kappa: f64,
    sigma: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    t_fix: f64,
    t_pay: f64,
    accrual: f64,
    strike: f64,
    is_cap: bool,
) -> f64 {
    let pf_fix = forward_df(t_fix);
    let pf_pay = forward_df(t_pay);
    let pd_pay = discount_df(t_pay);
    let valid_df = |p: f64| p.is_finite() && p > 0.0;
    if !valid_df(pf_fix) || !valid_df(pf_pay) || !valid_df(pd_pay) {
        return f64::NAN;
    }
    // Deterministic multiplicative discount/projection basis; 1.0 when the
    // two curves coincide (single-curve calibration).
    let basis = pd_pay / pf_pay;

    let gearing = 1.0 + accrual * strike;
    if gearing <= 0.0 {
        // Strike below −1/τ: a cap is always in the money (intrinsic), a
        // floor is worthless (assuming P(T,S) > 0 ⇔ 1 + τL > 0).
        if is_cap {
            let forward = (pf_fix / pf_pay - 1.0) / accrual;
            return basis * pf_pay * accrual * (forward - strike);
        }
        return 0.0;
    }
    let x_strike = 1.0 / gearing;

    let sigma_p = hw_bond_vol(kappa, sigma, 0.0, t_fix, t_pay);
    if sigma_p < 1e-15 {
        // Degenerate (zero vol or zero time to fixing): intrinsic value.
        let zcb_intrinsic = if is_cap {
            (x_strike * pf_fix - pf_pay).max(0.0)
        } else {
            (pf_pay - x_strike * pf_fix).max(0.0)
        };
        return basis * gearing * zcb_intrinsic;
    }

    let d1 = (pf_pay / (x_strike * pf_fix)).ln() / sigma_p + 0.5 * sigma_p;
    let d2 = d1 - sigma_p;
    // Caplet = (1+τK) × ZBP(0, T, S, X); floorlet = (1+τK) × ZBC(0, T, S, X).
    let zcb_option = if is_cap {
        x_strike * pf_fix * norm_cdf(-d2) - pf_pay * norm_cdf(-d1)
    } else {
        pf_pay * norm_cdf(d1) - x_strike * pf_fix * norm_cdf(d2)
    };
    let zcb_option_clamped = if zcb_option < 0.0 { 0.0 } else { zcb_option };
    basis * gearing * zcb_option_clamped
}

/// Return the flat normal vol that reproduces the HW1F cap/floor model price.
#[cfg(test)]
pub(crate) fn hw1f_cap_floor_implied_normal_vol(
    kappa: f64,
    sigma: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> f64 {
    let target = hw1f_cap_floor_price(kappa, sigma, discount_df, forward_df, spec);
    let residual = |vol: f64| -> f64 {
        bachelier_cap_floor_price(
            discount_df,
            forward_df,
            spec.maturity,
            spec.strike,
            vol,
            spec.is_cap,
            spec.frequency,
        ) - target
    };
    let mut hi = sigma.max(0.01);
    while residual(hi) < 0.0 && hi < 1.0 {
        hi *= 2.0;
    }
    BrentSolver::new()
        .tolerance(1e-12)
        .bracket_bounds(1e-10, hi)
        .solve(residual, hi * 0.5)
        .unwrap_or(hi)
}

pub(crate) fn hw1f_caplet_forward_rate_normal_vol(
    kappa: f64,
    sigma: f64,
    t_fix: f64,
    accrual: f64,
) -> f64 {
    if sigma <= 0.0 || t_fix <= 0.0 || accrual <= 0.0 {
        return 0.0;
    }
    const SMALL_KAPPA: f64 = 1e-8;
    let accrual_factor = if kappa.abs() < SMALL_KAPPA {
        1.0
    } else {
        (1.0 - (-kappa * accrual).exp()) / (kappa * accrual)
    };
    let integrated_variance_time = if kappa.abs() < SMALL_KAPPA {
        t_fix
    } else {
        (1.0 - (-2.0 * kappa * t_fix).exp()) / (2.0 * kappa)
    };
    sigma * accrual_factor * (integrated_variance_time / t_fix).sqrt()
}

/// Caplet periods `(t_start, t_end, accrual)` for a spot-start cap quote.
///
/// The first (spot-start) caplet is **excluded**: its rate fixes at `t = 0`,
/// so it carries no optionality, and standard market cap quotes exclude it.
/// Both the market (Bachelier) and model (HW1F) legs use this iterator, so
/// the convention is applied consistently to both sides of the calibration.
fn cap_floor_periods(
    maturity: f64,
    frequency: SwapFrequency,
) -> impl Iterator<Item = (f64, f64, f64)> {
    let periods = (maturity * frequency.periods_per_year() as f64)
        .round()
        .max(1.0) as usize;
    let accrual = maturity / periods as f64;
    (1..periods).map(move |idx| {
        let start = idx as f64 * accrual;
        let end = (idx + 1) as f64 * accrual;
        (start, end, accrual)
    })
}

/// Simple forward rate between `start` and `end` from a discount-factor
/// function.
///
/// Non-finite or non-positive discount factors propagate as `NaN` instead of
/// being clamped: callers (`HullWhiteCapFloorTarget::calculate_residuals`,
/// `solve_cap_floor_sigma_for_fixed_kappa`) rely on the non-finite-price
/// check to detect broken curves, and `f64::max` would silently absorb a NaN
/// (`NaN.max(1e-12) == 1e-12`), defeating that error contract.
fn forward_rate_from_df(df: &dyn Fn(f64) -> f64, start: f64, end: f64) -> f64 {
    let accrual = (end - start).max(1e-12);
    let p_start = df(start);
    let p_end = df(end);
    if !p_start.is_finite() || !p_end.is_finite() || p_start <= 0.0 || p_end <= 0.0 {
        return f64::NAN;
    }
    (p_start / p_end - 1.0) / accrual
}

fn normal_caplet_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    accrual: f64,
    df: f64,
    is_cap: bool,
) -> f64 {
    let annuity = accrual * df;
    if vol <= 0.0 || expiry <= 0.0 {
        let intrinsic = if is_cap {
            (forward - strike).max(0.0)
        } else {
            (strike - forward).max(0.0)
        };
        return intrinsic * annuity;
    }
    let sqrt_t = expiry.sqrt();
    let d = (forward - strike) / (vol * sqrt_t);
    let undiscounted = if is_cap {
        (forward - strike) * norm_cdf(d) + vol * sqrt_t * norm_pdf(d)
    } else {
        (strike - forward) * norm_cdf(-d) + vol * sqrt_t * norm_pdf(d)
    };
    undiscounted * annuity
}

fn normal_caplet_vega(forward: f64, strike: f64, vol: f64, expiry: f64) -> f64 {
    if vol <= 0.0 || expiry <= 0.0 {
        return 0.0;
    }
    let d = (forward - strike) / (vol * expiry.sqrt());
    expiry.sqrt() * norm_pdf(d)
}

// =============================================================================
// Futures convexity adjustment
// =============================================================================

/// Compute the Hull-White 1-factor futures convexity adjustment.
///
/// Returns the adjustment (in rate terms) to convert a futures rate to a forward rate:
/// `forward = futures_rate - convexity_adjustment`.
///
/// The full HW1F futures-forward adjustment (Hull, Technical Note #1;
/// Kirikos-Novak 1997):
///
/// $$
/// \text{CA} = \frac{\sigma^2}{4\kappa} \cdot \frac{B(T_1, T_2)}{T_2 - T_1}
/// \left[ B(T_1, T_2)\,\bigl(1 - e^{-2\kappa T_1}\bigr)
///      + 2\kappa\,B(0, T_1)^2 \right]
/// $$
///
/// where:
/// - $T_1$ = futures settlement time (years from today)
/// - $T_2$ = futures end time (maturity, years from today)
/// - $\sigma$ = HW1F short-rate volatility
/// - $\kappa$ = HW1F mean-reversion speed
/// - $B(t_1, t_2) = (1 - e^{-\kappa(t_2 - t_1)}) / \kappa$
///
/// In the $\kappa \to 0$ (Ho-Lee) limit this reduces to
/// $\tfrac{1}{2}\sigma^2 T_1 T_2$, which is handled by an explicit branch to
/// avoid $0/0$ cancellation.
///
/// # Arguments
/// * `kappa` - Mean-reversion speed
/// * `sigma` - Short-rate volatility
/// * `t_settle` - Settlement time in years ($T_1$)
/// * `t_end` - End/maturity time in years ($T_2$)
///
/// # Returns
/// The convexity adjustment in the same rate units as sigma.
pub fn hw1f_convexity_adjustment(kappa: f64, sigma: f64, t_settle: f64, t_end: f64) -> f64 {
    let tau = t_end - t_settle;
    if t_settle <= 0.0 || tau <= 0.0 {
        return 0.0;
    }
    const SMALL_KAPPA: f64 = 1e-8;
    if kappa.abs() < SMALL_KAPPA {
        // Ho-Lee limit: B(t1,t2) -> t2-t1 and the bracket collapses to
        // 2κ·T1·T2, cancelling the 1/(4κ) prefactor.
        return 0.5 * sigma * sigma * t_settle * t_end;
    }
    let b_0s = hw_b(kappa, 0.0, t_settle);
    let b_se = hw_b(kappa, t_settle, t_end);
    let bracket = b_se * (1.0 - (-2.0 * kappa * t_settle).exp()) + 2.0 * kappa * b_0s * b_0s;
    sigma * sigma / (4.0 * kappa) * (b_se / tau) * bracket
}

// =============================================================================
// Internal helpers
// =============================================================================

/// B(t₁, t₂) = (1 − e^{−κ(t₂−t₁)}) / κ
fn hw_b(kappa: f64, t1: f64, t2: f64) -> f64 {
    let tau = t2 - t1;
    if kappa.abs() < 1e-10 {
        tau
    } else {
        (1.0 - (-kappa * tau).exp()) / kappa
    }
}

/// Zero-coupon bond option volatility:
/// σ_P(t, T, S) = B(T,S) × σ × √((1 − e^{−2κ(T−t)}) / (2κ))
fn hw_bond_vol(kappa: f64, sigma: f64, t: f64, big_t: f64, s: f64) -> f64 {
    let b = hw_b(kappa, big_t, s);
    let var_factor = if kappa.abs() < 1e-10 {
        big_t - t
    } else {
        (1.0 - (-2.0 * kappa * (big_t - t)).exp()) / (2.0 * kappa)
    };
    b * sigma * var_factor.max(0.0).sqrt()
}

/// Compute ln A(t, T) for the HW1F affine bond price model.
///
/// ln A(t,T) = ln(P(0,T)/P(0,t)) + B(t,T) f(0,t) − (σ²/4κ)(1−e^{−2κt}) B(t,T)²
///
/// The instantaneous forward `f(0,t)` is approximated by a central finite
/// difference on `ln P(0,t)`. The FD error is benign for the Jamshidian
/// swaption decomposition: the same `ln A` enters both the strike
/// `K_i = A_i e^{−B_i r*}` (through the `r*` solve) and the bond-put
/// moneyness ratio `P(0,T_i)/(K_i P(0,T₀))`, so the `B(t,T)·f(0,t)` term —
/// and any error in it — cancels exactly between the two. An earlier
/// `forward_analytic` hook for supplying the curve's analytical forward was
/// removed for this reason: it was never wired and could not change prices.
fn hw_ln_a(kappa: f64, sigma: f64, t: f64, big_t: f64, df: &dyn Fn(f64) -> f64) -> f64 {
    let p0t = df(t);
    let p0_big_t = df(big_t);
    let b = hw_b(kappa, t, big_t);

    // Instantaneous forward rate: f(0,t) ≈ −d/dt ln P(0,t)
    let f0t = fd_forward_rate(df, t);

    let var_term = if kappa.abs() < 1e-10 {
        sigma * sigma * t * b * b / 2.0
    } else {
        sigma * sigma / (4.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp()) * b * b
    };

    (p0_big_t / p0t).ln() + b * f0t - var_term
}

#[inline]
fn fd_forward_rate(df: &dyn Fn(f64) -> f64, t: f64) -> f64 {
    let h = (t * 1e-3).clamp(1e-6, 1e-3);
    if t > h {
        -(df(t + h).ln() - df(t - h).ln()) / (2.0 * h)
    } else {
        // Near t = 0: use forward difference.
        -(df(h).ln()) / h
    }
}

/// Compute annuity and forward swap rate for a swap starting at `t0`
/// with given `tenor` and `periods_per_year` coupon payments.
///
/// The schedule is synthetic (constant `dt = tenor/n_periods`). For real
/// market day-counts (Act/360 USD SOFR, 30/360 EUR EURIBOR, etc.), use
/// [`compute_swap_annuity_and_rate_with_accruals`] and pass the actual
/// per-period year fractions.
pub(crate) fn compute_swap_annuity_and_rate(
    df: &dyn Fn(f64) -> f64,
    t0: f64,
    tenor: f64,
    periods_per_year: usize,
) -> (f64, f64) {
    compute_swap_annuity_and_rate_inner(df, t0, tenor, periods_per_year, None)
}

fn compute_swap_annuity_and_rate_inner(
    df: &dyn Fn(f64) -> f64,
    t0: f64,
    tenor: f64,
    periods_per_year: usize,
    accruals: Option<&[f64]>,
) -> (f64, f64) {
    let n_periods = (tenor * periods_per_year as f64).round().max(1.0) as usize;

    let real_accruals = valid_swap_accruals(accruals, n_periods);

    let mut annuity = 0.0;
    let mut t_running = t0;
    if let Some(accruals) = real_accruals {
        for &tau in accruals {
            t_running += tau;
            annuity += tau * df(t_running);
        }
    } else {
        let dt = tenor / n_periods as f64;
        for i in 1..=n_periods {
            let t_i = t0 + i as f64 * dt;
            annuity += dt * df(t_i);
        }
        t_running = t0 + tenor;
    }

    let t_n = t_running;
    let fwd_rate = if annuity > 1e-15 {
        (df(t0) - df(t_n)) / annuity
    } else {
        let p0 = df(t0).max(1e-12);
        let p_n = df(t_n).max(1e-12);
        ((p0 / p_n).ln() / tenor.max(1e-8)).max(0.0)
    };

    (annuity, fwd_rate)
}

#[inline]
fn valid_swap_accruals(accruals: Option<&[f64]>, n_periods: usize) -> Option<&[f64]> {
    accruals.filter(|a| a.len() == n_periods && a.iter().all(|x| x.is_finite() && *x > 0.0))
}

fn infer_hw_initial_guess(quotes: &[SwaptionQuote], fwd_swap_rates: &[f64]) -> (f64, f64) {
    let horizon = if quotes.is_empty() {
        5.0
    } else {
        quotes.iter().map(|q| q.expiry + 0.5 * q.tenor).sum::<f64>() / quotes.len() as f64
    };
    // Average ABSOLUTE-rate vol, branched per quote on the vol regime so
    // the σ seed never conflates Bachelier and Black quotes (W-39):
    //  - normal (Bachelier) quote: the vol is already an absolute-rate
    //    vol, so it contributes directly;
    //  - lognormal (Black) quote: the vol is dimensionless, so `vol·fwd`
    //    recovers an absolute-rate scale.
    // The HW1F σ is an absolute short-rate vol, so this average is the
    // right order of magnitude for the seed.
    let avg_abs_vol = if quotes.is_empty() {
        0.01 * 0.02 // fallback: ~1% Black vol at a 2% forward.
    } else {
        let sum: f64 = quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let v = q.volatility.abs();
                if q.is_normal_vol {
                    v
                } else {
                    // fwd_swap_rates is built quote-aligned by the callers;
                    // fall back to a 2% forward if the slice is short.
                    let fwd = fwd_swap_rates.get(i).map_or(0.02, |r| r.abs()).max(0.005);
                    v * fwd
                }
            })
            .sum();
        sum / quotes.len() as f64
    };

    let kappa_init = (1.0 / horizon.max(0.5)).clamp(0.01, 0.30);
    let sigma_init = avg_abs_vol.clamp(0.001, 0.05);
    (kappa_init, sigma_init)
}

/// Compute the market swaption price from the quoted volatility.
fn compute_swaption_market_price(
    annuity: f64,
    fwd_rate: f64,
    expiry: f64,
    vol: f64,
    is_normal: bool,
) -> f64 {
    if is_normal {
        // Bachelier: ATM payer price ≈ annuity × σ_n × √T × √(2/π) ≈ annuity × bachelier_call
        annuity
            * finstack_quant_core::math::volatility::bachelier_call(fwd_rate, fwd_rate, vol, expiry)
    } else {
        // Black-76: annuity × black_call(F, F, σ, T)
        annuity * finstack_quant_core::math::volatility::black_call(fwd_rate, fwd_rate, vol, expiry)
    }
}

/// Price a European payer swaption under HW1F using Jamshidian decomposition.
///
/// The Jamshidian decomposition expresses a swaption as a portfolio of
/// zero-coupon bond options. The key steps are:
///
/// 1. Find the critical short rate r* where the swap value equals par.
/// 2. Each leg becomes a put on a zero-coupon bond with strike K_i = P_HW(r*, T₀, T_i).
/// 3. Sum the individual zero-coupon bond put prices.
///
/// Uses a synthetic constant-`dt` schedule. The production HW1F calibrator
/// (`calibrate_hull_white_to_swaptions_with_schedules`) drives
/// [`hw1f_swaption_price_inner`] directly with real accrual fractions, so
/// this scalar-time wrapper exists primarily as a stable test harness.
#[allow(dead_code)]
pub(crate) fn hw1f_swaption_price(
    kappa: f64,
    sigma: f64,
    df: &dyn Fn(f64) -> f64,
    t0: f64,
    tenor: f64,
    swap_rate: f64,
    periods_per_year: usize,
) -> f64 {
    hw1f_swaption_price_inner(Hw1fSwaptionPriceInput {
        kappa,
        sigma,
        df,
        t0,
        tenor,
        swap_rate,
        periods_per_year,
        accruals: None,
    })
}

struct Hw1fSwaptionPriceInput<'a> {
    kappa: f64,
    sigma: f64,
    df: &'a dyn Fn(f64) -> f64,
    t0: f64,
    tenor: f64,
    swap_rate: f64,
    periods_per_year: usize,
    accruals: Option<&'a [f64]>,
}

fn hw1f_swaption_price_inner(
    Hw1fSwaptionPriceInput {
        kappa,
        sigma,
        df,
        t0,
        tenor,
        swap_rate,
        periods_per_year,
        accruals,
    }: Hw1fSwaptionPriceInput<'_>,
) -> f64 {
    let n_periods = (tenor * periods_per_year as f64).round().max(1.0) as usize;

    let real_accruals = valid_swap_accruals(accruals, n_periods);

    // Payment dates and cashflows
    let mut payment_times = Vec::with_capacity(n_periods);
    let mut cashflows = Vec::with_capacity(n_periods);
    if let Some(accruals) = real_accruals {
        let mut t_running = t0;
        for (i, &tau) in accruals.iter().enumerate() {
            t_running += tau;
            payment_times.push(t_running);
            let cf = if i + 1 < n_periods {
                swap_rate * tau
            } else {
                1.0 + swap_rate * tau
            };
            cashflows.push(cf);
        }
    } else {
        let dt = tenor / n_periods as f64;
        for i in 1..=n_periods {
            let t_i = t0 + i as f64 * dt;
            payment_times.push(t_i);
            let cf = if i < n_periods {
                swap_rate * dt
            } else {
                1.0 + swap_rate * dt
            };
            cashflows.push(cf);
        }
    }

    // Pre-compute B and ln A for each payment date
    let b_vals: Vec<f64> = payment_times
        .iter()
        .map(|&t_i| hw_b(kappa, t0, t_i))
        .collect();
    let ln_a_vals: Vec<f64> = payment_times
        .iter()
        .map(|&t_i| hw_ln_a(kappa, sigma, t0, t_i, df))
        .collect();

    // Find r* such that Σ c_i × A_i × exp(−B_i × r*) = 1
    // g(r) = Σ c_i exp(ln_A_i − B_i r) − 1
    // g'(r) = −Σ c_i B_i exp(ln_A_i − B_i r)
    let g = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_periods {
            sum += cashflows[i] * (ln_a_vals[i] - b_vals[i] * r).exp();
        }
        sum - 1.0
    };

    let g_prime = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_periods {
            sum -= cashflows[i] * b_vals[i] * (ln_a_vals[i] - b_vals[i] * r).exp();
        }
        sum
    };

    // Natural magnitude scale of `g'(r)` at a given `r`: the sum of the *absolute
    // values* of the per-cashflow terms that make up `g'`. `g'` itself is a signed sum
    // and can suffer catastrophic cancellation; comparing `|g'|` against this scale
    // (rather than a fixed absolute floor) detects a numerically near-flat objective.
    let g_prime_scale = |r: f64| -> f64 {
        let mut sum = 0.0;
        for i in 0..n_periods {
            sum += (cashflows[i] * b_vals[i] * (ln_a_vals[i] - b_vals[i] * r).exp()).abs();
        }
        sum
    };

    // Initial guess: the instantaneous forward rate at t0
    let h = (t0 * 1e-3).clamp(1e-6, 1e-3);
    let f0t0 = if t0 > h {
        -(df(t0 + h).ln() - df(t0 - h).ln()) / (2.0 * h)
    } else {
        -(df(h).ln()) / h
    };

    // Newton iterations to find r*.
    //
    // Derivative guard (item 5): a fixed `|g'| < 1e-15` *absolute* floor is the wrong
    // criterion. A `g'` of ~1e-10 — a near-flat objective — sails straight past it, and
    // `step = g / g'` then explodes to a ~1e8-scale jump that throws the iterate far
    // outside any plausible short-rate range. Two scale-aware guards replace it:
    //
    //  1. A *relative* derivative-magnitude guard: `|g'|` must be a non-trivial fraction
    //     of its own term-wise magnitude scale `Σ|c_i B_i e^…|`. This catches the
    //     catastrophic-cancellation regime where the signed sum `g'` collapses toward
    //     zero while its constituent terms are not.
    //  2. A safeguarded step bound: even a "large enough" `g'` can yield an absurd step
    //     when the objective is flat. A Newton step that would move `r` by more than
    //     `NEWTON_MAX_STEP` is untrustworthy; we hand off to the bracketed Brent
    //     fallback instead of accepting the jump.
    let mut r_star = f0t0;
    let mut newton_converged = false;
    const NEWTON_DERIV_REL_EPS: f64 = 1e-10;
    // Cap on a single Newton step in absolute short-rate units. A short rate moving by
    // more than 5.0 (500%) in one step is non-physical; the Brent fallback bracket is
    // sized to cover the plausible range under HW1F dynamics.
    const NEWTON_MAX_STEP: f64 = 5.0;
    for _ in 0..50 {
        let gv = g(r_star);
        let gp = g_prime(r_star);
        let gp_scale = g_prime_scale(r_star);
        // Near-flat / fully-cancelled derivative: hand off to Brent rather than take an
        // unbounded Newton step.
        if !gp.is_finite() || gp.abs() <= NEWTON_DERIV_REL_EPS * gp_scale.max(f64::MIN_POSITIVE) {
            break;
        }
        let step = gv / gp;
        // A non-finite or absurdly large step means the local linearisation is
        // unreliable (near-flat objective); stop and let Brent bracket the root.
        if !step.is_finite() || step.abs() > NEWTON_MAX_STEP {
            break;
        }
        r_star -= step;
        if step.abs() < 1e-12 {
            newton_converged = true;
            break;
        }
    }
    // Newton may have walked the iterate to a non-finite value before the step-size
    // convergence test fired; treat that as non-convergence so the Brent fallback runs.
    if !r_star.is_finite() {
        newton_converged = false;
    }

    // Brent fallback if Newton didn't converge.
    //
    // Bracket width must scale with both rate level and HW1F vol-to-expiry to
    // stay valid under negative-rate (EUR) and distressed-sovereign regimes.
    // The previous fixed `±0.20` bracket was too narrow for f0 ≈ 15% sovereign
    // yields and too narrow at long expiries where σ√t0 dominates.
    //
    // Heuristic: half-width = max(0.5, 5·σ√t0) — covers ±5σ of the short-rate
    // distribution under HW1F (more than enough to bracket r*) plus a 50bp
    // floor for short-expiry, low-vol cases.
    if !newton_converged {
        tracing::warn!(
            "HW1F r* Newton solver did not converge (kappa={kappa:.4}, sigma={sigma:.4}), \
             falling back to Brent"
        );
        let half_width = (5.0 * sigma * t0.sqrt()).max(0.5);
        let bracket_lo = f0t0 - half_width;
        let bracket_hi = f0t0 + half_width;
        let brent = BrentSolver::new()
            .tolerance(1e-12)
            .bracket_bounds(bracket_lo, bracket_hi);
        match brent.solve(g, f0t0) {
            Ok(r) => r_star = r,
            Err(_) => {
                tracing::warn!("HW1F r* Brent fallback also failed; returning NaN");
                r_star = f64::NAN;
            }
        }
    }

    // r* solver failure (NaN) and pathological discount factors must propagate
    // as NaN to the caller — `.max(0.0)` would silently turn NaN into 0.0
    // because IEEE 754 `max(NaN, 0.0) == 0.0`, fooling the LM closure into
    // treating the input as a legitimate zero-price swaption.
    if !r_star.is_finite() {
        return f64::NAN;
    }

    // Compute strike prices K_i = A_i × exp(−B_i × r*)
    let k_strikes: Vec<f64> = (0..n_periods)
        .map(|i| (ln_a_vals[i] - b_vals[i] * r_star).exp())
        .collect();

    // Sum zero-coupon bond put prices (payer swaption = portfolio of bond puts)
    // ZBO_put(0, T₀, T_i, K_i) = K_i P(0,T₀) N(−d₂) − P(0,T_i) N(−d₁)
    let p0_t0 = df(t0);
    if !(p0_t0 > 0.0 && p0_t0.is_finite()) {
        return f64::NAN;
    }
    let mut swaption_price = 0.0;

    for i in 0..n_periods {
        let t_i = payment_times[i];
        let p0_ti = df(t_i);
        if !(p0_ti > 0.0 && p0_ti.is_finite()) {
            return f64::NAN;
        }
        let sigma_p = hw_bond_vol(kappa, sigma, 0.0, t0, t_i);

        if sigma_p < 1e-15 {
            // Degenerate: intrinsic value. `< 0.0` is false for NaN so NaN
            // would propagate, but inputs are positive-finite by the checks
            // above, so the subtraction is safe.
            let put_intrinsic_raw = k_strikes[i] * p0_t0 - p0_ti;
            let put_intrinsic = if put_intrinsic_raw < 0.0 {
                0.0
            } else {
                put_intrinsic_raw
            };
            swaption_price += cashflows[i] * put_intrinsic;
            continue;
        }

        let d1 = ((p0_ti / (k_strikes[i] * p0_t0)).ln() + 0.5 * sigma_p * sigma_p) / sigma_p;
        let d2 = d1 - sigma_p;

        let put_price = k_strikes[i] * p0_t0 * norm_cdf(-d2) - p0_ti * norm_cdf(-d1);
        // Preserve NaN: `put_price < 0.0` is false for NaN, so NaN flows
        // through; only genuinely-negative numerical noise gets clamped.
        let put_price_clamped = if put_price < 0.0 { 0.0 } else { put_price };
        swaption_price += cashflows[i] * put_price_clamped;
    }

    if swaption_price < 0.0 {
        0.0
    } else {
        swaption_price
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat discount curve at a given continuously compounded rate.
    fn flat_df(rate: f64) -> impl Fn(f64) -> f64 {
        move |t: f64| (-rate * t).exp()
    }

    #[test]
    fn hw_params_validation() {
        assert!(HullWhiteParams::new(0.05, 0.01).is_ok());
        assert!(HullWhiteParams::new(0.0, 0.01).is_err()); // kappa = 0
        assert!(HullWhiteParams::new(-0.1, 0.01).is_err()); // kappa < 0
        assert!(HullWhiteParams::new(0.05, 0.0).is_err()); // sigma = 0
        assert!(HullWhiteParams::new(0.05, -0.01).is_err()); // sigma < 0
    }

    #[test]
    fn b_function_properties() {
        let p = HullWhiteParams::new(0.1, 0.01).expect("valid");
        let b = p.b_function(0.0, 1.0);
        // B(0, 1) = (1 − e^{−0.1}) / 0.1 ≈ 0.9516
        assert!((b - 0.9516).abs() < 0.001);

        // B should be positive and increasing in (t2 − t1)
        let b_short = p.b_function(0.0, 0.5);
        let b_long = p.b_function(0.0, 2.0);
        assert!(b_short < b);
        assert!(b < b_long);
    }

    #[test]
    fn bond_option_vol_positive() {
        let p = HullWhiteParams::new(0.05, 0.01).expect("valid");
        let vol = p.bond_option_vol(0.0, 1.0, 2.0);
        assert!(vol > 0.0, "Bond option vol should be positive: {vol}");
    }

    #[test]
    fn swaption_price_positive() {
        let df_fn = flat_df(0.03);
        let price = hw1f_swaption_price(0.05, 0.01, &df_fn, 1.0, 5.0, 0.03, 2);
        assert!(price > 0.0, "Swaption price should be positive: {price:.6}");
    }

    #[test]
    fn swaption_price_monotone_in_sigma() {
        let df_fn = flat_df(0.03);
        let fwd = {
            let (_, r) = compute_swap_annuity_and_rate(&df_fn, 1.0, 5.0, 2);
            r
        };
        let p_low = hw1f_swaption_price(0.05, 0.005, &df_fn, 1.0, 5.0, fwd, 2);
        let p_high = hw1f_swaption_price(0.05, 0.015, &df_fn, 1.0, 5.0, fwd, 2);
        assert!(
            p_high > p_low,
            "Higher sigma should give higher swaption price: {p_high:.6} vs {p_low:.6}"
        );
    }

    /// Item 5: under an extreme mean-reversion `kappa` the HW1F r* objective `g(r)`
    /// becomes near-flat — every `B(t0,t_i) ≈ 1/kappa` shrinks, so `g'(r) ≈ -Σ c_i/κ`
    /// is tiny (~1e-8 at κ=1e8, ~1e-10 at κ=1e10). The pre-fix Newton guard only
    /// rejected `|g'| < 1e-15`, so such a derivative passed through and `step = g/g'`
    /// exploded to a ~1e8–1e10-scale jump, throwing `r*` to a non-physical value that
    /// then poisoned the bond-option strikes and the swaption price.
    ///
    /// Post-fix the safeguarded step bound rejects the explosive Newton step and hands
    /// off to the bracketed Brent fallback, so the price stays finite and in the valid
    /// `[0, annuity]`-bounded range (a payer swaption can never be worth more than its
    /// fixed-leg annuity).
    #[test]
    fn item5_hw1f_r_star_extreme_kappa_does_not_explode() {
        let df_fn = flat_df(0.03);
        let (annuity, fwd) = compute_swap_annuity_and_rate(&df_fn, 1.0, 5.0, 2);

        for kappa in [1.0e6_f64, 1.0e8, 1.0e10] {
            let price = hw1f_swaption_price(kappa, 0.01, &df_fn, 1.0, 5.0, fwd, 2);
            assert!(
                price.is_finite(),
                "swaption price must stay finite under extreme kappa={kappa:e}; \
                 the r* Newton step must not explode (got {price})"
            );
            assert!(
                price >= 0.0,
                "swaption price must be non-negative under extreme kappa={kappa:e}; got {price}"
            );
            // A payer swaption is a portfolio of bond puts; its value cannot exceed the
            // fixed-leg annuity. An exploded r* would blow this bound.
            assert!(
                price <= annuity * 1.0001,
                "swaption price {price} exceeds the annuity bound {annuity} \
                 under extreme kappa={kappa:e} — r* likely exploded"
            );
        }
    }

    #[test]
    fn calibrate_hw1f_round_trip() {
        let true_kappa = 0.05;
        let true_sigma = 0.01;
        let rate = 0.03;
        let df_fn = flat_df(rate);
        let ppy = SwapFrequency::SemiAnnual.periods_per_year();

        let swaption_specs: Vec<(f64, f64)> =
            vec![(1.0, 5.0), (2.0, 5.0), (5.0, 5.0), (1.0, 10.0), (5.0, 10.0)];

        let quotes: Vec<SwaptionQuote> = swaption_specs
            .iter()
            .map(|&(expiry, tenor)| {
                let (annuity, fwd_rate) = compute_swap_annuity_and_rate(&df_fn, expiry, tenor, ppy);
                let model_price = hw1f_swaption_price(
                    true_kappa, true_sigma, &df_fn, expiry, tenor, fwd_rate, ppy,
                );

                let normal_vol = if annuity > 1e-15 && expiry > 0.0 {
                    let approx_vol =
                        model_price / (annuity * (expiry / (2.0 * std::f64::consts::PI)).sqrt());
                    approx_vol.max(1e-6)
                } else {
                    0.005
                };

                SwaptionQuote {
                    expiry,
                    tenor,
                    volatility: normal_vol,
                    is_normal_vol: true,
                }
            })
            .collect();

        let (params, report) =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::default(), None)
                .expect("Calibration should succeed");

        assert!(
            report.success,
            "Calibration should succeed: {}",
            report.convergence_reason
        );
        assert!(
            params.kappa > 0.0 && params.kappa < 1.0,
            "kappa should be reasonable: {:.4}",
            params.kappa
        );
        assert!(
            params.sigma > 0.0 && params.sigma < 0.1,
            "sigma should be reasonable: {:.4}",
            params.sigma
        );
    }

    #[test]
    fn calibrate_hw1f_annual_vs_semiannual_produces_different_params() {
        let df_fn = flat_df(0.03);
        let quotes = vec![
            SwaptionQuote {
                expiry: 1.0,
                tenor: 5.0,
                volatility: 0.005,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 5.0,
                tenor: 5.0,
                volatility: 0.006,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 10.0,
                tenor: 5.0,
                volatility: 0.005,
                is_normal_vol: true,
            },
        ];

        let (params_semi, _) =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::SemiAnnual, None)
                .expect("semi-annual");
        let (params_ann, _) =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::Annual, None)
                .expect("annual");

        assert!(
            (params_semi.kappa - params_ann.kappa).abs() > 1e-6
                || (params_semi.sigma - params_ann.sigma).abs() > 1e-6,
            "Different frequencies should produce different params: semi={:?} ann={:?}",
            params_semi,
            params_ann
        );
    }

    #[test]
    fn test_hw1f_brent_fallback_extreme_params() {
        let kappa = 5.0;
        let sigma = 0.03;
        let df = flat_df(0.03);

        let price = hw1f_swaption_price(kappa, sigma, &df, 1.0, 5.0, 0.03, 2);
        assert!(
            price.is_finite(),
            "Swaption price should be finite with Brent fallback"
        );
        assert!(price >= 0.0, "Swaption price must be non-negative");
    }

    #[test]
    fn calibrate_hw1f_rejects_insufficient_quotes() {
        let quotes = vec![SwaptionQuote {
            expiry: 1.0,
            tenor: 5.0,
            volatility: 0.005,
            is_normal_vol: true,
        }];
        let df_fn = flat_df(0.03);
        let result =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::default(), None);
        assert!(result.is_err(), "Should reject < 2 quotes");
    }

    // ========================================================================
    // HW1F vega-weighted calibration + multi-start
    // ========================================================================

    /// Wide-grid round-trip: generate ATM normal vols from a known
    /// `(κ*, σ*) = (0.08, 0.012)` on a 10-swaption co-terminal-style
    /// grid spanning 1Y to 10Y expiries × 5Y and 10Y tenors, then verify
    /// the calibrator recovers κ in a tight neighbourhood of κ*.
    ///
    /// Pre-fix: the **unweighted** price residual let the 10Y×10Y quote
    /// (largest annuity → largest price) dominate the objective; the LM
    /// solver minimised overall price error by pushing κ toward zero
    /// (which widens the long-dated bond-option vol and soaks up most of
    /// the residual) at the cost of a 20–30 bp vol error on the 1Y
    /// quotes. The vega-weighted residual (post-fix) puts every quote
    /// on an implied-vol scale and multi-start escapes the flat κ→0
    /// region of the objective surface.
    #[test]
    fn hw1f_calibration_recovers_kappa_on_wide_round_trip_grid() {
        let true_kappa = 0.08_f64;
        let true_sigma = 0.012_f64;
        let df_fn = flat_df(0.03);
        let ppy = SwapFrequency::SemiAnnual.periods_per_year();

        // 10-swaption co-terminal grid.
        let specs: &[(f64, f64)] = &[
            (1.0, 5.0),
            (2.0, 5.0),
            (3.0, 5.0),
            (5.0, 5.0),
            (7.0, 5.0),
            (10.0, 5.0),
            (1.0, 10.0),
            (3.0, 10.0),
            (5.0, 10.0),
            (10.0, 10.0),
        ];

        // Back out the implied normal vol from the model price so the
        // resulting quotes are internally consistent with (κ*, σ*). Use
        // the Bachelier ATM relation: price ≈ annuity · σ_n · √T / √(2π).
        let quotes: Vec<SwaptionQuote> = specs
            .iter()
            .map(|&(expiry, tenor)| {
                let (annuity, fwd_rate) = compute_swap_annuity_and_rate(&df_fn, expiry, tenor, ppy);
                let model_price = hw1f_swaption_price(
                    true_kappa, true_sigma, &df_fn, expiry, tenor, fwd_rate, ppy,
                );
                let vol = model_price / (annuity * (expiry / (2.0 * std::f64::consts::PI)).sqrt());
                SwaptionQuote {
                    expiry,
                    tenor,
                    volatility: vol.max(1e-6),
                    is_normal_vol: true,
                }
            })
            .collect();

        let (params, report) =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::SemiAnnual, None)
                .expect("calibration should succeed");

        assert!(
            report.success,
            "calibration should converge, got: {}",
            report.convergence_reason
        );

        // Recovery tolerance: κ within 20% of the true value — tight
        // enough to fail pre-fix (where the unweighted residual pulled κ
        // into the single-digit-bp range) but permissive enough to
        // accommodate the LM convergence tolerance and multi-start
        // noise.
        assert!(
            (true_kappa * 0.8..=true_kappa * 1.2).contains(&params.kappa),
            "κ = {:.6} not within 20% of κ* = {true_kappa:.6}; \
             pre-fix C8 behaviour was to push κ toward zero on wide \
             expiry grids because the unweighted price residual let \
             long-dated quotes dominate",
            params.kappa
        );
        assert!(
            (true_sigma * 0.5..=true_sigma * 1.5).contains(&params.sigma),
            "σ = {:.6} not within 50% of σ* = {true_sigma:.6}",
            params.sigma
        );
    }

    /// κ out of bounds `[0.001, 1.0]` must return `Err` rather than a
    /// `tracing::warn!`-and-succeed. Use synthetic quotes with
    /// inconsistent rate/tenor structure to push the calibration to a
    /// pathological κ if it converges at all.
    #[test]
    fn hw1f_calibration_errors_when_kappa_drives_out_of_bounds() {
        // Construct pathological quotes: essentially flat very low vol
        // across a wide expiry grid. The LM will tend toward κ → 0 +
        // σ → 0; the post-fix implementation should either (a) find a
        // feasible κ in-bounds thanks to multi-start or (b) return an
        // OutOfBounds error. Both outcomes are acceptable; a silent
        // warn-and-return path is NOT.
        let df_fn = flat_df(0.03);
        let quotes: Vec<SwaptionQuote> = (1..=10)
            .map(|i| SwaptionQuote {
                expiry: i as f64,
                tenor: 5.0,
                volatility: 1e-6, // ~0 bp
                is_normal_vol: true,
            })
            .collect();

        let result =
            calibrate_hull_white_to_swaptions(&df_fn, &quotes, SwapFrequency::SemiAnnual, None);

        match result {
            Ok((params, _)) => {
                assert!(
                    (0.001..=1.0).contains(&params.kappa),
                    "κ = {:.6} outside hard bounds [0.001, 1.0]; Err expected \
                     rather than a warn-and-succeed path",
                    params.kappa
                );
            }
            Err(e) => {
                let msg = format!("{e}");
                assert!(
                    msg.contains("κ") || msg.contains("kappa") || msg.contains("bounded"),
                    "error message must identify κ-bounds violation: {msg}"
                );
            }
        }
    }

    #[test]
    fn cap_floor_hw1f_calibration_rejects_one_quote_without_fixed_kappa() {
        let df_fn = flat_df(0.03);
        let quotes = vec![CapFloorQuote {
            maturity: 5.0,
            strike: 0.03,
            volatility: 0.0075,
            is_cap: true,
            is_normal_vol: true,
        }];

        let result = calibrate_hull_white_to_cap_floors(
            &df_fn,
            &df_fn,
            &quotes,
            CapFloorCalibrationConfig::default(),
        );

        assert!(
            result.is_err(),
            "one cap/floor quote cannot calibrate both kappa and sigma"
        );
    }

    /// Item 7: fixed-kappa cap/floor sigma calibration must minimise a residual NORM,
    /// not a signed sum. With an inconsistent basket (no single sigma fits every cap),
    /// the signed-sum root lets opposite errors cancel and lands on a sigma that is not
    /// the least-squares optimum.
    ///
    /// Construct two caps of differing maturity (hence differing vega) and feed market
    /// prices generated at *different* sigmas — `0.004` for the short cap, `0.020` for
    /// the long cap — so no single sigma reprices both. The calibrated sigma must be the
    /// SSE minimiser: `SSE(sigma*)` must be no worse than `SSE` a small step either side,
    /// and strictly better than the SSE at the signed-sum root.
    #[test]
    fn item7_cap_floor_fixed_kappa_minimises_norm_not_signed_sum() {
        let kappa = 0.03_f64;
        let df_fn = flat_df(0.035);
        let freq = SwapFrequency::Quarterly;

        // Two caps, very different maturities -> very different vega.
        let q_short = CapFloorQuote {
            maturity: 2.0,
            strike: 0.035,
            volatility: 0.0, // unused for price-basket construction below
            is_cap: true,
            is_normal_vol: true,
        };
        let q_long = CapFloorQuote {
            maturity: 10.0,
            strike: 0.035,
            volatility: 0.0,
            is_cap: true,
            is_normal_vol: true,
        };
        let quotes = [q_short, q_long];

        // Inconsistent market prices: short cap priced at sigma=0.004, long at 0.020.
        let spec_short = CapFloorPriceSpec::from_quote(&q_short, freq);
        let spec_long = CapFloorPriceSpec::from_quote(&q_long, freq);
        let market = [
            hw1f_cap_floor_price(kappa, 0.004, &df_fn, &df_fn, spec_short),
            hw1f_cap_floor_price(kappa, 0.020, &df_fn, &df_fn, spec_long),
        ];

        let sigma =
            solve_cap_floor_sigma_for_fixed_kappa(kappa, &df_fn, &df_fn, &quotes, &market, freq)
                .expect("fixed-kappa sigma calibration should succeed");

        // SSE objective replicated locally.
        let sse = |s: f64| -> f64 {
            let r0 = hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_short) - market[0];
            let r1 = hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_long) - market[1];
            r0 * r0 + r1 * r1
        };
        // Signed-sum objective (the pre-fix root-find target).
        let signed_sum = |s: f64| -> f64 {
            (hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_short) - market[0])
                + (hw1f_cap_floor_price(kappa, s, &df_fn, &df_fn, spec_long) - market[1])
        };

        // 1. The returned sigma is a genuine SSE minimum (no better point nearby).
        let delta = 1e-4;
        let f_star = sse(sigma);
        assert!(
            f_star <= sse(sigma + delta) && f_star <= sse(sigma - delta),
            "calibrated sigma={sigma} is not an SSE minimum: \
             SSE(sigma)={f_star:.3e}, SSE(+d)={:.3e}, SSE(-d)={:.3e}",
            sse(sigma + delta),
            sse(sigma - delta),
        );

        // 2. Bracket the signed-sum root and confirm it is a DIFFERENT, worse point.
        //    signed_sum is monotone increasing in sigma; bisect for its zero.
        let (mut lo, mut hi) = (1e-8_f64, 1.0_f64);
        if signed_sum(lo) < 0.0 && signed_sum(hi) > 0.0 {
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if signed_sum(mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let signed_root = 0.5 * (lo + hi);
            // The signed-sum root cancels opposite errors; its SSE is strictly worse.
            assert!(
                sse(signed_root) > f_star,
                "the signed-sum root sigma={signed_root} has SSE {:.3e} which is not \
                 worse than the norm-minimising SSE {f_star:.3e} — the fix did not \
                 change behaviour",
                sse(signed_root),
            );
        }
    }

    #[test]
    fn cap_floor_hw1f_calibration_solves_sigma_with_fixed_kappa() {
        let true_kappa = 0.0342;
        let true_sigma = 0.0095;
        let df_fn = flat_df(0.037);
        let quotes = vec![CapFloorQuote {
            maturity: 5.0,
            strike: 0.0365,
            volatility: hw1f_cap_floor_implied_normal_vol(
                true_kappa,
                true_sigma,
                &df_fn,
                &df_fn,
                CapFloorPriceSpec::new(5.0, 0.0365, true, SwapFrequency::Quarterly),
            ),
            is_cap: true,
            is_normal_vol: true,
        }];

        let (params, report) = calibrate_hull_white_to_cap_floors(
            &df_fn,
            &df_fn,
            &quotes,
            CapFloorCalibrationConfig {
                fixed_kappa: Some(true_kappa),
                ..CapFloorCalibrationConfig::default()
            },
        )
        .expect("fixed-kappa cap/floor calibration succeeds");

        assert!(report.success, "report should be successful: {report:?}");
        assert!((params.kappa - true_kappa).abs() < 1e-12);
        assert!(
            (params.sigma - true_sigma).abs() < 1e-4,
            "sigma {} should recover true sigma {true_sigma}",
            params.sigma
        );
    }

    #[test]
    fn cap_floor_hw1f_calibration_recovers_two_parameters_on_synthetic_grid() {
        let true_kappa = 0.05;
        let true_sigma = 0.011;
        let df_fn = flat_df(0.035);
        let specs = [(2.0, 0.034), (5.0, 0.036), (7.0, 0.037)];
        let quotes: Vec<CapFloorQuote> = specs
            .iter()
            .map(|(maturity, strike)| CapFloorQuote {
                maturity: *maturity,
                strike: *strike,
                volatility: hw1f_cap_floor_implied_normal_vol(
                    true_kappa,
                    true_sigma,
                    &df_fn,
                    &df_fn,
                    CapFloorPriceSpec::new(*maturity, *strike, true, SwapFrequency::Quarterly),
                ),
                is_cap: true,
                is_normal_vol: true,
            })
            .collect();

        let (params, report) = calibrate_hull_white_to_cap_floors(
            &df_fn,
            &df_fn,
            &quotes,
            CapFloorCalibrationConfig {
                frequency: SwapFrequency::Quarterly,
                initial_guess: Some(HullWhiteParams::new(0.04, 0.01).expect("guess")),
                ..CapFloorCalibrationConfig::default()
            },
        )
        .expect("two-parameter cap/floor calibration succeeds");

        assert!(report.success, "report should be successful: {report:?}");
        assert!(
            (true_kappa * 0.8..=true_kappa * 1.2).contains(&params.kappa),
            "kappa {} should recover true kappa {true_kappa}",
            params.kappa
        );
        assert!(
            (true_sigma * 0.8..=true_sigma * 1.2).contains(&params.sigma),
            "sigma {} should recover true sigma {true_sigma}",
            params.sigma
        );
    }

    /// Regression: a non-finite model price for one quote must cause
    /// `calculate_residuals` to return `Err` (which the global LM solver
    /// converts into a bounded penalty via `fill_penalty`) rather than
    /// injecting a magic `1e6` literal directly into the residual buffer.
    ///
    /// Pre-fix, a single bad quote contributed a hard-coded `1e6` as a
    /// genuine residual; scaled by `1/vega` it dominated the Gauss-Newton
    /// step. Post-fix the buffer is left untouched and the solver applies
    /// proper infeasibility handling.
    #[test]
    fn hw1f_residuals_signal_err_on_non_finite_price_no_magic_literal() {
        // A discount factor closure that returns NaN forces the swaption
        // pricer to produce a non-finite price deterministically.
        let nan_df = |_t: f64| f64::NAN;

        let quotes = vec![
            SwaptionQuote {
                expiry: 1.0,
                tenor: 5.0,
                volatility: 0.005,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 5.0,
                tenor: 5.0,
                volatility: 0.006,
                is_normal_vol: true,
            },
        ];

        let prepared: Vec<PreparedSwaption> = quotes
            .iter()
            .map(|_| PreparedSwaption {
                market_price: 0.01,
                fwd_swap_rate: 0.03,
                vega: 0.5,
                accruals: None,
            })
            .collect();

        let target = HullWhiteSwaptionTarget {
            df: &nan_df,
            ppy: SwapFrequency::SemiAnnual.periods_per_year(),
            initial_x0: [(-2.5_f64), (-4.0_f64)],
            prepared,
        };
        let curve = HullWhiteParams {
            kappa: 0.08,
            sigma: 0.012,
        };

        // Sentinel buffer: if the bug regressed, the implementation would
        // overwrite an entry with a `1e6`-style literal. We pre-fill with a
        // recognisable marker and assert it is never replaced by a magic
        // residual on the infeasible path.
        let mut residuals = vec![-7.0_f64; quotes.len()];
        let result = target.calculate_residuals(&curve, &quotes, &mut residuals);

        let err = result.expect_err("non-finite price must yield Err, not a 1e6 residual");
        let msg = format!("{err}");
        assert!(
            msg.contains("non-finite") && msg.contains("1Yx5Y"),
            "error must name the offending quote and the failure mode: {msg}"
        );
        // No entry was overwritten with a magic penalty literal: the marker
        // survives, proving `1e6` is no longer treated as a real residual.
        assert!(
            residuals.iter().all(|&r| r == -7.0),
            "residual buffer must not contain an injected magic literal: {residuals:?}"
        );

        // End-to-end: the full calibration with the same NaN curve must
        // fail cleanly rather than silently converge to a poisoned minimum.
        let calib =
            calibrate_hull_white_to_swaptions(&nan_df, &quotes, SwapFrequency::SemiAnnual, None);
        assert!(
            calib.is_err() || calib.as_ref().is_ok_and(|(_, report)| !report.success),
            "calibration on a degenerate (NaN-priced) curve must report \
             non-convergence rather than accept a 1e6-dominated minimum"
        );
    }

    /// W-39: the σ seed must not conflate Bachelier (normal) and Black
    /// (lognormal) vol regimes. For NORMAL swaption quotes the quoted vol
    /// is already an absolute short-rate-scale vol; multiplying it by the
    /// forward swap rate (`avg_fwd ≈ 0.03`) wrongly shrinks it by ~30×,
    /// and the `clamp(0.001, …)` floor then masks the bug while still
    /// leaving the seed an order of magnitude too small.
    #[test]
    fn infer_hw_initial_guess_normal_vol_seed_is_right_order_of_magnitude() {
        // A normal-vol swaption set with ~80 bp absolute-rate vol.
        let quotes = vec![
            SwaptionQuote {
                expiry: 1.0,
                tenor: 5.0,
                volatility: 0.0080,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 5.0,
                tenor: 5.0,
                volatility: 0.0085,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 10.0,
                tenor: 5.0,
                volatility: 0.0075,
                is_normal_vol: true,
            },
        ];
        let fwd_swap_rates = vec![0.03, 0.032, 0.031];
        let (_kappa, sigma) = infer_hw_initial_guess(&quotes, &fwd_swap_rates);

        // The HW1F σ is an absolute short-rate vol; for normal quotes the
        // seed should track the quoted absolute vol (~80 bp), i.e. land in
        // roughly [3e-3, 3e-2]. The buggy `avg_vol·avg_fwd` product yields
        // ~2.4e-4, clamped up to the 1e-3 floor — still ~8× too small.
        assert!(
            (3e-3..=3e-2).contains(&sigma),
            "normal-vol σ seed out of order of magnitude: {sigma}"
        );
    }

    /// W-39 companion: for LOGNORMAL quotes the σ seed *should* still
    /// multiply by the forward rate, since a Black vol is dimensionless
    /// and `vol·fwd` recovers an absolute-rate scale.
    #[test]
    fn infer_hw_initial_guess_lognormal_vol_seed_uses_forward_rate() {
        // 25% Black vol at a 3% forward → absolute vol ≈ 0.75%.
        let quotes = vec![
            SwaptionQuote {
                expiry: 1.0,
                tenor: 5.0,
                volatility: 0.25,
                is_normal_vol: false,
            },
            SwaptionQuote {
                expiry: 5.0,
                tenor: 5.0,
                volatility: 0.25,
                is_normal_vol: false,
            },
        ];
        let fwd_swap_rates = vec![0.03, 0.03];
        let (_kappa, sigma) = infer_hw_initial_guess(&quotes, &fwd_swap_rates);
        // 0.25 · 0.03 = 0.0075 — within the valid σ band.
        assert!(
            (3e-3..=3e-2).contains(&sigma),
            "lognormal-vol σ seed out of order of magnitude: {sigma}"
        );
    }

    /// M2.17: the HW futures convexity adjustment must reduce to the Ho-Lee
    /// limit `½σ²T₁T₂` as κ → 0 — both via the explicit small-κ branch and
    /// continuously through it.
    #[test]
    fn convexity_adjustment_ho_lee_limit() {
        let sigma = 0.01;
        let t1 = 5.0;
        let t2 = 5.25;
        let ho_lee = 0.5 * sigma * sigma * t1 * t2;

        // Explicit small-κ branch.
        let ca_branch = hw1f_convexity_adjustment(1e-12, sigma, t1, t2);
        assert!(
            (ca_branch - ho_lee).abs() < 1e-15,
            "κ→0 branch must equal ½σ²T₁T₂: got {ca_branch}, want {ho_lee}"
        );

        // Continuity across the branch threshold.
        let ca_small = hw1f_convexity_adjustment(1e-6, sigma, t1, t2);
        assert!(
            (ca_small - ho_lee).abs() / ho_lee < 1e-4,
            "full formula at κ=1e-6 must approach the Ho-Lee limit: \
             got {ca_small}, want {ho_lee}"
        );
    }

    /// M2.17: at realistic parameters (κ=0.03, σ=0.01, T₁=5y eurodollar) the
    /// adjustment is ~11–13bp; the previous formula `½σ²B(0,T₁)B(T₁,T₂)`
    /// gave ~0.58bp (≈20× understated) because it dropped the `½σ²T₁²` term.
    #[test]
    fn convexity_adjustment_magnitude_at_realistic_params() {
        let kappa = 0.03;
        let sigma = 0.01;
        let t1 = 5.0;
        let t2 = 5.25;

        let ca = hw1f_convexity_adjustment(kappa, sigma, t1, t2);
        // Below the Ho-Lee bound (mean reversion damps the adjustment)…
        let ho_lee = 0.5 * sigma * sigma * t1 * t2;
        assert!(
            ca < ho_lee,
            "κ>0 must damp the adjustment: {ca} vs {ho_lee}"
        );
        // …but on the same order, not 20× smaller.
        assert!(
            (1.0e-3..1.35e-3).contains(&ca),
            "expected ~11–13bp adjustment, got {ca}"
        );

        // The dropped-term formula for reference: it must NOT match.
        let old = 0.5 * sigma * sigma * hw_b(kappa, 0.0, t1) * hw_b(kappa, t1, t2);
        assert!(
            ca > 10.0 * old,
            "fixed adjustment {ca} should dwarf the old understated value {old}"
        );
    }

    /// Degenerate inputs return zero adjustment.
    #[test]
    fn convexity_adjustment_degenerate_inputs() {
        assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 0.0, 0.25), 0.0);
        assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 5.0, 5.0), 0.0);
        assert_eq!(hw1f_convexity_adjustment(0.03, 0.01, 5.0, 4.0), 0.0);
    }

    /// M2.19: non-finite or non-positive discount factors must propagate as
    /// NaN — `df.max(1e-12)` silently absorbed NaN (f64::max semantics) and
    /// produced a finite forward, defeating the non-finite-price error
    /// contract in the calibration residuals.
    #[test]
    fn forward_rate_from_df_propagates_bad_dfs() {
        let nan_df = |_: f64| f64::NAN;
        assert!(forward_rate_from_df(&nan_df, 0.25, 0.5).is_nan());

        let neg_df = |t: f64| if t > 0.3 { -1.0 } else { 1.0 };
        assert!(forward_rate_from_df(&neg_df, 0.25, 0.5).is_nan());

        let zero_df = |t: f64| if t > 0.3 { 0.0 } else { 1.0 };
        assert!(forward_rate_from_df(&zero_df, 0.25, 0.5).is_nan());

        // Sane curve still produces a sane forward.
        let df_fn = flat_df(0.03);
        let fwd = forward_rate_from_df(&df_fn, 0.25, 0.5);
        assert!((fwd - 0.03).abs() < 1e-3, "flat 3% curve forward: {fwd}");
    }

    /// M2.18: the spot-start caplet (fixing at t=0, no optionality) is
    /// excluded from cap decomposition, and caplet expiry is the fixing time
    /// `t_start`, not the payment time `t_end`.
    #[test]
    fn cap_floor_periods_exclude_spot_caplet_and_expiry_is_fixing_time() {
        let periods: Vec<(f64, f64, f64)> =
            cap_floor_periods(1.0, SwapFrequency::Quarterly).collect();
        assert_eq!(
            periods.len(),
            3,
            "1y quarterly cap: 3 caplets, spot excluded"
        );
        assert!(
            (periods[0].0 - 0.25).abs() < 1e-12,
            "first included caplet fixes at 0.25, got {}",
            periods[0].0
        );

        // Expiry convention: a cap priced with vol accruing to t_start must be
        // strictly cheaper than the same legs priced to t_end (more variance).
        let df_fn = flat_df(0.03);
        let price = bachelier_cap_floor_price(
            &df_fn,
            &df_fn,
            2.0,
            0.03,
            0.008,
            true,
            SwapFrequency::Quarterly,
        );
        let price_t_end: f64 = cap_floor_periods(2.0, SwapFrequency::Quarterly)
            .map(|(t_start, t_end, accrual)| {
                let forward = forward_rate_from_df(&df_fn, t_start, t_end);
                normal_caplet_price(forward, 0.03, 0.008, t_end, accrual, df_fn(t_end), true)
            })
            .sum();
        assert!(
            price < price_t_end,
            "fixing-time expiry must price below payment-time expiry: \
             {price} vs {price_t_end}"
        );
        assert!(price > 0.0);
    }

    /// M2.18: a quote spanning only the (excluded) spot-start caplet is
    /// rejected at validation rather than calibrated against a zero price.
    #[test]
    fn cap_floor_single_period_quote_rejected() {
        let df_fn = flat_df(0.03);
        let quote = CapFloorQuote {
            maturity: 0.25,
            strike: 0.03,
            volatility: 0.008,
            is_cap: true,
            is_normal_vol: true,
        };
        let config = CapFloorCalibrationConfig {
            frequency: SwapFrequency::Quarterly,
            fixed_kappa: Some(0.05),
            ..Default::default()
        };
        let result = calibrate_hull_white_to_cap_floors(&df_fn, &df_fn, &[quote], config);
        assert!(
            result.is_err(),
            "single-period cap quote must be rejected, got {:?}",
            result.map(|(p, _)| p)
        );
    }

    /// ZCB-option caplet pricing satisfies exact cap/floor parity:
    /// cap − floor = Σ P_d(0, S_i) · τ_i · (F_i − K).
    #[test]
    fn hw1f_cap_floor_zcb_option_parity() {
        let df_fn = flat_df(0.03);
        let (kappa, sigma, maturity, strike) = (0.05, 0.012, 5.0, 0.035);
        let freq = SwapFrequency::Quarterly;
        let cap = hw1f_cap_floor_price(
            kappa,
            sigma,
            &df_fn,
            &df_fn,
            CapFloorPriceSpec::new(maturity, strike, true, freq),
        );
        let floor = hw1f_cap_floor_price(
            kappa,
            sigma,
            &df_fn,
            &df_fn,
            CapFloorPriceSpec::new(maturity, strike, false, freq),
        );
        let forward_leg: f64 = cap_floor_periods(maturity, freq)
            .map(|(t_start, t_end, accrual)| {
                let fwd = forward_rate_from_df(&df_fn, t_start, t_end);
                df_fn(t_end) * accrual * (fwd - strike)
            })
            .sum();
        assert!(
            (cap - floor - forward_leg).abs() < 1e-12,
            "cap/floor parity violated: cap={cap}, floor={floor}, fwd_leg={forward_leg}"
        );
        assert!(cap > 0.0 && floor > 0.0);
    }

    /// The exact ZCB-put caplet price exceeds the old forward-rate-normal-vol
    /// approximation (which understated the caplet vol by ~(1+τF)) and the
    /// gap matches the (1+τF) vol scaling to first order.
    #[test]
    fn hw1f_zcb_option_caplet_prices_above_old_approximation() {
        let df_fn = flat_df(0.04);
        let (kappa, sigma) = (0.05, 0.012);
        let spec = CapFloorPriceSpec::new(5.0, 0.04, true, SwapFrequency::Annual);
        let exact = hw1f_cap_floor_price(kappa, sigma, &df_fn, &df_fn, spec);
        let approx: f64 = cap_floor_periods(spec.maturity, spec.frequency)
            .map(|(t_start, t_end, accrual)| {
                let forward = forward_rate_from_df(&df_fn, t_start, t_end);
                let hw_vol = hw1f_caplet_forward_rate_normal_vol(kappa, sigma, t_start, accrual);
                normal_caplet_price(
                    forward,
                    spec.strike,
                    hw_vol,
                    t_start,
                    accrual,
                    df_fn(t_end),
                    spec.is_cap,
                )
            })
            .sum();
        assert!(
            exact > approx,
            "exact ZCB-option price must exceed the understated approximation: \
             {exact} vs {approx}"
        );
        // The relative gap is on the order of τF ≈ 4% for annual caplets at
        // a 4% forward (ATM vega is linear in vol).
        let rel_gap = (exact - approx) / approx;
        assert!(
            rel_gap > 0.01 && rel_gap < 0.10,
            "expected ~τF vol understatement, got relative price gap {rel_gap}"
        );
    }

    /// A malformed per-quote schedule falls back to the synthetic recipe but
    /// is stamped in the report metadata instead of silently claiming
    /// real-day-count schedules.
    #[test]
    fn swaption_schedule_fallback_is_stamped() {
        let df_fn = flat_df(0.03);
        let quotes = vec![
            SwaptionQuote {
                expiry: 1.0,
                tenor: 5.0,
                volatility: 0.006,
                is_normal_vol: true,
            },
            SwaptionQuote {
                expiry: 5.0,
                tenor: 5.0,
                volatility: 0.006,
                is_normal_vol: true,
            },
        ];
        // First schedule valid (10 semi-annual accruals), second malformed
        // (wrong length).
        let schedules = vec![vec![0.5; 10], vec![0.5; 3]];
        let (_, report) = calibrate_hull_white_to_swaptions_with_schedules(
            &df_fn,
            &quotes,
            SwapFrequency::SemiAnnual,
            &schedules,
            None,
        )
        .expect("calibration should succeed with fallback");
        assert_eq!(
            report.metadata.get("schedule_source").map(String::as_str),
            Some("mixed"),
            "one fallback quote must downgrade schedule_source to 'mixed'"
        );
        assert_eq!(
            report
                .metadata
                .get("schedule_fallback_count")
                .map(String::as_str),
            Some("1")
        );
        assert!(
            report
                .metadata
                .get("schedule_fallback_quotes")
                .is_some_and(|q| q.contains("5Yx5Y")),
            "fallback quote label must be listed: {:?}",
            report.metadata.get("schedule_fallback_quotes")
        );

        // All-valid schedules keep the real_day_count stamp.
        let schedules_ok = vec![vec![0.5; 10], vec![0.5; 10]];
        let (_, report_ok) = calibrate_hull_white_to_swaptions_with_schedules(
            &df_fn,
            &quotes,
            SwapFrequency::SemiAnnual,
            &schedules_ok,
            None,
        )
        .expect("calibration should succeed");
        assert_eq!(
            report_ok
                .metadata
                .get("schedule_source")
                .map(String::as_str),
            Some("real_day_count")
        );
        assert!(!report_ok.metadata.contains_key("schedule_fallback_count"));
    }

    /// Fixed-κ guardrail parity: κ outside the LM box-constraint band is
    /// rejected up front.
    #[test]
    fn cap_floor_fixed_kappa_out_of_band_rejected() {
        let df_fn = flat_df(0.03);
        let quote = CapFloorQuote {
            maturity: 5.0,
            strike: 0.03,
            volatility: 0.008,
            is_cap: true,
            is_normal_vol: true,
        };
        for bad_kappa in [KAPPA_MAX * 2.0, KAPPA_MIN / 2.0] {
            let config = CapFloorCalibrationConfig {
                frequency: SwapFrequency::Quarterly,
                fixed_kappa: Some(bad_kappa),
                ..Default::default()
            };
            let result = calibrate_hull_white_to_cap_floors(&df_fn, &df_fn, &[quote], config);
            assert!(
                result.is_err(),
                "fixed_kappa={bad_kappa} outside [{KAPPA_MIN}, {KAPPA_MAX}] must be rejected"
            );
        }
    }

    /// Quote deserialization rejects unknown fields and invalid values.
    #[test]
    fn quote_deserialization_validates() {
        // Valid quotes round-trip.
        let q: SwaptionQuote = serde_json::from_str(
            r#"{"expiry": 1.0, "tenor": 5.0, "volatility": 0.006, "is_normal_vol": true}"#,
        )
        .expect("valid swaption quote");
        assert!((q.expiry - 1.0).abs() < 1e-15);
        let c: CapFloorQuote = serde_json::from_str(
            r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.008,
                "is_cap": true, "is_normal_vol": true}"#,
        )
        .expect("valid cap quote");
        assert!((c.maturity - 5.0).abs() < 1e-15);

        // Unknown fields are rejected.
        assert!(serde_json::from_str::<SwaptionQuote>(
            r#"{"expiry": 1.0, "tenor": 5.0, "volatility": 0.006,
                "is_normal_vol": true, "strike": 0.03}"#,
        )
        .is_err());
        assert!(serde_json::from_str::<CapFloorQuote>(
            r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.008,
                "is_cap": true, "is_normal_vol": true, "extra": 1}"#,
        )
        .is_err());

        // Invalid values are rejected at deserialization time.
        assert!(serde_json::from_str::<SwaptionQuote>(
            r#"{"expiry": -1.0, "tenor": 5.0, "volatility": 0.006, "is_normal_vol": true}"#,
        )
        .is_err());
        assert!(serde_json::from_str::<CapFloorQuote>(
            r#"{"maturity": 5.0, "strike": 0.03, "volatility": -0.008,
                "is_cap": true, "is_normal_vol": true}"#,
        )
        .is_err());
        // Lognormal cap/floor quotes are not accepted yet.
        assert!(serde_json::from_str::<CapFloorQuote>(
            r#"{"maturity": 5.0, "strike": 0.03, "volatility": 0.2,
                "is_cap": true, "is_normal_vol": false}"#,
        )
        .is_err());
    }
}
