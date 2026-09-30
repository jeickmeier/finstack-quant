//! WASM bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::{js_f64, js_f64_seq, js_opt_f64, js_opt_uint, js_uint};
use crate::utils::to_js_err;
use finstack_quant_models::correlation::{self as corr, Copula, CopulaSpec, RecoveryModel};
use wasm_bindgen::prelude::*;

/// Copula model specification for configuration and deferred construction.
#[wasm_bindgen(js_name = CopulaSpec)]
pub struct JsCopulaSpec {
    inner: CopulaSpec,
}

#[wasm_bindgen(js_class = CopulaSpec)]
impl JsCopulaSpec {
    /// One-factor Gaussian copula (market standard).
    #[wasm_bindgen(js_name = gaussian)]
    pub fn gaussian() -> Self {
        Self {
            inner: CopulaSpec::gaussian(),
        }
    }

    /// Student-t copula with specified degrees of freedom (must be > 2).
    /// @param degreesOfFreedom - Student-t degrees of freedom controlling tail thickness; finite and strictly greater than two.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `degreesOfFreedom` is not finite and
    /// strictly greater than two.
    #[wasm_bindgen(js_name = studentT)]
    pub fn student_t(degrees_of_freedom: JsValue) -> Result<JsCopulaSpec, JsValue> {
        let degrees_of_freedom = js_f64(&degrees_of_freedom, "degreesOfFreedom")?;
        CopulaSpec::student_t(degrees_of_freedom)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Random Factor Loading copula with stochastic correlation.
    /// @param loading_vol - Standard deviation used to randomize the factor loading.
    #[wasm_bindgen(js_name = randomFactorLoading)]
    pub fn random_factor_loading(loading_vol: JsValue) -> Result<Self, JsValue> {
        let loading_vol = js_f64(&loading_vol, "loadingVol")?;
        Ok(Self {
            inner: CopulaSpec::random_factor_loading(loading_vol),
        })
    }

    /// Global-plus-sector two-factor Gaussian copula.
    #[wasm_bindgen(js_name = multiFactor)]
    pub fn multi_factor() -> Self {
        Self {
            inner: CopulaSpec::multi_factor(),
        }
    }

    /// Build a concrete copula from this specification.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if a Student-t specification contains
    /// non-finite degrees of freedom or a value at most two.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<JsCopula, JsValue> {
        self.inner
            .build()
            .map(|inner| JsCopula { inner })
            .map_err(to_js_err)
    }

    /// True if this is a Gaussian spec.
    #[wasm_bindgen(getter, js_name = isGaussian)]
    pub fn is_gaussian(&self) -> bool {
        self.inner.is_gaussian()
    }

    /// True if this is a Student-t spec.
    #[wasm_bindgen(getter, js_name = isStudentT)]
    pub fn is_student_t(&self) -> bool {
        self.inner.is_student_t()
    }

    /// True if this is a Random Factor Loading spec.
    #[wasm_bindgen(getter, js_name = isRfl)]
    pub fn is_rfl(&self) -> bool {
        self.inner.is_rfl()
    }

    /// True if this is a Multi-factor spec.
    #[wasm_bindgen(getter, js_name = isMultiFactor)]
    pub fn is_multi_factor(&self) -> bool {
        self.inner.is_multi_factor()
    }
}

/// Concrete copula model for portfolio default correlation.
#[wasm_bindgen(js_name = Copula)]
pub struct JsCopula {
    inner: Box<dyn Copula + Send + Sync>,
}

#[wasm_bindgen(js_class = Copula)]
impl JsCopula {
    /// Conditional default probability given factor realization(s).
    /// @param default_threshold - Latent-variable default threshold corresponding to the marginal default probability.
    /// @param factor_realization - Realized systematic-factor value conditioning the default probability.
    /// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the factor count does not match the
    /// copula, any input is non-finite, `correlation` is outside `[0, 1]`, or
    /// the model produces a probability outside `[0, 1]`.
    #[wasm_bindgen(js_name = conditionalDefaultProb)]
    pub fn conditional_default_prob(
        &self,
        default_threshold: JsValue,
        factor_realization: JsValue,
        correlation: JsValue,
    ) -> Result<f64, JsValue> {
        let default_threshold = js_f64(&default_threshold, "defaultThreshold")?;
        let factor_realization: &[f64] = &js_f64_seq(&factor_realization, "factorRealization")?;
        let correlation = js_f64(&correlation, "correlation")?;
        self.inner
            .conditional_default_prob_checked(default_threshold, factor_realization, correlation)
            .map_err(to_js_err)
    }

    /// Number of systematic factors in the model.
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Model name for diagnostics.
    #[wasm_bindgen(getter, js_name = modelName)]
    pub fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    /// Strict lower-tail dependence coefficient `λ_L` at the given
    /// correlation.
    ///
    /// Returns `NaN` when the model has no closed-form `λ_L` (Random Factor
    /// Loading); check `Number.isNaN()` before using the result. For the
    /// RFL heuristic stress gauge use `stressCorrelationProxy` instead.
    /// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
    #[wasm_bindgen(js_name = tailDependence)]
    pub fn tail_dependence(&self, correlation: JsValue) -> Result<f64, JsValue> {
        let correlation = js_f64(&correlation, "correlation")?;
        Ok(self.inner.tail_dependence(correlation))
    }

    /// Heuristic stress-correlation proxy for the Random Factor Loading
    /// copula.
    ///
    /// This is **not** the strict copula lower-tail-dependence coefficient
    /// `λ_L` (which has no closed form for RFL — `tailDependence` returns
    /// `NaN`). It gauges the extra correlation mass in the high-loading
    /// tail and vanishes in the Gaussian (`loadingVol = 0`) limit.
    /// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if this copula is not a Random Factor
    /// Loading model.
    #[wasm_bindgen(js_name = stressCorrelationProxy)]
    pub fn stress_correlation_proxy(&self, correlation: JsValue) -> Result<f64, JsValue> {
        let correlation = js_f64(&correlation, "correlation")?;
        self.inner
            .stress_correlation_proxy(correlation)
            .map_err(to_js_err)
    }
}

/// Recovery model specification for configuration and deferred construction.
#[wasm_bindgen(js_name = RecoverySpec)]
pub struct JsRecoverySpec {
    inner: corr::RecoverySpec,
}

#[wasm_bindgen(js_class = RecoverySpec)]
impl JsRecoverySpec {
    /// Constant recovery rate.
    /// @param rate - Constant recovery rate expressed as a fraction from 0 through 1.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `rate` is not finite or lies outside
    /// `[0, 1]`.
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(rate: JsValue) -> Result<JsRecoverySpec, JsValue> {
        let rate = js_f64(&rate, "rate")?;
        corr::RecoverySpec::constant(rate)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Market-correlated (Andersen-Sidenius) stochastic recovery.
    /// @param mean - Mean recovery rate expressed as a fraction from 0 through 1.
    /// @param vol - Recovery-rate volatility scale in the correlated recovery model.
    /// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `mean` is not finite or lies outside
    /// `[0, 1]`, or if `vol` or `correlation` is non-finite. Finite volatility
    /// and correlation inputs are clamped to their supported ranges.
    #[wasm_bindgen(js_name = marketCorrelated)]
    pub fn market_correlated(
        mean: JsValue,
        vol: JsValue,
        correlation: JsValue,
    ) -> Result<JsRecoverySpec, JsValue> {
        let mean = js_f64(&mean, "mean")?;
        let vol = js_f64(&vol, "vol")?;
        let correlation = js_f64(&correlation, "correlation")?;
        corr::RecoverySpec::market_correlated(mean, vol, correlation)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Market-standard stochastic recovery (40% mean, 25% vol, +40% corr —
    /// recovery falls in stress under the canonical low-factor-stress
    /// convention).
    #[wasm_bindgen(js_name = marketStandardStochastic)]
    pub fn market_standard_stochastic() -> Self {
        Self {
            inner: corr::RecoverySpec::market_standard_stochastic(),
        }
    }

    /// Location-parameter recovery rate of this spec.
    ///
    /// For a constant spec this is the constant rate. For a
    /// market-correlated spec this returns the `mean` input — the target
    /// recovery at factor `Z = 0` — which differs from the Jensen-corrected
    /// unconditional mean `E_Z[R(Z)]` whenever the factor sensitivity is
    /// non-zero. For the true unconditional mean call
    /// `build().expectedRecovery`.
    #[wasm_bindgen(getter, js_name = expectedRecovery)]
    pub fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    /// Build a concrete recovery model from this specification.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> JsRecoveryModel {
        JsRecoveryModel {
            inner: self.inner.build(),
        }
    }
}

/// Concrete recovery model for credit portfolio pricing.
#[wasm_bindgen(js_name = RecoveryModel)]
pub struct JsRecoveryModel {
    inner: Box<dyn RecoveryModel + Send + Sync>,
}

#[wasm_bindgen(js_class = RecoveryModel)]
impl JsRecoveryModel {
    /// Expected (unconditional) recovery rate.
    #[wasm_bindgen(getter, js_name = expectedRecovery)]
    pub fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    /// Recovery conditional on the systematic market factor.
    /// @param market_factor - Realized standardized market factor used to condition recovery or loss given default.
    #[wasm_bindgen(js_name = conditionalRecovery)]
    pub fn conditional_recovery(&self, market_factor: JsValue) -> Result<f64, JsValue> {
        let market_factor = js_f64(&market_factor, "marketFactor")?;
        Ok(self.inner.conditional_recovery(market_factor))
    }

    /// Loss given default (1 − recovery).
    #[wasm_bindgen(getter, js_name = lgd)]
    pub fn lgd(&self) -> f64 {
        self.inner.lgd()
    }

    /// Conditional LGD given market factor.
    /// @param market_factor - Realized standardized market factor used to condition recovery or loss given default.
    #[wasm_bindgen(js_name = conditionalLgd)]
    pub fn conditional_lgd(&self, market_factor: JsValue) -> Result<f64, JsValue> {
        let market_factor = js_f64(&market_factor, "marketFactor")?;
        Ok(self.inner.conditional_lgd(market_factor))
    }

    /// Recovery-rate volatility scale (0 for constant models).
    #[wasm_bindgen(getter, js_name = recoveryVolatility)]
    pub fn recovery_volatility(&self) -> f64 {
        self.inner.recovery_volatility()
    }

    /// Whether recovery varies with the market factor.
    #[wasm_bindgen(getter, js_name = isStochastic)]
    pub fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    /// Model name for diagnostics.
    #[wasm_bindgen(getter, js_name = modelName)]
    pub fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }
}

/// Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
///
/// Returns `[rho_min, rho_max]`.
/// @param p1 - First marginal default probability from 0 through 1.
/// @param p2 - Second marginal default probability from 0 through 1.
///
/// # Errors
///
/// Throws a JavaScript exception if either marginal probability is non-finite
/// or outside `[0, 1]`.
#[wasm_bindgen(js_name = correlationBounds)]
pub fn correlation_bounds(p1: JsValue, p2: JsValue) -> Result<Box<[f64]>, JsValue> {
    let p1 = js_f64(&p1, "p1")?;
    let p2 = js_f64(&p2, "p2")?;
    let (lo, hi) = corr::correlation_bounds(p1, p2).map_err(to_js_err)?;
    Ok(Box::new([lo, hi]))
}

/// Joint probabilities for two correlated Bernoulli variables.
///
/// Returns `[p11, p10, p01, p00]`.
/// @param p1 - First marginal default probability from 0 through 1.
/// @param p2 - Second marginal default probability from 0 through 1.
/// @param correlation - Dependence correlation from -1 through 1 under the selected copula or recovery model.
///
/// # Errors
///
/// Throws a JavaScript exception if either marginal probability is non-finite
/// or outside `[0, 1]`, or `correlation` is non-finite or outside `[-1, 1]`.
#[wasm_bindgen(js_name = jointProbabilities)]
pub fn joint_probabilities(
    p1: JsValue,
    p2: JsValue,
    correlation: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let p1 = js_f64(&p1, "p1")?;
    let p2 = js_f64(&p2, "p2")?;
    let correlation = js_f64(&correlation, "correlation")?;
    let (p11, p10, p01, p00) = corr::joint_probabilities(p1, p2, correlation).map_err(to_js_err)?;
    Ok(Box::new([p11, p10, p01, p00]))
}

/// Validate a flat row-major correlation matrix.
///
/// Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
/// checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
/// semi-definiteness. Returns nothing on success; raises a descriptive error
/// (including the failing dimension or constraint) otherwise.
/// @param matrix - Flat row-major `n * n` correlation coefficients; unit diagonal, off-diagonals in `[-1, 1]`.
/// @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
///
/// # Errors
///
/// Throws a JavaScript exception if the flat length is not `n * n`, a diagonal
/// entry is not one, an entry is outside the correlation bounds, the matrix is
/// not symmetric, or the matrix is not positive semidefinite.
#[wasm_bindgen(js_name = validateCorrelationMatrix)]
pub fn validate_correlation_matrix(matrix: JsValue, n: JsValue) -> Result<(), JsValue> {
    let matrix: &[f64] = &js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    corr::validate_correlation_matrix(matrix, n).map_err(to_js_err)
}

/// Nearest correlation matrix (Higham 2002).
///
/// Given a flat row-major `n*n` matrix that is approximately a correlation
/// matrix but fails Cholesky by a small margin, returns the nearest valid
/// correlation matrix (symmetric, unit diagonal, PSD) in Frobenius norm.
/// Gross input violations raise rather than being silently reshaped.
/// @param matrix - Flat row-major `n * n` near-correlation matrix to project onto the correlation set.
/// @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
/// @param max_iter - Maximum number of Higham nearest-correlation projection iterations.
/// @param tol - Positive convergence tolerance for the nearest-correlation projection.
///
/// # Errors
///
/// Throws a `validation` error if the flat length is not `n * n` or the input
/// has a gross diagonal or symmetry violation, and a `computation` error if
/// the projection does not converge within `maxIter` iterations at `tol`.
#[wasm_bindgen(js_name = nearestCorrelation)]
pub fn nearest_correlation(
    matrix: JsValue,
    n: JsValue,
    max_iter: Option<JsValue>,
    tol: Option<JsValue>,
) -> Result<Box<[f64]>, JsValue> {
    let matrix = js_f64_seq(&matrix, "matrix")?;
    let tol = js_opt_f64(tol.as_ref(), "tol")?;
    let n: usize = js_uint(&n, "n")?;
    let max_iter: Option<usize> = js_opt_uint(max_iter.as_ref(), "maxIter")?;
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = corr::NearestCorrelationOpts::default();
    let opts = corr::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    corr::nearest_correlation_matrix(&matrix, n, opts)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Portfolio credit-loss distribution with loss-positive VaR and expected
/// shortfall.
///
/// Mirrors the Rust `PortfolioLossResult` and the Python class of the same
/// name. Build one from a simulated loss vector with `fromLosses`, or load one
/// with `fromJson`; the aggregates are always recomputed from the losses.
#[wasm_bindgen(js_name = PortfolioLossResult)]
pub struct JsPortfolioLossResult {
    pub(crate) inner: corr::PortfolioLossResult,
}

#[wasm_bindgen(js_class = PortfolioLossResult)]
impl JsPortfolioLossResult {
    /// Aggregate a finite loss distribution under loss-positive conventions.
    ///
    /// VaR is the nearest-rank loss quantile at `confidence`; expected
    /// shortfall averages exactly the worst `1 - confidence` probability
    /// mass, with fractional weight on the boundary observation.
    /// @param losses - Loss-positive path losses in one caller-defined unit, one entry per simulated path, as a `number[]` or `Float64Array`.
    /// @param confidence - Loss-positive VaR and expected-shortfall confidence strictly between 0 and 1.
    /// @returns A `PortfolioLossResult` handle.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `losses` is not an array of numbers, and a
    /// `validation` error if the distribution is empty, a loss is non-finite
    /// or negative, or `confidence` is outside `(0, 1)`.
    #[wasm_bindgen(js_name = fromLosses)]
    pub fn from_losses(
        losses: JsValue,
        confidence: JsValue,
    ) -> Result<JsPortfolioLossResult, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        let losses = crate::utils::input::js_f64_seq(&losses, "losses")?;
        corr::PortfolioLossResult::from_losses(losses, confidence)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Load a result from its canonical JSON form.
    ///
    /// The losses and confidence are validated and the aggregates recomputed;
    /// a payload whose aggregates disagree with its losses is rejected.
    /// @param json - `PortfolioLossResult` JSON (`losses`, `expected_loss`, `var`, `expected_shortfall`, `confidence`), as a string or plain object.
    /// @returns A `PortfolioLossResult` handle.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `json` is neither a string nor a plain object,
    /// and a `validation` error if it is malformed or fails the checks above.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsPortfolioLossResult, JsValue> {
        crate::utils::input::from_js_json(&json, "json").map(|inner| Self { inner })
    }

    /// Serialize to the canonical JSON wire format.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if serialization fails.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Loss per simulated path, in path order.
    #[wasm_bindgen(getter)]
    pub fn losses(&self) -> Box<[f64]> {
        self.inner.losses.clone().into_boxed_slice()
    }

    /// Arithmetic mean path loss.
    #[wasm_bindgen(getter, js_name = expectedLoss)]
    pub fn expected_loss(&self) -> f64 {
        self.inner.expected_loss
    }

    /// Loss-positive nearest-rank VaR at `confidence` (larger is worse).
    #[wasm_bindgen(getter)]
    pub fn var(&self) -> f64 {
        self.inner.var
    }

    /// Probability-weighted mean loss in the worst `1 - confidence` tail.
    #[wasm_bindgen(getter, js_name = expectedShortfall)]
    pub fn expected_shortfall(&self) -> f64 {
        self.inner.expected_shortfall
    }

    /// Confidence used for `var` and `expectedShortfall`, in `(0, 1)`.
    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Tranche loss statistics for one attachment/detachment pair.
    ///
    /// `attachment` and `detachment` are **fractions** of pool notional in
    /// `[0, 1]` — a 0-3% equity tranche is `(0.0, 0.03)`, not `(0.0, 3.0)`.
    /// Each path's pool loss fraction `L = loss / poolNotional` maps through
    /// `clamp(L - attachment, 0, width) / width`, and the resulting
    /// distribution is aggregated at this result's own `confidence`.
    ///
    /// Returns an object with `attachment`, `detachment`, `tranche_notional`,
    /// `expected_loss_fraction`, `expected_loss_amount`, `var_fraction`,
    /// `var_amount`, `expected_shortfall_fraction`, `expected_shortfall_amount`,
    /// `prob_attachment_breached`, and `prob_full_writedown`.
    /// @param attachment - Lower tranche boundary as a fraction of pool notional from 0 through 1.
    /// @param detachment - Upper tranche boundary as a fraction of pool notional, strictly above the attachment and at most 1.
    /// @param pool_notional - Total pool notional, finite and strictly positive, in the same unit as the losses.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the tranche boundaries are invalid,
    /// `poolNotional` is not finite and positive, or a derived statistic is
    /// non-finite.
    #[wasm_bindgen(js_name = trancheLossStatistics)]
    pub fn tranche_loss_statistics(
        &self,
        attachment: JsValue,
        detachment: JsValue,
        pool_notional: JsValue,
    ) -> Result<JsValue, JsValue> {
        let attachment = js_f64(&attachment, "attachment")?;
        let detachment = js_f64(&detachment, "detachment")?;
        let pool_notional = js_f64(&pool_notional, "poolNotional")?;
        let stats = self
            .inner
            .tranche_loss_statistics(attachment, detachment, pool_notional)
            .map_err(to_js_err)?;
        crate::utils::to_js_value(&stats)
    }
}
