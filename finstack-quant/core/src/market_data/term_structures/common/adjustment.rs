//! Merged piecewise-linear functions for exact continuous curve adjustments.

use crate::market_data::bumps::BumpType;
use crate::math::NeumaierAccumulator;

/// A flat function representation: repeated shocks merge on a breakpoint union
/// rather than forming a recursively nested transformation history.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(crate) struct PiecewiseLinearAdjustment {
    /// Constant value before the first breakpoint.
    initial_value: f64,
    /// Sorted, merged linear segments.
    segments: Vec<Segment>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(rename = "CurveAdjustmentSegment"))]
#[serde(deny_unknown_fields)]
struct Segment {
    /// Segment origin in the source curve's time coordinates.
    start: f64,
    /// Right-hand function value at the segment origin.
    value: f64,
    /// Function slope on this segment.
    slope: f64,
}

impl PiecewiseLinearAdjustment {
    fn coefficients(&self, x: f64) -> (f64, f64) {
        let index = self.segments.partition_point(|segment| segment.start <= x);
        if index == 0 {
            (self.initial_value, 0.0)
        } else {
            let segment = &self.segments[index - 1];
            (
                segment.value + segment.slope * (x - segment.start),
                segment.slope,
            )
        }
    }

    pub(crate) fn value(&self, x: f64) -> f64 {
        self.coefficients(x).0
    }

    pub(crate) fn integral(&self, from: f64, to: f64) -> f64 {
        if from.total_cmp(&to).is_eq() {
            return 0.0;
        }
        if to < from {
            return -self.integral(to, from);
        }
        let mut sum = NeumaierAccumulator::new();
        let mut current = from;
        let (mut value, mut slope) = self.coefficients(from);
        let first = self
            .segments
            .partition_point(|segment| segment.start <= from);
        for segment in &self.segments[first..] {
            if segment.start >= to {
                break;
            }
            let width = segment.start - current;
            sum.add(width * (value + 0.5 * slope * width));
            current = segment.start;
            value = segment.value;
            slope = segment.slope;
        }
        let width = to - current;
        sum.add(width * (value + 0.5 * slope * width));
        sum.total()
    }

    pub(crate) fn scale(&mut self, factor: f64) -> crate::Result<()> {
        self.initial_value *= factor;
        for segment in &mut self.segments {
            segment.value *= factor;
            segment.slope *= factor;
        }
        self.validate()
    }

    fn add_constant(&mut self, value: f64) -> crate::Result<()> {
        self.initial_value += value;
        for segment in &mut self.segments {
            segment.value += value;
        }
        self.validate()
    }

    /// Add the derivative of -rate*t*w(t), whose integral is the continuously
    /// compounded log-discount shock. Coordinates are relative to `offset`.
    pub(crate) fn add_zero_rate_bump(
        &mut self,
        offset: f64,
        rate: f64,
        shape: BumpType,
    ) -> crate::Result<()> {
        let BumpType::TriangularKeyRate {
            prev_bucket: prev,
            target_bucket: target,
            next_bucket: next,
        } = shape
        else {
            return self.add_constant(-rate);
        };
        super::validate_triangular_bucket_grid(prev, target, next)?;
        let coefficients = |x: f64| {
            let t = x - offset;
            if let Some(p) = prev {
                if x < offset + p {
                    return (0.0, 0.0);
                }
                if x < offset + target {
                    let coefficient = -rate / (target - p);
                    return (coefficient * (2.0 * t - p), 2.0 * coefficient);
                }
            } else if x < offset + target {
                return (-rate, 0.0);
            }
            if let Some(n) = next {
                if x < offset + n {
                    let coefficient = rate / (n - target);
                    (coefficient * (2.0 * t - n), 2.0 * coefficient)
                } else {
                    (0.0, 0.0)
                }
            } else {
                (-rate, 0.0)
            }
        };
        self.merge(
            if prev.is_none() { -rate } else { 0.0 },
            prev.into_iter()
                .chain([target])
                .chain(next)
                .map(|t| offset + t),
            coefficients,
        )
    }

    /// Add a continuous rate shock rate*w(t), including flat wing buckets.
    pub(crate) fn add_forward_rate_bump(
        &mut self,
        offset: f64,
        rate: f64,
        shape: BumpType,
    ) -> crate::Result<()> {
        let BumpType::TriangularKeyRate {
            prev_bucket: prev,
            target_bucket: target,
            next_bucket: next,
        } = shape
        else {
            return self.add_constant(rate);
        };
        super::validate_triangular_bucket_grid(prev, target, next)?;
        let coefficients = |x: f64| {
            let t = x - offset;
            if let Some(p) = prev {
                if x < offset + p {
                    return (0.0, 0.0);
                }
                if x < offset + target {
                    let slope = rate / (target - p);
                    return (slope * (t - p), slope);
                }
            } else if x < offset + target {
                return (rate, 0.0);
            }
            if let Some(n) = next {
                if x < offset + n {
                    let slope = -rate / (n - target);
                    (-slope * (n - t), slope)
                } else {
                    (0.0, 0.0)
                }
            } else {
                (rate, 0.0)
            }
        };
        self.merge(
            if prev.is_none() { rate } else { 0.0 },
            prev.into_iter()
                .chain([target])
                .chain(next)
                .map(|t| offset + t),
            coefficients,
        )
    }

    fn merge(
        &mut self,
        initial: f64,
        extra: impl Iterator<Item = f64>,
        coefficients: impl Fn(f64) -> (f64, f64),
    ) -> crate::Result<()> {
        let mut breakpoints: Vec<f64> = self.segments.iter().map(|segment| segment.start).collect();
        breakpoints.extend(extra);
        if breakpoints.iter().any(|time| !time.is_finite()) {
            return Err(crate::error::InputError::Invalid.into());
        }
        breakpoints.sort_by(f64::total_cmp);
        breakpoints.dedup();
        let initial_value = self.initial_value + initial;
        let mut segments: Vec<Segment> = Vec::with_capacity(breakpoints.len());
        for start in breakpoints {
            let (old_value, old_slope) = self.coefficients(start);
            let (added_value, added_slope) = coefficients(start);
            let value = old_value + added_value;
            let slope = old_slope + added_slope;
            let (previous_value, previous_slope) =
                segments.last().map_or((initial_value, 0.0), |previous| {
                    (
                        previous.value + previous.slope * (start - previous.start),
                        previous.slope,
                    )
                });
            if !value.total_cmp(&previous_value).is_eq()
                || !slope.total_cmp(&previous_slope).is_eq()
            {
                segments.push(Segment {
                    start,
                    value,
                    slope,
                });
            }
        }
        self.initial_value = initial_value;
        self.segments = segments;
        self.validate()
    }

    pub(crate) fn validate(&self) -> crate::Result<()> {
        if !self.initial_value.is_finite()
            || self.segments.iter().any(|segment| {
                !segment.start.is_finite()
                    || !segment.value.is_finite()
                    || !segment.slope.is_finite()
            })
            || self
                .segments
                .windows(2)
                .any(|pair| pair[0].start >= pair[1].start)
        {
            return Err(crate::Error::Validation(
                "invalid piecewise-linear curve adjustment".into(),
            ));
        }
        Ok(())
    }
}
