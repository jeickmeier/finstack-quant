//! Serializable SABR parameter nodes stored by volatility cubes.

/// Data-only SABR parameters for one volatility-cube node.
///
/// This type records calibrated market data. SABR evaluation, interpolation,
/// calibration, and convention conversion are owned by
/// `finstack-quant-models`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawSabrParameterData")]
pub struct SabrParameterData {
    /// Initial volatility level, strictly positive.
    pub alpha: f64,
    /// CEV exponent in the closed interval `[0, 1]`.
    pub beta: f64,
    /// Forward-volatility correlation in the open interval `(-1, 1)`.
    pub rho: f64,
    /// Volatility of volatility, nonnegative; zero is the deterministic-volatility limit.
    pub nu: f64,
    /// Optional finite displacement applied to forward and strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawSabrParameterData {
    alpha: f64,
    beta: f64,
    rho: f64,
    nu: f64,
    #[serde(default)]
    shift: Option<f64>,
}

impl TryFrom<RawSabrParameterData> for SabrParameterData {
    type Error = crate::Error;

    fn try_from(raw: RawSabrParameterData) -> crate::Result<Self> {
        Self::new_with_shift(raw.alpha, raw.beta, raw.rho, raw.nu, raw.shift)
    }
}

impl SabrParameterData {
    /// Construct a validated data node without a displacement.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Strictly positive initial SABR volatility level.
    /// * `beta` - CEV exponent in the closed interval `[0, 1]`.
    /// * `rho` - Forward-volatility correlation in the open interval `(-1, 1)`.
    /// * `nu` - Nonnegative volatility-of-volatility parameter; zero gives deterministic volatility.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when any value is non-finite or
    /// outside its documented range.
    pub fn new(alpha: f64, beta: f64, rho: f64, nu: f64) -> crate::Result<Self> {
        Self::new_with_shift(alpha, beta, rho, nu, None)
    }

    /// Construct a validated data node with an optional displacement.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Strictly positive initial SABR volatility level.
    /// * `beta` - CEV exponent in the closed interval `[0, 1]`.
    /// * `rho` - Forward-volatility correlation in the open interval `(-1, 1)`.
    /// * `nu` - Nonnegative volatility-of-volatility parameter; zero gives deterministic volatility.
    /// * `shift` - Optional finite displacement in the same units as the
    ///   associated forwards and strikes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when any value is non-finite or
    /// outside its documented range.
    pub fn new_with_shift(
        alpha: f64,
        beta: f64,
        rho: f64,
        nu: f64,
        shift: Option<f64>,
    ) -> crate::Result<Self> {
        if alpha <= 0.0 || !alpha.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR alpha must be positive, got {alpha}"
            )));
        }
        if !(0.0..=1.0).contains(&beta) || !beta.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR beta must be in [0, 1], got {beta}"
            )));
        }
        if rho <= -1.0 || rho >= 1.0 || !rho.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR rho must be in (-1, 1), got {rho}"
            )));
        }
        if nu < 0.0 || !nu.is_finite() {
            return Err(crate::Error::Validation(format!(
                "SABR nu (vol-of-vol) must be nonnegative, got {nu}"
            )));
        }
        if shift.is_some_and(|value| !value.is_finite()) {
            return Err(crate::Error::Validation(format!(
                "SABR shift must be finite, got {}",
                shift.unwrap_or_default()
            )));
        }
        Ok(Self {
            alpha,
            beta,
            rho,
            nu,
            shift,
        })
    }

    /// Return a copy with an explicit finite displacement.
    ///
    /// # Arguments
    ///
    /// * `shift` - Finite displacement in the same units as associated
    ///   forwards and strikes.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when `shift` is not finite.
    pub fn with_shift(self, shift: f64) -> crate::Result<Self> {
        Self::new_with_shift(self.alpha, self.beta, self.rho, self.nu, Some(shift))
    }
}
