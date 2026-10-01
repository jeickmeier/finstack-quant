//! WASM bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_opt_f64, js_opt_uint, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::math::probability::CorrelatedBernoulli;
use finstack_quant_models::correlation::{
    self as corr, Copula, CopulaSpec, CreditExposure, LatentFactorKind, LatentFactorSpec,
    LatentMultiFactor, LatentSingleFactor, LatentTwoFactor, PortfolioLossConfig, RecoveryModel,
};
use wasm_bindgen::prelude::*;

/// Copula model specification for configuration and deferred construction.
#[wasm_bindgen(js_name = CopulaSpec)]
pub struct JsCopulaSpec {
    inner: CopulaSpec,
}

json_round_trip!(JsCopulaSpec, CopulaSpec);

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
    /// @param loading_vol - Factor-loading volatility (standard deviation of the loading); clamped to `[0, 0.5]`, typically 0.05 to 0.20.
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
    /// @param correlation - Asset correlation as a decimal in `[0, 1]`; values outside this range throw.
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
    /// @param correlation - Asset correlation as a decimal in `[0, 1]`. Out-of-range values are clamped by the model, not rejected (Student-t clamps to its supported correlation range; Gaussian models ignore the value and return 0).
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
    /// @param correlation - Base asset correlation as a decimal in `[0, 1]`; out-of-range values are clamped to `[0, 1]`.
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

json_round_trip!(JsRecoverySpec, RecoverySpec);

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
    /// @param vol - Recovery-rate volatility; finite values are clamped to `[0, 0.5]`.
    /// @param correlation - Correlation between recovery and the systematic factor, from -1 through 1; finite values outside that range are clamped.
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
/// @param max_iter - Maximum number of Higham nearest-correlation projection iterations; omitted uses Rust `NearestCorrelationOpts::default()` (200).
/// @param tol - Positive convergence tolerance for the nearest-correlation projection; omitted uses Rust `NearestCorrelationOpts::default()` (`1e-10`).
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

/// Two correlated Bernoulli default indicators with exact joint probabilities.
///
/// The requested correlation is clamped to the Fréchet-Hoeffding bounds of the
/// two marginals; `correlation` reports the value actually achieved.
#[wasm_bindgen(js_name = CorrelatedBernoulli)]
pub struct JsCorrelatedBernoulli {
    pub(crate) inner: CorrelatedBernoulli,
}

#[wasm_bindgen(js_class = CorrelatedBernoulli)]
impl JsCorrelatedBernoulli {
    /// Joint distribution of two Bernoulli variables with a target correlation.
    /// @param p1 - First marginal probability from 0 through 1.
    /// @param p2 - Second marginal probability from 0 through 1.
    /// @param correlation - Requested correlation from -1 through 1; clamped to the attainable Fréchet-Hoeffding range.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a probability is outside `[0, 1]` or the
    /// correlation is non-finite or outside `[-1, 1]`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        p1: JsValue,
        p2: JsValue,
        correlation: JsValue,
    ) -> Result<JsCorrelatedBernoulli, JsValue> {
        CorrelatedBernoulli::new(
            js_f64(&p1, "p1")?,
            js_f64(&p2, "p2")?,
            js_f64(&correlation, "correlation")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// First marginal probability.
    #[wasm_bindgen(getter)]
    pub fn p1(&self) -> f64 {
        self.inner.p1()
    }

    /// Second marginal probability.
    #[wasm_bindgen(getter)]
    pub fn p2(&self) -> f64 {
        self.inner.p2()
    }

    /// Correlation actually achieved after clamping to the attainable range.
    #[wasm_bindgen(getter)]
    pub fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    /// Correlation requested at construction, before clamping.
    #[wasm_bindgen(getter, js_name = requestedCorrelation)]
    pub fn requested_correlation(&self) -> f64 {
        self.inner.requested_correlation()
    }

    /// Probability that both variables equal one.
    #[wasm_bindgen(getter, js_name = jointP11)]
    pub fn joint_p11(&self) -> f64 {
        self.inner.joint_p11()
    }

    /// Probability that the first is one and the second zero.
    #[wasm_bindgen(getter, js_name = jointP10)]
    pub fn joint_p10(&self) -> f64 {
        self.inner.joint_p10()
    }

    /// Probability that the first is zero and the second one.
    #[wasm_bindgen(getter, js_name = jointP01)]
    pub fn joint_p01(&self) -> f64 {
        self.inner.joint_p01()
    }

    /// Probability that both variables equal zero.
    #[wasm_bindgen(getter, js_name = jointP00)]
    pub fn joint_p00(&self) -> f64 {
        self.inner.joint_p00()
    }

    /// The four joint probabilities.
    /// @returns `[p11, p10, p01, p00]`, summing to one.
    #[wasm_bindgen(js_name = jointProbabilities)]
    pub fn joint_probabilities(&self) -> Box<[f64]> {
        let (p11, p10, p01, p00) = self.inner.joint_probabilities();
        Box::new([p11, p10, p01, p00])
    }

    /// Probability that the second variable is one given the first is one.
    /// @returns The conditional probability `P(X2 = 1 | X1 = 1)`.
    #[wasm_bindgen(js_name = conditionalP2GivenX1)]
    pub fn conditional_p2_given_x1(&self) -> f64 {
        self.inner.conditional_p2_given_x1()
    }

    /// Probability that the first variable is one given the second is one.
    /// @returns The conditional probability `P(X1 = 1 | X2 = 1)`.
    #[wasm_bindgen(js_name = conditionalP1GivenX2)]
    pub fn conditional_p1_given_x2(&self) -> f64 {
        self.inner.conditional_p1_given_x2()
    }

    /// Map one uniform draw to a joint outcome.
    /// @param u - Uniform draw from 0 through 1.
    /// @returns A `Uint8Array` `[x1, x2]` with each entry 0 or 1.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `u` is non-finite or outside `[0, 1]`.
    #[wasm_bindgen(js_name = sampleFromUniform)]
    pub fn sample_from_uniform(&self, u: JsValue) -> Result<Box<[u8]>, JsValue> {
        let (x1, x2) = self
            .inner
            .sample_from_uniform(js_f64(&u, "u")?)
            .map_err(to_js_err)?;
        Ok(Box::new([x1, x2]))
    }
}

/// Latent-factor model specification for deferred construction.
#[wasm_bindgen(js_name = LatentFactorSpec)]
pub struct JsLatentFactorSpec {
    inner: LatentFactorSpec,
}

#[wasm_bindgen(js_class = LatentFactorSpec)]
impl JsLatentFactorSpec {
    /// One mean-reverting systematic factor.
    /// @param volatility - Annualized factor volatility as a decimal; non-negative.
    /// @param mean_reversion - Mean-reversion speed per year; non-negative.
    /// @returns The single-factor specification.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(js_name = singleFactor)]
    pub fn single_factor(
        volatility: JsValue,
        mean_reversion: JsValue,
    ) -> Result<JsLatentFactorSpec, JsValue> {
        Ok(Self {
            inner: LatentFactorSpec::single_factor(
                js_f64(&volatility, "volatility")?,
                js_f64(&mean_reversion, "meanReversion")?,
            ),
        })
    }

    /// Correlated prepayment and credit factors.
    /// @param prepay_vol - Annualized prepayment-factor volatility as a decimal; non-negative.
    /// @param credit_vol - Annualized credit-factor volatility as a decimal; non-negative.
    /// @param correlation - Correlation between the two factors, from -1 through 1.
    /// @returns The two-factor specification.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(js_name = twoFactor)]
    pub fn two_factor(
        prepay_vol: JsValue,
        credit_vol: JsValue,
        correlation: JsValue,
    ) -> Result<JsLatentFactorSpec, JsValue> {
        Ok(Self {
            inner: LatentFactorSpec::two_factor(
                js_f64(&prepay_vol, "prepayVol")?,
                js_f64(&credit_vol, "creditVol")?,
                js_f64(&correlation, "correlation")?,
            ),
        })
    }

    /// Number of systematic factors the specification describes.
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Build the concrete latent-factor model.
    /// @returns The `LatentFactorKind` handle.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a multi-factor specification has an
    /// invalid correlation matrix or volatility vector.
    pub fn build(&self) -> Result<JsLatentFactorKind, JsValue> {
        self.inner
            .build()
            .map(|inner| JsLatentFactorKind { inner })
            .map_err(to_js_err)
    }
}

/// Concrete latent-factor model built from a `LatentFactorSpec`.
#[wasm_bindgen(js_name = LatentFactorKind)]
pub struct JsLatentFactorKind {
    inner: LatentFactorKind,
}

#[wasm_bindgen(js_class = LatentFactorKind)]
impl JsLatentFactorKind {
    /// Number of systematic factors.
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Factor correlation matrix, flat row-major (`numFactors * numFactors`).
    #[wasm_bindgen(getter, js_name = correlationMatrix)]
    pub fn correlation_matrix(&self) -> Box<[f64]> {
        self.inner.correlation_matrix().into()
    }

    /// Annualized factor volatilities, one per factor.
    #[wasm_bindgen(getter)]
    pub fn volatilities(&self) -> Box<[f64]> {
        self.inner.volatilities().into()
    }

    /// Descriptive factor names, one per factor.
    #[wasm_bindgen(getter, js_name = factorNames)]
    pub fn factor_names(&self) -> Vec<String> {
        self.inner
            .factor_names()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Model name for diagnostics.
    #[wasm_bindgen(getter, js_name = modelName)]
    pub fn model_name(&self) -> String {
        self.inner.model_name().to_string()
    }

    /// Contribution of one factor's own (diagonal) shock to its value.
    /// @param factor_index - Zero-based factor index; an index beyond the model returns 0.
    /// @param z - Independent standard normal draw for that factor.
    /// @returns The diagonal Cholesky loading times `z` times the factor volatility.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `factorIndex` is not a safe non-negative integer
    /// or `z` is not a number.
    #[wasm_bindgen(js_name = diagonalFactorContribution)]
    pub fn diagonal_factor_contribution(
        &self,
        factor_index: JsValue,
        z: JsValue,
    ) -> Result<f64, JsValue> {
        Ok(self
            .inner
            .diagonal_factor_contribution(js_uint(&factor_index, "factorIndex")?, js_f64(&z, "z")?))
    }
}

/// Single mean-reverting latent factor.
#[wasm_bindgen(js_name = LatentSingleFactor)]
pub struct JsLatentSingleFactor {
    inner: LatentSingleFactor,
}

#[wasm_bindgen(js_class = LatentSingleFactor)]
impl JsLatentSingleFactor {
    /// Single-factor model; out-of-range inputs are clamped by Rust.
    /// @param volatility - Annualized factor volatility as a decimal; non-negative.
    /// @param mean_reversion - Mean-reversion speed per year; non-negative.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(constructor)]
    pub fn new(
        volatility: JsValue,
        mean_reversion: JsValue,
    ) -> Result<JsLatentSingleFactor, JsValue> {
        Ok(Self {
            inner: LatentSingleFactor::new(
                js_f64(&volatility, "volatility")?,
                js_f64(&mean_reversion, "meanReversion")?,
            ),
        })
    }

    /// Annualized factor volatility, as a decimal.
    #[wasm_bindgen(getter)]
    pub fn volatility(&self) -> f64 {
        self.inner.volatility()
    }

    /// Mean-reversion speed per year.
    #[wasm_bindgen(getter, js_name = meanReversion)]
    pub fn mean_reversion(&self) -> f64 {
        self.inner.mean_reversion()
    }

    /// Number of systematic factors (always 1).
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }
}

/// Correlated prepayment and credit latent factors.
#[wasm_bindgen(js_name = LatentTwoFactor)]
pub struct JsLatentTwoFactor {
    inner: LatentTwoFactor,
}

#[wasm_bindgen(js_class = LatentTwoFactor)]
impl JsLatentTwoFactor {
    /// Two-factor model; out-of-range inputs are clamped by Rust.
    /// @param prepay_vol - Annualized prepayment-factor volatility as a decimal; non-negative.
    /// @param credit_vol - Annualized credit-factor volatility as a decimal; non-negative.
    /// @param correlation - Correlation between the two factors, from -1 through 1.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if an argument is not a number.
    #[wasm_bindgen(constructor)]
    pub fn new(
        prepay_vol: JsValue,
        credit_vol: JsValue,
        correlation: JsValue,
    ) -> Result<JsLatentTwoFactor, JsValue> {
        Ok(Self {
            inner: LatentTwoFactor::new(
                js_f64(&prepay_vol, "prepayVol")?,
                js_f64(&credit_vol, "creditVol")?,
                js_f64(&correlation, "correlation")?,
            ),
        })
    }

    /// Standard RMBS calibration of the two-factor model.
    /// @returns The RMBS-standard prepayment/credit factor model.
    #[wasm_bindgen(js_name = rmbsStandard)]
    pub fn rmbs_standard() -> JsLatentTwoFactor {
        Self {
            inner: LatentTwoFactor::rmbs_standard(),
        }
    }

    /// Standard CLO calibration of the two-factor model.
    /// @returns The CLO-standard prepayment/credit factor model.
    #[wasm_bindgen(js_name = cloStandard)]
    pub fn clo_standard() -> JsLatentTwoFactor {
        Self {
            inner: LatentTwoFactor::clo_standard(),
        }
    }

    /// Annualized prepayment-factor volatility, as a decimal.
    #[wasm_bindgen(getter, js_name = prepayVol)]
    pub fn prepay_vol(&self) -> f64 {
        self.inner.prepay_vol()
    }

    /// Annualized credit-factor volatility, as a decimal.
    #[wasm_bindgen(getter, js_name = creditVol)]
    pub fn credit_vol(&self) -> f64 {
        self.inner.credit_vol()
    }

    /// Correlation between the prepayment and credit factors.
    #[wasm_bindgen(getter)]
    pub fn correlation(&self) -> f64 {
        self.inner.correlation()
    }

    /// Number of systematic factors (always 2).
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Off-diagonal Cholesky entry `L[1,0]`, equal to the correlation.
    #[wasm_bindgen(getter, js_name = choleskyL10)]
    pub fn cholesky_l10(&self) -> f64 {
        self.inner.cholesky_l10()
    }

    /// Diagonal Cholesky entry `L[1,1]`, equal to `sqrt(1 - correlation^2)`.
    #[wasm_bindgen(getter, js_name = choleskyL11)]
    pub fn cholesky_l11(&self) -> f64 {
        self.inner.cholesky_l11()
    }
}

/// General correlated latent-factor model with a Cholesky factorization.
#[wasm_bindgen(js_name = LatentMultiFactor)]
pub struct JsLatentMultiFactor {
    inner: LatentMultiFactor,
}

#[wasm_bindgen(js_class = LatentMultiFactor)]
impl JsLatentMultiFactor {
    /// Multi-factor model from volatilities and a correlation matrix.
    /// @param num_factors - Number of systematic factors; a positive safe integer.
    /// @param volatilities - Annualized factor volatilities, one per factor; non-negative.
    /// @param correlations - Flat row-major `numFactors * numFactors` correlation matrix; symmetric, unit diagonal, positive semidefinite.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the lengths disagree with `numFactors`,
    /// a volatility is negative or non-finite, or the matrix is not a valid
    /// correlation matrix.
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_factors: JsValue,
        volatilities: JsValue,
        correlations: JsValue,
    ) -> Result<JsLatentMultiFactor, JsValue> {
        LatentMultiFactor::new(
            js_uint(&num_factors, "numFactors")?,
            js_f64_seq(&volatilities, "volatilities")?,
            js_f64_seq(&correlations, "correlations")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Model with independent factors (identity correlation).
    /// @param num_factors - Number of systematic factors; a positive safe integer.
    /// @param volatilities - Annualized factor volatilities, one per factor; a length mismatch falls back to unit volatilities.
    /// @returns The uncorrelated multi-factor model.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `numFactors` is not a safe non-negative integer
    /// or `volatilities` is not an array of numbers.
    pub fn uncorrelated(
        num_factors: JsValue,
        volatilities: JsValue,
    ) -> Result<JsLatentMultiFactor, JsValue> {
        Ok(Self {
            inner: LatentMultiFactor::uncorrelated(
                js_uint(&num_factors, "numFactors")?,
                js_f64_seq(&volatilities, "volatilities")?,
            ),
        })
    }

    /// Number of systematic factors.
    #[wasm_bindgen(getter, js_name = numFactors)]
    pub fn num_factors(&self) -> usize {
        self.inner.num_factors()
    }

    /// Factor correlation matrix, flat row-major.
    #[wasm_bindgen(getter, js_name = correlationMatrix)]
    pub fn correlation_matrix(&self) -> Box<[f64]> {
        self.inner.correlation_matrix().into()
    }

    /// Annualized factor volatilities, one per factor.
    #[wasm_bindgen(getter)]
    pub fn volatilities(&self) -> Box<[f64]> {
        self.inner.volatilities().into()
    }

    /// Turn independent standard normal draws into correlated, volatility-scaled factors.
    /// @param independent_z - Independent standard normal draws, exactly one per factor.
    /// @returns The correlated factor values, one per factor.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `independentZ` does not hold exactly
    /// `numFactors` entries.
    #[wasm_bindgen(js_name = generateCorrelatedFactors)]
    pub fn generate_correlated_factors(
        &self,
        independent_z: JsValue,
    ) -> Result<Box<[f64]>, JsValue> {
        self.inner
            .try_generate_correlated_factors(&js_f64_seq(&independent_z, "independentZ")?)
            .map(Vec::into_boxed_slice)
            .map_err(to_js_err)
    }
}

/// Largest path count accepted by `simulatePortfolioLoss`. Twin of the Rust
/// and Python constant `MAX_PORTFOLIO_LOSS_PATHS`.
/// @returns The path-count ceiling, `1000000`.
#[wasm_bindgen(js_name = maxPortfolioLossPaths)]
pub fn max_portfolio_loss_paths() -> usize {
    corr::MAX_PORTFOLIO_LOSS_PATHS
}

/// Cholesky factor of a correlation matrix.
/// @param matrix - Flat row-major `n * n` correlation matrix; symmetric with unit diagonal.
/// @param n - Positive square-matrix dimension; `matrix` must contain exactly `n * n` entries.
/// @returns The lower-triangular factor `L` with `L * L^T = matrix`, flat row-major.
///
/// # Errors
///
/// Throws a `validation` error if the length is not `n * n` or the matrix is
/// not a valid positive-semidefinite correlation matrix.
#[wasm_bindgen(js_name = choleskyDecompose)]
pub fn cholesky_decompose(matrix: JsValue, n: JsValue) -> Result<Box<[f64]>, JsValue> {
    let matrix = js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    corr::cholesky_decompose(&matrix, n)
        .map(|factor| factor.factor_matrix().into())
        .map_err(to_js_err)
}

/// Simulate the portfolio credit-loss distribution under a factor copula.
///
/// Paths use the deterministic path-indexed Philox scheme, so equal inputs
/// give equal losses in every host.
/// @param exposures - Array of `CreditExposure` objects (`id`, `notional`, `default_probability`, `lgd`, `factor_loadings`), or its JSON text; all in one currency.
/// @param config - `PortfolioLossConfig` object or JSON: `num_paths` (1 to `maxPortfolioLossPaths()`), `seed`, `confidence` in `(0, 1)` and `copula` (a `CopulaSpec` in JSON form, for example `CopulaSpec.gaussian().toJson()` parsed).
/// @param recovery - Optional `RecoverySpec` in JSON form (`spec.toJson()` or a plain object); when given, its conditional LGD replaces each exposure's `lgd` and every exposure needs exactly one factor loading.
/// @returns The simulated `PortfolioLossResult` handle.
///
/// # Errors
///
/// Throws a `TypeError` if an input is neither a string nor a plain object,
/// and a `validation` error if the exposures or configuration are invalid, the
/// recovery specification cannot build, or a simulated loss is non-finite.
#[wasm_bindgen(js_name = simulatePortfolioLoss)]
pub fn simulate_portfolio_loss(
    exposures: JsValue,
    config: JsValue,
    recovery: Option<JsValue>,
) -> Result<JsPortfolioLossResult, JsValue> {
    let exposures: Vec<CreditExposure> = from_js_json(&exposures, "exposures")?;
    let config: PortfolioLossConfig = from_js_json(&config, "config")?;
    let result = match recovery.filter(|value| !value.is_null() && !value.is_undefined()) {
        Some(recovery) => {
            let recovery: corr::RecoverySpec = from_js_json(&recovery, "recovery")?;
            corr::simulate_portfolio_loss_with_recovery(&exposures, &config, &recovery)
        }
        None => corr::simulate_portfolio_loss(&exposures, &config),
    };
    result
        .map(|inner| JsPortfolioLossResult { inner })
        .map_err(to_js_err)
}
