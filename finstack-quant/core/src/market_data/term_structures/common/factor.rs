//! Shared positive-factor evaluation, validation, conditioning, and bumping helpers.

use super::{
    CurveCalendar, PiecewiseLinearAdjustment, signed_curve_year_fraction,
    stable_positive_log_ratio, validate_pillars,
};
use crate::dates::{Date, DayCount, DayCountContext};
use crate::market_data::bumps::{BumpSpec, BumpType};
use crate::math::Compounding;
use crate::math::interp::{ExtrapolationPolicy, Interp, exact_f64_eq};

/// Snapshot of a positive-factor curve transformation for cross-curve factor cloning.
#[derive(Clone, Debug)]
pub(crate) struct FactorTransformSnapshot {
    pub(crate) source_base_date: Date,
    pub(crate) source_points: Vec<(f64, f64)>,
    pub(crate) adjustment: PiecewiseLinearAdjustment,
    pub(crate) offset: f64,
    pub(crate) normalization: f64,
}

/// Validate a single-point conditioned positive-factor curve anchored at `(0.0, 1.0)`.
pub(crate) fn validate_origin_only(points: &[(f64, f64)], curve_label: &str) -> crate::Result<()> {
    let [(t, factor)] = points else {
        return Err(crate::InputError::TooFewPoints.into());
    };
    if !exact_f64_eq(*t, 0.0) || !exact_f64_eq(*factor, 1.0) {
        return Err(crate::Error::Validation(format!(
            "{curve_label} must start at (time = 0, factor = 1)"
        )));
    }
    Ok(())
}

/// Validate strictly ordered positive-factor pillars without sorting or merging.
///
/// Prepends `(0.0, 1.0)` when the first pillar time is strictly positive, or
/// requires an explicit `t = 0.0` pillar to carry factor `1.0` exactly.
pub(crate) fn validate_positive_factor_points(
    mut points: Vec<(f64, f64)>,
    curve_label: &str,
) -> crate::Result<(Vec<f64>, Vec<f64>)> {
    if points.is_empty() {
        return Err(crate::InputError::TooFewPoints.into());
    }
    for (idx, &(t, factor)) in points.iter().enumerate() {
        if !t.is_finite() || t < 0.0 {
            return Err(crate::Error::Validation(format!(
                "{curve_label} time must be finite and non-negative, got {t}"
            )));
        }
        if !factor.is_finite() || factor <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        if idx > 0 && t <= points[idx - 1].0 {
            return Err(crate::InputError::NonMonotonicKnots.into());
        }
    }
    if exact_f64_eq(points[0].0, 0.0) {
        points[0].0 = 0.0;
        if !exact_f64_eq(points[0].1, 1.0) {
            return Err(crate::Error::Validation(format!(
                "{curve_label} must have factor(0) = 1.0 exactly, got {}",
                points[0].1
            )));
        }
    } else {
        points.insert(0, (0.0, 1.0));
    }
    if points.len() < 2 {
        return Err(crate::InputError::TooFewPoints.into());
    }
    let (knots, factors): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
    crate::math::interp::validate_knots(&knots)?;
    crate::math::interp::validate_finite_series(&factors)?;
    Ok((knots, factors))
}

/// Convert strictly ascending zero-rate pillars into `(time, positive_factor)` pairs.
pub(crate) fn zero_rate_points_to_factors(
    curve_label: &str,
    points: &[(f64, f64)],
    compounding: Compounding,
) -> crate::Result<Vec<(f64, f64)>> {
    if points.is_empty() {
        return Err(crate::InputError::TooFewPoints.into());
    }
    let mut knots = Vec::with_capacity(points.len());
    for (idx, &(t, r)) in points.iter().enumerate() {
        if !t.is_finite() || t < 0.0 || !r.is_finite() {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::from_zero_rates requires finite non-negative times and finite rates, got (t={t}, r={r})"
            )));
        }
        if idx > 0 && t <= points[idx - 1].0 {
            return Err(crate::InputError::NonMonotonicKnots.into());
        }
        let factor = compounding.df_from_rate(r, t)?;
        knots.push((t, factor));
    }
    Ok(knots)
}

/// Map strictly ascending dated positive factors onto a curve clock.
pub(crate) fn dated_points_to_factors(
    curve_label: &str,
    base_date: Date,
    points: &[(Date, f64)],
    day_count: DayCount,
    calendar: Option<&CurveCalendar>,
) -> crate::Result<Vec<(f64, f64)>> {
    if points.is_empty() {
        return Err(crate::InputError::TooFewPoints.into());
    }
    let mut knots = Vec::with_capacity(points.len());
    for &(date, factor) in points {
        if date < base_date {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::from_dates pillar {date} precedes base date {base_date}"
            )));
        }
        let t = signed_curve_year_fraction(base_date, date, day_count, calendar)?;
        knots.push((t, factor));
    }
    Ok(knots)
}

/// Evaluate the positive-factor source at `source_time` under an accumulated adjustment.
#[inline]
pub(crate) fn df_at_source_time(
    interp: &Interp,
    adjustment: &PiecewiseLinearAdjustment,
    offset: f64,
    normalization: f64,
    source_time: f64,
    curve_label: &str,
) -> crate::Result<f64> {
    if !source_time.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{curve_label} query coordinate must be finite, got {source_time}"
        )));
    }
    let base_df = interp.interp(source_time)?;
    if !base_df.is_finite() || base_df <= 0.0 {
        return Err(crate::InputError::NonPositiveValue.into());
    }
    let exponent = adjustment.checked_integral(offset, source_time)?;
    if exact_f64_eq(source_time, offset) {
        return Ok(1.0);
    }
    let direct = (base_df / normalization) * exponent.exp();
    if direct.is_finite() && direct > 0.0 {
        return Ok(direct);
    }
    let log_ratio = stable_positive_log_ratio(base_df, normalization)?;
    let scaled = (log_ratio + exponent).exp();
    if !scaled.is_finite() || scaled <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "{curve_label} transformed factor overflowed or underflowed at source_t={source_time}"
        )));
    }
    Ok(scaled)
}

/// Active transformation parameters for evaluating instantaneous forward derivatives.
pub(crate) struct FactorTransformEval<'a> {
    pub(crate) adjustment: &'a PiecewiseLinearAdjustment,
    pub(crate) offset: f64,
    pub(crate) normalization: f64,
    pub(crate) source_points: &'a [(f64, f64)],
}

/// Evaluate `-P'(t) / P(t)` cleanly on a positive-factor curve (transformed or raw).
#[inline]
pub(crate) fn instantaneous_forward(
    interp: &Interp,
    transform: Option<FactorTransformEval<'_>>,
    t: f64,
    curve_label: &str,
) -> crate::Result<f64> {
    if !t.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{curve_label}::instantaneous_forward requires finite time t, got {t}"
        )));
    }
    let (source_time, adj) = match transform {
        Some(tr) => {
            let source_time = tr.offset + t;
            let _ = df_at_source_time(
                interp,
                tr.adjustment,
                tr.offset,
                tr.normalization,
                source_time,
                curve_label,
            )?;
            let first_knot = tr.source_points.first().map_or(0.0, |&(k, _)| k);
            (
                source_time,
                tr.adjustment
                    .value_with_left_interior(source_time, first_knot),
            )
        }
        None => (t, 0.0),
    };
    let base_df = interp.interp(source_time)?;
    if !base_df.is_finite() || base_df <= 0.0 {
        return Err(crate::InputError::NonPositiveValue.into());
    }
    let base_deriv = interp.interp_prime(source_time)?;
    let raw = -(base_deriv / base_df) - adj;
    if !raw.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{curve_label} instantaneous_forward is non-finite at t={t}: {raw}"
        )));
    }
    Ok(if exact_f64_eq(raw, 0.0) { 0.0 } else { raw })
}

/// Refresh normalization factor at `offset` after validating the adjustment.
pub(crate) fn refresh_normalization(
    adjustment: &PiecewiseLinearAdjustment,
    offset: f64,
    interp: &Interp,
    curve_label: &str,
) -> crate::Result<f64> {
    adjustment.validate()?;
    if !offset.is_finite() {
        return Err(crate::Error::Validation(format!(
            "{curve_label} condition offset must be finite"
        )));
    }
    let normalization = interp.interp(offset)?;
    if !normalization.is_finite() || normalization <= 0.0 {
        return Err(crate::InputError::NonPositiveValue.into());
    }
    Ok(normalization)
}

/// Restore origin offset and normalization after deserializing or building a transformed curve.
pub(crate) fn validate_transform_state(
    source_base_date: Date,
    base_date: Date,
    clock: (DayCount, DayCountContext<'_>),
    interp: &Interp,
    adjustment: &PiecewiseLinearAdjustment,
    pillars: (&[f64], &[f64]),
    curve_label: &str,
) -> crate::Result<(f64, f64)> {
    let (day_count, ctx) = clock;
    let (knots, factors) = pillars;
    if base_date < source_base_date {
        return Err(crate::Error::Validation(format!(
            "{curve_label} base_date ({base_date}) cannot precede source_base_date ({source_base_date})"
        )));
    }
    let offset = day_count.signed_year_fraction(source_base_date, base_date, ctx)?;
    let normalization = refresh_normalization(adjustment, offset, interp, curve_label)?;
    validate_pillars(
        knots.iter().copied().zip(factors.iter().copied()),
        true,
        "positive-factor curve transformation",
        |time| {
            df_at_source_time(
                interp,
                adjustment,
                offset,
                normalization,
                offset + time,
                curve_label,
            )
            .unwrap_or(f64::NAN)
        },
    )?;
    Ok((offset, normalization))
}

/// Verify in-coverage triangular key-rate breakpoints remain admissible on the curve.
pub(crate) fn verify_triangular_breakpoints<F>(
    spec: &BumpSpec,
    interp: &Interp,
    min_src: f64,
    max_src: f64,
    offset: f64,
    mut check_source_time: F,
) -> crate::Result<()>
where
    F: FnMut(f64) -> crate::Result<f64>,
{
    if let BumpType::TriangularKeyRate {
        prev_bucket,
        target_bucket,
        next_bucket,
    } = spec.bump_type
    {
        for rel_t in prev_bucket
            .into_iter()
            .chain([target_bucket])
            .chain(next_bucket)
        {
            let src_t = offset + rel_t;
            let in_bounds = interp.extrapolation() != ExtrapolationPolicy::Error
                || (src_t >= min_src && src_t <= max_src);
            if in_bounds {
                let _ = check_source_time(src_t)?;
            }
        }
    }
    Ok(())
}

/// Compute remaining normalized `(t, P)` pillars when conditioning onto `new_offset`.
pub(crate) fn roll_positive_factor_points<F>(
    source_points: &[(f64, f64)],
    interp: &Interp,
    new_offset: f64,
    mut eval_source_time: F,
) -> crate::Result<(Vec<f64>, Vec<f64>)>
where
    F: FnMut(f64) -> crate::Result<f64>,
{
    let mut rolled_points: Vec<(f64, f64)> = Vec::new();
    for &(source_t, _) in source_points {
        let remaining_t = source_t - new_offset;
        if remaining_t > 0.0 && source_t > new_offset {
            let factor_rem = eval_source_time(source_t)?;
            rolled_points.push((remaining_t, factor_rem));
        }
    }
    if rolled_points.is_empty() && interp.extrapolation() != ExtrapolationPolicy::Error {
        let factor_1 = eval_source_time(new_offset + 1.0)?;
        rolled_points.push((1.0, factor_1));
    }

    let mut points = Vec::with_capacity(rolled_points.len() + 1);
    points.push((0.0, 1.0));
    points.extend(rolled_points);
    Ok(points.into_iter().unzip())
}

/// Compute the simple rate implied by two positive endpoint factors over accrual fraction `alpha > 0`.
#[inline]
pub(crate) fn simple_rate_from_factors(
    p1: f64,
    p2: f64,
    alpha: f64,
    context: &str,
) -> crate::Result<f64> {
    if !alpha.is_finite() || alpha <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "{context} requires finite positive accrual_year_fraction, got {alpha}"
        )));
    }
    let ell = stable_positive_log_ratio(p1, p2)?;
    if exact_f64_eq(ell, 0.0) {
        return Ok(0.0);
    }
    let expm1 = ell.exp_m1();
    let rate = if expm1.is_finite() {
        if ell < 0.0 && expm1 <= -1.0 {
            return Err(crate::Error::Validation(format!(
                "{context} inverse simple growth factor rounded onto zero boundary for ell={ell}"
            )));
        }
        expm1 / alpha
    } else if ell > 0.0 {
        // Growth factor exp(ell) overflowed while rate expm1(ell)/alpha may still fit in f64.
        (ell - alpha.ln()).exp() - 1.0 / alpha
    } else {
        f64::NAN
    };
    if !rate.is_finite() || exact_f64_eq(rate, 0.0) {
        return Err(crate::Error::Validation(format!(
            "{context} simple rate overflowed or underflowed to zero (rate={rate}, ell={ell}, alpha={alpha})"
        )));
    }
    if rate < 0.0 && rate.mul_add(alpha, 1.0) <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "{context} simple rate {rate} with alpha={alpha} hits non-positive gross factor domain"
        )));
    }
    Ok(rate)
}

/// Compute relative factor `p_to / p_from` after endpoint admission with overflow/underflow fallback.
#[inline]
pub(crate) fn df_between_admitted_factors(
    p_from: f64,
    p_to: f64,
    from: Date,
    to: Date,
) -> crate::Result<f64> {
    if !p_from.is_finite() || p_from <= 0.0 || !p_to.is_finite() || p_to <= 0.0 {
        return Err(crate::InputError::NonPositiveValue.into());
    }
    if from == to {
        return Ok(1.0);
    }
    let direct = p_to / p_from;
    if direct.is_finite() && direct > 0.0 {
        return Ok(direct);
    }
    let log_rel = -stable_positive_log_ratio(p_from, p_to)?;
    let scaled = log_rel.exp();
    if !scaled.is_finite() || scaled <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Invalid relative factor from {from} to {to}: {scaled}"
        )));
    }
    Ok(scaled)
}

/// Define a curve-specific `CurveTransform` newtype wrapper preserving its schema `$defs` name
/// while delegating all numerical behavior to shared [`common::factor`] helpers.
macro_rules! define_factor_curve_transform {
    ($schema_name:literal, $curve_label:literal) => {
        /// Source interpolation and a single accumulated transformation, never a chain
        /// of nested curves. The adjustment is stored as its piecewise-linear derivative
        /// so evaluating beyond a completed triangular shock does not subtract large
        /// quadratic polynomials.
        #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "json-schema", schemars(rename = $schema_name))]
        #[serde(deny_unknown_fields)]
        pub(super) struct CurveTransform {
            /// Original, untransformed curve base date defining the source coordinate system.
            #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
            source_base_date: crate::dates::Date,
            /// Original, untransformed interpolation pillars.
            source_points: Vec<(f64, f64)>,
            /// Cumulative derivative of the additive log-factor adjustment.
            adjustment: super::super::common::PiecewiseLinearAdjustment,
            /// Current origin in the source interpolation's year-fraction coordinates.
            #[serde(skip)]
            offset: f64,
            /// Derived source factor at the current origin; rebuilt on deserialization.
            #[serde(skip)]
            normalization: f64,
        }

        impl CurveTransform {
            pub(super) fn from_parts(
                source_base_date: crate::dates::Date,
                source_points: Vec<(f64, f64)>,
            ) -> Self {
                Self {
                    source_base_date,
                    source_points,
                    adjustment: super::super::common::PiecewiseLinearAdjustment::default(),
                    offset: 0.0,
                    normalization: 1.0,
                }
            }

            pub(super) fn source_base_date(&self) -> crate::dates::Date {
                self.source_base_date
            }

            pub(super) fn offset(&self) -> f64 {
                self.offset
            }

            pub(super) fn source_first_knot(&self) -> f64 {
                self.source_points.first().map_or(0.0, |&(t, _)| t)
            }

            pub(super) fn source_points(&self) -> &[(f64, f64)] {
                &self.source_points
            }

            pub(super) fn df_at_source_time(
                &self,
                interp: &crate::math::interp::Interp,
                source_time: f64,
            ) -> crate::Result<f64> {
                super::super::common::factor::df_at_source_time(
                    interp,
                    &self.adjustment,
                    self.offset,
                    self.normalization,
                    source_time,
                    $curve_label,
                )
            }

            pub(super) fn df(
                &self,
                interp: &crate::math::interp::Interp,
                t: f64,
            ) -> crate::Result<f64> {
                if !t.is_finite() {
                    return Err(crate::Error::Validation(format!(
                        "{} time t must be finite, got {t}",
                        $curve_label
                    )));
                }
                self.df_at_source_time(interp, self.offset + t)
            }

            pub(super) fn instantaneous_forward(
                &self,
                interp: &crate::math::interp::Interp,
                t: f64,
            ) -> crate::Result<f64> {
                super::super::common::factor::instantaneous_forward(
                    interp,
                    Some(super::super::common::factor::FactorTransformEval {
                        adjustment: &self.adjustment,
                        offset: self.offset,
                        normalization: self.normalization,
                        source_points: &self.source_points,
                    }),
                    t,
                    $curve_label,
                )
            }

            pub(super) fn bump(
                &mut self,
                rate: f64,
                shape: crate::market_data::bumps::BumpType,
            ) -> crate::Result<()> {
                self.adjustment.add_bump(
                    self.offset,
                    rate,
                    shape,
                    super::super::common::RateShock::ZeroRate,
                )
            }

            pub(super) fn condition(
                &mut self,
                interp: &crate::math::interp::Interp,
                reference_date: crate::dates::Date,
                day_count: crate::dates::DayCount,
                ctx: crate::dates::DayCountContext<'_>,
            ) -> crate::Result<()> {
                self.offset =
                    day_count.signed_year_fraction(self.source_base_date, reference_date, ctx)?;
                self.normalization = super::super::common::factor::refresh_normalization(
                    &self.adjustment,
                    self.offset,
                    interp,
                    $curve_label,
                )?;
                Ok(())
            }

            /// Restore origin offset and normalization and check visible pillars against the source interpolation.
            pub(super) fn validate(
                &mut self,
                base_date: crate::dates::Date,
                day_count: crate::dates::DayCount,
                ctx: crate::dates::DayCountContext<'_>,
                interp: &crate::math::interp::Interp,
                knots: &[f64],
                factors: &[f64],
            ) -> crate::Result<()> {
                let (offset, normalization) =
                    super::super::common::factor::validate_transform_state(
                        self.source_base_date,
                        base_date,
                        (day_count, ctx),
                        interp,
                        &self.adjustment,
                        (knots, factors),
                        $curve_label,
                    )?;
                self.offset = offset;
                self.normalization = normalization;
                Ok(())
            }
        }
    };
}

pub(crate) use define_factor_curve_transform;
