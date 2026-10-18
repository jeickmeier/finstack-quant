//! Roll state shared by the pillar-interpolated term structures.
//!
//! Inflation, basis-spread and price curves roll forward by keeping their
//! source interpolation and translating its time origin: a rolled curve
//! evaluates `source(t + offset)` instead of refitting the rolled pillars, so
//! the curve shape between pillars survives the roll. [`InterpRollState`] is
//! that serialized source grid and origin; a curve stores it beside its
//! unrolled source interpolation. Discount and forward curves carry richer
//! transforms (`CurveTransform`) and share only [`validate_pillars`].

use crate::math::interp::types::Interp;
use crate::math::interp::{ExtrapolationPolicy, InterpStyle, ValidationPolicy};

/// Serialized source grid and time origin of a rolled curve.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct InterpRollState {
    /// Source pillars `(time, value)` in the source curve's own coordinates.
    pub(crate) source_points: Vec<(f64, f64)>,
    /// Current origin in the source interpolation's year-fraction coordinates.
    pub(crate) offset: f64,
}

impl InterpRollState {
    /// Value of the rolled curve at `t`: the source interpolation at `t + offset`.
    #[inline]
    pub(crate) fn interp(&self, source: &Interp, t: f64) -> f64 {
        source.interp(t + self.offset)
    }

    /// Roll state after a further `years` shift.
    ///
    /// An unrolled curve (`current == None`) seeds the source grid from
    /// `current_points`. Returns `None` when the new origin precedes the
    /// source's own origin (a net backward roll): the source holds no market
    /// shape before its base date, so the caller rebuilds the curve from its
    /// rolled pillars instead.
    pub(crate) fn advance(
        current: Option<&Self>,
        years: f64,
        current_points: impl IntoIterator<Item = (f64, f64)>,
    ) -> crate::Result<Option<Self>> {
        let mut state = current.cloned().unwrap_or_else(|| Self {
            source_points: current_points.into_iter().collect(),
            offset: 0.0,
        });
        state.offset += years;
        if !state.offset.is_finite() {
            return Err(crate::error::InputError::Invalid.into());
        }
        Ok((state.offset >= 0.0).then_some(state))
    }

    /// Rebuild the source interpolation from the source pillars.
    ///
    /// `validation` is the value policy of the owning curve, applied to every
    /// source pillar (including pillars before the origin).
    pub(crate) fn restore(
        &self,
        style: InterpStyle,
        extrapolation: ExtrapolationPolicy,
        validation: ValidationPolicy,
    ) -> crate::Result<Interp> {
        if !self.offset.is_finite() {
            return Err(crate::error::InputError::Invalid.into());
        }
        let (knots, values): (Vec<_>, Vec<_>) = self.source_points.iter().copied().unzip();
        style.build_enum(
            knots.into_boxed_slice(),
            values.into_boxed_slice(),
            extrapolation,
            validation,
        )
    }

    /// Shock a rolled curve in level space while keeping its shape.
    ///
    /// Every source pillar is mapped with `map(current_time, value)`, where
    /// `current_time` is the pillar's time on the rolled curve (negative for
    /// pillars before the origin). When the shocked source would not carry the
    /// current origin to `map(0, origin_level)`, the origin is added as a
    /// source pillar first, which leaves linear and log-linear interpolants
    /// unchanged. Returns the shocked state and the shocked curve's values at
    /// `current_times`, read back from the shocked source.
    pub(crate) fn bump(
        &self,
        source: &Interp,
        validation: ValidationPolicy,
        current_times: &[f64],
        map: impl Fn(f64, f64) -> f64,
    ) -> crate::Result<(Self, Vec<f64>)> {
        let origin = self.offset;
        let origin_level = map(0.0, source.interp(origin));
        let shock = |points: &[(f64, f64)]| -> crate::Result<(Self, Interp)> {
            let state = Self {
                source_points: points
                    .iter()
                    .map(|&(time, value)| (time, map(time - origin, value)))
                    .collect(),
                offset: origin,
            };
            let shocked = state.restore(source.style(), source.extrapolation(), validation)?;
            Ok((state, shocked))
        };
        let (mut shocked_state, mut shocked) = shock(&self.source_points)?;
        let level = shocked_state.interp(&shocked, 0.0);
        if (level - origin_level).abs() > 1e-12 * level.abs().max(origin_level.abs()).max(1.0) {
            let mut points = self.source_points.clone();
            let at = points.partition_point(|&(time, _)| time < origin);
            points.insert(at, (origin, source.interp(origin)));
            (shocked_state, shocked) = shock(&points)?;
        }
        let values: Vec<f64> = current_times
            .iter()
            .map(|&t| shocked_state.interp(&shocked, t))
            .collect();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(crate::error::InputError::Invalid.into());
        }
        Ok((shocked_state, values))
    }
}

/// Reject a transformation whose evaluation disagrees with the stored pillars.
///
/// `actual` evaluates the transformation at a pillar time and `what` names it
/// in the error. A non-finite value (or, with `require_positive`, a
/// non-positive one), or a mismatch beyond `1e-12` relative tolerance, is a
/// validation error.
pub(crate) fn validate_pillars(
    points: impl IntoIterator<Item = (f64, f64)>,
    require_positive: bool,
    what: &str,
    actual: impl Fn(f64) -> f64,
) -> crate::Result<()> {
    for (time, value) in points {
        let actual = actual(time);
        if !actual.is_finite()
            || (require_positive && actual <= 0.0)
            || (actual - value).abs() > 1e-12 * actual.abs().max(value.abs()).max(1.0)
        {
            return Err(crate::Error::Validation(format!(
                "{what} disagrees with its pillar samples"
            )));
        }
    }
    Ok(())
}
