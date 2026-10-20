//! Rolling per-ticker metrics on [`Performance`].

use super::Performance;
use crate::risk_metrics::{
    rolling_returns, rolling_sharpe, rolling_sortino, rolling_volatility, DatedSeries,
};

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
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_returns(&self, ticker_idx: usize, window: usize) -> crate::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        Ok(rolling_returns(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
        ))
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
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
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
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
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
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
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
