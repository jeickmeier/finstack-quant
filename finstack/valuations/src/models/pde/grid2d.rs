//! Two-dimensional tensor-product grid for ADI finite difference methods.
//!
//! Combines two [`Grid1D`] axes (e.g., log-spot and variance) into a
//! structured 2D grid. Values are stored in row-major order: index
//! `(i, j)` maps to `i * n_y + j`, where `i` indexes the x-axis and
//! `j` indexes the y-axis.

use super::grid::find_interval;
use super::grid::Grid1D;

/// Tensor-product 2D grid built from two [`Grid1D`] axes.
///
/// The x-axis typically represents log-spot and the y-axis variance
/// (or log-variance). Grid operations index in row-major layout:
/// `flat_index = i * n_y + j`.
#[derive(Debug, Clone)]
pub struct Grid2D {
    /// Grid along the x-axis (e.g., log-spot).
    x: Grid1D,
    /// Grid along the y-axis (e.g., variance).
    y: Grid1D,
}

impl Grid2D {
    /// Create a tensor-product grid from two 1D grids.
    pub fn new(x: Grid1D, y: Grid1D) -> Self {
        Self { x, y }
    }

    /// Reference to the x-axis grid.
    #[inline]
    pub fn x(&self) -> &Grid1D {
        &self.x
    }

    /// Reference to the y-axis grid.
    #[inline]
    pub fn y(&self) -> &Grid1D {
        &self.y
    }

    /// Number of points along the x-axis.
    #[inline]
    pub fn nx(&self) -> usize {
        self.x.n()
    }

    /// Number of points along the y-axis.
    #[inline]
    pub fn ny(&self) -> usize {
        self.y.n()
    }

    /// Total number of grid points (`nx * ny`).
    #[inline]
    pub fn total(&self) -> usize {
        self.x.n() * self.y.n()
    }

    /// Number of interior x-points (excluding x-boundaries).
    #[inline]
    pub fn nx_interior(&self) -> usize {
        self.x.n_interior()
    }

    /// Number of interior y-points (excluding y-boundaries).
    #[inline]
    pub fn ny_interior(&self) -> usize {
        self.y.n_interior()
    }

    /// Convert 2D indices to flat row-major index.
    ///
    /// `i` indexes x (rows), `j` indexes y (columns).
    #[inline]
    pub fn flat_index(&self, i: usize, j: usize) -> usize {
        i * self.y.n() + j
    }

    /// Convert flat row-major index to `(i, j)`.
    #[inline]
    pub fn index_2d(&self, flat: usize) -> (usize, usize) {
        (flat / self.y.n(), flat % self.y.n())
    }

    /// Bilinear interpolation of a 2D solution at point `(x, y)`.
    ///
    /// `values` must have length `nx * ny` in row-major order.
    /// Falls back to boundary values for points outside the grid domain.
    pub fn interpolate(&self, values: &[f64], x: f64, y: f64) -> f64 {
        debug_assert_eq!(values.len(), self.total());
        let ny = self.ny();

        let x_pts = self.x.points();
        let y_pts = self.y.points();

        // Find x-interval
        let ix = find_interval(x_pts, x);
        let jy = find_interval(y_pts, y);

        // Clamp to valid ranges for interpolation
        let i0 = ix.min(x_pts.len().saturating_sub(2));
        let j0 = jy.min(y_pts.len().saturating_sub(2));
        let i1 = i0 + 1;
        let j1 = j0 + 1;

        let tx = if x_pts[i1] > x_pts[i0] {
            ((x - x_pts[i0]) / (x_pts[i1] - x_pts[i0])).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ty = if y_pts[j1] > y_pts[j0] {
            ((y - y_pts[j0]) / (y_pts[j1] - y_pts[j0])).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Bilinear: f(x,y) ≈ (1-tx)*(1-ty)*f00 + tx*(1-ty)*f10 + (1-tx)*ty*f01 + tx*ty*f11
        let f00 = values[i0 * ny + j0];
        let f10 = values[i1 * ny + j0];
        let f01 = values[i0 * ny + j1];
        let f11 = values[i1 * ny + j1];

        (1.0 - tx) * (1.0 - ty) * f00
            + tx * (1.0 - ty) * f10
            + (1.0 - tx) * ty * f01
            + tx * ty * f11
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid2d_basics() {
        let gx = Grid1D::uniform(0.0, 1.0, 5).expect("valid grid");
        let gy = Grid1D::uniform(0.0, 2.0, 3).expect("valid grid");
        let g = Grid2D::new(gx, gy);

        assert_eq!(g.nx(), 5);
        assert_eq!(g.ny(), 3);
        assert_eq!(g.total(), 15);
        assert_eq!(g.nx_interior(), 3);
        assert_eq!(g.ny_interior(), 1);
        assert_eq!(g.flat_index(2, 1), 7);
        assert_eq!(g.index_2d(7), (2, 1));
    }

    #[test]
    fn grid2d_bilinear_interpolation() {
        let gx = Grid1D::uniform(0.0, 1.0, 3).expect("valid grid");
        let gy = Grid1D::uniform(0.0, 1.0, 3).expect("valid grid");
        let g = Grid2D::new(gx, gy);

        // f(x, y) = x + y
        let values: Vec<f64> = (0..9)
            .map(|flat| {
                let (i, j) = g.index_2d(flat);
                i as f64 * 0.5 + j as f64 * 0.5
            })
            .collect();

        // At grid nodes
        let v = g.interpolate(&values, 0.0, 0.0);
        assert!((v - 0.0).abs() < 1e-12);

        let v = g.interpolate(&values, 1.0, 1.0);
        assert!((v - 2.0).abs() < 1e-12);

        // Midpoint
        let v = g.interpolate(&values, 0.5, 0.5);
        assert!((v - 1.0).abs() < 1e-12);
    }

    #[test]
    fn grid2d_interpolation_outside_domain() {
        let gx = Grid1D::uniform(0.0, 1.0, 3).expect("valid grid");
        let gy = Grid1D::uniform(0.0, 1.0, 3).expect("valid grid");
        let g = Grid2D::new(gx, gy);

        let values = vec![1.0; 9];

        // Outside domain clamps gracefully
        let v = g.interpolate(&values, -1.0, -1.0);
        assert!((v - 1.0).abs() < 1e-12);

        let v = g.interpolate(&values, 2.0, 2.0);
        assert!((v - 1.0).abs() < 1e-12);
    }
}
