//! Static factor correlation matrix storage and validation.

use crate::FactorId;
use serde::{Deserialize, Serialize};

/// Static factor correlation matrix `ρ` for the covariance decomposition
/// `Σ(t) = D(t) · ρ · D(t)` where `D(t)` is the diagonal vol matrix.
///
/// `factor_ids` defines the row/column ordering; `data[i][j]` is
/// `ρ_{factor_ids[i], factor_ids[j]}`. The matrix must be square, symmetric,
/// and have unit diagonal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FactorCorrelationMatrix {
    /// Factor IDs in row/column order.
    pub factor_ids: Vec<FactorId>,
    /// Row-major correlation data. `data[i]` is row `i`.
    pub data: Vec<Vec<f64>>,
}

impl FactorCorrelationMatrix {
    /// Construct and validate a correlation matrix.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `data.len() != factor_ids.len()`
    /// - Any row has length `!= factor_ids.len()`
    /// - Any diagonal entry deviates from `1.0` by more than `1e-9`
    /// - The matrix is not symmetric within `1e-9`
    /// - the `factor_ids` list contains duplicates.
    ///
    /// # Arguments
    ///
    /// * `factor_ids` - Factor identifiers aligned with the exposure or loading columns.
    /// * `data` - Validated market-data payload to insert or transform.
    pub fn new(
        factor_ids: Vec<FactorId>,
        data: Vec<Vec<f64>>,
    ) -> finstack_quant_core::Result<Self> {
        let matrix = Self { factor_ids, data };
        matrix.check_structure()?;
        Ok(matrix)
    }

    /// Construct an identity correlation matrix for the given factor IDs.
    #[must_use]
    pub fn identity(factor_ids: Vec<FactorId>) -> Self {
        let n = factor_ids.len();
        let data = (0..n)
            .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        Self { factor_ids, data }
    }

    /// Check the structural validity of `&self` (shape, diagonal, symmetry, no duplicate IDs).
    ///
    /// Called by [`CreditFactorModel::validate`] to catch matrices that were
    /// constructed via direct field assignment rather than through [`Self::new`].
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - `data.len() != factor_ids.len()`
    /// - Any row has length `!= factor_ids.len()`
    /// - Any diagonal entry deviates from `1.0` by more than `1e-9`
    /// - The matrix is not symmetric within `1e-9`
    /// - `factor_ids` contains duplicates
    pub(crate) fn check_structure(&self) -> finstack_quant_core::Result<()> {
        let n = self.factor_ids.len();
        let mut seen = std::collections::BTreeSet::new();
        for fid in &self.factor_ids {
            if !seen.insert(fid) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: duplicate factor_id {fid:?}"
                )));
            }
        }
        if self.data.len() != n {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FactorCorrelationMatrix: expected {n} rows, got {}",
                self.data.len()
            )));
        }
        for (i, row) in self.data.iter().enumerate() {
            if row.len() != n {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: row {i} has length {}, expected {n}",
                    row.len()
                )));
            }
            for (j, &val) in row.iter().enumerate() {
                if !val.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FactorCorrelationMatrix: entry [{i}][{j}] is not finite ({val})"
                    )));
                }
                if i != j && val.abs() > 1.0 + 1e-10 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FactorCorrelationMatrix: off-diagonal entry [{i}][{j}] = {val} outside [-1, 1]"
                    )));
                }
            }
            let diag = row[i];
            if (diag - 1.0).abs() > 1e-9 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FactorCorrelationMatrix: diagonal entry [{i}][{i}] = {diag}, expected 1.0"
                )));
            }
        }
        // Check symmetry: data[i][j] must equal data[j][i]. Two-dimensional
        // cross-indexing keeps range loops clearer than the clippy suggestion.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            for j in (i + 1)..n {
                let lo = self.data[i][j];
                let hi = self.data[j][i];
                if (lo - hi).abs() > 1e-9 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "FactorCorrelationMatrix: not symmetric at [{i}][{j}]: {lo} vs {hi}"
                    )));
                }
            }
        }
        Ok(())
    }
}
