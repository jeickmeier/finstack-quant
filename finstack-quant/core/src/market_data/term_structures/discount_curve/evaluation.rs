//! Exact, non-recursive evaluation of rolled and continuously shocked curves.

use super::super::common::PiecewiseLinearAdjustment;
use super::DiscountCurve;
use crate::market_data::bumps::BumpType;
use crate::math::interp::types::Interp;

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
        self.adjustment.add_zero_rate_bump(self.offset, rate, shape)
    }

    pub(super) fn roll(&mut self, interp: &Interp, years: f64) -> crate::Result<()> {
        self.offset += years;
        self.refresh_normalization(interp)
    }

    fn refresh_normalization(&mut self, interp: &Interp) -> crate::Result<()> {
        self.adjustment.validate()?;
        if !self.offset.is_finite() {
            return Err(crate::error::InputError::Invalid.into());
        }
        self.normalization = interp.interp(self.offset);
        if !self.normalization.is_finite() || self.normalization <= 0.0 {
            return Err(crate::error::InputError::NonPositiveValue.into());
        }
        Ok(())
    }
}

impl DiscountCurve {
    /// Restore the original interpolation, validating the redundant public
    /// pillar samples instead of trusting either representation independently.
    pub(super) fn restore_transform(&mut self, mut transform: CurveTransform) -> crate::Result<()> {
        let source = Self::builder(self.id.clone())
            .base_date(self.base)
            .knots(transform.source_points.iter().copied())
            .interp(self.style)
            .extrapolation(self.extrapolation)
            .validation(super::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            })
            .build()?;
        transform.refresh_normalization(&source.interp)?;
        for (&time, &df) in self.knots.iter().zip(self.dfs.iter()) {
            let actual = transform.df(&source.interp, time);
            if !actual.is_finite()
                || actual <= 0.0
                || (actual - df).abs() > 1e-12 * actual.abs().max(df.abs()).max(1.0)
            {
                return Err(crate::Error::Validation(
                    "discount-curve transformation disagrees with its pillar samples".into(),
                ));
            }
        }
        self.interp = source.interp;
        self.transform = Some(transform);
        Ok(())
    }
}
