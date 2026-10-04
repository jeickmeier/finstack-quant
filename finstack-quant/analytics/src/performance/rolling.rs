//! Rolling per-ticker metrics on [`Performance`].

use super::Performance;
use crate::math::summation::NeumaierAccumulator;
use crate::risk_metrics::{
    rolling_sharpe, rolling_sortino, rolling_volatility, DatedSeries, RollingMetric,
    ROLLING_KERNEL_RECOMPUTE_INTERVAL,
};

fn recompute_log_sum(window: &[f64]) -> NeumaierAccumulator {
    let mut acc = NeumaierAccumulator::new();
    for &r in window {
        acc.add(r.ln_1p());
    }
    acc
}

impl Performance {
    /// Rolling compounded returns for a specific ticker.
    ///
    /// Computes the total compounded return over each `window`-length slice
    /// of the active return series, right-labelled by the window-end date.
    /// Produces `n - window + 1` values where `n = active_returns.len()`.
    /// Compensated log-growth updates preserve small returns, matching
    /// [`Self::cumulative_returns`] for each individual window.
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

        // Construction and deserialization guarantee finite returns > -1.
        // Update their log growth directly, retaining numerical recovery.
        let mut log_sum = recompute_log_sum(&returns[..window]);
        let total = log_sum.total();
        values.push(if total.is_finite() {
            total.exp_m1()
        } else {
            f64::NAN
        });
        out_dates.push(dates[window - 1]);

        let mut steps_since_recompute = 0_usize;
        for end in (window + 1)..=n {
            let add = returns[end - 1];
            let rem = returns[end - 1 - window];
            log_sum.add(add.ln_1p());
            log_sum.add(-rem.ln_1p());
            steps_since_recompute += 1;
            if !log_sum.total().is_finite()
                || steps_since_recompute >= ROLLING_KERNEL_RECOMPUTE_INTERVAL
            {
                let start = end - window;
                log_sum = recompute_log_sum(&returns[start..end]);
                steps_since_recompute = 0;
            }
            let total = log_sum.total();
            values.push(if total.is_finite() {
                total.exp_m1()
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
