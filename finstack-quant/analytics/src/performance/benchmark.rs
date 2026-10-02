//! Benchmark-relative metrics on [`Performance`] (alpha/beta/greeks,
//! tracking error, information ratio, R², capture, multi-factor).

use super::Performance;
use crate::benchmark::{
    batting_average, beta, beta_only, capture_ratio, down_capture, greeks, information_ratio,
    m_squared, multi_factor_greeks, r_squared, rolling_greeks, tracking_error, treynor, up_capture,
    BetaResult, GreeksResult, MultiFactorResult, ReturnKind, RollingGreeks,
};
use crate::risk_metrics;

impl Performance {
    /// Treynor ratio for each ticker.
    ///
    /// Excess return uses the same geometric rf decompounding as
    /// [`Self::sharpe`]: `(mean(r) − rf_period) × N`.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form.
    ///   Decompounded to the panel frequency before subtraction.
    ///
    /// # Returns
    ///
    /// One Treynor ratio per ticker in column order, using the active
    /// benchmark to estimate beta. Returns [`f64::NAN`] when the overlapping
    /// ticker/benchmark window has fewer than 2 observations (beta is not
    /// estimable from one point), the benchmark variance is zero, or the
    /// annualized excess return is non-finite (including a non-finite
    /// `risk_free_rate`). For an estimable beta with magnitude below `1e-10`,
    /// finite excess return gives signed infinity, or zero when excess return
    /// is exactly zero.
    pub fn treynor(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            if r.len().min(bench.len()) < 2 {
                return f64::NAN;
            }
            let ann_ret = risk_metrics::mean_return(r, true, ann);
            let beta = beta_only(r, bench);
            treynor(ann_ret, risk_free_rate, beta, ann)
        })
    }

    /// Empyrical-style annualized geometric up-capture for each ticker versus the active benchmark.
    pub fn up_capture(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            up_capture(r, bench, self.ann())
        })
    }

    /// Empyrical-style annualized geometric down-capture for each ticker versus the active benchmark.
    pub fn down_capture(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            down_capture(r, bench, self.ann())
        })
    }

    /// Empyrical-style annualized geometric capture ratio for each ticker.
    pub fn capture_ratio(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            capture_ratio(r, bench, self.ann())
        })
    }

    /// Annualized tracking error for each ticker versus the active benchmark.
    pub fn tracking_error(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            tracking_error(r, bench, true, self.ann())
        })
    }

    /// Annualized information ratio for each ticker versus the active benchmark.
    ///
    /// Returns [`f64::NAN`] when the overlapping ticker/benchmark window has
    /// fewer than 2 observations (tracking error is not estimable from one
    /// point, so the ratio is undefined rather than a plausible `±∞`).
    pub fn information_ratio(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            if r.len().min(bench.len()) < 2 {
                return f64::NAN;
            }
            information_ratio(r, bench, true, self.ann())
        })
    }

    /// R-squared for each ticker versus the active benchmark.
    ///
    /// Returns [`f64::NAN`] for a ticker when its overlap with the benchmark
    /// has fewer than two observations or either overlapping series has zero
    /// variance.
    pub fn r_squared(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            r_squared(r, bench)
        })
    }

    /// OLS beta estimates for each ticker versus the active benchmark.
    ///
    /// Each [`BetaResult`] includes a standard error and 95% confidence
    /// interval and therefore requires at least three overlapping
    /// observations. All result fields are [`f64::NAN`] when the overlap is
    /// shorter or the benchmark variance is zero.
    pub fn beta(&self) -> Vec<BetaResult> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            beta(r, bench)
        })
    }

    /// Single-factor greeks for each ticker versus the active benchmark.
    ///
    /// Alpha is annualized Jensen alpha using the configured observation
    /// frequency and the supplied annualized risk-free rate.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form,
    ///   geometrically decompounded to the panel frequency for Jensen alpha.
    ///
    /// # Returns
    ///
    /// One [`GreeksResult`] per ticker in column order. Fields that cannot be
    /// estimated are [`f64::NAN`]: all fields for fewer than two overlapping
    /// observations or zero benchmark variance, both R-squared fields for
    /// zero ticker variance, and adjusted R-squared for exactly two
    /// observations.
    pub fn greeks(&self, risk_free_rate: f64) -> Vec<GreeksResult> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            greeks(r, bench, self.ann(), risk_free_rate)
        })
    }

    /// Rolling greeks (alpha, beta) for a specific ticker vs the benchmark.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> crate::Result<RollingGreeks> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        let (r, bench) = self.active_pair_returns(ticker_idx);
        Ok(rolling_greeks(
            r,
            bench,
            self.active_pair_dates(ticker_idx),
            window,
            self.ann(),
            risk_free_rate,
        ))
    }

    /// Batting average for each ticker versus the active benchmark.
    ///
    /// Fraction of periods where the ticker's return exceeds the benchmark's
    /// return over the active window.
    pub fn batting_average(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            batting_average(r, bench)
        })
    }

    /// M-squared (Modigliani-Modigliani) for each ticker.
    ///
    /// Excess return uses the same geometric rf decompounding as
    /// [`Self::sharpe`]. The cash return added back is the supplied
    /// annualized risk-free rate.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form.
    ///   Decompounded to the panel frequency before forming the excess
    ///   return; the same annual rate is added back as the cash return.
    pub fn m_squared(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_tickers(|i| {
            let (r, bench) = self.active_pair_returns(i);
            let (ann_ret, ann_vol) = risk_metrics::mean_vol_annualized(r, ann);
            let (_, bench_vol) = risk_metrics::mean_vol_annualized(bench, ann);
            m_squared(ann_ret, ann_vol, bench_vol, risk_free_rate, ann)
        })
    }

    /// Multi-factor regression for a specific ticker.
    ///
    /// Factor series are treated as already-excess. [`ReturnKind::Total`]
    /// subtracts the geometrically decompounded period risk-free rate from
    /// the ticker's returns only. [`ReturnKind::Excess`] leaves those
    /// returns unchanged.
    ///
    /// # Arguments
    ///
    /// * `ticker_idx` - Zero-based ticker column index in ticker-name order.
    /// * `factor_returns` - Already-excess factor series, each the same
    ///   length as the ticker's active return window.
    /// * `return_kind` - Whether the ticker series is already excess or
    ///   total. Total subtracts `(1 + rf)^{1/N} − 1` from `y` only.
    ///
    /// # Errors
    ///
    /// Propagates errors when factor inputs are mismatched, non-finite,
    /// insufficient, or numerically singular, or when computed coefficients,
    /// annualized alpha, or residual volatility are non-finite.
    pub fn multi_factor_greeks(
        &self,
        ticker_idx: usize,
        factor_returns: &[&[f64]],
        return_kind: ReturnKind,
    ) -> crate::Result<MultiFactorResult> {
        self.ensure_ticker_idx(ticker_idx)?;
        multi_factor_greeks(
            self.active_returns(ticker_idx),
            factor_returns,
            self.ann(),
            return_kind,
        )
    }
}
