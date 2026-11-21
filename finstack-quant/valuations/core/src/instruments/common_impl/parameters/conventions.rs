//! Market conventions shared across multi-family instrument types.

use serde::{Deserialize, Serialize};

/// Principal exchange schedule options for cross-currency swaps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NotionalExchange {
    /// No principal exchange.
    None,
    /// Exchange principal at maturity only.
    Final,
    /// Exchange principal at start and maturity (typical for fixed-notional XCCY basis swaps).
    #[default]
    InitialAndFinal,
    /// Mark-to-market resetting. The notional of `resetting_side` is re-marked at each
    /// of its coupon reset dates to match the constant leg's notional in current FX.
    /// A rebalancing cashflow is paid on the resetting leg only — the constant leg's
    /// principal-and-coupon schedule is unchanged, matching standard MtM-XCCY market
    /// convention (QuantLib's `MtMCrossCurrencyBasisSwap` follows the same pattern).
    /// Under CIP no-FX-vol the constant-currency leg of the FX swap that funds the
    /// rebalancing is PV-fair from today's perspective, so the resetting-leg flow is
    /// the only cashflow that needs to be emitted explicitly. Implies initial AND
    /// final principal exchange.
    MtmResetting {
        /// Which leg (`Leg1` or `Leg2`) has its notional reset each period.
        resetting_side: ResettingSide,
    },
}

impl std::fmt::Display for NotionalExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Final => write!(f, "final"),
            Self::InitialAndFinal => write!(f, "initial_and_final"),
            Self::MtmResetting { resetting_side } => {
                write!(f, "mtm_resetting:{resetting_side}")
            }
        }
    }
}

/// Identifies which leg of an XCCY swap has its notional reset under
/// MtM-resetting. `Leg1` and `Leg2` refer to `XccySwap::leg1` and `XccySwap::leg2`
/// respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResettingSide {
    /// The first leg (`XccySwap::leg1`) has its notional reset each period.
    Leg1,
    /// The second leg (`XccySwap::leg2`) has its notional reset each period.
    Leg2,
}

impl std::fmt::Display for ResettingSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leg1 => write!(f, "leg1"),
            Self::Leg2 => write!(f, "leg2"),
        }
    }
}

impl std::str::FromStr for ResettingSide {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "leg1" => Ok(Self::Leg1),
            "leg2" => Ok(Self::Leg2),
            _ => Err(format!("Unknown resetting side: '{s}'. Valid: leg1, leg2")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notional_exchange_display_and_serde_roundtrip() {
        let cases = [
            (NotionalExchange::None, "none", "\"none\""),
            (NotionalExchange::Final, "final", "\"final\""),
            (
                NotionalExchange::InitialAndFinal,
                "initial_and_final",
                "\"initial_and_final\"",
            ),
            (
                NotionalExchange::MtmResetting {
                    resetting_side: ResettingSide::Leg1,
                },
                "mtm_resetting:leg1",
                r#"{"mtm_resetting":{"resetting_side":"leg1"}}"#,
            ),
        ];

        for (value, display, json) in cases {
            assert_eq!(value.to_string(), display);
            let encoded = serde_json::to_string(&value).expect("serialize");
            assert_eq!(encoded, json);
            let decoded: NotionalExchange = serde_json::from_str(&encoded).expect("deserialize");
            assert_eq!(decoded, value);
        }
        assert_eq!(
            NotionalExchange::default(),
            NotionalExchange::InitialAndFinal
        );
    }

    #[test]
    fn resetting_side_display_and_from_str() {
        assert_eq!(ResettingSide::Leg1.to_string(), "leg1");
        assert_eq!(ResettingSide::Leg2.to_string(), "leg2");
        assert_eq!("leg1".parse::<ResettingSide>(), Ok(ResettingSide::Leg1));
        assert_eq!("leg2".parse::<ResettingSide>(), Ok(ResettingSide::Leg2));
        assert!("leg3".parse::<ResettingSide>().is_err());
    }
}
