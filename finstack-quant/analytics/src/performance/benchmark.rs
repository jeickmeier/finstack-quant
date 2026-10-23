//! Benchmark-relative metrics on [`Performance`] (alpha/beta/greeks,
//! tracking error, information ratio, R², capture, multi-factor).

use super::Performance;
use crate::benchmark::{
    batting_average, beta, beta_only, capture_ratio, down_capture, greeks, information_ratio,
    m_squared, r_squared, rolling_greeks, tracking_error, treynor, up_capture, BetaResult,
    GreeksResult, RollingGreeks,
};
use crate::multi_factor::{multi_factor_greeks, MultiFactorResult, ReturnKind};
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
        self.map_pair_returns(|r, bench| {
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
        let ann = self.ann();
        self.map_pair_returns(|r, bench| up_capture(r, bench, ann))
    }

    /// Empyrical-style annualized geometric down-capture for each ticker versus the active benchmark.
    pub fn down_capture(&self) -> Vec<f64> {
        let ann = self.ann();
        self.map_pair_returns(|r, bench| down_capture(r, bench, ann))
    }

    /// Empyrical-style annualized geometric capture ratio for each ticker.
    pub fn capture_ratio(&self) -> Vec<f64> {
        let ann = self.ann();
        self.map_pair_returns(|r, bench| capture_ratio(r, bench, ann))
    }

    /// Annualized tracking error for each ticker versus the active benchmark.
    pub fn tracking_error(&self) -> Vec<f64> {
        let ann = self.ann();
        self.map_pair_returns(|r, bench| tracking_error(r, bench, ann))
    }

    /// Annualized information ratio for each ticker versus the active benchmark.
    ///
    /// Returns [`f64::NAN`] when the overlapping ticker/benchmark window has
    /// fewer than 2 observations (tracking error is not estimable from one
    /// point, so the ratio is undefined rather than a plausible `±∞`).
    pub fn information_ratio(&self) -> Vec<f64> {
        let ann = self.ann();
        self.map_pair_returns(|r, bench| {
            if r.len().min(bench.len()) < 2 {
                return f64::NAN;
            }
            information_ratio(r, bench, ann)
        })
    }

    /// R-squared for each ticker versus the active benchmark.
    ///
    /// Returns [`f64::NAN`] for a ticker when its overlap with the benchmark
    /// has fewer than two observations or either overlapping series has zero
    /// variance.
    pub fn r_squared(&self) -> Vec<f64> {
        self.map_pair_returns(r_squared)
    }

    /// OLS beta estimates for each ticker versus the active benchmark.
    ///
    /// Each [`BetaResult`] includes a standard error and 95% confidence
    /// interval and therefore requires at least three overlapping
    /// observations. All result fields are [`f64::NAN`] when the overlap is
    /// shorter or the benchmark variance is zero.
    pub fn beta(&self) -> Vec<BetaResult> {
        self.map_pair_returns(beta)
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
        let ann = self.ann();
        self.map_pair_returns(|r, bench| greeks(r, bench, ann, risk_free_rate))
    }

    /// Rolling greeks (alpha, beta) for a specific ticker vs the benchmark.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::InputError::InvalidReturnSeries`] when
    /// `ticker_idx` is outside the loaded ticker columns.
    pub fn rolling_greeks(
        &self,
        ticker_idx: usize,
        window: usize,
        risk_free_rate: f64,
    ) -> finstack_quant_core::Result<RollingGreeks> {
        self.ensure_ticker_idx(ticker_idx)?;
        Self::ensure_window(window)?;
        let span = self.active_two_ticker_span(ticker_idx, self.benchmark_idx);
        let r = self.returns_for_span(ticker_idx, span);
        let bench = self.returns_for_span(self.benchmark_idx, span);
        Ok(rolling_greeks(
            r,
            bench,
            self.dates_for_span(span),
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
        self.map_pair_returns(batting_average)
    }

    /// M-squared (Modigliani-Modigliani) for each ticker.
    ///
    /// Excess return uses the same geometric rf decompounding as
    /// [`Self::sharpe`]. The cash return added back is that decompounded
    /// period rate times the panel's periods per year, `rf_period × N`
    /// (close to `ln(1 + rf)` at daily frequency), not the annual rate itself.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form.
    ///   Decompounded to the panel frequency as
    ///   `rf_period = (1 + rf)^{1/N} − 1`; `rf_period × N` is both subtracted
    ///   to form the excess return and added back as the cash return.
    pub fn m_squared(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_pair_returns(|r, bench| {
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
    ) -> finstack_quant_core::Result<MultiFactorResult> {
        self.ensure_ticker_idx(ticker_idx)?;
        multi_factor_greeks(
            self.active_returns(ticker_idx),
            factor_returns,
            self.ann(),
            return_kind,
        )
    }
}
