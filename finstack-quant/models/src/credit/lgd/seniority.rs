//! Seniority-based recovery rate distributions.
//!
//! Models recovery rates using Beta distributions parameterized by debt
//! seniority class. Provides built-in Moody's and S&P historical calibrations.
//!
//! # References
//!
//! - Altman, E. I., Resti, A., & Sironi, A. (2005). "Recovery Risk." Risk Books. `docs/REFERENCES.md#altman-et-al-2005-recovery`
//! - Schuermann, T. (2004). "What Do We Know About Loss Given Default?"
//!   Wharton Financial Institutions Center Working Paper 04-01. `docs/REFERENCES.md#schuermann-2004-lgd`

use finstack_quant_core::error::InputError;
use finstack_quant_core::math::random::Pcg64Rng;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::Result;

/// Debt seniority classification for recovery rate modeling.
///
/// Recovery rates vary significantly by position in the capital structure.
/// These classes align with rating agency (Moody's, S&P) reporting categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SeniorityClass {
    /// First-lien secured debt with explicit collateral priority.
    #[serde(rename = "1st_lien_secured")]
    FirstLienSecured,
    /// Second-lien secured debt, junior to first-lien secured claims.
    #[serde(rename = "2nd_lien_secured")]
    SecondLienSecured,
    /// First-lien secured debt (bank loans, secured bonds).
    SeniorSecured,
    /// Senior unsecured bonds and loans.
    SeniorUnsecured,
    /// Subordinated debt (mezzanine, second-lien).
    Subordinated,
    /// Junior subordinated and deeply subordinated instruments.
    JuniorSubordinated,
}

impl std::str::FromStr for SeniorityClass {
    type Err = finstack_quant_core::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "1st_lien_secured" => Ok(Self::FirstLienSecured),
            "2nd_lien_secured" => Ok(Self::SecondLienSecured),
            "senior_secured" => Ok(Self::SeniorSecured),
            "senior_unsecured" => Ok(Self::SeniorUnsecured),
            "subordinated" => Ok(Self::Subordinated),
            "junior_subordinated" => Ok(Self::JuniorSubordinated),
            _ => Err(finstack_quant_core::Error::Validation(format!(
                "unknown seniority class: '{s}' (expected 1st_lien_secured, 2nd_lien_secured, senior_secured, senior_unsecured, subordinated, junior_subordinated)"
            ))),
        }
    }
}

/// Beta distribution parameterization for recovery rates.
///
/// The Beta distribution is defined on \[0,1\] and parameterized here by
/// its mean and standard deviation, which map to shape parameters:
///
/// ```text
/// alpha = mean * ((mean * (1 - mean) / variance) - 1)
/// beta = (1 - mean) * ((mean * (1 - mean) / variance) - 1)
/// ```
///
/// Constraint: variance < mean * (1 - mean), ensuring alpha > 0 and beta > 0.
///
/// # References
///
/// - Altman, E. I., Resti, A., & Sironi, A. (2005). "Recovery Risk." Risk Books. `docs/REFERENCES.md#altman-et-al-2005-recovery`
/// - Schuermann, T. (2004). "What Do We Know About Loss Given Default?"
///   Wharton Financial Institutions Center Working Paper 04-01. `docs/REFERENCES.md#schuermann-2004-lgd`
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "BetaRecoveryWire")]
pub struct BetaRecovery {
    /// Mean recovery rate in (0, 1).
    mean: f64,
    /// Standard deviation of recovery rate.
    std_dev: f64,
    /// Precomputed Beta shape parameter alpha.
    alpha: f64,
    /// Precomputed Beta shape parameter beta.
    beta_param: f64,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct BetaRecoveryWire {
    mean: f64,
    std_dev: f64,
    alpha: f64,
    beta_param: f64,
}

impl TryFrom<BetaRecoveryWire> for BetaRecovery {
    type Error = finstack_quant_core::Error;

    fn try_from(wire: BetaRecoveryWire) -> Result<Self> {
        let recovery = BetaRecovery::new(wire.mean, wire.std_dev)?;
        if recovery.alpha.to_bits() != wire.alpha.to_bits()
            || recovery.beta_param.to_bits() != wire.beta_param.to_bits()
        {
            return Err(finstack_quant_core::Error::Validation(
                "serialized BetaRecovery shape parameters do not match mean/std_dev".to_string(),
            ));
        }
        Ok(recovery)
    }
}

impl BetaRecovery {
    /// Create a Beta recovery distribution from mean and standard deviation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `mean` is not in (0, 1) exclusive
    /// - `std_dev <= 0`
    /// - `std_dev^2 >= mean * (1 - mean)` (Beta shape parameters would be non-positive)
    ///
    /// # Arguments
    ///
    /// * `mean` - Finite distribution mean in the same units as the modeled variable.
    /// * `std_dev` - Non-negative distribution standard deviation in modeled-variable units.
    pub fn new(mean: f64, std_dev: f64) -> Result<Self> {
        if !mean.is_finite() || mean <= 0.0 || mean >= 1.0 {
            return Err(InputError::Invalid.into());
        }
        if !std_dev.is_finite() || std_dev <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        let variance = std_dev * std_dev;
        let m1m = mean * (1.0 - mean);
        if variance >= m1m {
            return Err(InputError::Invalid.into());
        }
        let common = m1m / variance - 1.0;
        let alpha = mean * common;
        let beta_param = (1.0 - mean) * common;
        Ok(Self {
            mean,
            std_dev,
            alpha,
            beta_param,
        })
    }

    /// Mean recovery rate.
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Standard deviation of recovery.
    pub fn std_dev(&self) -> f64 {
        self.std_dev
    }

    /// Beta shape parameter alpha.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Beta shape parameter beta.
    pub fn beta_param(&self) -> f64 {
        self.beta_param
    }

    /// Variance of the distribution.
    pub fn variance(&self) -> f64 {
        self.std_dev * self.std_dev
    }

    /// Mode of the Beta distribution (valid when alpha > 1 and beta > 1).
    pub fn mode(&self) -> Option<f64> {
        if self.alpha > 1.0 && self.beta_param > 1.0 {
            Some((self.alpha - 1.0) / (self.alpha + self.beta_param - 2.0))
        } else {
            None
        }
    }

    /// Sample a single recovery rate draw using the `statrs` Beta distribution.
    ///
    /// # Errors
    ///
    /// Propagates sampling errors instead of silently falling back to the
    /// mean (). With the shape
    /// parameters validated by [`BetaRecovery::new`] this is near-unreachable,
    /// but a silent mean fallback would corrupt tail statistics undetected.
    ///
    /// # Arguments
    ///
    /// * `rng` - Random-number generator supplying uniform or Gaussian draws for simulation
    pub fn sample(
        &self,
        rng: &mut dyn finstack_quant_core::math::RandomNumberGenerator,
    ) -> Result<f64> {
        finstack_quant_core::math::distributions::sample_beta(rng, self.alpha, self.beta_param)
    }

    /// Sample N recovery rates with a deterministic PCG64 seed.
    ///
    /// # Errors
    ///
    /// Propagates sampling errors (see [`BetaRecovery::sample`]).
    pub fn sample_seeded(&self, n_samples: usize, seed: u64) -> Result<Vec<f64>> {
        let mut rng = Pcg64Rng::new(seed);
        let mut out = vec![0.0_f64; n_samples];
        for value in &mut out {
            *value = self.sample(&mut rng)?;
        }
        Ok(out)
    }

    /// Quantile function (inverse CDF) of the Beta distribution.
    ///
    /// Returns the value x such that P(X <= x) = p.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the Beta distribution cannot be
    /// constructed from the precomputed shape parameters. This is
    /// near-unreachable (the parameters are validated positive and finite by
    /// [`BetaRecovery::new`]); it previously fell back silently to the mean,
    /// which would corrupt quantile-based tail measures undetected
    /// ().
    ///
    /// # Arguments
    ///
    /// * `p` - Probability or model parameter in the range required by the enclosing calculation.
    pub fn quantile(&self, p: f64) -> Result<f64> {
        use statrs::distribution::{Beta, ContinuousCDF};
        if !p.is_finite() || !(0.0..=1.0).contains(&p) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "BetaRecovery::quantile probability must be finite and in [0, 1], got {p}"
            )));
        }
        let dist = Beta::new(self.alpha, self.beta_param).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "BetaRecovery::quantile: invalid Beta(alpha={}, beta={}): {e}",
                self.alpha, self.beta_param
            ))
        })?;
        Ok(dist.inverse_cdf(p))
    }

    /// Expected recovery rate (same as mean).
    pub fn expected(&self) -> f64 {
        self.mean
    }

    /// LGD = 1 - recovery. Returns mean LGD.
    pub fn mean_lgd(&self) -> f64 {
        1.0 - self.mean
    }
}

/// Historical recovery calibration by seniority class.
///
/// Provides Beta distribution parameters derived from long-run empirical
/// recovery studies. Users can construct custom calibrations or use the
/// built-in Moody's and S&P historical defaults.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SeniorityCalibration {
    /// Source label (e.g., "Moody's 1982-2023").
    pub source: String,
    /// Per-seniority Beta recovery parameters.
    pub classes: Vec<(SeniorityClass, BetaRecovery)>,
}

impl SeniorityCalibration {
    /// Load a built-in rating-agency calibration by name.
    ///
    /// Accepted names are case-insensitive. Moody's accepts `moodys`,
    /// `moody`, and `moody_s`; S&P accepts `sp`, `s_p`,
    /// `standard_and_poors`, and `standardandpoors`.
    ///
    /// # Errors
    /// Returns a validation error for unknown agencies.
    ///
    /// # Arguments
    ///
    /// * `agency` - Agency used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn from_agency(agency: &str) -> Result<Self> {
        let norm = agency.trim().to_ascii_lowercase().replace(['&', '.'], "");
        match norm.as_str() {
            "moodys" | "moody" | "moody_s" => Self::moodys_historical(),
            "sp" | "s_p" | "standard_and_poors" | "standardandpoors" => Self::sp_historical(),
            _ => Err(finstack_quant_core::Error::Validation(format!(
                "unknown rating agency: '{agency}' (expected 'moodys' or 'sp')"
            ))),
        }
    }

    /// Load a seniority recovery calibration from the credit assumptions registry.
    pub fn from_registry_id(id: &str) -> Result<Self> {
        crate::credit::registry::embedded_registry()?.seniority_calibration(id)
    }

    /// Moody's historical average recovery rates (1982-2023, issuer-weighted).
    ///
    /// Approximate long-run averages:
    /// - Senior Secured:       ~52%, std_dev ~25%
    /// - Senior Unsecured:     ~37%, std_dev ~24%
    /// - Subordinated:         ~28%, std_dev ~20%
    /// - Junior Subordinated:  ~17%, std_dev ~15%
    pub fn moodys_historical() -> Result<Self> {
        Self::from_registry_id(
            crate::credit::registry::embedded_registry()?.default_seniority_calibration_id(),
        )
    }

    /// S&P historical average recovery rates (approximate long-run).
    ///
    /// Approximate long-run averages:
    /// - Senior Secured:       ~53%, std_dev ~24%
    /// - Senior Unsecured:     ~36%, std_dev ~23%
    /// - Subordinated:         ~27%, std_dev ~20%
    /// - Junior Subordinated:  ~15%, std_dev ~13%
    pub fn sp_historical() -> Result<Self> {
        Self::from_registry_id("sp")
    }

    /// Look up the BetaRecovery for a given seniority class.
    pub fn get(&self, class: SeniorityClass) -> Option<&BetaRecovery> {
        self.classes
            .iter()
            .find(|(c, _)| *c == class)
            .map(|(_, b)| b)
    }
}

/// Recovery model driven by seniority-class Beta distributions.
///
/// Wraps a `BetaRecovery` to provide seniority-aware recovery estimation.
/// Can be used directly or plugged into portfolio default simulation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SeniorityRecovery {
    class: SeniorityClass,
    dist: BetaRecovery,
}

impl SeniorityRecovery {
    /// Create from a seniority class and calibration.
    ///
    /// # Errors
    ///
    /// Returns an error if `class` is not found in `calibration`.
    pub fn from_calibration(
        class: SeniorityClass,
        calibration: &SeniorityCalibration,
    ) -> Result<Self> {
        let dist = calibration.get(class).ok_or(InputError::Invalid)?;
        Ok(Self { class, dist: *dist })
    }

    /// Create directly from a BetaRecovery.
    pub fn new(class: SeniorityClass, dist: BetaRecovery) -> Self {
        Self { class, dist }
    }

    /// The seniority class.
    pub fn seniority(&self) -> SeniorityClass {
        self.class
    }

    /// The underlying Beta distribution.
    pub fn distribution(&self) -> &BetaRecovery {
        &self.dist
    }

    /// Expected (mean) recovery rate.
    pub fn expected_recovery(&self) -> f64 {
        self.dist.mean()
    }

    /// Recovery volatility (standard deviation).
    pub fn recovery_volatility(&self) -> f64 {
        self.dist.std_dev()
    }

    /// Conditional recovery given a standard-normal market factor Z.
    ///
    /// Maps the market factor through the Beta quantile function:
    /// R(Z) = Beta_inv(Phi(Z))
    ///
    /// This creates systematic variation while preserving the marginal
    /// Beta distribution.
    ///
    /// # Errors
    ///
    /// Propagates quantile errors (see [`BetaRecovery::quantile`]).
    pub fn conditional_recovery(&self, market_factor: f64) -> Result<f64> {
        let p = norm_cdf(market_factor).clamp(1e-10, 1.0 - 1e-10);
        self.dist.quantile(p)
    }

    /// Mean LGD = 1 - expected recovery.
    pub fn lgd(&self) -> f64 {
        1.0 - self.dist.mean()
    }

    /// Model name string.
    pub fn model_name(&self) -> &'static str {
        "Seniority-Based Beta Recovery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::random::Pcg64Rng;
    use finstack_quant_core::math::RandomNumberGenerator;
    use std::str::FromStr;

    #[test]
    fn beta_recovery_mean_matches_input() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        assert!((br.mean() - 0.45).abs() < 1e-12);
        assert!((br.std_dev() - 0.20).abs() < 1e-12);
    }

    #[test]
    fn seniority_class_from_str_rejects_noncanonical_spellings() {
        for rejected in [
            "first-lien secured",
            "first_lien_secured",
            "2nd lien secured",
            "sub",
            "SeniorSecured",
        ] {
            let error = SeniorityClass::from_str(rejected)
                .expect_err("noncanonical seniority should be rejected");
            assert!(error.to_string().contains("unknown seniority class"));
        }
    }

    #[test]
    fn beta_recovery_alpha_beta_positive() {
        let br = BetaRecovery::new(0.52, 0.25).expect("valid params");
        assert!(br.alpha() > 0.0);
        assert!(br.beta_param() > 0.0);
    }

    #[test]
    fn beta_recovery_shape_parameter_derivation() {
        let mean = 0.40;
        let std_dev = 0.20;
        let br = BetaRecovery::new(mean, std_dev).expect("valid params");

        // Verify: mean = alpha / (alpha + beta)
        let computed_mean = br.alpha() / (br.alpha() + br.beta_param());
        assert!(
            (computed_mean - mean).abs() < 1e-12,
            "alpha/(alpha+beta) = {}, expected {}",
            computed_mean,
            mean
        );

        // Verify: variance = alpha*beta / ((alpha+beta)^2 * (alpha+beta+1))
        let ab = br.alpha() + br.beta_param();
        let computed_var = (br.alpha() * br.beta_param()) / (ab * ab * (ab + 1.0));
        assert!(
            (computed_var - std_dev * std_dev).abs() < 1e-12,
            "computed var = {}, expected {}",
            computed_var,
            std_dev * std_dev
        );
    }

    #[test]
    fn beta_recovery_validation_rejects_invalid() {
        // mean out of range
        assert!(BetaRecovery::new(0.0, 0.1).is_err());
        assert!(BetaRecovery::new(1.0, 0.1).is_err());
        assert!(BetaRecovery::new(-0.1, 0.1).is_err());
        assert!(BetaRecovery::new(1.1, 0.1).is_err());

        // std_dev non-positive
        assert!(BetaRecovery::new(0.5, 0.0).is_err());
        assert!(BetaRecovery::new(0.5, -0.1).is_err());

        // variance >= mean*(1-mean)
        assert!(BetaRecovery::new(0.5, 0.5).is_err()); // variance = 0.25 = mean*(1-mean)
        assert!(BetaRecovery::new(0.5, 0.6).is_err());
    }

    #[test]
    fn beta_recovery_sampling_in_range() {
        let br = BetaRecovery::new(0.52, 0.25).expect("valid params");
        let mut rng = Pcg64Rng::new(42);

        for _ in 0..1000 {
            let s = br
                .sample(&mut rng as &mut dyn RandomNumberGenerator)
                .expect("sampling should succeed");
            assert!((0.0..=1.0).contains(&s), "sample {} out of [0,1]", s);
        }
    }

    #[test]
    fn beta_recovery_sampling_converges_to_mean() {
        let mean = 0.40;
        let br = BetaRecovery::new(mean, 0.20).expect("valid params");
        let n = 50_000;
        let buf = br.sample_seeded(n, 12345).expect("sampling should succeed");

        let sample_mean = buf.iter().sum::<f64>() / n as f64;
        assert!(
            (sample_mean - mean).abs() < 0.01,
            "sample mean {} vs expected {}",
            sample_mean,
            mean
        );
    }

    #[test]
    fn beta_recovery_quantile_roundtrip() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        // quantile(0.5) should be near the median
        let median = br.quantile(0.5).expect("quantile");
        assert!(median > 0.0 && median < 1.0);

        // quantile ordering
        let q10 = br.quantile(0.10).expect("quantile");
        let q50 = br.quantile(0.50).expect("quantile");
        let q90 = br.quantile(0.90).expect("quantile");
        assert!(q10 < q50);
        assert!(q50 < q90);
    }

    #[test]
    fn beta_recovery_quantile_rejects_invalid_probability() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        for invalid in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(br.quantile(invalid).is_err());
        }
    }

    #[test]
    fn beta_recovery_deserialization_rejects_inconsistent_shapes() {
        let recovery = BetaRecovery::new(0.45, 0.20).expect("valid params");
        let json = serde_json::to_value(recovery).expect("serialize");
        let restored: BetaRecovery =
            serde_json::from_value(json.clone()).expect("valid round trip");
        assert_eq!(restored.alpha().to_bits(), recovery.alpha().to_bits());
        assert_eq!(
            restored.beta_param().to_bits(),
            recovery.beta_param().to_bits()
        );

        let mut json = json;
        json["alpha"] = serde_json::json!(-1.0);
        assert!(serde_json::from_value::<BetaRecovery>(json).is_err());
    }

    #[test]
    fn beta_recovery_mean_lgd() {
        let br = BetaRecovery::new(0.40, 0.20).expect("valid params");
        assert!((br.mean_lgd() - 0.60).abs() < 1e-12);
    }

    #[test]
    fn seniority_calibration_moodys_constructs() {
        let cal = SeniorityCalibration::moodys_historical().expect("valid calibration");
        assert_eq!(cal.classes.len(), 6);
        assert!(cal.get(SeniorityClass::FirstLienSecured).is_some());
        assert!(cal.get(SeniorityClass::SecondLienSecured).is_some());
        assert!(cal.get(SeniorityClass::SeniorSecured).is_some());
        assert!(cal.get(SeniorityClass::JuniorSubordinated).is_some());
    }

    #[test]
    fn seniority_calibration_sp_constructs() {
        let cal = SeniorityCalibration::sp_historical().expect("valid calibration");
        assert_eq!(cal.classes.len(), 4);
    }

    #[test]
    fn seniority_recovery_ordering() {
        let cal = SeniorityCalibration::moodys_historical().expect("valid calibration");

        let secured = cal
            .get(SeniorityClass::SeniorSecured)
            .expect("found")
            .mean();
        let unsecured = cal
            .get(SeniorityClass::SeniorUnsecured)
            .expect("found")
            .mean();
        let sub = cal.get(SeniorityClass::Subordinated).expect("found").mean();
        let junior = cal
            .get(SeniorityClass::JuniorSubordinated)
            .expect("found")
            .mean();

        assert!(
            secured > unsecured,
            "secured {} > unsecured {}",
            secured,
            unsecured
        );
        assert!(
            unsecured > sub,
            "unsecured {} > subordinated {}",
            unsecured,
            sub
        );
        assert!(sub > junior, "subordinated {} > junior {}", sub, junior);
    }

    #[test]
    fn seniority_recovery_from_calibration() {
        let cal = SeniorityCalibration::moodys_historical().expect("valid calibration");
        let sr = SeniorityRecovery::from_calibration(SeniorityClass::SeniorSecured, &cal)
            .expect("found");
        assert_eq!(sr.seniority(), SeniorityClass::SeniorSecured);
        assert!((sr.expected_recovery() - 0.52).abs() < 1e-12);
    }

    #[test]
    fn seniority_recovery_conditional_varies() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        let sr = SeniorityRecovery::new(SeniorityClass::SeniorUnsecured, br);

        // Stress scenario (negative market factor) should give lower recovery
        let r_stress = sr.conditional_recovery(-3.0).expect("conditional");
        let r_base = sr.conditional_recovery(0.0).expect("conditional");
        let r_boom = sr.conditional_recovery(3.0).expect("conditional");

        assert!(
            r_stress < r_base,
            "stress {} should be < base {}",
            r_stress,
            r_base
        );
        assert!(
            r_base < r_boom,
            "base {} should be < boom {}",
            r_base,
            r_boom
        );
    }

    #[test]
    fn seniority_recovery_conditional_at_zero_near_median() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        let sr = SeniorityRecovery::new(SeniorityClass::SeniorUnsecured, br);

        // At market_factor = 0, Phi(0) = 0.5, so conditional = Beta median
        let r0 = sr.conditional_recovery(0.0).expect("conditional");
        let median = br.quantile(0.5).expect("quantile");
        assert!(
            (r0 - median).abs() < 1e-6,
            "conditional at 0 = {}, median = {}",
            r0,
            median
        );
    }

    #[test]
    fn seniority_recovery_serialization_roundtrip() {
        let br = BetaRecovery::new(0.45, 0.20).expect("valid params");
        let sr = SeniorityRecovery::new(SeniorityClass::SeniorUnsecured, br);

        let json = serde_json::to_string(&sr).expect("serialize");
        let sr2: SeniorityRecovery = serde_json::from_str(&json).expect("deserialize");

        assert!((sr2.expected_recovery() - sr.expected_recovery()).abs() < 1e-12);
        assert_eq!(sr2.seniority(), sr.seniority());
    }
}
