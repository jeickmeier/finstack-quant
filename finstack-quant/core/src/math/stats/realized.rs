//! Realized-variance estimators from close and OHLC price series.

use super::{MAX_EXACT_COUNT, variance};
use crate::math::{log_ratio::checked_log_cross_ratio, summation::neumaier_sum};

/// Methods for calculating realized variance from price series.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RealizedVarMethod {
    /// Standard close-to-close returns (market standard default)
    #[default]
    CloseToClose,
    /// Parkinson (1980) high-low estimator
    Parkinson,
    /// Garman-Klass (1980) OHLC estimator
    GarmanKlass,
    /// Rogers-Satchell (1991) drift-independent OHLC estimator
    RogersSatchell,
    /// Yang-Zhang (2000) drift and opening gaps estimator
    YangZhang,
}

/// Displays the serde name (`close_to_close`, `yang_zhang`, ...).
impl std::fmt::Display for RealizedVarMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::wire::serde_label(self).map_err(|_| std::fmt::Error)?)
    }
}

impl RealizedVarMethod {
    /// Returns `true` if this method requires OHLC data.
    pub fn requires_ohlc(self) -> bool {
        !matches!(self, Self::CloseToClose)
    }

    /// Estimator [`realized_variance_ohlc`] uses when no method is given.
    ///
    /// Yang-Zhang (2000) is the minimum-variance OHLC estimator that is robust
    /// to both drift and opening gaps, so it is the natural choice once all
    /// four bar fields are supplied. The close-series entry point
    /// [`realized_variance`] instead defaults to [`RealizedVarMethod::default`]
    /// (`CloseToClose`), the only method it supports.
    pub const OHLC_DEFAULT: Self = Self::YangZhang;
}

/// Annualization factor applied when a realized-variance caller passes none:
/// [`PeriodKind::Daily`](crate::dates::PeriodKind::Daily) observations, i.e.
/// 252 trading days per year.
fn default_annualization_factor() -> f64 {
    f64::from(crate::dates::PeriodKind::Daily.annualization_factor())
}

/// Calculate log returns from a price series.
///
/// Computes each adjacent log ratio, preserving close prices and wide price ranges.
/// Empty and singleton series yield an empty vector after input validation.
///
/// # Arguments
///
/// * `prices` - Finite, strictly positive price levels in chronological order.
///
/// # Errors
///
/// Returns a validation error if any supplied level is nonpositive or nonfinite.
pub fn log_returns(prices: &[f64]) -> crate::Result<Vec<f64>> {
    validate_prices(prices)?;
    Ok(prices.windows(2).map(|w| log_ratio(w[1], w[0])).collect())
}

fn validate_prices(prices: &[f64]) -> crate::Result<()> {
    if prices.iter().any(|&p| !p.is_finite() || p <= 0.0) {
        return Err(crate::Error::Validation(
            "prices must be finite and positive".into(),
        ));
    }
    Ok(())
}

// Price admission is complete at each public entry point.
fn log_ratio(x: f64, y: f64) -> f64 {
    checked_log_cross_ratio(x, 1.0, y, 1.0)
}

fn annualize(per_observation: f64, factor: f64) -> crate::Result<Option<f64>> {
    let result = per_observation * factor;
    if !result.is_finite() || result < 0.0 || (result == 0.0 && per_observation != 0.0) {
        return Err(crate::Error::Internal(
            "annualized variance outside finite representable range".into(),
        ));
    }
    Ok(Some(result))
}

/// Calculate annualized close-to-close realized variance from a close price series.
///
/// Only the `CloseToClose` method is supported; OHLC-based estimators
/// (`Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`) require
/// intraday high/low/open data and must be called via
/// [`realized_variance_ohlc`].
///
/// For `CloseToClose`, this function returns the annualized mean of squared log
/// returns. That is a close-to-close second moment estimator, not a de-meaned
/// sample variance. The name follows common market terminology, but callers
/// comparing against statistical variance formulas should account for that
/// convention explicitly.
///
/// # Arguments
/// * `prices` - Close price series ordered in time; every price must be finite
///   and strictly positive.
/// * `annualization_factor` - Observations per year used to annualize the
///   per-period variance (for example `252.0` for daily closes); must be
///   finite and positive. `None` selects the daily convention,
///   `PeriodKind::Daily.annualization_factor()` (252).
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if the annualization
/// factor is not finite and positive, or a price is not finite and positive.
///
/// # Returns
/// Annualized realized variance, or `None` when fewer than two prices are available.
pub fn realized_variance(
    prices: &[f64],
    annualization_factor: Option<f64>,
) -> crate::Result<Option<f64>> {
    let annualization_factor = annualization_factor.unwrap_or_else(default_annualization_factor);
    if !annualization_factor.is_finite() || annualization_factor <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "realized_variance: annualization_factor must be positive and finite, got {annualization_factor}"
        )));
    }
    validate_prices(prices)?;
    let intervals = prices.len().saturating_sub(1);
    if intervals > MAX_EXACT_COUNT {
        return Err(crate::Error::Validation("too many return intervals".into()));
    }
    if intervals == 0 {
        return Ok(None);
    }
    let mean_square =
        neumaier_sum(prices.windows(2).map(|w| log_ratio(w[1], w[0]).powi(2)))? / intervals as f64;
    annualize(mean_square, annualization_factor)
}

/// Calculate realized variance from OHLC data using advanced estimators.
///
/// All five methods are supported. For `CloseToClose`, only the `close` slice
/// enters its return calculation. All supplied bars are validated.
///
/// # Arguments
/// * `open` - Opening prices (required for `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `high` - High prices (required for `Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `low` - Low prices (required for `Parkinson`, `GarmanKlass`, `RogersSatchell`, `YangZhang`)
/// * `close` - Closing prices (required for all methods)
/// * `previous_close` - Finite positive close immediately preceding the first target bar for close-to-close and Yang–Zhang. Other estimators require `None`.
/// * `method` - Estimator to use. `None` selects
///   [`RealizedVarMethod::OHLC_DEFAULT`] (Yang-Zhang).
/// * `annualization_factor` - Bars per year used to annualize the per-bar
///   variance (for example `252.0` for daily bars); must be finite and
///   positive. `None` selects the daily convention,
///   `PeriodKind::Daily.annualization_factor()` (252).
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if the four slices
/// have different lengths, the annualization factor is not finite and
/// positive, or a bar is not finite, positive and internally consistent.
///
/// # Returns
/// Annualized realized variance. Parkinson, Garman-Klass, and Rogers-Satchell
/// use each complete OHLC bar, including a single bar. Close-to-close requires
/// one target bar and Yang–Zhang requires two. Shorter samples return `None`.
pub fn realized_variance_ohlc(
    open: &[f64],
    high: &[f64],
    low: &[f64],
    close: &[f64],
    method: Option<RealizedVarMethod>,
    annualization_factor: Option<f64>,
    previous_close: Option<f64>,
) -> crate::Result<Option<f64>> {
    let method = method.unwrap_or(RealizedVarMethod::OHLC_DEFAULT);
    let annualization_factor = annualization_factor.unwrap_or_else(default_annualization_factor);
    let n = close.len();
    if open.len() != n || high.len() != n || low.len() != n {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc: open ({}), high ({}), low ({}) and close ({}) \
             series must have the same length",
            open.len(),
            high.len(),
            low.len(),
            n
        )));
    }
    if !annualization_factor.is_finite() || annualization_factor <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc: annualization_factor must be positive and finite, got {annualization_factor}"
        )));
    }
    for i in 0..n {
        let (o, h, l, c) = (open[i], high[i], low[i], close[i]);
        if !o.is_finite()
            || !h.is_finite()
            || !l.is_finite()
            || !c.is_finite()
            || o <= 0.0
            || h <= 0.0
            || l <= 0.0
            || c <= 0.0
        {
            return Err(crate::Error::Validation(format!(
                "realized_variance_ohlc: bar {i} must contain finite positive OHLC values"
            )));
        }
        if l > o.min(c) || h < o.max(c) || l > h {
            return Err(crate::Error::Validation(format!(
                "realized_variance_ohlc: bar {i} violates low <= min(open, close) <= max(open, close) <= high"
            )));
        }
    }
    let anchored = matches!(
        method,
        RealizedVarMethod::CloseToClose | RealizedVarMethod::YangZhang
    );
    if anchored != previous_close.is_some()
        || previous_close.is_some_and(|p| !p.is_finite() || p <= 0.0)
    {
        return Err(crate::Error::Validation(format!(
            "previous_close must be finite and positive for close-to-close and Yang-Zhang, and absent for range estimators (method {method:?})"
        )));
    }
    if n == 0 || (method == RealizedVarMethod::YangZhang && n < 2) {
        return Ok(None);
    }

    if n > MAX_EXACT_COUNT {
        return Err(crate::Error::Validation(
            "too many OHLC observations".into(),
        ));
    }
    if method == RealizedVarMethod::CloseToClose {
        let previous = previous_close.into_iter().chain(close.iter().copied());
        let sum = neumaier_sum(
            close
                .iter()
                .zip(previous)
                .map(|(&c, p)| log_ratio(c, p).powi(2)),
        )?;
        return annualize(sum / n as f64, annualization_factor);
    }

    // For each bar, a and b are the upper and lower wick log lengths, d is the body.
    // All range contributions, including Eq. 19a of Garman and Klass, are nonnegative.
    let ranges = (0..n).map(|i| {
        let upper = open[i].max(close[i]);
        let lower = open[i].min(close[i]);
        let a = log_ratio(high[i], upper);
        let b = log_ratio(lower, low[i]);
        let d = log_ratio(upper, lower);
        let s = a + b;
        match method {
            RealizedVarMethod::Parkinson => (s + d).powi(2) / (4.0 * std::f64::consts::LN_2),
            RealizedVarMethod::GarmanKlass => {
                0.5 * s * s + s * d + (1.5 - 2.0 * std::f64::consts::LN_2) * d * d
            }
            _ => a * a + b * b + d * s,
        }
    });
    let range = neumaier_sum(ranges)? / n as f64;
    if method != RealizedVarMethod::YangZhang {
        return annualize(range, annualization_factor);
    }

    let previous = previous_close.into_iter().chain(close.iter().copied());
    let overnight: Vec<_> = open
        .iter()
        .zip(previous)
        .map(|(&o, p)| log_ratio(o, p))
        .collect();
    let intraday: Vec<_> = close
        .iter()
        .zip(open)
        .map(|(&c, &o)| log_ratio(c, o))
        .collect();
    let k = 0.34 / (2.34 + 2.0 / (n - 1) as f64);
    let (Some(vo), Some(vc)) = (variance(&overnight)?, variance(&intraday)?) else {
        return Err(crate::Error::Internal(
            "Yang-Zhang requires two observed intervals".into(),
        ));
    };
    annualize(vo + k * vc + (1.0 - k) * range, annualization_factor)
}
