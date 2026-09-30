//! Rolling per-ticker metrics on [`Performance`].

use super::Performance;
use crate::math::summation::NeumaierAccumulator;
use crate::returns::MIN_GROWTH_FACTOR;
use crate::risk_metrics::{
    rolling_sharpe, rolling_sortino, rolling_volatility, DatedSeries, RollingMetric,
    ROLLING_KERNEL_RECOMPUTE_INTERVAL,
};

#[inline]
fn log_factor(r: f64) -> Option<f64> {
    if !r.is_finite() {
        None
    } else {
        Some((1.0 + r).max(MIN_GROWTH_FACTOR).ln())
    }
}

fn recompute_log_sum(window: &[f64]) -> Option<f64> {
    let mut acc = NeumaierAccumulator::new();
    for &r in window {
        acc.add(log_factor(r)?);
    }
    Some(acc.total())
}

impl Performance {
    /// Rolling compounded returns for a specific ticker.
    ///
    /// Computes the total compounded return over each `window`-length slice
    /// of the active return series, right-labelled by the window-end date.
    /// Produces `n - window + 1` values where `n = active_returns.len()`.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `window`     - Look-back window length in periods (e.g. 252 for 1Y daily).
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_returns(&self, ticker_idx: usize, window: usize) -> crate::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        let returns = self.active_returns(ticker_idx);
        let dates = self.active_dates_for_ticker_unchecked(ticker_idx);
        let n = returns.len().min(dates.len());
        if window > n {
            return Ok(DatedSeries::empty(RollingMetric::Return));
        }
        let count = n - window + 1;
        let mut values = Vec::with_capacity(count);
        let mut out_dates = Vec::with_capacity(count);

        // Incremental sliding log-sum. NaN/Inf inside the active window mark
        // the affected outputs as NaN, matching `comp_total`'s contract.
        let mut log_sum = recompute_log_sum(&returns[..window]).unwrap_or(f64::NAN);
        values.push(if log_sum.is_finite() {
            log_sum.exp() - 1.0
        } else {
            f64::NAN
        });
        out_dates.push(dates[window - 1]);

        let mut steps_since_recompute = 0_usize;
        for end in (window + 1)..=n {
            let add = returns[end - 1];
            let rem = returns[end - 1 - window];
            match (log_factor(add), log_factor(rem)) {
                (Some(a), Some(r)) if log_sum.is_finite() => {
                    log_sum += a - r;
                }
                _ => {
                    // A non-finite return entering or leaving the window
                    // forces a full recompute against the next slice.
                    log_sum = f64::NAN;
                }
            }
            steps_since_recompute += 1;
            if !log_sum.is_finite() || steps_since_recompute >= ROLLING_KERNEL_RECOMPUTE_INTERVAL {
                let start = end - window;
                log_sum = recompute_log_sum(&returns[start..end]).unwrap_or(f64::NAN);
                steps_since_recompute = 0;
            }
            values.push(if log_sum.is_finite() {
                log_sum.exp() - 1.0
            } else {
                f64::NAN
            });
            out_dates.push(dates[end - 1]);
        }
        Ok(DatedSeries {
            values,
            dates: out_dates,
            value_column: RollingMetric::Return,
        })
    }

    /// Rolling annualized volatility for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `window`     - Look-back window length in periods.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_volatility(
        &self,
        ticker_idx: usize,
        window: usize,
    ) -> crate::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        Ok(rolling_volatility(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann(),
        ))
    }

    /// Rolling Sortino ratio for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `window`     - Look-back window length in periods.
    /// * `mar`        - Minimum acceptable per-period return (decimal). Pass
    ///   `0.0` for the conventional zero-MAR Sortino, or e.g. a risk-free or
    ///   target rate scaled to the observation frequency.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_sortino(
        &self,
        ticker_idx: usize,
        window: usize,
        mar: f64,
    ) -> crate::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        Ok(rolling_sortino(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann(),
            mar,
        ))
    }

    /// Rolling Sharpe ratio for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx`     - Zero-based column index of the ticker.
    /// * `window`         - Look-back window length in periods.
    /// * `risk_free_rate` - Annualized risk-free rate, geometrically
    ///   decompounded to the panel frequency before subtraction.
    ///
    /// # Returns
    ///
    /// A [`DatedSeries`] with parallel date and Sharpe value vectors.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_sharpe(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> crate::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        Ok(rolling_sharpe(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann(),
            risk_free_rate,
        ))
    }
}
