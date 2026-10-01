//! Factor covariance matrix storage and validation.
//!
//! [`FactorCovarianceMatrix`] stores a row-major symmetric PSD matrix with
//! canonical factor ordering. See the type-level docs for the units contract
//! and validation guarantees.

use super::FactorId;
use finstack_quant_core::HashMap;
use serde::{Deserialize, Deserializer, Serialize};

/// User-supplied factor covariance matrix with row-major storage.
///
/// The factor ID order is part of the contract: row `i`, column `j`
/// corresponds to `factor_ids[i]` and `factor_ids[j]`.
///
/// # Units contract
///
/// Entries are **annualized (co)variances of factor moves expressed in each
/// factor's canonical bump unit** — the same unit the sensitivity engines use
/// for deltas (see [`crate::factor::BumpSizeConfig`] / [`crate::factor::FactorType::bump_units`]):
/// basis points for rates/credit/inflation, percent for equity/commodity/FX,
/// vol points for volatility. With sensitivities `s` in P&L-per-canonical-unit
/// and `Σ` in canonical-unit², `sᵀΣs` is directly an annual P&L variance.
/// Mixing conventions — e.g. a covariance in decimal² (`0.0001²` per bp²)
/// against per-bp sensitivities — mis-scales portfolio variance by `1e8`.
/// The credit calibrator produces matrices in this convention from the spread
/// panel's native units; hand-built matrices must match it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FactorCovarianceMatrix {
    /// Factor identifiers, in the row and column order of `data`.
    factor_ids: Vec<FactorId>,
    /// Matrix dimension. Equals `factor_ids.len()`.
    n: usize,
    /// Annualized (co)variances in row-major order, `n * n` entries, each in
    /// its factors' canonical bump units squared. The matrix is symmetric, so
    /// `data[i * n + j]` equals `data[j * n + i]`.
    data: Vec<f64>,
    /// Factor-to-row lookup, rebuilt on deserialization rather than carried on
    /// the wire.
    #[serde(skip)]
    index: HashMap<FactorId, usize>,
    /// Cached `√max(variance, 0)` per factor, rebuilt with [`Self::new`].
    #[serde(skip)]
    stdevs: Vec<f64>,
}

impl FactorCovarianceMatrix {
    /// Construct a covariance matrix with full validation.
    ///
    /// Validation checks:
    /// - `data.len() == n * n`
    /// - factor identifiers are unique
    /// - the matrix is symmetric within a small floating-point tolerance
    /// - the matrix is positive semi-definite according to a Cholesky-style test,
    ///   which also rejects every non-finite entry
    ///
    /// Deserialization goes through this constructor too, so every matrix has
    /// finite entries and serializes to JSON without loss.
    ///
    /// The `data` vector is row-major in the exact `factor_ids` order. Its
    /// entries must use the canonical annualized covariance units described on
    /// [`FactorCovarianceMatrix`]; this constructor validates structure and
    /// numerical matrix properties, not market-unit consistency.
    ///
    /// # Arguments
    ///
    /// * `factor_ids` - Ordered, unique factor identifiers defining both matrix axes.
    /// * `data` - Row-major annualized covariance entries in the exact
    ///   `factor_ids` order; length must equal the squared identifier count.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is not square for the supplied identifiers,
    /// an identifier is duplicated, the matrix is asymmetric beyond the
    /// scale-relative tolerance, or the Cholesky-style test finds it not
    /// positive semidefinite.
    pub fn new(factor_ids: Vec<FactorId>, data: Vec<f64>) -> finstack_quant_core::Result<Self> {
        let n = factor_ids.len();
        if data.len() != n * n {
            return Err(finstack_quant_core::InputError::DimensionMismatch.into());
        }

        let index = Self::build_index(&factor_ids)?;

        for i in 0..n {
            for j in (i + 1)..n {
                let lhs = data[i * n + j];
                let rhs = data[j * n + i];
                // Scale-relative tolerance: an absolute 1e-12 would reject
                // machine-symmetric matrices on bp² scales (entries ~1e4)
                // while being needlessly loose on tiny-variance scales.
                let tol = 1e-12 * lhs.abs().max(rhs.abs()).max(1.0);
                if (lhs - rhs).abs() > tol {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Covariance matrix is not symmetric at ({i}, {j}): {lhs} vs {rhs}"
                    )));
                }
            }
        }

        if let Err(e) = finstack_quant_core::math::linalg::cholesky_correlation(&data, n) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Covariance matrix is not positive semi-definite: {e}"
            )));
        }

        let stdevs = (0..n)
            .map(|i| {
                let variance = data[i * n + i];
                if variance > 0.0 {
                    variance.sqrt()
                } else {
                    0.0
                }
            })
            .collect();

        Ok(Self {
            factor_ids,
            n,
            data,
            index,
            stdevs,
        })
    }

    /// Number of factors represented by the matrix.
    #[must_use]
    pub fn n_factors(&self) -> usize {
        self.n
    }

    /// Ordered factor identifiers corresponding to the matrix axes.
    #[must_use]
    pub fn factor_ids(&self) -> &[FactorId] {
        &self.factor_ids
    }

    /// Borrow the raw row-major covariance storage.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Row/column index of `factor`, or `None` if the identifier is unknown.
    ///
    /// # Arguments
    ///
    /// * `factor` - Identifier to locate on the matrix axes. A missing id
    ///   returns `None` rather than `0`; pair this with the `*_at` accessors
    ///   when a caller already walked [`Self::factor_ids`].
    #[must_use]
    pub fn index_of(&self, factor: &FactorId) -> Option<usize> {
        self.index.get(factor).copied()
    }

    /// Variance at a known axis index.
    ///
    /// # Arguments
    ///
    /// * `idx` - Row/column in `[0, n_factors)`. Out of range is a hard
    ///   assert, matching [`crate::factor::SensitivityMatrix::position_deltas`].
    ///
    /// # Panics
    ///
    /// Panics when `idx` is out of bounds.
    #[must_use]
    pub fn variance_at(&self, idx: usize) -> f64 {
        assert!(
            idx < self.n,
            "idx {idx} out of bounds for {} factors",
            self.n
        );
        self.data[idx * self.n + idx]
    }

    /// Covariance at a known pair of axis indices.
    ///
    /// # Arguments
    ///
    /// * `i` - Row index in `[0, n_factors)`.
    /// * `j` - Column index in `[0, n_factors)`.
    ///
    /// # Panics
    ///
    /// Panics when `i` or `j` is out of bounds.
    #[must_use]
    pub fn covariance_at(&self, i: usize, j: usize) -> f64 {
        assert!(
            i < self.n && j < self.n,
            "covariance index ({i}, {j}) out of bounds for {} factors",
            self.n
        );
        self.data[i * self.n + j]
    }

    /// Covariance entries as one row per factor, in [`Self::factor_ids`]
    /// order.
    #[must_use]
    pub fn to_rows(&self) -> Vec<Vec<f64>> {
        if self.n == 0 {
            return Vec::new();
        }
        self.data.chunks(self.n).map(<[f64]>::to_vec).collect()
    }

    /// Correlation at a known pair of axis indices, using cached standard
    /// deviations.
    ///
    /// # Arguments
    ///
    /// * `i` - Row index in `[0, n_factors)`.
    /// * `j` - Column index in `[0, n_factors)`.
    ///
    /// # Panics
    ///
    /// Panics when `i` or `j` is out of bounds.
    #[must_use]
    pub fn correlation_at(&self, i: usize, j: usize) -> f64 {
        assert!(
            i < self.n && j < self.n,
            "correlation index ({i}, {j}) out of bounds for {} factors",
            self.n
        );
        let stdev_i = self.stdevs[i];
        let stdev_j = self.stdevs[j];
        if stdev_i <= 0.0 || stdev_j <= 0.0 {
            return 0.0;
        }
        self.data[i * self.n + j] / (stdev_i * stdev_j)
    }

    /// Return the variance for a factor, or `0.0` if the factor is unknown.
    ///
    /// The `0.0` fallback means an unknown (e.g. mistyped) factor ID silently
    /// contributes zero risk. Use [`Self::factor_ids`] to validate a query
    /// universe up front, or rely on
    /// [`crate::factor::FactorModelConfig::validate_matching_factor_ids`] (called by
    /// `CreditFactorModel::validate`) to reject undeclared factors at
    /// config-load time.
    ///
    /// # Arguments
    ///
    /// * `factor` - Factor whose diagonal variance is requested. An identifier
    ///   that is not a row of this matrix returns `0.0`.
    #[must_use]
    pub fn variance(&self, factor: &FactorId) -> f64 {
        self.index_of(factor)
            .map_or(0.0, |idx| self.variance_at(idx))
    }

    /// Return the covariance between two factors, or `0.0` if either is unknown.
    ///
    /// # Arguments
    ///
    /// * `lhs` - First factor. Unknown identifiers return `0.0`.
    /// * `rhs` - Second factor. Unknown identifiers return `0.0`.
    #[must_use]
    pub fn covariance(&self, lhs: &FactorId, rhs: &FactorId) -> f64 {
        match (self.index_of(lhs), self.index_of(rhs)) {
            (Some(i), Some(j)) => self.covariance_at(i, j),
            _ => 0.0,
        }
    }

    /// Return the correlation between two factors.
    ///
    /// # Arguments
    ///
    /// * `lhs` - First factor. Unknown identifiers, or a non-positive
    ///   variance on either axis, return `0.0`.
    /// * `rhs` - Second factor.
    #[must_use]
    pub fn correlation(&self, lhs: &FactorId, rhs: &FactorId) -> f64 {
        match (self.index_of(lhs), self.index_of(rhs)) {
            (Some(i), Some(j)) => self.correlation_at(i, j),
            _ => 0.0,
        }
    }

    fn build_index(
        factor_ids: &[FactorId],
    ) -> finstack_quant_core::Result<HashMap<FactorId, usize>> {
        let mut index = HashMap::with_capacity_and_hasher(factor_ids.len(), Default::default());
        for (idx, factor_id) in factor_ids.iter().cloned().enumerate() {
            if index.insert(factor_id.clone(), idx).is_some() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Duplicate factor id '{factor_id}' in covariance matrix"
                )));
            }
        }
        Ok(index)
    }
}

impl<'de> Deserialize<'de> for FactorCovarianceMatrix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct FactorCovarianceMatrixSerde {
            /// Factor identifiers, in the row and column order of `data`.
            factor_ids: Vec<FactorId>,
            /// Matrix dimension. Must equal `factor_ids.len()`.
            n: usize,
            /// Covariances in row-major order, `n * n` entries. The matrix is
            /// symmetric, so `data[i * n + j]` equals `data[j * n + i]`.
            data: Vec<f64>,
        }

        let helper = FactorCovarianceMatrixSerde::deserialize(deserializer)?;
        if helper.factor_ids.len() != helper.n {
            return Err(serde::de::Error::custom(
                "Factor covariance matrix factor count does not match n",
            ));
        }

        if helper.data.len() != helper.n * helper.n {
            return Err(serde::de::Error::custom(
                "Factor covariance matrix data length does not match n x n",
            ));
        }

        FactorCovarianceMatrix::new(helper.factor_ids, helper.data)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn two_factor_ids() -> Vec<FactorId> {
        vec![FactorId::new("Rates"), FactorId::new("Credit")]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn two_factor_psd_covariance_accepts_valid_correlation(
            variance_a in 1.0e-10_f64..1.0,
            variance_b in 1.0e-10_f64..1.0,
            correlation in -0.999_f64..0.999,
        ) {
            let covariance = correlation * (variance_a * variance_b).sqrt();
            let matrix = FactorCovarianceMatrix::new(
                two_factor_ids(),
                vec![variance_a, covariance, covariance, variance_b],
            )
            .unwrap();

            prop_assert!((matrix.variance(&FactorId::new("Rates")) - variance_a).abs() < 1e-12);
            prop_assert!((matrix.variance(&FactorId::new("Credit")) - variance_b).abs() < 1e-12);
            prop_assert!((matrix.correlation(&FactorId::new("Rates"), &FactorId::new("Credit")) - correlation).abs() < 1e-10);
        }

        #[test]
        fn two_factor_covariance_rejects_out_of_bounds_correlation(
            variance_a in 1.0e-6_f64..1.0,
            variance_b in 1.0e-6_f64..1.0,
            // Keep the excess away from the marginally-indefinite corner:
            // the pivoted-Cholesky PSD test has a numerical tolerance, so
            // |rho| = 1 + 1e-6 can be accepted as PSD-within-tolerance.
            excess in 1.0e-3_f64..1.0,
        ) {
            let covariance = (1.0 + excess) * (variance_a * variance_b).sqrt();
            let result = FactorCovarianceMatrix::new(
                two_factor_ids(),
                vec![variance_a, covariance, covariance, variance_b],
            );

            prop_assert!(result.is_err());
        }
    }

    #[test]
    fn test_valid_2x2_covariance() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        assert_eq!(covariance.n_factors(), 2);
    }

    #[test]
    fn test_variance_accessor() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        assert!((covariance.variance(&FactorId::new("Rates")) - 0.04).abs() < 1e-12);
        assert!((covariance.variance(&FactorId::new("Credit")) - 0.09).abs() < 1e-12);
        assert_eq!(covariance.index_of(&FactorId::new("Rates")), Some(0));
        assert_eq!(covariance.index_of(&FactorId::new("missing")), None);
        assert!((covariance.variance_at(0) - 0.04).abs() < 1e-12);
        assert!((covariance.variance_at(1) - 0.09).abs() < 1e-12);
    }

    #[test]
    fn test_covariance_accessor() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        let value = covariance.covariance(&FactorId::new("Rates"), &FactorId::new("Credit"));
        assert!((value - 0.01).abs() < 1e-12);
        assert!((covariance.covariance_at(0, 1) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn test_correlation_accessor() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        let correlation = covariance.correlation(&FactorId::new("Rates"), &FactorId::new("Credit"));
        assert!((correlation - (1.0 / 6.0)).abs() < 1e-10);
        assert!((covariance.correlation_at(0, 1) - (1.0 / 6.0)).abs() < 1e-10);
    }

    #[test]
    fn test_deserialize_rejects_non_psd_matrix() {
        let json = r#"{
            "factor_ids":["Rates","Credit"],
            "n":2,
            "data":[1.0,2.0,2.0,1.0]
        }"#;
        let result: Result<FactorCovarianceMatrix, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_dimensions_rejected() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01];
        assert!(FactorCovarianceMatrix::new(ids, data).is_err());
    }

    #[test]
    fn test_asymmetric_matrix_rejected() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.02, 0.01, 0.09];
        assert!(FactorCovarianceMatrix::new(ids, data).is_err());
    }

    #[test]
    fn test_not_psd_rejected() {
        let ids = two_factor_ids();
        let data = vec![1.0, 3.0, 3.0, 1.0];
        assert!(FactorCovarianceMatrix::new(ids, data).is_err());
    }

    #[test]
    fn test_single_factor() {
        let ids = vec![FactorId::new("Only")];
        let data = vec![0.25];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        assert!((covariance.variance(&FactorId::new("Only")) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_as_slice() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data.clone());
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };
        assert_eq!(covariance.as_slice(), &data[..]);
    }

    #[test]
    fn test_serde_roundtrip() {
        let ids = two_factor_ids();
        let data = vec![0.04, 0.01, 0.01, 0.09];
        let covariance_result = FactorCovarianceMatrix::new(ids, data);
        assert!(covariance_result.is_ok());
        let Ok(covariance) = covariance_result else {
            return;
        };

        let json_result = serde_json::to_string(&covariance);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<FactorCovarianceMatrix, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(covariance.as_slice(), back.as_slice());
        assert_eq!(covariance.n_factors(), back.n_factors());
        assert_eq!(
            covariance.covariance(&FactorId::new("Rates"), &FactorId::new("Credit")),
            back.covariance(&FactorId::new("Rates"), &FactorId::new("Credit"))
        );
    }
}
