//! Shared positive-factor evaluation, validation, conditioning, and bumping core.

use super::{
    CurveCalendar, PiecewiseLinearAdjustment, signed_curve_year_fraction,
    stable_positive_log_ratio, validate_curve_clock, validate_pillars,
};
use crate::dates::{Date, DayCount, DayCountContext};
use crate::market_data::bumps::{BumpSpec, BumpType};
use crate::math::Compounding;
use crate::math::interp::{ExtrapolationPolicy, Interp, InterpStyle, exact_f64_eq};

/// Internal evaluation state for a single accumulated roll and continuous rate shock.
#[derive(Clone, Debug)]
pub(crate) struct FactorTransform {
    pub(crate) source_base_date: Date,
    pub(crate) source_points: Vec<(f64, f64)>,
    pub(crate) adjustment: PiecewiseLinearAdjustment,
    offset: f64,
    normalization: f64,
}

impl FactorTransform {
    pub(crate) fn from_wire(
        source_base_date: Date,
        source_points: Vec<(f64, f64)>,
        adjustment: PiecewiseLinearAdjustment,
    ) -> Self {
        Self {
            source_base_date,
            source_points,
            adjustment,
            offset: 0.0,
            normalization: 1.0,
        }
    }

    fn from_parts(source_base_date: Date, source_points: Vec<(f64, f64)>) -> Self {
        Self::from_wire(
            source_base_date,
            source_points,
            PiecewiseLinearAdjustment::default(),
        )
    }

    pub(crate) fn offset(&self) -> f64 {
        self.offset
    }

    pub(crate) fn source_points(&self) -> &[(f64, f64)] {
        &self.source_points
    }

    #[inline]
    pub(crate) fn df_at_source_time(
        &self,
        interp: &Interp,
        source_time: f64,
        curve_label: &str,
    ) -> crate::Result<f64> {
        eval_transformed_factor(
            interp,
            &self.adjustment,
            self.offset,
            self.normalization,
            source_time,
            curve_label,
        )
    }

    fn refresh_normalization(
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

    fn condition(
        &mut self,
        interp: &Interp,
        reference_date: Date,
        day_count: DayCount,
        ctx: DayCountContext<'_>,
        curve_label: &str,
    ) -> crate::Result<()> {
        self.offset = day_count.signed_year_fraction(self.source_base_date, reference_date, ctx)?;
        self.normalization =
            Self::refresh_normalization(&self.adjustment, self.offset, interp, curve_label)?;
        Ok(())
    }

    fn validate(
        &mut self,
        base_date: Date,
        day_count: DayCount,
        ctx: DayCountContext<'_>,
        interp: &Interp,
        pillars: (&[f64], &[f64]),
        curve_label: &str,
    ) -> crate::Result<()> {
        if base_date < self.source_base_date {
            return Err(crate::Error::Validation(format!(
                "{curve_label} base_date ({base_date}) cannot precede source_base_date ({})",
                self.source_base_date
            )));
        }
        let offset = day_count.signed_year_fraction(self.source_base_date, base_date, ctx)?;
        let normalization =
            Self::refresh_normalization(&self.adjustment, offset, interp, curve_label)?;
        let (knots, factors) = pillars;
        validate_pillars(
            knots.iter().copied().zip(factors.iter().copied()),
            true,
            "positive-factor curve transformation",
            |time| {
                eval_transformed_factor(
                    interp,
                    &self.adjustment,
                    offset,
                    normalization,
                    offset + time,
                    curve_label,
                )
                .unwrap_or(f64::NAN)
            },
        )?;
        self.offset = offset;
        self.normalization = normalization;
        Ok(())
    }
}

/// Shared representation of a positive-factor curve (`DiscountCurve` / `ForwardCurve`).
#[derive(Debug, Clone)]
pub(crate) struct FactorCore {
    pub(crate) base: Date,
    pub(crate) day_count: DayCount,
    pub(crate) calendar: Option<CurveCalendar>,
    pub(crate) knots: Box<[f64]>,
    pub(crate) values: Box<[f64]>,
    pub(crate) interp: Interp,
    pub(crate) transform: Option<FactorTransform>,
}

/// Inputs extracted from a positive-factor curve builder.
pub(crate) struct FactorBuildInput<'a> {
    pub(crate) id: &'a str,
    pub(crate) base: Option<Date>,
    pub(crate) day_count: Option<DayCount>,
    pub(crate) calendar_id: Option<&'a str>,
    pub(crate) points: Vec<(f64, f64)>,
    pub(crate) style: Option<InterpStyle>,
    pub(crate) extrapolation: Option<ExtrapolationPolicy>,
    pub(crate) transform: Option<FactorTransform>,
}

impl FactorCore {
    /// Validate builder inputs and construct a positive-factor curve core.
    pub(crate) fn build(curve_label: &str, mut input: FactorBuildInput<'_>) -> crate::Result<Self> {
        let base = input
            .base
            .ok_or_else(|| crate::Error::Validation(format!("{curve_label} requires base_date")))?;
        let day_count = input
            .day_count
            .ok_or_else(|| crate::Error::Validation(format!("{curve_label} requires day_count")))?;
        let style = input.style.ok_or_else(|| {
            crate::Error::Validation(format!("{curve_label} requires interp style"))
        })?;
        let extrapolation = input.extrapolation.ok_or_else(|| {
            crate::Error::Validation(format!("{curve_label} requires extrapolation policy"))
        })?;

        let calendar = validate_curve_clock(curve_label, input.id, day_count, input.calendar_id)?;

        let (knots, values) = if input.transform.is_some()
            && input.points.len() == 1
            && extrapolation == ExtrapolationPolicy::Error
        {
            validate_origin_only(&input.points, curve_label)?;
            (vec![0.0], vec![1.0])
        } else {
            validate_positive_factor_points(input.points, curve_label)?
        };

        let (interp_knots, interp_values) = match &input.transform {
            Some(transform) => {
                validate_positive_factor_points(transform.source_points().to_vec(), curve_label)?
            }
            None => (knots.clone(), values.clone()),
        };

        let interp = style.build_enum(
            interp_knots.into_boxed_slice(),
            interp_values.into_boxed_slice(),
            extrapolation,
        )?;

        if let Some(transform) = &mut input.transform {
            let ctx = DayCountContext {
                calendar: calendar.as_ref().map(|cal| cal.calendar),
                ..DayCountContext::default()
            };
            transform.validate(
                base,
                day_count,
                ctx,
                &interp,
                (&knots, &values),
                curve_label,
            )?;
        }

        Ok(Self {
            base,
            day_count,
            calendar,
            knots: knots.into_boxed_slice(),
            values: values.into_boxed_slice(),
            interp,
            transform: input.transform,
        })
    }

    #[inline]
    pub(crate) fn source_base_date(&self) -> Date {
        self.transform
            .as_ref()
            .map_or(self.base, |tr| tr.source_base_date)
    }

    pub(crate) fn calendar_id(&self) -> Option<&str> {
        self.calendar.as_ref().map(|cal| cal.id.as_str())
    }

    pub(crate) fn day_count_context(&self) -> DayCountContext<'static> {
        DayCountContext {
            calendar: self.calendar.as_ref().map(|cal| cal.calendar),
            ..DayCountContext::default()
        }
    }

    pub(crate) fn knot_points(&self) -> Vec<(f64, f64)> {
        self.knots
            .iter()
            .copied()
            .zip(self.values.iter().copied())
            .collect()
    }

    #[inline]
    pub(crate) fn df(&self, t: f64, curve_label: &str) -> crate::Result<f64> {
        if !t.is_finite() {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::df requires finite time t, got {t}"
            )));
        }
        if let Some(transform) = &self.transform {
            return transform.df_at_source_time(&self.interp, transform.offset + t, curve_label);
        }
        let val = self.interp.interp(t)?;
        if !val.is_finite() || val <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        Ok(val)
    }

    #[inline]
    pub(crate) fn df_on_date(&self, date: Date, curve_label: &str) -> crate::Result<f64> {
        if let Some(transform) = &self.transform {
            let source_t = signed_curve_year_fraction(
                transform.source_base_date,
                date,
                self.day_count,
                self.calendar.as_ref(),
            )?;
            return transform.df_at_source_time(&self.interp, source_t, curve_label);
        }
        let t =
            signed_curve_year_fraction(self.base, date, self.day_count, self.calendar.as_ref())?;
        self.df(t, curve_label)
    }

    #[inline]
    pub(crate) fn instantaneous_forward(&self, t: f64, curve_label: &str) -> crate::Result<f64> {
        if !t.is_finite() {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::instantaneous_forward requires finite time t, got {t}"
            )));
        }
        let (source_time, adj) = match &self.transform {
            Some(tr) => {
                let source_time = tr.offset + t;
                let _ = tr.df_at_source_time(&self.interp, source_time, curve_label)?;
                let first_knot = tr.source_points.first().map_or(0.0, |&(k, _)| k);
                (
                    source_time,
                    tr.adjustment
                        .value_with_left_interior(source_time, first_knot),
                )
            }
            None => (t, 0.0),
        };
        let base_df = self.interp.interp(source_time)?;
        if !base_df.is_finite() || base_df <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        let base_deriv = self.interp.interp_prime(source_time)?;
        let raw = -(base_deriv / base_df) - adj;
        if !raw.is_finite() {
            return Err(crate::Error::Validation(format!(
                "{curve_label} instantaneous_forward is non-finite at t={t}: {raw}"
            )));
        }
        Ok(if exact_f64_eq(raw, 0.0) { 0.0 } else { raw })
    }

    /// Apply an additive bump in-place, returning `true` when mutated.
    pub(crate) fn bump_in_place(
        &mut self,
        curve_label: &str,
        spec: &BumpSpec,
    ) -> crate::Result<bool> {
        let val = spec.resolve_additive_or_error(curve_label)?;
        if exact_f64_eq(val, 0.0) {
            return Ok(false);
        }
        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| FactorTransform::from_parts(self.base, self.knot_points()));
        transform
            .adjustment
            .add_bump(transform.offset, val, spec.bump_type)?;

        let mut values = Vec::with_capacity(self.knots.len());
        for &time in self.knots.iter() {
            values.push(transform.df_at_source_time(
                &self.interp,
                transform.offset + time,
                curve_label,
            )?);
        }

        // Check in-coverage shape breakpoints for triangular key-rate shocks.
        if let BumpType::TriangularKeyRate {
            prev_bucket,
            target_bucket,
            next_bucket,
        } = spec.bump_type
        {
            let min_src = self.transform.as_ref().map_or(self.knots[0], |tr| {
                tr.source_points.first().map_or(0.0, |&(k, _)| k)
            });
            let max_src = self.transform.as_ref().map_or_else(
                || self.knots[self.knots.len() - 1],
                |tr| tr.source_points[tr.source_points.len() - 1].0,
            );
            for rel_t in prev_bucket
                .into_iter()
                .chain([target_bucket])
                .chain(next_bucket)
            {
                let src_t = transform.offset + rel_t;
                let in_bounds = self.interp.extrapolation() != ExtrapolationPolicy::Error
                    || (src_t >= min_src && src_t <= max_src);
                if in_bounds {
                    let _ = transform.df_at_source_time(&self.interp, src_t, curve_label)?;
                }
            }
        }

        self.values = values.into_boxed_slice();
        self.transform = Some(transform);
        Ok(true)
    }

    /// Condition the curve core to `reference_date`, returning `None` when already anchored there.
    pub(crate) fn condition(
        &self,
        curve_label: &str,
        reference_date: Date,
    ) -> crate::Result<Option<Self>> {
        let src_base = self.source_base_date();
        if reference_date < src_base {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::condition requires reference_date ({reference_date}) on or after source base date ({src_base})"
            )));
        }
        let _ = signed_curve_year_fraction(
            src_base,
            reference_date,
            self.day_count,
            self.calendar.as_ref(),
        )?;
        let c_w = self.df_on_date(reference_date, curve_label)?;
        if !c_w.is_finite() || c_w <= 0.0 {
            return Err(crate::InputError::NonPositiveValue.into());
        }
        if reference_date == self.base {
            return Ok(None);
        }

        let mut transform = self
            .transform
            .clone()
            .unwrap_or_else(|| FactorTransform::from_parts(self.base, self.knot_points()));
        transform.condition(
            &self.interp,
            reference_date,
            self.day_count,
            self.day_count_context(),
            curve_label,
        )?;
        let new_offset = transform.offset;

        let mut rolled_points: Vec<(f64, f64)> = Vec::new();
        for &(source_t, _) in &transform.source_points {
            let remaining_t = source_t - new_offset;
            if remaining_t > 0.0 && source_t > new_offset {
                let factor_rem =
                    transform.df_at_source_time(&self.interp, source_t, curve_label)?;
                rolled_points.push((remaining_t, factor_rem));
            }
        }
        if rolled_points.is_empty() && self.interp.extrapolation() != ExtrapolationPolicy::Error {
            let factor_1 =
                transform.df_at_source_time(&self.interp, new_offset + 1.0, curve_label)?;
            rolled_points.push((1.0, factor_1));
        }

        let mut points = Vec::with_capacity(rolled_points.len() + 1);
        points.push((0.0, 1.0));
        points.extend(rolled_points);
        let (knots, values): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();

        let mut conditioned = self.clone();
        conditioned.base = reference_date;
        conditioned.knots = knots.into_boxed_slice();
        conditioned.values = values.into_boxed_slice();
        conditioned.transform = Some(transform);
        Ok(Some(conditioned))
    }
}

/// Validate a single-point conditioned positive-factor curve anchored at `(0.0, 1.0)`.
fn validate_origin_only(points: &[(f64, f64)], curve_label: &str) -> crate::Result<()> {
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
fn validate_positive_factor_points(
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

/// Validate canonical serialized `(0.0, 1.0)` origin and transform pillar invariants.
pub(crate) fn validate_canonical_raw_origin(
    curve_label: &str,
    knot_points: &[(f64, f64)],
    source_points: Option<&[(f64, f64)]>,
) -> crate::Result<()> {
    let first = knot_points.first().ok_or(crate::InputError::TooFewPoints)?;
    if !exact_f64_eq(first.0, 0.0) || !exact_f64_eq(first.1, 1.0) {
        return Err(crate::Error::Validation(format!(
            "canonical {curve_label} state must already start at (0.0, 1.0)"
        )));
    }
    if let Some(src_points) = source_points {
        let src_first = src_points.first().ok_or(crate::InputError::TooFewPoints)?;
        if !exact_f64_eq(src_first.0, 0.0)
            || !exact_f64_eq(src_first.1, 1.0)
            || src_points.len() < 2
        {
            return Err(crate::Error::Validation(format!(
                "canonical {curve_label} transform source_points must contain origin (0.0, 1.0) and at least one positive pillar"
            )));
        }
    } else if knot_points.len() < 2 {
        return Err(crate::InputError::TooFewPoints.into());
    }
    Ok(())
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

/// Map dated quote levels to native times. The owning curve admits the resulting knots.
pub(crate) fn dated_points_to_knots(
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
    for &(date, level) in points {
        if date < base_date {
            return Err(crate::Error::Validation(format!(
                "{curve_label}::from_dates pillar {date} precedes base date {base_date}"
            )));
        }
        let t = signed_curve_year_fraction(base_date, date, day_count, calendar)?;
        knots.push((t, level));
    }
    Ok(knots)
}

#[inline]
fn eval_transformed_factor(
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

/// Define a curve-specific canonical wire struct preserving a distinct schema
/// `$defs` name while sharing [`FactorTransform`] runtime evaluation.
macro_rules! define_factor_transform_wire {
    ($name:ident, $schema_name:literal) => {
        /// Source interpolation and a single accumulated transformation, never a chain
        /// of nested curves. The adjustment is stored as its piecewise-linear derivative
        /// so evaluating beyond a completed triangular shock does not subtract large
        /// quadratic polynomials.
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "json-schema", schemars(rename = $schema_name))]
        #[serde(deny_unknown_fields)]
        struct $name {
            /// Original, untransformed curve base date defining the source coordinate system.
            #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
            source_base_date: $crate::dates::Date,
            /// Original, untransformed interpolation pillars.
            source_points: Vec<(f64, f64)>,
            /// Cumulative derivative of the additive log-factor adjustment.
            adjustment: $crate::market_data::term_structures::common::PiecewiseLinearAdjustment,
        }

        impl From<$crate::market_data::term_structures::common::factor::FactorTransform> for $name {
            fn from(
                tr: $crate::market_data::term_structures::common::factor::FactorTransform,
            ) -> Self {
                Self {
                    source_base_date: tr.source_base_date,
                    source_points: tr.source_points,
                    adjustment: tr.adjustment,
                }
            }
        }

        impl From<$name> for $crate::market_data::term_structures::common::factor::FactorTransform {
            fn from(wire: $name) -> Self {
                Self::from_wire(wire.source_base_date, wire.source_points, wire.adjustment)
            }
        }
    };
}

pub(crate) use define_factor_transform_wire;
