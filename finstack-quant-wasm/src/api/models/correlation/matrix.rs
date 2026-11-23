//! Correlation matrix bounds, validation, nearest matrix, portfolio loss, and Bernoulli bindings.

//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_models::correlation`]. The JS facade nests these exports
//! under `models.correlation`.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_opt_f64, js_opt_uint, js_uint};
use crate::utils::to_js_err;
use finstack_quant_core::math::linalg;
use finstack_quant_core::math::probability::{self, CorrelatedBernoulli};
use finstack_quant_models::correlation::{self as corr, CreditExposure, PortfolioLossConfig};
use wasm_bindgen::prelude::*;

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
/// @returns `[rho_min, rho_max]` Fréchet-Hoeffding bounds for the two Bernoulli marginals.
#[wasm_bindgen(js_name = correlationBounds)]
pub fn correlation_bounds(
    #[wasm_bindgen(unchecked_param_type = "number")] p1: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] p2: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let p1 = js_f64(&p1, "p1")?;
    let p2 = js_f64(&p2, "p2")?;
    let (lo, hi) = probability::correlation_bounds(p1, p2).map_err(to_js_err)?;
    Ok(Box::new([lo, hi]))
}

/// Validate a flat row-major correlation matrix.
///
/// Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
/// checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
/// semi-definiteness. Returns nothing on success; raises a descriptive error
/// (including the failing dimension or constraint) otherwise.
///
/// # Errors
///
/// Throws a JavaScript exception if `n` is non-finite, fractional, negative,
/// or exceeds the WebAssembly `usize` range; if `n * n` overflows or the flat
/// length is not `n * n`; if a diagonal entry is not one, an entry is outside
/// the correlation bounds, the matrix is not symmetric, or the matrix is not
/// positive semidefinite.
/// @param matrix - Flat row-major `n * n` correlation coefficients; unit diagonal, off-diagonals in `[-1, 1]`.
/// @param n - Finite non-negative integer square-matrix dimension no greater than `4294967295`; `matrix` must contain exactly `n * n` entries, and an empty matrix uses zero.
#[wasm_bindgen(js_name = validateCorrelationMatrix)]
pub fn validate_correlation_matrix(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
) -> Result<(), JsValue> {
    let matrix: &[f64] = &js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    linalg::validate_correlation_matrix(matrix, n).map_err(to_js_err)
}

/// Nearest correlation matrix (Higham 2002).
///
/// Given a flat row-major `n*n` matrix that is approximately a correlation
/// matrix but fails Cholesky by a small margin, returns the nearest valid
/// correlation matrix (symmetric, unit diagonal, PSD) in Frobenius norm.
/// Gross input violations raise rather than being silently reshaped.
///
/// # Errors
///
/// Throws a `TypeError` if `n` or `maxIter` is non-finite, fractional,
/// negative, or exceeds the WebAssembly `usize` range; a `validation` error
/// if `n * n` overflows, the flat length is not `n * n` or the input has a
/// gross diagonal or symmetry violation; and a `computation` error if the
/// projection does not converge within `maxIter` iterations at `tol`.
/// @param matrix - Flat row-major `n * n` near-correlation matrix to project onto the correlation set.
/// @param n - Non-negative integer square-matrix dimension; `matrix` must contain exactly `n * n` entries, and an empty matrix uses zero.
/// @param maxIter - Optional non-negative integer limit on Higham projection iterations; omitted uses Rust `NearestCorrelationOpts::default()` (200).
/// @param tol - Optional positive convergence tolerance for the projection; omitted uses Rust `NearestCorrelationOpts::default()` (`1e-10`).
/// @returns Nearest valid correlation matrix as a flat row-major `Float64Array` of `n * n` entries.
#[wasm_bindgen(js_name = nearestCorrelation)]
pub fn nearest_correlation(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
    #[wasm_bindgen(js_name = "maxIter", unchecked_optional_param_type = "number")] max_iter: Option<
        JsValue,
    >,
    #[wasm_bindgen(unchecked_optional_param_type = "number")] tol: Option<JsValue>,
) -> Result<Box<[f64]>, JsValue> {
    let matrix = js_f64_seq(&matrix, "matrix")?;
    let tol = js_opt_f64(tol.as_ref(), "tol")?;
    let n: usize = js_uint(&n, "n")?;
    let max_iter: Option<usize> = js_opt_uint(max_iter.as_ref(), "maxIter")?;
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = linalg::NearestCorrelationOpts::default();
    let opts = linalg::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    linalg::nearest_correlation(&matrix, n, opts)
        .map(Vec::into_boxed_slice)
        .map_err(to_js_err)
}

/// Portfolio credit-loss distribution with loss-positive VaR and expected
/// shortfall.
///
/// Mirrors the Rust `PortfolioLossResult` and the Python class of the same
/// name. Build one from a simulated loss vector with `fromLosses`, or load one
/// with `fromJson`; the aggregates are always recomputed from the losses.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const result = models.correlation.PortfolioLossResult.fromLosses([0, 1, 2, 5, 10], 0.75);
/// const equity = result.trancheLossStatistics(0, 3, 100);
/// console.log(result.var, equity.expected_loss_fraction);
/// ```
#[wasm_bindgen(js_name = PortfolioLossResult)]
pub struct JsPortfolioLossResult {
    pub(crate) inner: corr::PortfolioLossResult,
}

json_round_trip!(JsPortfolioLossResult, PortfolioLossResult);

wasm_getters!(JsPortfolioLossResult, PortfolioLossResult |s| {
    /// Loss per simulated path, in path order.
    losses -> Box<[f64]> = s.inner.losses.clone().into_boxed_slice();
    /// Arithmetic mean path loss.
    expected_loss as expectedLoss -> f64 = s.inner.expected_loss;
    /// Loss-positive nearest-rank VaR at `confidence` (larger is worse).
    var -> f64 = s.inner.var;
    /// Probability-weighted mean loss in the worst `1 - confidence` tail.
    expected_shortfall as expectedShortfall -> f64 = s.inner.expected_shortfall;
    /// Confidence used for `var` and `expectedShortfall`, in `(0, 1)`.
    confidence -> f64 = s.inner.confidence;
});

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
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] losses: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] confidence: JsValue,
    ) -> Result<JsPortfolioLossResult, JsValue> {
        let confidence = js_f64(&confidence, "confidence")?;
        let losses = crate::utils::input::js_f64_seq(&losses, "losses")?;
        corr::PortfolioLossResult::from_losses(losses, confidence)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
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
    /// @param attachPct - Lower tranche boundary in percentage points of pool notional from 0 through 100.
    /// @param detachPct - Upper tranche boundary in percentage points of pool notional, above attachPct and at most 100.
    /// @param poolNotional - Total pool notional, finite and strictly positive, in the same unit as the losses.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the tranche boundaries are invalid,
    /// `poolNotional` is not finite and positive, or a derived statistic is
    /// non-finite.
    /// @returns The tranche notional, expected loss, VaR, expected shortfall, and breach probabilities.
    #[wasm_bindgen(js_name = trancheLossStatistics, unchecked_return_type = "TrancheLossStatistics")]
    pub fn tranche_loss_statistics(
        &self,
        #[wasm_bindgen(js_name = "attachPct", unchecked_param_type = "number")] attach_pct: JsValue,
        #[wasm_bindgen(js_name = "detachPct", unchecked_param_type = "number")] detach_pct: JsValue,
        #[wasm_bindgen(js_name = "poolNotional", unchecked_param_type = "number")]
        pool_notional: JsValue,
    ) -> Result<JsValue, JsValue> {
        let attach_pct = js_f64(&attach_pct, "attachPct")?;
        let detach_pct = js_f64(&detach_pct, "detachPct")?;
        let pool_notional = js_f64(&pool_notional, "poolNotional")?;
        let stats = self
            .inner
            .tranche_loss_statistics(attach_pct, detach_pct, pool_notional)
            .map_err(to_js_err)?;
        crate::utils::to_js_value(&stats)
    }
}

/// Two correlated Bernoulli default indicators with exact joint probabilities.
///
/// The requested correlation must satisfy the Fréchet-Hoeffding bounds of the
/// two marginals; `correlation` reports the value actually achieved.
///
/// @example
/// ```typescript
/// import init, { models } from "finstack-quant-wasm";
/// await init();
/// const pair = new models.correlation.CorrelatedBernoulli(0.1, 0.2, 0.3);
/// console.log(pair.jointP11, pair.jointProbabilities(), pair.sampleFromUniform(0.5));
/// pair.free();
/// ```
#[wasm_bindgen(js_name = CorrelatedBernoulli)]
pub struct JsCorrelatedBernoulli {
    pub(crate) inner: CorrelatedBernoulli,
}

wasm_getters!(JsCorrelatedBernoulli, CorrelatedBernoulli |s| {
    /// First marginal probability.
    p1 -> f64 = s.inner.p1();
    /// Second marginal probability.
    p2 -> f64 = s.inner.p2();
    /// Feasible correlation supplied at construction.
    correlation -> f64 = s.inner.correlation();
    /// Probability that both variables equal one.
    joint_p11 as jointP11 -> f64 = s.inner.joint_p11();
    /// Probability that the first is one and the second zero.
    joint_p10 as jointP10 -> f64 = s.inner.joint_p10();
    /// Probability that the first is zero and the second one.
    joint_p01 as jointP01 -> f64 = s.inner.joint_p01();
    /// Probability that both variables equal zero.
    joint_p00 as jointP00 -> f64 = s.inner.joint_p00();
});

#[wasm_bindgen(js_class = CorrelatedBernoulli)]
impl JsCorrelatedBernoulli {
    /// Joint distribution of two Bernoulli variables with a target correlation.
    /// @param p1 - First marginal probability from 0 through 1.
    /// @param p2 - Second marginal probability from 0 through 1.
    /// @param correlation - Finite correlation within the attainable Fréchet-Hoeffding range for these marginals.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a probability is outside `[0, 1]` or the
    /// correlation is non-finite or outside `[-1, 1]`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "number")] p1: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] p2: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] correlation: JsValue,
    ) -> Result<JsCorrelatedBernoulli, JsValue> {
        CorrelatedBernoulli::new(
            js_f64(&p1, "p1")?,
            js_f64(&p2, "p2")?,
            js_f64(&correlation, "correlation")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
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
    pub fn sample_from_uniform(
        &self,
        #[wasm_bindgen(unchecked_param_type = "number")] u: JsValue,
    ) -> Result<Box<[u8]>, JsValue> {
        let (x1, x2) = self
            .inner
            .sample_from_uniform(js_f64(&u, "u")?)
            .map_err(to_js_err)?;
        Ok(Box::new([x1, x2]))
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
/// @returns A factor `F` in original variable order with `F * F^T = matrix`, flat row-major. It is generally not lower-triangular, so apply it with a full matrix product, not `applyLowerTriangular`.
///
/// # Errors
///
/// Throws a `validation` error if the length is not `n * n` or the matrix is
/// not a valid positive-semidefinite correlation matrix.
#[wasm_bindgen(js_name = choleskyCorrelation)]
pub fn cholesky_correlation(
    #[wasm_bindgen(unchecked_param_type = "NumericArray")] matrix: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] n: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let matrix = js_f64_seq(&matrix, "matrix")?;
    let n: usize = js_uint(&n, "n")?;
    finstack_quant_core::math::linalg::cholesky_correlation(&matrix, n)
        .map(|factor| factor.factor_matrix().into())
        .map_err(|e| to_js_err(corr::Error::from(e)))
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
    #[wasm_bindgen(unchecked_param_type = "readonly generated.models.CreditExposure[] | string")]
    exposures: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.models.PortfolioLossConfig | string")] config: JsValue,
    #[wasm_bindgen(
        unchecked_optional_param_type = "generated.models.RecoverySpec | string | null"
    )]
    recovery: Option<JsValue>,
) -> Result<JsPortfolioLossResult, JsValue> {
    let exposures: Vec<CreditExposure> = from_js_json(&exposures, "exposures")?;
    let config: PortfolioLossConfig = from_js_json(&config, "config")?;
    let recovery: Option<corr::RecoverySpec> = recovery
        .filter(|value| !value.is_null() && !value.is_undefined())
        .map(|value| from_js_json(&value, "recovery"))
        .transpose()?;
    corr::simulate_portfolio_loss(&exposures, &config, recovery.as_ref())
        .map(|inner| JsPortfolioLossResult { inner })
        .map_err(to_js_err)
}
