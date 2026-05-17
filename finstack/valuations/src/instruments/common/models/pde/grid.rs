//! One-dimensional spatial grids for finite difference methods.
//!
//! Provides uniform, non-uniform (sinh-concentrated), and user-supplied grids.
//! Grid concentration near strikes/barriers dramatically improves FD accuracy
//! without increasing the total number of nodes.

/// A sorted one-dimensional grid of spatial points.
///
/// Supports uniform spacing, sinh-based concentration near a point of interest
/// (strike, barrier), and user-supplied arbitrary grids. All constructors
/// guarantee monotonically increasing points.
#[derive(Debug, Clone)]
pub struct Grid1D {
    /// Sorted grid points (monotonically increasing).
    points: Vec<f64>,
}

impl Grid1D {
    /// Create a uniform grid with `n` points from `x_min` to `x_max` (inclusive).
    ///
    /// # Panics
    ///
    /// Returns an error if `n < 3` or `x_min >= x_max`.
    pub fn uniform(x_min: f64, x_max: f64, n: usize) -> Result<Self, PdeGridError> {
        if n < 3 {
            return Err(PdeGridError::TooFewPoints { n, min: 3 });
        }
        if x_min >= x_max {
            return Err(PdeGridError::InvalidBounds { x_min, x_max });
        }
        let dx = (x_max - x_min) / (n - 1) as f64;
        let points: Vec<f64> = (0..n).map(|i| x_min + i as f64 * dx).collect();
        Ok(Self { points })
    }

    /// Create a non-uniform grid concentrated near `center` using a sinh transformation.
    ///
    /// The grid maps a uniform parameter `xi` in `[0, 1]` through:
    /// ```text
    /// x(xi) = center + d * sinh(a_min + xi * (a_max - a_min))
    /// ```
    /// where `d` controls the local scale near `center`, and `a_min`, `a_max` are
    /// chosen so that `x(0) = x_min` and `x(1) = x_max` exactly. Since `sinh` is
    /// monotonically increasing, the mapping is guaranteed monotonic.
    ///
    /// # Arguments
    ///
    /// * `x_min`, `x_max` — domain bounds
    /// * `n` — number of grid points (must be >= 3)
    /// * `center` — concentration point (typically `ln(K)` for strike `K`)
    /// * `intensity` — local scale as fraction of domain width; smaller = tighter concentration (typical: 0.05–0.5)
    pub fn sinh_concentrated(
        x_min: f64,
        x_max: f64,
        n: usize,
        center: f64,
        intensity: f64,
    ) -> Result<Self, PdeGridError> {
        if n < 3 {
            return Err(PdeGridError::TooFewPoints { n, min: 3 });
        }
        if x_min >= x_max {
            return Err(PdeGridError::InvalidBounds { x_min, x_max });
        }
        if intensity <= 0.0 {
            return Err(PdeGridError::InvalidParameter {
                name: "intensity",
                value: intensity,
                reason: "must be positive",
            });
        }

        // d = intensity * domain_width controls local spacing near center.
        // Small d → tight concentration; large d → approaches uniform.
        let d = intensity * (x_max - x_min);

        // Inverse sinh at the endpoints:
        // a_min = asinh((x_min - center) / d)
        // a_max = asinh((x_max - center) / d)
        let a_min = ((x_min - center) / d).asinh();
        let a_max = ((x_max - center) / d).asinh();
        let a_range = a_max - a_min;

        if a_range.abs() < 1e-15 {
            // Degenerate: the sinh parameter range has collapsed, so the
            // mapping is effectively linear and the requested concentration
            // near `center` is lost. This happens when `intensity` is so
            // large that `d` dwarfs the domain width, or when `center` sits
            // far outside the domain. Silently returning a uniform grid would
            // hand back a grid with none of the requested strike resolution
            // and no signal — so this is reported as an error instead.
            return Err(PdeGridError::DegenerateConcentration {
                intensity,
                center,
                x_min,
                x_max,
            });
        }

        let mut points = Vec::with_capacity(n);
        for i in 0..n {
            let xi = i as f64 / (n - 1) as f64;
            let x = center + d * (a_min + xi * a_range).sinh();
            points.push(x);
        }

        // Exact endpoints (remove floating-point drift)
        points[0] = x_min;
        points[n - 1] = x_max;

        Ok(Self { points })
    }

    /// Create a grid from user-supplied points.
    ///
    /// Points must be sorted in strictly increasing order with at least 3 elements.
    pub fn from_points(points: Vec<f64>) -> Result<Self, PdeGridError> {
        if points.len() < 3 {
            return Err(PdeGridError::TooFewPoints {
                n: points.len(),
                min: 3,
            });
        }
        for i in 1..points.len() {
            if points[i] <= points[i - 1] {
                return Err(PdeGridError::NotMonotonic);
            }
        }
        Ok(Self { points })
    }

    /// Total number of grid points (including boundaries).
    #[inline]
    pub fn n(&self) -> usize {
        self.points.len()
    }

    /// Number of interior points (excluding boundaries).
    #[inline]
    pub fn n_interior(&self) -> usize {
        self.points.len() - 2
    }

    /// Reference to the grid points.
    #[inline]
    pub fn points(&self) -> &[f64] {
        &self.points
    }

    /// Grid spacing to the left of point `i`: `x[i] - x[i-1]`.
    ///
    /// Valid for `i` in `1..n`.
    #[inline]
    pub fn h_left(&self, i: usize) -> f64 {
        self.points[i] - self.points[i - 1]
    }

    /// Grid spacing to the right of point `i`: `x[i+1] - x[i]`.
    ///
    /// Valid for `i` in `0..n-1`.
    #[inline]
    pub fn h_right(&self, i: usize) -> f64 {
        self.points[i + 1] - self.points[i]
    }

    /// Lower boundary value `x[0]`.
    #[inline]
    pub fn x_min(&self) -> f64 {
        self.points[0]
    }

    /// Upper boundary value `x[n-1]`.
    #[inline]
    pub fn x_max(&self) -> f64 {
        self.points[self.points.len() - 1]
    }

    /// Linearly interpolate a solution vector at an arbitrary point `x`.
    ///
    /// `values` must have the same length as the grid. Returns the boundary
    /// value for points outside the grid domain.
    pub fn interpolate(&self, values: &[f64], x: f64) -> f64 {
        debug_assert_eq!(values.len(), self.points.len());

        if x <= self.points[0] {
            return values[0];
        }
        if x >= self.points[self.points.len() - 1] {
            return values[values.len() - 1];
        }

        // Binary search for the interval
        let idx = match self.points.binary_search_by(|p| p.total_cmp(&x)) {
            Ok(i) => return values[i], // exact match
            Err(i) => i - 1,           // x is between points[i-1] and points[i]
        };

        let x0 = self.points[idx];
        let x1 = self.points[idx + 1];
        let t = (x - x0) / (x1 - x0);
        values[idx] * (1.0 - t) + values[idx + 1] * t
    }
}

/// Find the interval index containing `x`: the largest `i` where `pts[i] <= x`.
///
/// Clamps to `0` for values below and `pts.len() - 2` for values above.
/// Uses [`f64::total_cmp`] for NaN-safety.
pub(crate) fn find_interval(pts: &[f64], x: f64) -> usize {
    match pts.binary_search_by(|p| p.total_cmp(&x)) {
        Ok(i) => i.min(pts.len().saturating_sub(2)),
        Err(i) => {
            if i == 0 {
                0
            } else {
                (i - 1).min(pts.len().saturating_sub(2))
            }
        }
    }
}

/// Find the grid index nearest to `x` using binary search.
///
/// Returns the index `i` minimizing `|pts[i] - x|`. O(log n).
pub(crate) fn find_nearest(pts: &[f64], x: f64) -> usize {
    match pts.binary_search_by(|p| p.total_cmp(&x)) {
        Ok(i) => i,
        Err(i) => {
            if i == 0 {
                0
            } else if i >= pts.len() {
                pts.len() - 1
            } else if (pts[i] - x).abs() < (pts[i - 1] - x).abs() {
                i
            } else {
                i - 1
            }
        }
    }
}

/// Errors during grid construction.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PdeGridError {
    /// Grid requires more points.
    #[error("Grid needs at least {min} points, got {n}")]
    TooFewPoints {
        /// Number of points provided.
        n: usize,
        /// Minimum required.
        min: usize,
    },
    /// Invalid domain bounds.
    #[error("Invalid bounds: x_min={x_min} must be less than x_max={x_max}")]
    InvalidBounds {
        /// Lower bound.
        x_min: f64,
        /// Upper bound.
        x_max: f64,
    },
    /// Grid points are not monotonically increasing.
    #[error("Grid points must be strictly increasing")]
    NotMonotonic,
    /// Invalid construction parameter.
    #[error("Invalid parameter '{name}'={value}: {reason}")]
    InvalidParameter {
        /// Parameter name.
        name: &'static str,
        /// Parameter value.
        value: f64,
        /// Reason it is invalid.
        reason: &'static str,
    },
    /// A `sinh_concentrated` grid degenerated to (effectively) uniform.
    ///
    /// The sinh parameter range collapsed below numerical resolution, so the
    /// transformation is effectively linear and the requested concentration
    /// near `center` is entirely lost. This happens when `intensity` is so
    /// large that the local scale dwarfs the domain width, or when `center`
    /// lies far outside `[x_min, x_max]`. Use a smaller `intensity`, place
    /// `center` inside the domain, or call [`Grid1D::uniform`] explicitly.
    #[error(
        "sinh_concentrated grid degenerated to uniform: intensity={intensity} with \
         center={center} on domain [{x_min}, {x_max}] collapses the sinh range — \
         the requested strike concentration is lost; use a smaller intensity, a \
         center inside the domain, or Grid1D::uniform"
    )]
    DegenerateConcentration {
        /// The `intensity` argument that produced the degenerate grid.
        intensity: f64,
        /// The `center` argument.
        center: f64,
        /// Lower domain bound.
        x_min: f64,
        /// Upper domain bound.
        x_max: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_grid_basic() {
        let g = Grid1D::uniform(-1.0, 1.0, 5).expect("valid grid");
        assert_eq!(g.n(), 5);
        assert_eq!(g.n_interior(), 3);
        assert!((g.points()[0] - (-1.0)).abs() < 1e-15);
        assert!((g.points()[4] - 1.0).abs() < 1e-15);
        assert!((g.h_left(1) - 0.5).abs() < 1e-15);
    }

    #[test]
    fn uniform_grid_rejects_too_few() {
        assert!(Grid1D::uniform(0.0, 1.0, 2).is_err());
    }

    #[test]
    fn sinh_grid_concentrates_near_center() {
        let g = Grid1D::sinh_concentrated(-5.0, 5.0, 101, 0.0, 0.1).expect("valid grid");
        assert_eq!(g.n(), 101);
        // Check spacing is smaller near center than near boundaries
        let mid = g.n() / 2;
        let h_center = g.h_right(mid);
        let h_boundary = g.h_right(0);
        assert!(h_center < h_boundary, "center spacing should be smaller");
    }

    #[test]
    fn from_points_rejects_unsorted() {
        assert!(Grid1D::from_points(vec![1.0, 0.5, 2.0]).is_err());
    }

    /// [P6-7] `sinh_concentrated` must **error** when the sinh parameter range
    /// collapses, rather than silently returning a uniform grid that has none
    /// of the requested strike concentration.
    ///
    /// An enormous `intensity` makes the local scale `d` dwarf the domain
    /// width, collapsing `a_range` below `1e-15`. The old code silently
    /// returned `Grid1D::uniform` here; the fix reports
    /// [`PdeGridError::DegenerateConcentration`].
    #[test]
    fn sinh_concentrated_errors_on_degenerate_concentration() {
        // intensity = 1e16 on a unit domain with a centred concentration
        // point collapses the sinh range (a_range ≈ 1e-16 < 1e-15).
        let result = Grid1D::sinh_concentrated(0.0, 1.0, 51, 0.5, 1e16);
        match result {
            Err(PdeGridError::DegenerateConcentration {
                intensity,
                center,
                x_min,
                x_max,
            }) => {
                assert!(
                    (intensity - 1e16).abs() < 1.0,
                    "the error must cite the offending intensity"
                );
                assert!(
                    (center - 0.5).abs() < 1e-12
                        && x_min.abs() < 1e-12
                        && (x_max - 1.0).abs() < 1e-12,
                    "the error must cite the center and domain bounds"
                );
            }
            other => panic!("expected DegenerateConcentration error, got {other:?}"),
        }
    }

    /// [P6-7] A genuine, well-posed concentration request must still succeed
    /// and actually concentrate near the center — the new error path must not
    /// false-positive on normal `intensity` values.
    #[test]
    fn sinh_concentrated_still_succeeds_for_normal_intensity() {
        // intensity = 0.1 is a typical strike-concentration value.
        let g = Grid1D::sinh_concentrated(-5.0, 5.0, 101, 0.0, 0.1)
            .expect("a normal intensity must produce a valid concentrated grid");
        assert_eq!(g.n(), 101);
        // Concentration: spacing near the center is tighter than near the edge.
        let mid = g.n() / 2;
        assert!(
            g.h_right(mid) < g.h_right(0),
            "spacing near the concentration point must be smaller than at the edge"
        );
    }

    #[test]
    fn interpolation_exact_at_nodes() {
        let g = Grid1D::uniform(0.0, 1.0, 5).expect("valid grid");
        let values: Vec<f64> = g.points().iter().map(|x| x * x).collect();
        for (i, &x) in g.points().iter().enumerate() {
            let interp = g.interpolate(&values, x);
            assert!((interp - values[i]).abs() < 1e-14);
        }
    }
}
