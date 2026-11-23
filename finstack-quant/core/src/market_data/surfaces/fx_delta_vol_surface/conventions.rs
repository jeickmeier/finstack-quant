//! FX delta, ATM, and butterfly market quoting conventions.

#[cfg(doc)]
use super::surface::FxDeltaVolSurface;

/// Default relative delta matching tolerance used by checked delta-to-strike inversion.
pub const DEFAULT_FX_DELTA_REL_TOL: f64 = 1e-3;

/// Delta convention governing how [`FxDeltaVolSurface`] maps wing deltas onto strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(rename = "FxSurfaceDeltaConvention")
)]
#[serde(rename_all = "snake_case")]
pub enum FxDeltaConvention {
    /// Premium-unadjusted forward delta (`Delta_call = N(d1)`).
    ForwardUnadjusted,
}

impl FxDeltaConvention {
    /// Return the canonical `snake_case` wire label for this delta convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForwardUnadjusted => "forward_unadjusted",
        }
    }
}

impl std::fmt::Display for FxDeltaConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxDeltaConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "forward_unadjusted" => Ok(Self::ForwardUnadjusted),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX delta convention '{s}'; expected 'forward_unadjusted'"
            ))),
        }
    }
}

/// At-the-money strike convention on [`FxDeltaVolSurface`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxAtmConvention {
    /// Forward delta-neutral straddle (`K_ATM = F * exp(0.5 * sigma_ATM^2 * T)`).
    DeltaNeutralStraddle,
}

impl FxAtmConvention {
    /// Return the canonical `snake_case` wire label for this ATM convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeltaNeutralStraddle => "delta_neutral_straddle",
        }
    }
}

impl std::fmt::Display for FxAtmConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxAtmConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "delta_neutral_straddle" => Ok(Self::DeltaNeutralStraddle),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX ATM convention '{s}'; expected 'delta_neutral_straddle'"
            ))),
        }
    }
}

/// Butterfly quote convention on [`FxDeltaVolSurface`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FxButterflyConvention {
    /// Arithmetic two-volatility smile butterfly (`0.5 * (sigma_call + sigma_put) - sigma_ATM`),
    /// distinct from a market-strangle premium-consistency solve.
    Smile,
}

impl FxButterflyConvention {
    /// Return the canonical `snake_case` wire label for this butterfly convention.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Smile => "smile",
        }
    }
}

impl std::fmt::Display for FxButterflyConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for FxButterflyConvention {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "smile" => Ok(Self::Smile),
            _ => Err(crate::Error::Validation(format!(
                "unsupported FX butterfly convention '{s}'; expected 'smile'"
            ))),
        }
    }
}
