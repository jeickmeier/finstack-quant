//! Exact continuous rate shocks and translations without nested curve wrappers.

use super::{ForwardCurve, InputError};
use crate::market_data::bumps::BumpType;
use crate::market_data::term_structures::common::PiecewiseLinearAdjustment;
use crate::math::interp::types::Interp;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "ForwardCurveTransform"))]
#[serde(deny_unknown_fields)]
pub(super) struct CurveTransform {
    /// Original interpolation pillars, independent of current curve samples.
    source_points: Vec<(f64, f64)>,
    /// Current curve origin in the source curve's year-fraction coordinates.
    pub(super) offset: f64,
    /// Accumulated parallel multiplicative factor on the source interpolation.
    pub(super) scale: f64,
    /// Cumulative additive rate adjustment in source time coordinates.
    pub(super) adjustment: PiecewiseLinearAdjustment,
}

impl CurveTransform {
    pub(super) fn from_curve(curve: &ForwardCurve) -> Self {
        Self {
            source_points: curve
                .knots
                .iter()
                .copied()
                .zip(curve.forwards.iter().copied())
                .collect(),
            offset: 0.0,
            scale: 1.0,
            adjustment: PiecewiseLinearAdjustment::default(),
        }
    }

    pub(super) fn source_points(&self) -> &[(f64, f64)] {
        &self.source_points
    }

    pub(super) fn rate(&self, interp: &Interp, t: f64) -> f64 {
        let source_time = self.offset + t;
        self.scale * interp.interp(source_time) + self.adjustment.value(source_time)
    }

    pub(super) fn bump(
        &mut self,
        value: f64,
        multiplicative: bool,
        shape: BumpType,
    ) -> crate::Result<()> {
        if multiplicative {
            if !matches!(shape, BumpType::Parallel) {
                return Err(InputError::UnsupportedBump {
                    reason: "ForwardCurve key-rate shocks require additive rate units".into(),
                }
                .into());
            }
            self.scale *= value;
            self.adjustment.scale(value)?;
        } else {
            self.adjustment
                .add_forward_rate_bump(self.offset, value, shape)?;
        }
        self.validate()
    }

    pub(super) fn validate(&self) -> crate::Result<()> {
        if !self.offset.is_finite() || !self.scale.is_finite() {
            return Err(InputError::Invalid.into());
        }
        self.adjustment.validate()
    }
}

impl ForwardCurve {
    pub(super) fn restore_transform(&mut self, transform: CurveTransform) -> crate::Result<()> {
        transform.validate()?;
        for (&time, &rate) in self.knots.iter().zip(self.forwards.iter()) {
            let actual = transform.rate(&self.interp, time);
            if !actual.is_finite()
                || (actual - rate).abs() > 1e-12 * actual.abs().max(rate.abs()).max(1.0)
            {
                return Err(crate::Error::Validation(
                    "forward-curve transformation disagrees with its pillar samples".into(),
                ));
            }
        }
        self.transform = Some(transform);
        Ok(())
    }
}
