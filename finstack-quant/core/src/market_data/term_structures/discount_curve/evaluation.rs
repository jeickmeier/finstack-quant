//! Exact, non-recursive evaluation of rolled and continuously shocked curves.

use super::super::common::{validate_pillars, PiecewiseLinearAdjustment, RateShock};
use super::DiscountCurve;
use crate::market_data::bumps::BumpType;
use crate::math::interp::Interp;

/// Source interpolation and a single accumulated transformation, never a chain
/// of nested curves. The adjustment is stored as its piecewise-linear derivative
/// so evaluating beyond a completed triangular shock does not subtract large
/// quadratic polynomials.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "DiscountCurveTransform"))]
#[serde(deny_unknown_fields)]
pub(super) struct CurveTransform {
    /// Original, untransformed interpolation pillars.
    source_points: Vec<(f64, f64)>,
    /// Current origin in the source interpolation's year-fraction coordinates.
    offset: f64,
    /// Cumulative derivative of the additive log-discount adjustment.
    adjustment: PiecewiseLinearAdjustment,
    /// Derived source DF at the current origin; rebuilt on deserialization.
    #[serde(skip)]
    normalization: f64,
}

impl CurveTransform {
    pub(super) fn from_curve(curve: &DiscountCurve) -> Self {
        Self {
            source_points: curve
                .knots
                .iter()
                .copied()
                .zip(curve.dfs.iter().copied())
                .collect(),
            offset: 0.0,
            adjustment: PiecewiseLinearAdjustment::default(),
            normalization: 1.0,
        }
    }

    pub(super) fn df(&self, interp: &Interp, t: f64) -> f64 {
        if t == 0.0 {
            return 1.0;
        }
        let source_time = self.offset + t;
        interp.interp(source_time) / self.normalization
            * self.adjustment.integral(self.offset, source_time).exp()
    }

    pub(super) fn bump(&mut self, rate: f64, shape: BumpType) -> crate::Result<()> {
        self.adjustment
            .add_bump(self.offset, rate, shape, RateShock::ZeroRate)
    }

    pub(super) fn roll(&mut self, interp: &Interp, years: f64) -> crate::Result<()> {
        self.offset += years;
        self.refresh_normalization(interp)
    }

    fn refresh_normalization(&mut self, interp: &Interp) -> crate::Result<()> {
        self.adjustment.validate()?;
        if !self.offset.is_finite() {
            return Err(crate::Error::Validation(
                "discount curve roll offset must be finite".to_string(),
            ));
        }
        self.normalization = interp.interp(self.offset);
        if !self.normalization.is_finite() || self.normalization <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        Ok(())
    }

    pub(super) fn source_points(&self) -> &[(f64, f64)] {
        &self.source_points
    }

    /// Restore normalization and check visible pillars against the source interpolation.
    pub(super) fn validate(
        &mut self,
        interp: &Interp,
        knots: &[f64],
        dfs: &[f64],
    ) -> crate::Result<()> {
        self.refresh_normalization(interp)?;
        validate_pillars(
            knots.iter().copied().zip(dfs.iter().copied()),
            true,
            "discount-curve transformation",
            |time| self.df(interp, time),
        )
    }
}
