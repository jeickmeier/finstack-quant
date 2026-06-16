//! Volatility surface bumping for vega risk.
//!
//! Provides [`VolBumpRequest`] and [`bump_vol_surface`] for applying additive
//! volatility shifts to a [`VolSurface`]. This mirrors the rates
//! [`BumpRequest`](super::BumpRequest) infrastructure but targets implied
//! volatility grids instead of rate curves.
//!
//! # Bump Types
//!
//! - **Parallel**: Uniform absolute shift across all expiries and strikes.
//! - **ByExpiry**: Per-expiry shifts applied to the closest matching expiry row.
//! - **ByExpiryStrike**: Per-point shifts applied to the closest matching
//!   (expiry, strike) grid cell.
//!
//! All shifts are **additive** in volatility space (e.g., +0.01 = +1 vol point).
//! Resulting vols are floored at zero.

use finstack_quant_core::market_data::surfaces::VolSurface;

/// Request for a volatility surface bump (vega risk).
#[derive(Debug, Clone, PartialEq)]
pub enum VolBumpRequest {
    /// Flat absolute vol shift across all strikes and expiries.
    ///
    /// # Example
    /// ```rust
    /// # use finstack_quant_valuations::calibration::bumps::VolBumpRequest;
    /// let vega_1pt = VolBumpRequest::Parallel(0.01); // +1 vol point
    /// ```
    Parallel(f64),
    /// Per-expiry vol shifts: `(expiry_years, vol_shift)`.
    ///
    /// Each shift is applied to the closest matching expiry row on the grid.
    ByExpiry(Vec<(f64, f64)>),
    /// Per-expiry-strike vol shifts: `(expiry, strike, vol_shift)`.
    ///
    /// Each shift is applied to the closest matching `(expiry, strike)` cell.
    ByExpiryStrike(Vec<(f64, f64, f64)>),
}

/// Apply an additive vol bump to a [`VolSurface`], returning a new surface.
///
/// The function reads every grid point from the original surface, applies the
/// requested shift(s), floors each vol at zero, and rebuilds a new surface via
/// [`VolSurface::from_grid`].
///
/// # Errors
///
/// Returns an error if the rebuilt surface fails validation (should not happen
/// for well-formed inputs, since vols are floored at zero).
pub fn bump_vol_surface(
    surface: &VolSurface,
    request: &VolBumpRequest,
) -> finstack_quant_core::Result<VolSurface> {
    let expiries = surface.expiries();
    let strikes = surface.strikes();
    let n_expiries = expiries.len();
    let n_strikes = strikes.len();

    // Read the entire vol grid from the surface by querying each grid point.
    // VolSurface stores vols in row-major order but the field is private,
    // so we reconstruct by evaluating at exact grid coordinates.
    let mut vols: Vec<f64> = Vec::with_capacity(n_expiries * n_strikes);
    for &exp in expiries {
        for &strike in strikes {
            let v = surface.value_checked(exp, strike).map_err(|e| {
                finstack_quant_core::error::InputError::UnsupportedBump {
                    reason: format!("Failed to read vol at ({exp}, {strike}) during bump: {e}"),
                }
            })?;
            vols.push(v);
        }
    }

    // Apply bumps
    match request {
        VolBumpRequest::Parallel(shift) => {
            for v in &mut vols {
                *v = (*v + shift).max(0.0);
            }
        }
        VolBumpRequest::ByExpiry(expiry_shifts) => {
            for &(target_exp, shift) in expiry_shifts {
                let exp_idx = closest_index(expiries, target_exp);
                let row_start = exp_idx * n_strikes;
                for si in 0..n_strikes {
                    vols[row_start + si] = (vols[row_start + si] + shift).max(0.0);
                }
            }
        }
        VolBumpRequest::ByExpiryStrike(point_shifts) => {
            for &(target_exp, target_strike, shift) in point_shifts {
                let exp_idx = closest_index(expiries, target_exp);
                let strike_idx = closest_index(strikes, target_strike);
                let idx = exp_idx * n_strikes + strike_idx;
                vols[idx] = (vols[idx] + shift).max(0.0);
            }
        }
    }

    VolSurface::from_grid(surface.id().as_str(), expiries, strikes, &vols)
}

/// Find the index of the element in `arr` closest to `target`.
fn closest_index(arr: &[f64], target: f64) -> usize {
    arr.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (*a - target).abs();
            let db = (*b - target).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple 2x3 surface for testing:
    ///
    ///           Strike=90  Strike=100  Strike=110
    /// Exp=1.0     0.20       0.21        0.22
    /// Exp=2.0     0.19       0.20        0.21
    fn test_surface() -> VolSurface {
        VolSurface::builder("TEST-VOL")
            .expiries(&[1.0, 2.0])
            .strikes(&[90.0, 100.0, 110.0])
            .row(&[0.20, 0.21, 0.22])
            .row(&[0.19, 0.20, 0.21])
            .build()
            .expect("test surface should build")
    }

    #[test]
    fn parallel_bump_shifts_all_vols() {
        let surface = test_surface();
        let bumped =
            bump_vol_surface(&surface, &VolBumpRequest::Parallel(0.01)).expect("bump should work");

        // Every vol should be shifted by +0.01
        for &exp in surface.expiries() {
            for &strike in surface.strikes() {
                let orig = surface.value_checked(exp, strike).expect("original lookup");
                let new = bumped.value_checked(exp, strike).expect("bumped lookup");
                assert!(
                    (new - (orig + 0.01)).abs() < 1e-12,
                    "at ({exp}, {strike}): expected {}, got {new}",
                    orig + 0.01
                );
            }
        }
    }

    #[test]
    fn parallel_bump_negative_floors_at_zero() {
        let surface = test_surface();
        // Shift down by -0.25, which exceeds some vols (max is 0.22)
        let bumped =
            bump_vol_surface(&surface, &VolBumpRequest::Parallel(-0.25)).expect("bump should work");

        for &exp in surface.expiries() {
            for &strike in surface.strikes() {
                let v = bumped.value_checked(exp, strike).expect("bumped lookup");
                assert!(v >= 0.0, "vol at ({exp}, {strike}) should be >= 0, got {v}");
            }
        }
    }

    #[test]
    fn by_expiry_bumps_only_target_row() {
        let surface = test_surface();
        // Bump only the 1Y expiry row by +0.05
        let request = VolBumpRequest::ByExpiry(vec![(1.0, 0.05)]);
        let bumped = bump_vol_surface(&surface, &request).expect("bump should work");

        // 1Y row should be shifted
        for &strike in surface.strikes() {
            let orig = surface.value_checked(1.0, strike).expect("original lookup");
            let new = bumped.value_checked(1.0, strike).expect("bumped lookup");
            assert!(
                (new - (orig + 0.05)).abs() < 1e-12,
                "1Y row at strike {strike}: expected {}, got {new}",
                orig + 0.05
            );
        }

        // 2Y row should be unchanged
        for &strike in surface.strikes() {
            let orig = surface.value_checked(2.0, strike).expect("original lookup");
            let new = bumped.value_checked(2.0, strike).expect("bumped lookup");
            assert!(
                (new - orig).abs() < 1e-12,
                "2Y row at strike {strike}: expected {orig}, got {new}"
            );
        }
    }

    #[test]
    fn by_expiry_snaps_to_nearest() {
        let surface = test_surface();
        // Target 1.3Y should snap to the 1.0Y row (closer than 2.0Y)
        let request = VolBumpRequest::ByExpiry(vec![(1.3, 0.05)]);
        let bumped = bump_vol_surface(&surface, &request).expect("bump should work");

        let orig_1y = surface.value_checked(1.0, 100.0).expect("orig 1Y");
        let new_1y = bumped.value_checked(1.0, 100.0).expect("bumped 1Y");
        assert!(
            (new_1y - (orig_1y + 0.05)).abs() < 1e-12,
            "1Y ATM should be bumped"
        );

        let orig_2y = surface.value_checked(2.0, 100.0).expect("orig 2Y");
        let new_2y = bumped.value_checked(2.0, 100.0).expect("bumped 2Y");
        assert!(
            (new_2y - orig_2y).abs() < 1e-12,
            "2Y ATM should be unchanged"
        );
    }

    #[test]
    fn by_expiry_strike_bumps_single_cell() {
        let surface = test_surface();
        // Bump only (2.0, 100.0) by +0.10
        let request = VolBumpRequest::ByExpiryStrike(vec![(2.0, 100.0, 0.10)]);
        let bumped = bump_vol_surface(&surface, &request).expect("bump should work");

        // Target cell should be bumped
        let orig = surface.value_checked(2.0, 100.0).expect("orig");
        let new = bumped.value_checked(2.0, 100.0).expect("bumped");
        assert!(
            (new - (orig + 0.10)).abs() < 1e-12,
            "target cell: expected {}, got {new}",
            orig + 0.10
        );

        // All other cells should be unchanged
        for &exp in surface.expiries() {
            for &strike in surface.strikes() {
                if (exp - 2.0).abs() < 1e-12 && (strike - 100.0).abs() < 1e-12 {
                    continue; // skip the bumped cell
                }
                let o = surface.value_checked(exp, strike).expect("orig");
                let n = bumped.value_checked(exp, strike).expect("bumped");
                assert!(
                    (n - o).abs() < 1e-12,
                    "cell ({exp}, {strike}): expected {o}, got {n}"
                );
            }
        }
    }

    #[test]
    fn zero_bump_is_identity() {
        let surface = test_surface();
        let bumped =
            bump_vol_surface(&surface, &VolBumpRequest::Parallel(0.0)).expect("bump should work");

        for &exp in surface.expiries() {
            for &strike in surface.strikes() {
                let orig = surface.value_checked(exp, strike).expect("orig");
                let new = bumped.value_checked(exp, strike).expect("bumped");
                assert!(
                    (new - orig).abs() < 1e-12,
                    "zero bump should be identity at ({exp}, {strike})"
                );
            }
        }
    }

    #[test]
    fn surface_id_preserved() {
        let surface = test_surface();
        let bumped =
            bump_vol_surface(&surface, &VolBumpRequest::Parallel(0.01)).expect("bump should work");
        assert_eq!(
            surface.id().as_str(),
            bumped.id().as_str(),
            "bumped surface should preserve the original id"
        );
    }
}
