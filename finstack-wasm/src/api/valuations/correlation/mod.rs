//! WASM bindings for the credit-correlation module.
//!
//! Exposes copula models, recovery models, and joint probability utilities
//! to JavaScript/TypeScript via `wasm-bindgen`, mirroring the Rust module
//! [`finstack_valuations::correlation`]. The JS facade nests these exports
//! under `fs.valuations.correlation`.

use crate::utils::to_js_err;
use finstack_valuations::correlation::{self as corr, Copula, CopulaSpec, RecoveryModel};
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
        if !df.is_finite() || df <= 2.0 {
            return Err(to_js_err(
                "Student-t degrees of freedom must be a finite number > 2",
            ));
        }
        Ok(Self {
            inner: CopulaSpec::student_t(df),
        })
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
    pub fn build(&self) -> WasmCopula {
        WasmCopula {
            inner: self.inner.build(),
        }
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
}

// ---------------------------------------------------------------------------
// Copula (trait object wrapper)
// ---------------------------------------------------------------------------

/// Concrete copula model for portfolio default correlation.
#[wasm_bindgen(js_name = Copula)]
pub struct WasmCopula {
    #[wasm_bindgen(skip)]
    inner: Box<dyn Copula + Send + Sync>,
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

    /// Lower-tail dependence coefficient at the given correlation.
    #[wasm_bindgen(js_name = tailDependence)]
    pub fn tail_dependence(&self, correlation: f64) -> f64 {
        self.inner.tail_dependence(correlation)
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

/// Validate a recovery rate before it reaches the (silently clamping) core
/// [`corr::RecoverySpec`] constructors.
///
/// The core builders clamp out-of-range inputs and propagate `NaN`, masking
/// caller errors. Validating here — and throwing on bad input — keeps these
/// constructors consistent with the validating `MultiFactorModel` siblings.
fn validate_recovery_rate(value: f64, label: &str) -> Result<f64, JsValue> {
    if !value.is_finite() {
        return Err(to_js_err(format!("{label} must be finite, got {value}")));
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(to_js_err(format!("{label} must be in [0, 1], got {value}")));
    }
    Ok(value)
}

#[wasm_bindgen(js_class = RecoverySpec)]
impl WasmRecoverySpec {
    /// Constant recovery rate.
    ///
    /// Throws if `rate` is not finite or lies outside `[0, 1]`.
    #[wasm_bindgen(js_name = constant)]
    pub fn constant(rate: f64) -> Result<WasmRecoverySpec, JsValue> {
        let rate = validate_recovery_rate(rate, "recovery rate")?;
        Ok(Self {
            inner: corr::RecoverySpec::constant(rate),
        })
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
        let mean = validate_recovery_rate(mean, "mean recovery")?;
        if !vol.is_finite() {
            return Err(to_js_err(format!(
                "recovery volatility must be finite, got {vol}"
            )));
        }
        if !correlation.is_finite() {
            return Err(to_js_err(format!(
                "factor correlation must be finite, got {correlation}"
            )));
        }
        Ok(Self {
            inner: corr::RecoverySpec::market_correlated(mean, vol, correlation),
        })
    }

    /// Expected (unconditional) recovery rate.
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
///
/// Unique wasm export name (`validateValuationsCorrelationMatrix`) so it does
/// not collide with `core/math`'s nested-array `validateCorrelationMatrix`;
/// the `valuations.correlation` JS facade re-exports it as
/// `validateCorrelationMatrix`.
#[wasm_bindgen(js_name = validateValuationsCorrelationMatrix)]
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
    let opts = corr::NearestCorrelationOpts {
        max_iter: max_iter.unwrap_or(200),
        tol: tol.unwrap_or(1e-10),
    };
    corr::nearest_correlation_matrix(&matrix, n, opts).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::math::standard_normal_inv_cdf;

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
        let rfl_copula = rfl.build();
        assert_eq!(rfl_copula.num_factors(), 2);

        let mf = WasmCopulaSpec::multi_factor(2);
        let mf_copula = mf.build();
        assert_eq!(mf_copula.num_factors(), 2);
    }

    #[test]
    fn wasm_copula_from_gaussian_spec() {
        let copula = WasmCopulaSpec::gaussian().build();
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
    }

    #[test]
    fn wasm_recovery_spec_constant_rejects_out_of_range_and_nan() {
        // The core `RecoverySpec::constant` silently clamps; the binding must
        // instead reject rates outside [0, 1] and non-finite values.
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
        // Non-finite vol / correlation must also be rejected (NaN survives the
        // core's `clamp`, so it would otherwise leak into the model).
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
        let out =
            nearest_correlation(good.clone(), 3, None, None).expect("good matrix should project");
        assert_eq!(out.len(), 9);
        for i in 0..3 {
            assert!((out[i * 3 + i] - 1.0).abs() < 1e-9);
        }
    }
}
