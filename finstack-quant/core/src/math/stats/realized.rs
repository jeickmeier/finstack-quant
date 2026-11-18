//! Realized-variance estimators from close and OHLC price series.

use super::variance;

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
/// Each element is `ln(p_t / p_{t-1})` when both prices are finite and strictly
/// positive. Windows with a non-positive prior price, non-finite values, or a
/// non-positive ratio (including total wipeout to a zero price) return
/// [`f64::NAN`] so downstream variance estimates do not silently absorb
/// infinities.
///
/// # Arguments
///
/// * `prices` - Chronologically ordered price levels. Adjacent prices must be
///   finite and strictly positive to produce a finite return.
pub fn log_returns(prices: &[f64]) -> Vec<f64> {
    if prices.len() < 2 {
        return vec![];
    }
    prices
        .windows(2)
        .map(|w| {
            let p0 = w[0];
            let p1 = w[1];
            if p0 <= 0.0 || !p0.is_finite() || !p1.is_finite() {
                return f64::NAN;
            }
            let ratio = p1 / p0;
            if !ratio.is_finite() || ratio <= 0.0 {
                f64::NAN
            } else {
                ratio.ln()
            }
        })
        .collect()
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
/// Annualized realized variance
pub fn realized_variance(prices: &[f64], annualization_factor: Option<f64>) -> crate::Result<f64> {
    let annualization_factor = annualization_factor.unwrap_or_else(default_annualization_factor);
    if !annualization_factor.is_finite() || annualization_factor <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "realized_variance: annualization_factor must be positive and finite, got {annualization_factor}"
        )));
    }
    if prices
        .iter()
        .any(|price| !price.is_finite() || *price <= 0.0)
    {
        return Err(crate::Error::Validation(
            "realized_variance: prices must be finite and positive".into(),
        ));
    }
    if prices.len() < 2 {
        return Ok(0.0);
    }

    let returns = log_returns(prices);
    let var =
        returns.iter().map(|r| r * r).sum::<f64>() / returns.len() as f64 * annualization_factor;
    if !var.is_finite() || var < 0.0 {
        return Err(crate::Error::Validation(
            "realized_variance: non-positive or non-finite prices produce undefined log returns"
                .into(),
        ));
    }
    Ok(var)
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

    let result = match method {
        RealizedVarMethod::CloseToClose => {
            let mut levels = Vec::with_capacity(n + 1);
            levels.extend(previous_close);
            levels.extend_from_slice(close);
            realized_variance(&levels, Some(annualization_factor))
        }
        RealizedVarMethod::Parkinson => {
            // Parkinson (1980) high-low range estimator
            // More efficient than close-to-close, using intraday range information
            //
            // Formula: σ² = [1/(4·ln(2))] · (1/n) · Σ[ln(H/L)]²
            //
            // Reference: Parkinson, M. (1980). "The Extreme Value Method for
            // Estimating the Variance of the Rate of Return."
            // Journal of Business, 53(1), 61-65.
            let sum: f64 = (0..n)
                .map(|i| {
                    let hl_ratio = high[i] / low[i];
                    (hl_ratio.ln()).powi(2)
                })
                .sum();
            let factor = 1.0 / (4.0 * std::f64::consts::LN_2);
            Ok((sum / n as f64) * factor * annualization_factor)
        }
        RealizedVarMethod::GarmanKlass => {
            // Garman-Klass (1980) OHLC estimator
            // Extends Parkinson by incorporating open and close prices for improved efficiency
            //
            // Formula: σ² = (1/n) · Σ[0.5·[ln(H/L)]² - (2·ln(2) - 1)·[ln(C/O)]²]
            //
            // The coefficient (2·ln(2) - 1) ≈ 0.386 is the optimal weight for the
            // close-open component under the assumption of Brownian motion.
            //
            // Reference: Garman, M. B., & Klass, M. J. (1980). "On the Estimation of
            // Security Price Volatilities from Historical Data."
            // Journal of Business, 53(1), 67-78.
            let gk_coeff = 2.0 * std::f64::consts::LN_2 - 1.0;
            let sum: f64 = (0..n)
                .map(|i| {
                    let hl = (high[i] / low[i]).ln();
                    let co = (close[i] / open[i]).ln();
                    0.5 * hl.powi(2) - gk_coeff * co.powi(2)
                })
                .sum();
            Ok((sum / n as f64) * annualization_factor)
        }
        RealizedVarMethod::RogersSatchell => {
            // Rogers-Satchell (1991) drift-independent OHLC estimator
            // Allows for non-zero drift, making it more robust than Parkinson or Garman-Klass
            // when the underlying asset has significant directional movement.
            //
            // Formula: σ² = (1/n) · Σ[ln(H/C)·ln(H/O) + ln(L/C)·ln(L/O)]
            //
            // Reference: Rogers, L. C. G., & Satchell, S. E. (1991). "Estimating Variance
            // From High, Low and Closing Prices." Annals of Applied Probability, 1(4), 504-512.
            let sum: f64 = (0..n)
                .map(|i| {
                    let hc = (high[i] / close[i]).ln();
                    let ho = (high[i] / open[i]).ln();
                    let lc = (low[i] / close[i]).ln();
                    let lo = (low[i] / open[i]).ln();
                    hc * ho + lc * lo
                })
                .sum();
            Ok((sum / n as f64) * annualization_factor)
        }
        RealizedVarMethod::YangZhang => {
            // Yang-Zhang (2000) estimator: includes overnight jumps and opening gaps
            // Combines overnight variance with Rogers-Satchell intraday variance
            // using optimal weighting to minimize bias and variance of the estimator.
            //
            // Reference: Yang, D., & Zhang, Q. (2000). "Drift-Independent Volatility
            // Estimation Based on High, Low, Open, and Close Prices."
            // Journal of Business, 73(3), 477-491.

            /// Yang-Zhang optimal weight numerator for combining variance components
            const YANG_ZHANG_K_NUMERATOR: f64 = 0.34;
            /// Yang-Zhang optimal weight denominator base adjustment
            const YANG_ZHANG_K_DENOMINATOR_BASE: f64 = 1.34;

            let mut overnight_returns = Vec::with_capacity(n);
            let mut open_close_returns = Vec::with_capacity(n);
            let mut sum_rs = 0.0;

            for (i, previous) in previous_close
                .into_iter()
                .chain(close.iter().copied())
                .take(n)
                .enumerate()
            {
                let ratio = open[i] / previous;
                let overnight = if ratio.is_finite() && ratio > 0.0 {
                    ratio.ln()
                } else {
                    open[i].ln() - previous.ln()
                };
                overnight_returns.push(overnight);

                // Open-to-close component
                let open_close = (close[i] / open[i]).ln();
                open_close_returns.push(open_close);

                // Rogers-Satchell component for intraday
                let hc = (high[i] / close[i]).ln();
                let ho = (high[i] / open[i]).ln();
                let lc = (low[i] / close[i]).ln();
                let lo = (low[i] / open[i]).ln();
                sum_rs += hc * ho + lc * lo;
            }

            let m = n;
            let k = YANG_ZHANG_K_NUMERATOR
                / (YANG_ZHANG_K_DENOMINATOR_BASE + (m + 1) as f64 / (m - 1) as f64);
            let var_overnight = variance(&overnight_returns)?.ok_or_else(|| {
                crate::Error::Internal(
                    "realized_variance_ohlc requires at least two overnight returns".into(),
                )
            })?;
            let var_open_close = variance(&open_close_returns)?.ok_or_else(|| {
                crate::Error::Internal(
                    "realized_variance_ohlc requires at least two open-close returns".into(),
                )
            })?;
            let var_rs = sum_rs / m as f64;

            Ok((var_overnight + k * var_open_close + (1.0 - k) * var_rs) * annualization_factor)
        }
    }?;
    if !result.is_finite() || result < -1.0e-14 {
        return Err(crate::Error::Validation(format!(
            "realized_variance_ohlc produced invalid variance {result}"
        )));
    }
    Ok(Some(result.max(0.0)))
}
