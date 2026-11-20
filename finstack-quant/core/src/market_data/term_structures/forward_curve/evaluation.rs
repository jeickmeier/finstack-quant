//! Exact, non-recursive evaluation of conditioned and continuously shocked forward curves.

use super::super::common::define_factor_curve_transform;
use super::super::common::factor::FactorTransformSnapshot;

define_factor_curve_transform!("ForwardCurveTransform", "ForwardCurve");

impl CurveTransform {
    pub(super) fn from_snapshot(snap: FactorTransformSnapshot) -> Self {
        Self {
            source_base_date: snap.source_base_date,
            source_points: snap.source_points,
            adjustment: snap.adjustment,
            offset: snap.offset,
            normalization: snap.normalization,
        }
    }
}
