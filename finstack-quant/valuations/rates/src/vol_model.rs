//! Volatility convention enum for interest-rate options.

/// Volatility convention (Black lognormal or Bachelier normal) used to price
/// a swaption. Wire values: `black`, `normal`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VolatilityModel {
    /// Black (lognormal).
    #[default]
    Black,
    /// Bachelier / normal model.
    Normal,
}

impl std::fmt::Display for VolatilityModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VolatilityModel::Black => write!(f, "black"),
            VolatilityModel::Normal => write!(f, "normal"),
        }
    }
}

impl std::str::FromStr for VolatilityModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "black" => Ok(Self::Black),
            "normal" => Ok(Self::Normal),
            _ => Err(format!(
                "Unknown volatility model: '{}'. Valid: black, normal",
                s
            )),
        }
    }
}

/// Finite-difference step for Hull-White short-rate σ sensitivities
/// (`MetricId::HwSigmaVega`): an **absolute** shift of 1e-4 (1bp of annual
/// short-rate volatility). Results are reported per 0.01 absolute σ.
pub(crate) const HW_SIGMA_BUMP: f64 = 1e-4;

/// Finite-difference Hull-White short-rate σ vega per 0.01 absolute σ.
///
/// Uses central differences when `use_central` is true (i.e. `σ > h`) and falls
/// back to a forward difference from `shift = 0.0` otherwise.
///
/// # Arguments
///
/// * `h` - Positive absolute short-rate volatility bump step in decimal annual units.
/// * `use_central` - Whether `σ > h` holds so `price(-h)` remains strictly positive.
/// * `price` - Callback pricing raw PV given an additive shift `Δσ` to the base HW σ.
pub(crate) fn hw_sigma_vega_fd<F>(
    h: f64,
    use_central: bool,
    mut price: F,
) -> finstack_quant_core::Result<f64>
where
    F: FnMut(f64) -> finstack_quant_core::Result<f64>,
{
    let pv_up = price(h)?;
    if use_central {
        let pv_down = price(-h)?;
        Ok((pv_up - pv_down) / (2.0 * h) * 0.01)
    } else {
        let pv_base = price(0.0)?;
        Ok((pv_up - pv_base) / h * 0.01)
    }
}
