//! Deterministic diffusion amplitudes for the Cheyette rate process.

use finstack_quant_core::{Error, Result, math::interp::LinearStrategy};

/// Cheyette base volatility in decimal rate units per square-root model year.
///
/// A level of 0.005 is a 50 basis point rate amplitude. The process samples
/// the amplitude at the left endpoint of each step and squares it for the
/// variance-state drift.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "AmplitudeData")]
pub struct BaseVolatility(AmplitudeData);

/// Ordered point samples or right-continuous intervals ending at the given times.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    rename_all = "snake_case",
    tag = "interpolation",
    content = "amplitudes",
    deny_unknown_fields
)]
enum AmplitudeData {
    Linear(Vec<(f64, f64)>),
    ConstantIntervals(Vec<(f64, f64)>),
}

impl TryFrom<AmplitudeData> for BaseVolatility {
    type Error = Error;
    fn try_from(data: AmplitudeData) -> Result<Self> {
        let (nodes, intervals) = match &data {
            AmplitudeData::Linear(nodes) => (nodes, false),
            AmplitudeData::ConstantIntervals(nodes) => (nodes, true),
        };
        if nodes.is_empty() {
            return Err(Error::Validation(
                "Cheyette base volatility requires at least one amplitude".into(),
            ));
        }
        let mut previous = None;
        for &(time, amplitude) in nodes {
            if !time.is_finite()
                || time < 0.0
                || (intervals && time == 0.0)
                || previous.is_some_and(|p| time <= p)
                || !amplitude.is_finite()
                || amplitude < 0.0
            {
                return Err(Error::Validation(format!(
                    "invalid Cheyette base volatility node ({time}, {amplitude}): expected ordered model times and finite nonnegative amplitudes"
                )));
            }
            previous = Some(time);
        }
        Ok(Self(data))
    }
}

impl BaseVolatility {
    /// Construct a constant annualized rate amplitude.
    ///
    /// # Arguments
    ///
    /// * `sigma` - Finite nonnegative decimal rate volatility per square-root year.
    pub fn flat(sigma: f64) -> Result<Self> {
        Self::from_points(&[(0.0, sigma)])
    }

    /// Interpolate the supplied amplitudes linearly with flat tails.
    ///
    /// # Arguments
    ///
    /// * `points` - Nonempty, strictly time-ordered (nonnegative model years,
    ///   nonnegative decimal rate volatility) samples.
    pub fn from_points(points: &[(f64, f64)]) -> Result<Self> {
        AmplitudeData::Linear(points.to_vec()).try_into()
    }

    /// Construct a right-continuous interval amplitude schedule starting at zero.
    ///
    /// # Arguments
    ///
    /// * `intervals` - Nonempty (strictly increasing positive end time in years,
    ///   nonnegative decimal rate volatility) pairs. The final amplitude extends flat.
    pub fn from_intervals(intervals: &[(f64, f64)]) -> Result<Self> {
        AmplitudeData::ConstantIntervals(intervals.to_vec()).try_into()
    }

    /// Evaluate the deterministic diffusion amplitude at the supplied model time.
    ///
    /// # Arguments
    ///
    /// * `t` - Finite nonnegative time in model years from the initial state.
    pub fn get_value(&self, t: f64) -> Result<f64> {
        if !t.is_finite() || t < 0.0 {
            return Err(Error::Validation(format!(
                "invalid Cheyette base volatility time {t}"
            )));
        }
        match &self.0 {
            AmplitudeData::ConstantIntervals(nodes) => {
                let next = nodes.partition_point(|&(end, _)| end <= t);
                Ok(nodes[next.min(nodes.len() - 1)].1)
            }
            AmplitudeData::Linear(nodes) => {
                let right = nodes.partition_point(|&(time, _)| time <= t);
                if right == 0 || right == nodes.len() {
                    return Ok(nodes[right.saturating_sub(1)].1);
                }
                LinearStrategy::interpolate_segment(nodes[right - 1], nodes[right], t)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitude_sampling_preserves_units_and_switches_at_interval_ends() {
        let linear = BaseVolatility::from_points(&[(0.0, 0.01), (2.0, 0.03)]).unwrap();
        let midpoint = linear.get_value(1.0).unwrap();
        assert!((midpoint * midpoint - 0.0004).abs() < 1e-18);
        let intervals = BaseVolatility::from_intervals(&[(1.0, 0.0), (2.0, 0.005)]).unwrap();
        assert_eq!(intervals.get_value(0.5).unwrap(), 0.0);
        assert_eq!(intervals.get_value(1.0).unwrap(), 0.005);
        assert!(intervals.get_value(-0.1).is_err());
        assert!(linear.get_value(f64::NAN).is_err());
        assert!(BaseVolatility::from_points(&[(1.0, 0.5), (0.5, 0.4)]).is_err());
        assert!(
            serde_json::from_value::<BaseVolatility>(serde_json::json!({
                "interpolation": "linear", "times": [0.0], "values": [0.04]
            }))
            .is_err()
        );
    }
}
