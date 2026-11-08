//! Common-sample Pearson correlation and its validated result carrier.

use finstack_quant_core::dates::Date;
use finstack_quant_core::math::linalg::validate_correlation_matrix;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize, Serializer};

/// A finite common-sample Pearson matrix with its observation provenance.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "CorrelationWire", deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct CorrelationResult {
    /// Ordered unique ticker labels for both matrix axes.
    pub tickers: Vec<String>,
    /// Symmetric positive-semidefinite Pearson matrix, with exact unit diagonal.
    pub matrix: Vec<Vec<f64>>,
    /// Number of shared return observations used for every entry.
    pub n_obs: usize,
    /// First included return-ending date, excluding the initial wealth date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start_date: Date,
    /// Last included return-ending date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub end_date: Date,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrelationWire {
    tickers: Vec<String>,
    matrix: Vec<Vec<f64>>,
    n_obs: usize,
    start_date: Date,
    end_date: Date,
}

#[derive(Serialize)]
struct CorrelationWireRef<'a> {
    tickers: &'a [String],
    matrix: &'a [Vec<f64>],
    n_obs: usize,
    start_date: Date,
    end_date: Date,
}

impl CorrelationResult {
    /// Construct a validated common-sample correlation result.
    ///
    /// # Arguments
    ///
    /// * `tickers` - Nonempty unique ticker labels in matrix-axis order.
    /// * `matrix` - Complete square finite Pearson matrix in `tickers` order;
    ///   entries must lie in `[-1, 1]`, mirror exactly and have unit diagonals.
    /// * `n_obs` - Shared return-observation count, at least two and no greater
    ///   than the inclusive number of calendar days between the date bounds.
    /// * `start_date` - First shared return-ending date, strictly before `end_date`.
    /// * `end_date` - Last shared return-ending date.
    ///
    /// # Errors
    ///
    /// Returns a validation error for malformed labels, dimensions, coefficients,
    /// numerical PSD, observation count or chronological date bounds.
    pub fn new(
        tickers: Vec<String>,
        matrix: Vec<Vec<f64>>,
        n_obs: usize,
        start_date: Date,
        end_date: Date,
    ) -> Result<Self> {
        let result = Self {
            tickers,
            matrix,
            n_obs,
            start_date,
            end_date,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        let n = self.tickers.len();
        if n == 0
            || self
                .tickers
                .iter()
                .enumerate()
                .any(|(i, label)| label.trim().is_empty() || self.tickers[..i].contains(label))
        {
            return Err(Error::Validation(
                "correlation tickers must be nonempty and unique".into(),
            ));
        }
        let days = (self.end_date - self.start_date).whole_days();
        if days <= 0 || self.n_obs < 2 || self.n_obs as u128 > (days as u128 + 1) {
            return Err(Error::Validation(
                "correlation observation count and date bounds disagree".into(),
            ));
        }
        if self.matrix.len() != n || self.matrix.iter().any(|row| row.len() != n) {
            return Err(Error::Validation(
                "correlation matrix dimensions must match tickers".into(),
            ));
        }
        for (i, row) in self.matrix.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                if !value.is_finite()
                    || !(-1.0..=1.0).contains(&value)
                    || (i == j && value.to_bits() != 1.0_f64.to_bits())
                    || value.to_bits() != self.matrix[j][i].to_bits()
                {
                    return Err(Error::Validation("correlation matrix requires finite bounded symmetric entries and unit diagonals".into()));
                }
            }
        }
        let flat: Vec<_> = self.matrix.iter().flatten().copied().collect();
        validate_correlation_matrix(&flat, n)
            .map_err(|error| Error::Validation(format!("invalid correlation matrix: {error}")))
    }
}

impl TryFrom<CorrelationWire> for CorrelationResult {
    type Error = Error;

    fn try_from(wire: CorrelationWire) -> Result<Self> {
        Self::new(
            wire.tickers,
            wire.matrix,
            wire.n_obs,
            wire.start_date,
            wire.end_date,
        )
    }
}

impl Serialize for CorrelationResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        CorrelationWireRef {
            tickers: &self.tickers,
            matrix: &self.matrix,
            n_obs: self.n_obs,
            start_date: self.start_date,
            end_date: self.end_date,
        }
        .serialize(serializer)
    }
}

fn unit_centered(values: &[f64]) -> Result<Vec<f64>> {
    let anchor = values[0];
    let scale = values
        .iter()
        .map(|&value| (value - anchor).abs())
        .fold(0.0, f64::max);
    if scale <= 0.0 {
        return Err(Error::Validation(
            "correlation is undefined for a constant common-sample column".into(),
        ));
    }
    if !scale.is_finite() {
        return Err(Error::Internal(
            "correlation centering exceeded the numeric range".into(),
        ));
    }
    let shifted: Vec<_> = values
        .iter()
        .map(|&value| (value - anchor) / scale)
        .collect();
    let mean = neumaier_sum(shifted.iter().map(|value| value / values.len() as f64));
    let mut centered: Vec<_> = shifted.iter().map(|value| value - mean).collect();
    let norm = neumaier_sum(centered.iter().map(|value| value * value)).sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return Err(Error::Internal(
            "correlation normalization failed for a varying column".into(),
        ));
    }
    for value in &mut centered {
        *value /= norm;
    }
    Ok(centered)
}

pub(crate) fn estimate(
    tickers: &[String],
    columns: &[&[f64]],
    dates: &[Date],
) -> Result<CorrelationResult> {
    if dates.len() < 2 || tickers.is_empty() {
        return Err(Error::Validation(
            "correlation requires at least two common observations and one ticker".into(),
        ));
    }
    if columns.len() != tickers.len()
        || columns.iter().any(|column| {
            column.len() != dates.len() || column.iter().any(|value| !value.is_finite())
        })
    {
        return Err(Error::Validation(
            "correlation requires matched finite common-sample columns".into(),
        ));
    }
    let unit: Vec<_> = columns
        .iter()
        .map(|column| unit_centered(column))
        .collect::<Result<_>>()?;
    let n = tickers.len();
    let mut matrix = vec![vec![0.0; n]; n];
    for i in 0..n {
        matrix[i][i] = 1.0;
        for j in i + 1..n {
            let mut coefficient = neumaier_sum(unit[i].iter().zip(&unit[j]).map(|(x, y)| x * y));
            let absolute_products =
                neumaier_sum(unit[i].iter().zip(&unit[j]).map(|(x, y)| (x * y).abs()));
            if coefficient.abs() <= f64::EPSILON.sqrt() * absolute_products {
                // Reconstruct the same covariance from exact binary inputs when
                // cancellation can expose information lost while centering.
                coefficient = crate::exact::pearson(columns[i], columns[j])?;
            }
            if !coefficient.is_finite() || coefficient.abs() > 1.0 + 1e-12 {
                return Err(Error::Internal(
                    "Pearson coefficient failed numerical bounds".into(),
                ));
            }
            if coefficient.abs() > 1.0 {
                coefficient = coefficient.signum();
            }
            matrix[i][j] = coefficient;
            matrix[j][i] = coefficient;
        }
    }
    if let Ok(result) = CorrelationResult::new(
        tickers.to_vec(),
        matrix,
        dates.len(),
        dates[0],
        dates[dates.len() - 1],
    ) {
        return Ok(result);
    }
    // Reconstruct this same Pearson matrix before reporting an unresolved
    // numerical failure. Projection would change the empirical estimator.
    let mut matrix = vec![vec![0.0; n]; n];
    for i in 0..n {
        matrix[i][i] = 1.0;
        for j in i + 1..n {
            let coefficient = crate::exact::pearson(columns[i], columns[j])?;
            matrix[i][j] = coefficient;
            matrix[j][i] = coefficient;
        }
    }
    CorrelationResult::new(
        tickers.to_vec(),
        matrix,
        dates.len(),
        dates[0],
        dates[dates.len() - 1],
    )
    .map_err(|error| Error::Internal(format!("common-sample Pearson validation failed: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Month;

    fn dates() -> Vec<Date> {
        (1..=3)
            .map(|day| Date::from_calendar_date(2025, Month::January, day).unwrap())
            .collect()
    }

    #[test]
    fn tiny_variation_and_singular_inverse_columns_remain_eligible() {
        let a = [1e-200, 2e-200, 3e-200];
        let b = [-1e-200, -2e-200, -3e-200];
        let result = estimate(&["A".into(), "B".into()], &[&a, &b], &dates()).unwrap();
        assert!((result.matrix[0][1] + 1.0).abs() < 1e-14);
        assert_eq!(result.n_obs, 3);
        assert!(estimate(&["A".into()], &[&[1.0, 1.0, 1.0]], &dates()).is_err());
    }

    #[test]
    fn anchored_centering_preserves_small_variation_at_high_offsets() {
        let offset = 2.0_f64.powi(600);
        let step = 2.0_f64.powi(550);
        let a = [offset, offset + step, offset + 2.0 * step];
        let b = [offset, offset + 2.0 * step, offset + step];
        // Centered patterns [-1,0,1] and [-1,1,0] have Pearson rho = 1/2.
        let result = estimate(&["A".into(), "B".into()], &[&a, &b], &dates()).unwrap();
        assert!((result.matrix[0][1] - 0.5).abs() < 1e-14);
    }

    #[test]
    fn severe_covariance_cancellation_preserves_tiny_correlation() {
        let a = [1e308, 0.0, 1.0];
        let b = [0.0, 1.0, -1.0];
        let result = estimate(&["A".into(), "B".into()], &[&a, &b], &dates()).unwrap();
        let expected = -(3.0_f64.sqrt() / 2.0) / 1e308;
        assert!(result.matrix[0][1] < 0.0);
        assert!((result.matrix[0][1] / expected - 1.0).abs() < 1e-14);
    }

    #[test]
    fn panel_uses_every_tickers_common_span_without_frequency_or_benchmark() {
        use crate::{PanelSeriesKind, Performance, PerformancePanel};
        let dates: Vec<_> = (1..=5)
            .map(|day| Date::from_calendar_date(2025, Month::January, day).unwrap())
            .collect();
        let mut panel = PerformancePanel {
            dates: dates.clone(),
            series: vec![
                vec![0.01, 0.02, 0.03, 0.04],
                vec![f64::NAN, 0.02, 0.01, f64::NAN],
            ],
            kind: PanelSeriesKind::Returns,
            ticker_names: vec!["A".into(), "B".into()],
            benchmark_ticker: None,
            frequency: None,
        };
        let result = Performance::new(&panel)
            .unwrap()
            .correlation_matrix()
            .unwrap();
        assert_eq!(result.n_obs, 2);
        assert_eq!(result.start_date, dates[2]);
        assert_eq!(result.end_date, dates[3]);
        assert!((result.matrix[0][1] + 1.0).abs() < 1e-14);
        panel.ticker_names.push("C".into());
        panel.series.push(vec![f64::NAN, f64::NAN, 0.02, 0.03]);
        assert!(
            Performance::new(&panel)
                .unwrap()
                .correlation_matrix()
                .is_err()
        );
    }

    #[test]
    fn carrier_rejects_invalid_provenance_unknown_fields_and_non_psd() {
        let result = estimate(&["A".into()], &[&[0.01, 0.02, 0.03]], &dates()).unwrap();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            serde_json::from_value::<CorrelationResult>(json.clone()).unwrap(),
            result
        );
        let mut invalid = json.clone();
        invalid["n_obs"] = serde_json::json!(4);
        assert!(serde_json::from_value::<CorrelationResult>(invalid).is_err());
        let mut invalid = json;
        invalid["repaired"] = serde_json::json!(false);
        assert!(serde_json::from_value::<CorrelationResult>(invalid).is_err());
        let mut invalid = result;
        invalid.matrix[0][0] = f64::INFINITY;
        assert!(serde_json::to_value(invalid).is_err());
        let dates = dates();
        assert!(
            CorrelationResult::new(
                vec!["A".into(), "B".into(), "C".into()],
                vec![
                    vec![1., -0.9, -0.9],
                    vec![-0.9, 1., -0.9],
                    vec![-0.9, -0.9, 1.]
                ],
                3,
                dates[0],
                dates[2]
            )
            .is_err()
        );
    }
}
