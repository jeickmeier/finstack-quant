//! Rebased geometric drawdown paths and strictly recovered dated episodes.

use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::{Error, Result};

/// An observed drawdown episode measured from the most recent local peak.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, try_from = "EpisodeWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct DrawdownEpisode {
    /// Actual valuation date of the peak immediately preceding the decline.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub start: Date,
    /// First actual observation attaining the deepest loss in the episode.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub valley: Date,
    /// First observed recovery to zero drawdown; absent for an open episode.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub end: Option<Date>,
    /// Calendar days from peak to recovery, or last observation when open.
    pub duration_days: i64,
    /// Signed decimal peak-to-valley loss in the closed interval `[-1, 0)`.
    pub max_drawdown: f64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EpisodeWire {
    start: Date,
    valley: Date,
    end: Option<Date>,
    duration_days: i64,
    max_drawdown: f64,
}

impl TryFrom<EpisodeWire> for DrawdownEpisode {
    type Error = Error;

    fn try_from(value: EpisodeWire) -> Result<Self> {
        if value.start >= value.valley
            || value.duration_days <= 0
            || !value.max_drawdown.is_finite()
            || !(-1.0..0.0).contains(&value.max_drawdown)
            || value.end.is_some_and(|end| {
                end <= value.valley || (end - value.start).whole_days() != value.duration_days
            })
            || value.duration_days < (value.valley - value.start).whole_days()
        {
            return Err(Error::Validation(
                "invalid drawdown episode dates, duration or depth".into(),
            ));
        }
        Ok(Self {
            start: value.start,
            valley: value.valley,
            end: value.end,
            duration_days: value.duration_days,
            max_drawdown: value.max_drawdown,
        })
    }
}

/// Construct one peak-relative drawdown per observed investment return.
///
/// # Arguments
///
/// * `returns` - Finite simple investment returns no smaller than `-1`;
///   exact `-1` is permitted only as the final observation.
///
/// # Errors
///
/// Rejects invalid return histories and unrepresentable log-wealth arithmetic.
pub(crate) fn to_drawdown_series(returns: &[f64]) -> Result<Vec<f64>> {
    crate::returns::validate_history(returns)?;
    // Accumulate relative to the latest peak, so a tiny loss following a
    // large absolute wealth level is not cancelled by subtracting log peaks.
    let mut relative_log_wealth = NeumaierAccumulator::new();
    let mut path = Vec::with_capacity(returns.len());
    for &value in returns {
        if value <= -1.0 {
            path.push(-1.0);
            continue;
        }
        relative_log_wealth.add(value.ln_1p());
        let wealth = relative_log_wealth.total();
        if !wealth.is_finite() {
            return Err(Error::Internal(
                "drawdown log wealth is not representable".into(),
            ));
        }
        let loss = if wealth >= 0.0 {
            relative_log_wealth = NeumaierAccumulator::new();
            0.0
        } else {
            wealth.exp_m1()
        };
        if !loss.is_finite() || !(-1.0..=0.0).contains(&loss) {
            return Err(Error::Internal("drawdown loss is not representable".into()));
        }
        path.push(loss);
    }
    Ok(path)
}

fn validate_path(drawdown: &[f64]) -> Result<()> {
    if drawdown
        .iter()
        .any(|v| !v.is_finite() || !(-1.0..=0.0).contains(v))
    {
        return Err(Error::Validation(
            "drawdown path must contain finite fractions in [-1, 0]".into(),
        ));
    }
    Ok(())
}

fn validate_dated_path(drawdown: &[f64], dates: &[Date]) -> Result<()> {
    validate_path(drawdown)?;
    if drawdown.len() != dates.len() || dates.windows(2).any(|w| w[0] >= w[1]) {
        return Err(Error::Validation(
            "drawdown dates must exactly align and strictly increase".into(),
        ));
    }
    if !drawdown.is_empty() && drawdown[0] != 0.0 {
        return Err(Error::Validation(
            "dated drawdown path must include its actual opening zero".into(),
        ));
    }
    Ok(())
}

fn for_each_episode<F>(drawdown: &[f64], mut emit: F)
where
    F: FnMut(usize, usize, Option<usize>, f64),
{
    let mut state: Option<(usize, usize, f64)> = None;
    for (i, &value) in drawdown.iter().enumerate() {
        if value < 0.0 {
            match state.as_mut() {
                Some((_, valley, depth)) if value < *depth => {
                    *valley = i;
                    *depth = value;
                }
                None => state = Some((i.saturating_sub(1), i, value)),
                _ => {}
            }
        } else if let Some((start, valley, depth)) = state.take() {
            emit(start, valley, Some(i), depth);
        }
    }
    if let Some((start, valley, depth)) = state {
        emit(start, valley, None, depth);
    }
}

/// Find at most `n` episodes, deepest first and chronological on equal depths.
///
/// # Arguments
///
/// * `drawdown` - Baseline-inclusive drawdown fractions starting at zero.
/// * `dates` - Exactly aligned actual valuation dates in strict order.
/// * `n` - Nonnegative display count; zero yields an empty valid selection.
///
/// # Errors
///
/// Rejects malformed paths, missing opening anchors and invalid date alignment.
pub(crate) fn drawdown_details(
    drawdown: &[f64],
    dates: &[Date],
    n: usize,
) -> Result<Vec<DrawdownEpisode>> {
    validate_dated_path(drawdown, dates)?;
    let mut episodes = Vec::new();
    if drawdown.is_empty() {
        return Ok(episodes);
    }
    for_each_episode(drawdown, |start, valley, end, max_drawdown| {
        let end_date = dates[end.unwrap_or(dates.len() - 1)];
        episodes.push(DrawdownEpisode {
            start: dates[start],
            valley: dates[valley],
            end: end.map(|i| dates[i]),
            duration_days: (end_date - dates[start]).whole_days(),
            max_drawdown,
        });
    });
    episodes.sort_by(|a, b| {
        a.max_drawdown
            .total_cmp(&b.max_drawdown)
            .then(a.start.cmp(&b.start))
            .then(a.valley.cmp(&b.valley))
    });
    episodes.truncate(n);
    Ok(episodes)
}

/// Select deepest episode valleys from an internally validated path.
///
/// # Arguments
///
/// * `drawdown` - Validated drawdown endpoint fractions.
/// * `n` - Maximum number of episode valleys retained.
pub(crate) fn worst_episode_depths(drawdown: &[f64], n: usize) -> Vec<f64> {
    let mut depths = Vec::new();
    for_each_episode(drawdown, |_, _, _, depth| depths.push(depth));
    depths.sort_by(f64::total_cmp);
    depths.truncate(n);
    depths
}

/// Mean of the deepest up-to-`n` episode valleys, zero for no episodes.
///
/// # Arguments
///
/// * `drawdown` - Internally validated endpoint fractions.
/// * `n` - Positive financial episode count validated by the caller.
pub(crate) fn mean_episode_drawdown(drawdown: &[f64], n: usize) -> f64 {
    let depths = worst_episode_depths(drawdown, n);
    if depths.is_empty() {
        return 0.0;
    }
    let count = depths.len() as f64;
    let scale = depths.iter().map(|d| d.abs()).fold(0.0, f64::max);
    let mut sum = NeumaierAccumulator::new();
    for depth in depths {
        sum.add(depth / scale);
    }
    scale * (sum.total() / count)
}

/// Longest recovered or open episode in actual calendar days.
///
/// # Arguments
///
/// * `drawdown` - Baseline-inclusive path with an actual opening zero.
/// * `dates` - Exactly aligned strictly increasing actual valuation dates.
///
/// # Errors
///
/// Rejects invalid paths or chronology even when no episode exists.
pub(crate) fn max_drawdown_duration(drawdown: &[f64], dates: &[Date]) -> Result<Option<i64>> {
    validate_dated_path(drawdown, dates)?;
    if drawdown.len() < 2 {
        return Ok(None);
    }
    Ok(Some(
        drawdown_details(drawdown, dates, usize::MAX)?
            .iter()
            .map(|episode| episode.duration_days)
            .max()
            .unwrap_or(0),
    ))
}

/// Root mean square of validated endpoint drawdowns; empty is undefined.
///
/// # Arguments
///
/// * `drawdown` - Validated fractions in `[-1, 0]`, excluding the opening zero.
pub(crate) fn ulcer_index(drawdown: &[f64]) -> Result<f64> {
    if drawdown.is_empty() {
        return Ok(f64::NAN);
    }
    let scale = drawdown.iter().map(|d| d.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut sum = NeumaierAccumulator::new();
    for &d in drawdown {
        sum.add((d / scale).powi(2));
    }
    nonzero_path_statistic(scale * (sum.total() / drawdown.len() as f64).sqrt())
}

/// Mean absolute validated endpoint drawdown; empty is undefined.
///
/// # Arguments
///
/// * `drawdown` - Validated fractions in `[-1, 0]`, excluding the opening zero.
pub(crate) fn pain_index(drawdown: &[f64]) -> Result<f64> {
    mean_drawdown(drawdown).map(|mean| -mean)
}

/// Mean signed endpoint drawdown, including observed zeros.
///
/// # Arguments
///
/// * `drawdown` - Validated fractions in `[-1, 0]`, excluding the opening zero.
pub(crate) fn mean_drawdown(drawdown: &[f64]) -> Result<f64> {
    if drawdown.is_empty() {
        return Ok(f64::NAN);
    }
    let scale = drawdown.iter().map(|d| d.abs()).fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }
    let mut sum = NeumaierAccumulator::new();
    for &d in drawdown {
        sum.add(d / scale);
    }
    nonzero_path_statistic(scale * (sum.total() / drawdown.len() as f64))
}

fn nonzero_path_statistic(value: f64) -> Result<f64> {
    if !value.is_finite() || value == 0.0 {
        return Err(Error::Internal(
            "nonzero drawdown risk is not representable".into(),
        ));
    }
    Ok(value)
}

/// Deepest validated endpoint loss; an empty history is undefined.
///
/// # Arguments
///
/// * `drawdown` - Validated drawdown endpoint fractions.
pub(crate) fn deepest(drawdown: &[f64]) -> f64 {
    if drawdown.is_empty() {
        return f64::NAN;
    }
    drawdown.iter().copied().fold(0.0, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Duration, Month};

    fn dates(n: usize) -> Vec<Date> {
        let first = Date::from_calendar_date(2025, Month::January, 1).unwrap();
        (0..n).map(|i| first + Duration::days(i as i64)).collect()
    }

    #[test]
    fn tiny_losses_survive_large_prior_wealth_and_scaled_risk() {
        let path = to_drawdown_series(&[f64::MAX, -1e-17]).unwrap();
        assert_eq!(path[0], 0.0);
        assert_eq!(path[1], -1e-17);
        let tiny = -f64::MIN_POSITIVE;
        assert_eq!(ulcer_index(&[tiny, tiny]).unwrap(), -tiny);
        assert_eq!(mean_drawdown(&[tiny, tiny]).unwrap(), tiny);
        assert!(mean_drawdown(&[-f64::from_bits(1), 0.0]).is_err());
    }

    #[test]
    fn episode_restore_rejects_impossible_dates_and_obsolete_metadata() {
        let valid = r#"{"start":"2025-01-01","valley":"2025-01-02","end":null,"duration_days":1,"max_drawdown":-1.0}"#;
        let episode: DrawdownEpisode = serde_json::from_str(valid).unwrap();
        assert_eq!(episode.max_drawdown, -1.0);
        assert!(serde_json::from_str::<DrawdownEpisode>(&valid.replace("-1.0", "0.0")).is_err());
        assert!(
            serde_json::from_str::<DrawdownEpisode>(
                &valid.replace("\"end\":null", "\"end\":\"2025-01-01\"")
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<DrawdownEpisode>(&valid.replace(
                "\"end\":null",
                "\"near_recovery_threshold\":-0.01,\"end\":null"
            ))
            .is_err()
        );
    }

    #[test]
    fn strict_recovery_preserves_tiny_unrecovered_loss_and_first_valley() {
        let path = [0.0, -0.1, -0.2, -0.2, -1e-16, 0.0, -0.05];
        let ds = dates(path.len());
        let episodes = drawdown_details(&path, &ds, 10).unwrap();
        assert_eq!(episodes.len(), 2);
        assert_eq!(episodes[0].valley, ds[2]);
        assert_eq!(episodes[0].end, Some(ds[5]));
        assert_eq!(episodes[0].duration_days, 5);
        assert_eq!(episodes[1].end, None);
    }

    #[test]
    fn selection_validates_shapes_before_zero_count() {
        assert!(drawdown_details(&[0.0, -0.1], &dates(3), 0).is_err());
        assert!(drawdown_details(&[0.0, -0.1], &[dates(2)[0]; 2], 0).is_err());
        assert!(
            drawdown_details(&[0.0, -0.1], &dates(2), 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn terminal_loss_is_exact_and_cannot_continue() {
        assert_eq!(to_drawdown_series(&[0.1, -1.0]).unwrap(), [0.0, -1.0]);
        assert!(to_drawdown_series(&[-1.0, 0.1]).is_err());
        assert!(to_drawdown_series(&[-1.1]).is_err());
        assert!(to_drawdown_series(&[f64::NAN]).is_err());
    }

    #[test]
    fn endpoint_statistics_exclude_baseline_and_empty_is_unavailable() {
        let path = [0.0, -0.1, -0.2, 0.0, -0.05];
        assert!((ulcer_index(&path).unwrap() - (0.0525_f64 / 5.0).sqrt()).abs() < 1e-15);
        assert!((pain_index(&path).unwrap() - 0.07).abs() < 1e-15);
        assert!(deepest(&[]).is_nan());
        assert!(ulcer_index(&[]).unwrap().is_nan());
        assert_eq!(max_drawdown_duration(&[], &[]).unwrap(), None);
        assert_eq!(
            max_drawdown_duration(&[0.0, 0.0], &dates(2)).unwrap(),
            Some(0)
        );
    }
}
