//! Exact, non-recursive evaluation of conditioned and continuously shocked curves.

use super::super::common::define_factor_curve_transform;
use super::super::common::factor::FactorTransformSnapshot;
use super::DiscountCurve;

define_factor_curve_transform!("DiscountCurveTransform", "DiscountCurve");

impl CurveTransform {
    pub(super) fn from_curve(curve: &DiscountCurve) -> Self {
        Self::from_parts(
            curve.base,
            curve
                .knots
                .iter()
                .copied()
                .zip(curve.dfs.iter().copied())
                .collect(),
        )
    }

    pub(super) fn to_snapshot(&self) -> FactorTransformSnapshot {
        FactorTransformSnapshot {
            source_base_date: self.source_base_date,
            source_points: self.source_points.clone(),
            adjustment: self.adjustment.clone(),
            offset: self.offset,
            normalization: self.normalization,
        }
    }
}
