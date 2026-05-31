//! Shared curve binding helpers.

use finstack_core::dates::DayCount;
use finstack_core::market_data::surfaces::{VolInterpolationMode, VolSurfaceAxis};
use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use pyo3::prelude::*;

// Helpers
// ---------------------------------------------------------------------------

/// Parse a DayCount from a Python string like `"act_365f"`, `"act_360"`, etc.
pub(super) fn parse_day_count(s: &str) -> PyResult<DayCount> {
    s.parse::<DayCount>()
        .map_err(|e| crate::errors::value_error(format!("Invalid day_count {s:?}: {e}")))
}

/// Parse an [`InterpStyle`] from a Python string.
pub(super) fn parse_interp_style(s: &str) -> PyResult<InterpStyle> {
    s.parse::<InterpStyle>()
        .map_err(|e| crate::errors::value_error(format!("Invalid interp style {s:?}: {e}")))
}

/// Parse an [`ExtrapolationPolicy`] from a Python string.
pub(super) fn parse_extrapolation(s: &str) -> PyResult<ExtrapolationPolicy> {
    s.parse::<ExtrapolationPolicy>()
        .map_err(|e| crate::errors::value_error(format!("Invalid extrapolation {s:?}: {e}")))
}

/// Parse a [`VolSurfaceAxis`] from a Python string.
pub(super) fn parse_vol_surface_axis(s: &str) -> PyResult<VolSurfaceAxis> {
    match s {
        "strike" => Ok(VolSurfaceAxis::Strike),
        "tenor" => Ok(VolSurfaceAxis::Tenor),
        _ => Err(crate::errors::value_error(format!(
            "Invalid vol surface axis {s:?}: expected 'strike' or 'tenor'",
        ))),
    }
}

/// Parse a [`VolInterpolationMode`] from a Python string.
pub(super) fn parse_vol_interpolation_mode(s: &str) -> PyResult<VolInterpolationMode> {
    match s {
        "vol" => Ok(VolInterpolationMode::Vol),
        "total_variance" => Ok(VolInterpolationMode::TotalVariance),
        _ => Err(crate::errors::value_error(format!(
            "Invalid vol interpolation mode {s:?}: expected 'vol' or 'total_variance'",
        ))),
    }
}
