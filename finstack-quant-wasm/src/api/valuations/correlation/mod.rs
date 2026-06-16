//! WASM bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_quant_valuations::correlation`]. The JS facade nests these exports
//! under `fs.valuations.correlation`.

use crate::utils::to_js_err;
use finstack_quant_valuations::correlation::{self as corr, Copula, CopulaSpec, RecoveryModel};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// CopulaSpec
// ---------------------------------------------------------------------------

/// Copula model specification for configuration and deferred construction.
#[wasm_bindgen(js_name = CopulaSpec)]
pub struct WasmCopulaSpec {
    #[wasm_bindgen(skip)]
    inner: CopulaSpec,
}

#[wasm_bindgen(js_class = CopulaSpec)]
impl WasmCopulaSpec {
    /// One-factor Gaussian copula (market standard).
    #[wasm_bindgen(js_name = gaussian)]
    pub fn gaussian() -> Self {
        Self {
            inner: CopulaSpec::gaussian(),
        }
    }

    /// Student-t copula with specified degrees of freedom (must be > 2).
    #[wasm_bindgen(js_name = studentT)]
    pub fn student_t(df: f64) -> Result<WasmCopulaSpec, JsValue> {
        CopulaSpec::student_t(df)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Random Factor Loading copula with stochastic correlation.
    #[wasm_bindgen(js_name = randomFactorLoading)]
    pub fn random_factor_loading(loading_vol: f64) -> Self {
        Self {
            inner: CopulaSpec::random_factor_loading(loading_vol),
        }
    }

    /// Multi-factor Gaussian copula with sector structure.
    #[wasm_bindgen(js_name = multiFactor)]
    pub fn multi_factor(num_factors: usize) -> Self {
        Self {
            inner: CopulaSpec::multi_factor(num_factors),
        }
    }

    /// Build a concrete copula from this specification.
    #[wasm_bindgen(js_name = build)]
    pub fn build(&self) -> Result<WasmCopula, JsValue> {
        self.inner
            .build()
            .map(|inner| WasmCopula {
                inner,
                spec: self.inner.clone(),
            })
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

// ---------------------------------------------------------------------------
// Copula (trait object wrapper)
// ---------------------------------------------------------------------------

/// Concrete copula model for portfolio default correlation.
#[wasm_bindgen(js_name = Copula)]
pub struct WasmCopula {
    #[wasm_bindgen(skip)]
    inner: Box<dyn Copula + Send + Sync>,
    /// Originating spec, retained so concrete-model-only diagnostics
    /// (`stressCorrelationProxy`) can be dispatched.
    #[wasm_bindgen(skip)]
    spec: CopulaSpec,
}

#[wasm_bindgen(js_class = Copula)]
impl WasmCopula {
    /// Conditional default probability given factor realization(s).
    #[wasm_bindgen(js_name = conditionalDefaultProb)]
    pub fn conditional_default_prob(
        &self,
        default_threshold: f64,
        factor_realization: &[f64],
        correlation: f64,
    ) -> f64 {
        self.inner
            .conditional_default_prob(default_threshold, factor_realization, correlation)
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
    #[wasm_bindgen(js_name = tailDependence)]
    pub fn tail_dependence(&self, correlation: f64) -> f64 {
        self.inner.tail_dependence(correlation)
    }

    /// Heuristic stress-correlation proxy for the Random Factor Loading
    /// copula.
    ///
    /// This is **not** the strict copula lower-tail-dependence coefficient
    /// `λ_L` (which has no closed form for RFL — `tailDependence` returns
    /// `NaN`). It gauges the extra correlation mass in the high-loading
    /// tail and vanishes in the Gaussian (`loadingVol = 0`) limit.
    ///
    /// Throws for non-RFL copulas.
    #[wasm_bindgen(js_name = stressCorrelationProxy)]
    pub fn stress_correlation_proxy(&self, correlation: f64) -> Result<f64, JsValue> {
        match &self.spec {
            CopulaSpec::RandomFactorLoading { loading_volatility } => {
                Ok(corr::RandomFactorLoadingCopula::new(*loading_volatility)
                    .stress_correlation_proxy(correlation))
            }
            _ => Err(JsValue::from_str(&format!(
                "stressCorrelationProxy is only defined for the Random Factor Loading \
                 copula, got '{}'",
                self.inner.model_name()
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// RecoverySpec
// ---------------------------------------------------------------------------

/// Recovery model specification for configuration and deferred construction.
#[wasm_bindgen(js_name = RecoverySpec)]
pub struct WasmRecoverySpec {
    #[wasm_bindgen(skip)]
    inner: corr::RecoverySpec,
}

#[wasm_bindgen(js_class = RecoverySpec)]
impl WasmRecoverySpec {
    /// Constant recovery rate.
    ///
    /// Throws if `rate` is not finite or lies outside `[0, 1]`.
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(rate: f64) -> Result<WasmRecoverySpec, JsValue> {
        corr::RecoverySpec::constant(rate)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Market-correlated (Andersen-Sidenius) stochastic recovery.
    ///
    /// Throws if `mean` is not finite or lies outside `[0, 1]`, or if `vol` /
    /// `correlation` are not finite.
    #[wasm_bindgen(js_name = marketCorrelated)]
    pub fn market_correlated(
        mean: f64,
        vol: f64,
        correlation: f64,
    ) -> Result<WasmRecoverySpec, JsValue> {
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
    pub fn build(&self) -> WasmRecoveryModel {
        WasmRecoveryModel {
            inner: self.inner.build(),
        }
    }
}

// ---------------------------------------------------------------------------
// RecoveryModel (trait object wrapper)
// ---------------------------------------------------------------------------

/// Concrete recovery model for credit portfolio pricing.
#[wasm_bindgen(js_name = RecoveryModel)]
pub struct WasmRecoveryModel {
    #[wasm_bindgen(skip)]
    inner: Box<dyn RecoveryModel + Send + Sync>,
}

#[wasm_bindgen(js_class = RecoveryModel)]
impl WasmRecoveryModel {
    /// Expected (unconditional) recovery rate.
    #[wasm_bindgen(getter, js_name = expectedRecovery)]
    pub fn expected_recovery(&self) -> f64 {
        self.inner.expected_recovery()
    }

    /// Recovery conditional on the systematic market factor.
    #[wasm_bindgen(js_name = conditionalRecovery)]
    pub fn conditional_recovery(&self, market_factor: f64) -> f64 {
        self.inner.conditional_recovery(market_factor)
    }

    /// Loss given default (1 − recovery).
    #[wasm_bindgen(getter, js_name = lgd)]
    pub fn lgd(&self) -> f64 {
        self.inner.lgd()
    }

    /// Conditional LGD given market factor.
    #[wasm_bindgen(js_name = conditionalLgd)]
    pub fn conditional_lgd(&self, market_factor: f64) -> f64 {
        self.inner.conditional_lgd(market_factor)
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

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Fréchet-Hoeffding correlation bounds for two Bernoulli marginals.
///
/// Returns `[rho_min, rho_max]`.
#[wasm_bindgen(js_name = correlationBounds)]
pub fn correlation_bounds(p1: f64, p2: f64) -> Vec<f64> {
    let (lo, hi) = corr::correlation_bounds(p1, p2);
    vec![lo, hi]
}

/// Joint probabilities for two correlated Bernoulli variables.
///
/// Returns `[p11, p10, p01, p00]`.
#[wasm_bindgen(js_name = jointProbabilities)]
pub fn joint_probabilities(p1: f64, p2: f64, correlation: f64) -> Vec<f64> {
    let (p11, p10, p01, p00) = corr::joint_probabilities(p1, p2, correlation);
    vec![p11, p10, p01, p00]
}

/// Validate a flat row-major correlation matrix.
///
/// Accepts a `Float64Array`/`number[]` of `n * n` row-major entries and
/// checks unit diagonal, off-diagonal in `[-1, 1]`, symmetry, and positive
/// semi-definiteness. Returns nothing on success; raises a descriptive error
/// (including the failing dimension or constraint) otherwise.
#[wasm_bindgen(js_name = validateCorrelationMatrix)]
pub fn validate_correlation_matrix(matrix: &[f64], n: usize) -> Result<(), JsValue> {
    corr::validate_correlation_matrix(matrix, n).map_err(to_js_err)
}

/// Nearest correlation matrix (Higham 2002).
///
/// Given a flat row-major `n*n` matrix that is approximately a correlation
/// matrix but fails Cholesky by a small margin, returns the nearest valid
/// correlation matrix (symmetric, unit diagonal, PSD) in Frobenius norm.
/// Gross input violations raise rather than being silently reshaped.
#[wasm_bindgen(js_name = nearestCorrelation)]
pub fn nearest_correlation(
    matrix: Vec<f64>,
    n: usize,
    max_iter: Option<usize>,
    tol: Option<f64>,
) -> Result<Vec<f64>, JsValue> {
    // Single source of truth for the defaults: the Rust
    // `NearestCorrelationOpts::default()` (max_iter = 200, tol = 1e-10).
    let defaults = corr::NearestCorrelationOpts::default();
    let opts = corr::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(defaults.max_iter),
        tol: tol.unwrap_or(defaults.tol),
    };
    corr::nearest_correlation_matrix(&matrix, n, opts).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::standard_normal_inv_cdf;

    #[test]
    fn wasm_copula_spec_gaussian_and_student_t() {
        let g = WasmCopulaSpec::gaussian();
        assert!(g.is_gaussian());
        assert!(!g.is_student_t());

        let Ok(t) = WasmCopulaSpec::student_t(5.0) else {
            panic!("student_t(5.0) should succeed");
        };
        assert!(t.is_student_t());
        assert!(!t.is_gaussian());
    }

    #[test]
    fn wasm_copula_spec_random_factor_loading_and_multi_factor_build() {
        let rfl = WasmCopulaSpec::random_factor_loading(0.5);
        assert!(!rfl.is_gaussian());
        assert!(!rfl.is_student_t());
        assert!(rfl.is_rfl());
        assert!(!rfl.is_multi_factor());
        let rfl_copula = rfl.build().expect("RFL copula should build");
        assert_eq!(rfl_copula.num_factors(), 2);

        let mf = WasmCopulaSpec::multi_factor(2);
        assert!(mf.is_multi_factor());
        assert!(!mf.is_rfl());
        let mf_copula = mf.build().expect("multi-factor copula should build");
        assert_eq!(mf_copula.num_factors(), 2);
    }

    #[test]
    fn wasm_copula_stress_correlation_proxy_rfl_only() {
        let rfl = WasmCopulaSpec::random_factor_loading(0.2)
            .build()
            .expect("RFL copula should build");
        // RFL has no closed-form λ_L: NaN per the tail-dependence contract.
        assert!(rfl.tail_dependence(0.3).is_nan());
        let proxy = rfl
            .stress_correlation_proxy(0.3)
            .expect("proxy defined for RFL");
        assert!(proxy > 0.0, "proxy should be positive for σ_β > 0: {proxy}");

        // The non-RFL error path constructs a `JsValue`, which panics on
        // non-wasm32 targets, so it can only be asserted in wasm tests.
        #[cfg(target_arch = "wasm32")]
        {
            let gaussian = WasmCopulaSpec::gaussian()
                .build()
                .expect("Gaussian copula should build");
            assert!(
                gaussian.stress_correlation_proxy(0.3).is_err(),
                "proxy must throw for non-RFL copulas"
            );
        }
    }

    #[test]
    fn wasm_copula_from_gaussian_spec() {
        let copula = WasmCopulaSpec::gaussian()
            .build()
            .expect("Gaussian copula should build");
        assert_eq!(copula.num_factors(), 1);
        assert_eq!(copula.model_name(), "One-Factor Gaussian Copula");
        assert_eq!(copula.tail_dependence(0.3), 0.0);

        let pd = 0.05_f64;
        let threshold = standard_normal_inv_cdf(pd);
        let correlation = 0.3_f64;
        let cond = copula.conditional_default_prob(threshold, &[0.0], correlation);
        assert!(cond > 0.0 && cond < 1.0);
    }

    #[test]
    fn wasm_recovery_spec_and_model() {
        let c = WasmRecoverySpec::constant(0.4).expect("0.4 is a valid recovery rate");
        assert!((c.expected_recovery() - 0.4).abs() < 1e-12);
        let m = c.build();
        assert!((m.expected_recovery() - 0.4).abs() < 1e-12);
        assert!((m.conditional_recovery(0.0) - 0.4).abs() < 1e-12);
        assert!((m.lgd() - 0.6).abs() < 1e-12);
        assert!(!m.is_stochastic());
        assert!(!m.model_name().is_empty());

        let mc = WasmRecoverySpec::market_correlated(0.4, 0.1, 0.3)
            .expect("valid market-correlated spec")
            .build();
        assert!(mc.is_stochastic());
        assert!(mc.recovery_volatility() > 0.0);
        assert!(
            (mc.conditional_lgd(0.0) - (1.0 - mc.conditional_recovery(0.0))).abs() < 1e-12,
            "conditional_lgd must complement conditional_recovery"
        );

        let std = WasmRecoverySpec::market_standard_stochastic().build();
        assert!(std.is_stochastic());
        assert!((std.recovery_volatility() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn wasm_recovery_spec_constant_rejects_out_of_range_and_nan() {
        // RecoverySpec::constant rejects rates outside [0, 1] and non-finite
        // values at the Rust API boundary.
        assert!(
            WasmRecoverySpec::constant(1.5).is_err(),
            "recovery rate above 1 must be rejected, not clamped"
        );
        assert!(
            WasmRecoverySpec::constant(-0.2).is_err(),
            "negative recovery rate must be rejected, not clamped"
        );
        assert!(
            WasmRecoverySpec::constant(f64::NAN).is_err(),
            "NaN recovery rate must be rejected"
        );
        // The valid endpoints must still be accepted.
        assert!(WasmRecoverySpec::constant(0.0).is_ok());
        assert!(WasmRecoverySpec::constant(1.0).is_ok());
    }

    #[test]
    fn wasm_recovery_spec_market_correlated_validates_inputs() {
        // Mean recovery outside [0, 1] or non-finite must be rejected.
        assert!(WasmRecoverySpec::market_correlated(1.5, 0.1, 0.3).is_err());
        assert!(WasmRecoverySpec::market_correlated(f64::NAN, 0.1, 0.3).is_err());
        // Non-finite vol / correlation must also be rejected.
        assert!(WasmRecoverySpec::market_correlated(0.4, f64::NAN, 0.3).is_err());
        assert!(WasmRecoverySpec::market_correlated(0.4, 0.1, f64::INFINITY).is_err());
        // A fully valid spec is still accepted.
        assert!(WasmRecoverySpec::market_correlated(0.4, 0.25, -0.4).is_ok());
    }

    #[test]
    fn correlation_bounds_ordered() {
        let b = correlation_bounds(0.05, 0.10);
        assert_eq!(b.len(), 2);
        assert!(b[0] <= b[1]);
    }

    #[test]
    fn joint_probabilities_sum_to_one() {
        let j = joint_probabilities(0.05, 0.10, 0.3);
        assert_eq!(j.len(), 4);
        let sum: f64 = j.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn validate_correlation_matrix_accepts_valid_and_rejects_invalid() {
        #[rustfmt::skip]
        let good = vec![
            1.0, 0.5, 0.3,
            0.5, 1.0, 0.4,
            0.3, 0.4, 1.0,
        ];
        assert!(validate_correlation_matrix(&good, 3).is_ok());

        // Off-diagonal outside [-1, 1] must be rejected.
        #[rustfmt::skip]
        let bad = vec![
            1.0, 1.5,
            1.5, 1.0,
        ];
        assert!(validate_correlation_matrix(&bad, 2).is_err());

        // Length / dimension mismatch must be rejected, not panic.
        assert!(validate_correlation_matrix(&good, 2).is_err());
    }

    #[test]
    fn nearest_correlation_repairs_near_psd_input() {
        // Valid correlation matrix passes through unchanged.
        #[rustfmt::skip]
        let good = vec![
            1.0, 0.5, 0.3,
            0.5, 1.0, 0.4,
            0.3, 0.4, 1.0,
        ];
        let out = nearest_correlation(good, 3, None, None).expect("good matrix should project");
        assert_eq!(out.len(), 9);
        for i in 0..3 {
            assert!((out[i * 3 + i] - 1.0).abs() < 1e-9);
        }
    }
}
