//! Fast extraction helpers for dense portfolio risk matrices.
//!
//! Python lists retain the existing `Vec<Vec<f64>>` conversion path and its
//! canonical Rust validation. C-contiguous `float64` NumPy arrays are copied
//! directly from their backing buffer, avoiding per-element Python extraction.

use finstack_quant_portfolio::factor_model::{
    flatten_position_pnls as core_flatten_position_pnls,
    flatten_square_matrix as core_flatten_square_matrix,
};
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;

/// Storage order of an extracted position-by-scenario P&L matrix.
enum PositionPnlOrder {
    /// Already transposed into the scenario-major engine layout.
    ScenarioMajor,
    /// Position-major row order copied directly from a NumPy array.
    PositionMajor,
}

/// Owned position P&L matrix ready to cross a GIL-release boundary.
pub(crate) struct PositionPnlMatrix {
    data: Vec<f64>,
    n_scenarios: usize,
    order: PositionPnlOrder,
}

impl PositionPnlMatrix {
    /// Number of scenarios represented by each position row.
    pub(crate) fn n_scenarios(&self) -> usize {
        self.n_scenarios
    }

    /// Convert into the scenario-major layout expected by the Rust engine.
    ///
    /// NumPy inputs defer this O(`n_positions * n_scenarios`) transpose until
    /// after the caller releases the GIL.
    pub(crate) fn into_scenario_major(self, n_positions: usize) -> Vec<f64> {
        match self.order {
            PositionPnlOrder::ScenarioMajor => self.data,
            PositionPnlOrder::PositionMajor if n_positions <= 1 || self.n_scenarios <= 1 => {
                self.data
            }
            PositionPnlOrder::PositionMajor => {
                let mut transposed = Vec::with_capacity(self.data.len());
                for scenario in 0..self.n_scenarios {
                    for position in 0..n_positions {
                        transposed.push(self.data[position * self.n_scenarios + scenario]);
                    }
                }
                transposed
            }
        }
    }
}

/// Copy a NumPy matrix into logical C/row-major order.
fn numpy_row_major(array: &PyReadonlyArray2<'_, f64>) -> Vec<f64> {
    if array.is_c_contiguous() {
        if let Ok(slice) = array.as_slice() {
            return slice.to_vec();
        }
    }
    array.as_array().iter().copied().collect()
}

/// Extract and flatten an `n x n` covariance-style matrix.
pub(crate) fn extract_square_matrix(
    matrix: &Bound<'_, PyAny>,
    n: usize,
    label: &str,
) -> PyResult<Vec<f64>> {
    if let Ok(array) = matrix.extract::<PyReadonlyArray2<'_, f64>>() {
        let shape = array.shape();
        if shape[0] != n {
            return Err(crate::errors::value_error(format!(
                "{label} must have {n} rows, got {}",
                shape[0]
            )));
        }
        if shape[1] != n {
            return Err(crate::errors::value_error(format!(
                "{label} row 0 must have {n} columns, got {}",
                shape[1]
            )));
        }
        return Ok(numpy_row_major(&array));
    }

    let nested = matrix.extract::<Vec<Vec<f64>>>()?;
    core_flatten_square_matrix(nested, n, label)
        .map_err(|error| crate::errors::value_error(error.to_string()))
}

/// Extract position-major P&Ls from lists or a two-dimensional NumPy array.
pub(crate) fn extract_position_pnls(
    position_pnls: &Bound<'_, PyAny>,
    n_positions: usize,
) -> PyResult<PositionPnlMatrix> {
    if let Ok(array) = position_pnls.extract::<PyReadonlyArray2<'_, f64>>() {
        let shape = array.shape();
        if shape[0] != n_positions {
            return Err(crate::errors::value_error(format!(
                "position_pnls must have {n_positions} rows, got {}",
                shape[0]
            )));
        }
        if n_positions == 0 {
            return Ok(PositionPnlMatrix {
                data: Vec::new(),
                n_scenarios: 0,
                order: PositionPnlOrder::ScenarioMajor,
            });
        }
        return Ok(PositionPnlMatrix {
            data: numpy_row_major(&array),
            n_scenarios: shape[1],
            order: PositionPnlOrder::PositionMajor,
        });
    }

    let nested = position_pnls.extract::<Vec<Vec<f64>>>()?;
    let (data, n_scenarios) = core_flatten_position_pnls(nested, n_positions)
        .map_err(|error| crate::errors::value_error(error.to_string()))?;
    Ok(PositionPnlMatrix {
        data,
        n_scenarios,
        order: PositionPnlOrder::ScenarioMajor,
    })
}
