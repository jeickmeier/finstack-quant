//! Volatility quoting conventions and validation helpers.
//!
//! This submodule defines the supported quoting regimes for rate and option
//! volatility inputs and the forward-rate validation rules required before
//! pricing or convention conversion.

use crate::error::InputError;
use crate::Result;

/// Volatility quoting convention.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VolatilityConvention {
    /// Normal (absolute) volatility in basis points
    Normal,
    /// Lognormal (Black) volatility as percentage
    Lognormal,
    /// Shifted lognormal for negative rates
    ShiftedLognormal {
        /// Shift amount for negative rate handling
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
                Err(InputError::NonPositiveForwardForLognormal {
                    forward: forward_rate,
                    required_shift: (-forward_rate).max(0.0) + 1e-4,
                }
                .into())
            } else {
                Ok(())
            }
        }
        VolatilityConvention::ShiftedLognormal { shift } => {
            let shifted = forward_rate + shift;
            if shifted <= 0.0 {
                Err(InputError::NonPositiveShiftedForward {
                    forward: forward_rate,
                    shift,
                    shifted,
                }
                .into())
            } else {
                Ok(())
            }
        }
    }
}
