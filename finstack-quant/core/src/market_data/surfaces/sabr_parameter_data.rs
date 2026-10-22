//! Serializable SABR parameter nodes stored by volatility cubes.

/// Data-only SABR parameters for one volatility-cube node.
///
/// This type records calibrated market data. SABR evaluation, interpolation,
/// calibration, and convention conversion are owned by
/// `finstack-quant-models`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawSabrParameterData")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct SabrParameterData {
    /// Initial volatility level, strictly positive.
    alpha: f64,
    /// CEV exponent in the closed interval `[0, 1]`.
    beta: f64,
    /// Forward-volatility correlation in the open interval `(-1, 1)`.
    rho: f64,
    /// Volatility of volatility, nonnegative; zero is the deterministic-volatility limit.
    nu: f64,
    /// Optional finite displacement applied to forward and strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shift: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
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
        Self::new(raw.alpha, raw.beta, raw.nu, raw.rho, raw.shift)
    }
}

impl SabrParameterData {
    /// Construct a validated data node with an optional displacement.
    ///
    /// # Arguments
    ///
    /// * `alpha` - Strictly positive initial SABR volatility level.
    /// * `beta` - CEV exponent in the closed interval `[0, 1]`.
    /// * `nu` - Nonnegative volatility-of-volatility parameter; zero gives deterministic volatility.
    /// * `rho` - Forward-volatility correlation in the open interval `(-1, 1)`.
    /// * `shift` - Optional finite displacement in the same units as the
    ///   associated forwards and strikes; `None` records an unshifted node.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when any value is non-finite or
    /// outside its documented range.
    pub fn new(
        alpha: f64,
        beta: f64,
        nu: f64,
        rho: f64,
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

    /// Initial volatility level, strictly positive.
    #[inline]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// CEV exponent in the closed interval `[0, 1]`.
    #[inline]
    pub const fn beta(&self) -> f64 {
        self.beta
    }

    /// Forward-volatility correlation in the open interval `(-1, 1)`.
    #[inline]
    pub const fn rho(&self) -> f64 {
        self.rho
    }

    /// Volatility of volatility, nonnegative; zero is the deterministic-volatility limit.
    #[inline]
    pub const fn nu(&self) -> f64 {
        self.nu
    }

    /// Optional finite displacement applied to forward and strike.
    #[inline]
    pub const fn shift(&self) -> Option<f64> {
        self.shift
    }
}
