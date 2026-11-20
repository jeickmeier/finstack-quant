//! Exact, non-recursive evaluation of conditioned and continuously shocked curves.

use super::super::common::define_factor_curve_transform;
use super::super::common::factor::FactorTransformSnapshot;

define_factor_curve_transform!("DiscountCurveTransform", "DiscountCurve");

impl CurveTransform {
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
