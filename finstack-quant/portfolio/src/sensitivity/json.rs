//! Wire form of the factor-sensitivity matrix and the P&L-profile grid default.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor::FactorId;
use finstack_quant_factor::SensitivityMatrix;
use serde::{Deserialize, Serialize};

/// Default scenario count for symmetric P&L profile grids.
///
/// `5` produces `[-2, -1, 0, 1, 2]`.
pub const DEFAULT_PNL_SCENARIO_POINTS: usize = 5;

/// Canonical wire form of a factor-sensitivity matrix, shared by both hosts.
///
/// Serializes as `{base_currency, position_ids, factor_ids, data}` with `data`
/// as nested rows, `data[position][factor]`. It is what the WASM
/// `computeFactorSensitivities` and `FactorModel.computeSensitivities` return and `models.factor.risk.decomposeFactors`
/// accepts, and
/// what the Python `SensitivityMatrix.to_json` / `from_json` emit and accept,
/// so a matrix produced by either host is readable by the other. Unknown keys
/// are rejected and `base_currency` must be an ISO-4217 code; converting to a
/// [`SensitivityMatrix`] (via [`TryFrom`]) validates the row dimensions.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_factor::SensitivityMatrix;
/// use finstack_quant_portfolio::sensitivity::SensitivityMatrixJson;
///
/// let wire: SensitivityMatrixJson = serde_json::from_str(
///     r#"{"base_currency":"USD","position_ids":["A"],"factor_ids":["F1","F2"],"data":[[1.0,2.0]]}"#,
/// )?;
/// let matrix = SensitivityMatrix::try_from(wire)?;
/// assert_eq!(matrix.delta(0, 1)?, 2.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SensitivityMatrixJson {
    /// Reporting currency of every monetary sensitivity.
    pub base_currency: Currency,
    /// Ordered position identifiers, one per row of `data`.
    pub position_ids: Vec<String>,
    /// Ordered factor identifiers, one per column of `data`.
    pub factor_ids: Vec<FactorId>,
    /// Sensitivities as nested rows, `data[position][factor]`.
    pub data: Vec<Vec<f64>>,
}

impl SensitivityMatrixJson {
    /// Tag a matrix with its monetary reporting currency for the wire.
    ///
    /// # Arguments
    ///
    /// * `matrix` - Position-by-factor monetary sensitivities to serialize.
    /// * `base_currency` - Currency in which all matrix entries were calculated.
    pub fn from_matrix(matrix: &SensitivityMatrix, base_currency: Currency) -> Self {
        let n_factors = matrix.n_factors();
        Self {
            base_currency,
            position_ids: matrix.position_ids().to_vec(),
            factor_ids: matrix.factor_ids().to_vec(),
            data: (0..matrix.n_positions())
                .map(|idx| matrix.as_slice()[idx * n_factors..(idx + 1) * n_factors].to_vec())
                .collect(),
        }
    }
}

impl TryFrom<SensitivityMatrixJson> for SensitivityMatrix {
    type Error = Error;

    /// Rebuild the matrix, validating its dimensions via
    /// [`SensitivityMatrix::from_rows`]; the reporting currency is dropped.
    fn try_from(wire: SensitivityMatrixJson) -> Result<Self> {
        SensitivityMatrix::from_rows(wire.position_ids, wire.factor_ids, wire.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monetary_matrix_wire_roundtrip_preserves_reporting_currency_and_rows() {
        let matrix = SensitivityMatrix::from_rows(
            vec!["A".into()],
            vec![FactorId::new("FX")],
            vec![vec![2.5]],
        )
        .expect("valid matrix");
        let wire = SensitivityMatrixJson::from_matrix(&matrix, Currency::EUR);
        let value = serde_json::to_value(&wire).expect("serialize");
        assert_eq!(
            value,
            serde_json::json!({
                "base_currency": "EUR", "position_ids": ["A"],
                "factor_ids": ["FX"], "data": [[2.5]],
            })
        );
        let restored: SensitivityMatrixJson = serde_json::from_value(value).expect("deserialize");
        assert_eq!(restored.base_currency, Currency::EUR);
        assert_eq!(
            SensitivityMatrix::try_from(restored).expect("valid rows"),
            matrix
        );
    }

    #[test]
    fn monetary_matrix_wire_rejects_missing_currency_and_unknown_fields() {
        for value in [
            serde_json::json!({"position_ids": [], "factor_ids": [], "data": []}),
            serde_json::json!({"base_currency": "BAD", "position_ids": [], "factor_ids": [], "data": []}),
            serde_json::json!({"base_currency": "USD", "position_ids": [], "factor_ids": [], "data": [], "n_factors": 0}),
        ] {
            assert!(serde_json::from_value::<SensitivityMatrixJson>(value).is_err());
        }
    }

    fn wire_json(value: serde_json::Value) -> String {
        value.to_string()
    }

    #[test]
    fn sensitivity_wire_round_trips_through_the_matrix() {
        let json = wire_json(serde_json::json!({
            "base_currency": "EUR",
            "position_ids": ["A", "B"],
            "factor_ids": ["F1", "F2"],
            "data": [[1.0, 2.0], [3.0, -1.0]]
        }));
        let wire: SensitivityMatrixJson = serde_json::from_str(&json).unwrap();
        let matrix = SensitivityMatrix::try_from(wire.clone()).unwrap();
        assert_eq!(matrix.as_slice(), &[1.0, 2.0, 3.0, -1.0]);
        let back = SensitivityMatrixJson::from_matrix(&matrix, Currency::EUR);
        assert_eq!(back, wire);
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            serde_json::from_str::<serde_json::Value>(&json).unwrap()
        );
    }

    #[test]
    fn sensitivity_wire_rejects_unknown_keys_and_bad_currency() {
        let unknown = wire_json(serde_json::json!({
            "base_currency": "USD", "position_ids": [], "factor_ids": [], "data": [],
            "n_factors": 0
        }));
        let err = serde_json::from_str::<SensitivityMatrixJson>(&unknown).unwrap_err();
        assert!(
            err.to_string().contains("unknown field `n_factors`"),
            "{err}"
        );
        let bad_currency = wire_json(serde_json::json!({
            "base_currency": "NOT_A_CCY", "position_ids": [], "factor_ids": [], "data": []
        }));
        assert!(serde_json::from_str::<SensitivityMatrixJson>(&bad_currency).is_err());
        let missing_currency = wire_json(serde_json::json!({
            "position_ids": [], "factor_ids": [], "data": []
        }));
        assert!(serde_json::from_str::<SensitivityMatrixJson>(&missing_currency).is_err());
    }

    #[test]
    fn sensitivity_wire_conversion_rejects_malformed_rows() {
        for data in [
            serde_json::json!([[1.0, 2.0]]),
            serde_json::json!([[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]),
            serde_json::json!([[1.0, 2.0], [3.0]]),
        ] {
            let wire: SensitivityMatrixJson = serde_json::from_value(serde_json::json!({
                "base_currency": "USD",
                "position_ids": ["A", "B"],
                "factor_ids": ["F1", "F2"],
                "data": data
            }))
            .unwrap();
            let err = SensitivityMatrix::try_from(wire).unwrap_err();
            assert!(matches!(err, Error::Validation(_)), "{err}");
        }
    }
}
