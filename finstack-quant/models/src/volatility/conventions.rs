//! Volatility quoting conventions and validation helpers.
//!
//! This submodule defines the supported quoting regimes for rate and option
//! volatility inputs and the forward-rate validation rules required before
//! pricing or convention conversion.

use finstack_quant_core::error::InputError;
use finstack_quant_core::Result;

/// Volatility quoting convention.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum VolatilityConvention {
    /// Normal absolute volatility in decimal rate units per square-root year
    Normal,
    /// Lognormal (Black) relative volatility as a decimal per square-root year
    Lognormal,
    /// Shifted lognormal for negative rates
    ShiftedLognormal {
        /// Additive displacement in the same decimal rate or price units as forward and strike
        shift: f64,
    },
}

/// Validate that forward rate is valid for the given convention.
pub(super) fn validate_forward_for_convention(
    forward_rate: f64,
    convention: VolatilityConvention,
) -> Result<()> {
    match convention {
        VolatilityConvention::Normal => Ok(()),
        VolatilityConvention::Lognormal => {
            if forward_rate <= 0.0 {
                return Err(InputError::NonPositiveForwardForLognormal {
                    forward: forward_rate,
                    required_shift: (-forward_rate).max(0.0) + 1e-4,
                }
                .into());
            }
            Ok(())
        }
        VolatilityConvention::ShiftedLognormal { shift } => {
            let shifted = forward_rate + shift;
            if shifted <= 0.0 {
                return Err(InputError::NonPositiveShiftedForward {
                    forward: forward_rate,
                    shift,
                    shifted,
                }
                .into());
            }
            Ok(())
        }
    }
}
