//! Rolling per-ticker metrics on [`Performance`].

use super::Performance;
use crate::risk_metrics::{
    DatedSeries, rolling_returns, rolling_sharpe, rolling_sortino, rolling_volatility,
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
    /// * `window` - Explicit positive observation count. Only complete trailing
    ///   windows within the active ticker span are emitted; an oversized count
    ///   produces an empty series.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid ticker or zero window, and a
    /// calculation error if a requested compounded return cannot be represented.
    pub fn rolling_returns(
        &self,
        ticker_idx: usize,
        window: usize,
    ) -> finstack_quant_core::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        rolling_returns(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
        )
    }

    /// Rolling annualized volatility for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `window` - Explicit positive number of observations per complete
    ///   trailing window. A singleton window emits aligned undefined values.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid ticker, zero window or missing
    /// observation frequency. Numerical evaluation or output-range failure
    /// returns a calculation error.
    pub fn rolling_volatility(
        &self,
        ticker_idx: usize,
        window: usize,
    ) -> finstack_quant_core::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        rolling_volatility(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann()?,
        )
    }

    /// Rolling Sortino ratio for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based column index of the ticker.
    /// * `window` - Explicit positive number of observations per complete
    ///   trailing window. A singleton window emits aligned undefined values.
    /// * `mar`        - Minimum acceptable per-period return (decimal). Pass
    ///   `0.0` for the conventional zero-MAR Sortino, or e.g. a risk-free or
    ///   target rate scaled to the observation frequency.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid ticker, zero window, nonfinite
    /// `mar` or missing observation frequency. Numerical evaluation or
    /// output-range failure returns a calculation error.
    pub fn rolling_sortino(
        &self,
        ticker_idx: usize,
        window: usize,
        mar: f64,
    ) -> finstack_quant_core::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        rolling_sortino(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann()?,
            mar,
        )
    }

    /// Rolling Sharpe ratio for a specific ticker.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx`     - Zero-based column index of the ticker.
    /// * `window` - Explicit positive number of observations per complete
    ///   trailing window. A singleton window emits aligned undefined values.
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash
    ///   return strictly greater than -1, geometrically decompounded to the
    ///   declared observation frequency before subtraction.
    ///
    /// # Returns
    ///
    /// A [`DatedSeries`] with parallel date and Sharpe value vectors.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid ticker, zero window, invalid
    /// cash return or missing observation frequency. Numerical evaluation or
    /// output-range failure returns a calculation error.
    pub fn rolling_sharpe(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> finstack_quant_core::Result<DatedSeries> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        rolling_sharpe(
            self.active_returns(ticker_idx),
            self.active_dates_for_ticker_unchecked(ticker_idx),
            window,
            self.ann()?,
            risk_free_rate,
        )
    }
}
