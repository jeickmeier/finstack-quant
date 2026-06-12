//! Per-ticker scalar metrics on [`Performance`] (CAGR, Sharpe, Sortino,
//! Calmar, drawdown statistics, VaR family, etc.).
//!
//! Pure layout split from `performance.rs`; no behavior changes.

use super::Performance;
use crate::drawdown::{
    calmar, cdar, martin_ratio, max_drawdown, max_drawdown_duration as dd_max_duration,
    mean_drawdown, pain_index, pain_ratio, recovery_factor, sterling_ratio, ulcer_index,
};
use crate::returns::comp_total;
use crate::risk_metrics;

impl Performance {
    /// Compound annual growth rate for each ticker.
    ///
    /// Uses the active date window and annualizes from the actual holding
    /// period implied by the price-date grid.
    ///
    /// # Returns
    ///
    /// One CAGR per ticker in column order.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::InputError::Invalid`] if the active range has
    /// no valid positive holding period.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use finstack_analytics::Performance;
    /// # use finstack_core::dates::{Date, Month, PeriodKind};
    /// # let dates: Vec<Date> = (1..=4)
    /// #     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    /// #     .collect();
    /// # let perf = Performance::new(
    /// #     dates,
    /// #     vec![vec![100.0, 101.0, 102.0, 103.0]],
    /// #     vec!["SPY".to_string()],
    /// #     None,
    /// #     PeriodKind::Daily,
    /// # )?;
    /// let cagr = perf.cagr()?;
    /// assert_eq!(cagr.len(), 1);
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    pub fn cagr(&self) -> crate::Result<Vec<f64>> {
        let Some((start, end)) = self.active_holding_period() else {
            return Err(crate::error::InputError::InvalidReturnSeries {
                ticker: "<panel>".into(),
                index: self.start_idx,
                reason: format!(
                    "active range [{}..{}] has no positive holding period on the price-date grid",
                    self.start_idx, self.end_idx
                ),
            }
            .into());
        };
        let basis = risk_metrics::CagrBasis::dates(start, end);
        (0..self.ticker_names.len())
            .map(|i| risk_metrics::cagr(self.active_returns(i), basis))
            .collect()
    }

    /// Mean return for each ticker.
    ///
    /// # Arguments
    ///
    /// * `annualize` - If `true`, scales the mean by the annualization factor.
    ///
    /// # Returns
    ///
    /// One value per ticker in column order.
    pub fn mean_return(&self, annualize: bool) -> Vec<f64> {
        self.map_tickers(|i| {
            risk_metrics::mean_return(self.active_returns(i), annualize, self.ann())
        })
    }

    /// Volatility (sample standard deviation) for each ticker.
    ///
    /// # Arguments
    ///
    /// * `annualize` - If `true`, scales by `sqrt(ann_factor)`.
    ///
    /// # Returns
    ///
    /// One value per ticker in column order.
    pub fn volatility(&self, annualize: bool) -> Vec<f64> {
        self.map_tickers(|i| {
            risk_metrics::volatility(self.active_returns(i), annualize, self.ann())
        })
    }

    /// Sharpe ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate (e.g. `0.02` for 2%).
    ///
    /// # Returns
    ///
    /// One Sharpe ratio per ticker. Returns `0.0` for tickers with zero volatility.
    pub fn sharpe(&self, risk_free_rate: f64) -> Vec<f64> {
        let ann = self.ann();
        self.map_tickers(|i| {
            let (m, v) = risk_metrics::mean_vol_annualized(self.active_returns(i), ann);
            risk_metrics::sharpe(m, v, risk_free_rate)
        })
    }

    /// Annualized Sortino ratio for each ticker.
    ///
    /// Uses the active date window, annualizes with the observation frequency
    /// configured on this [`Performance`] instance, and uses the supplied
    /// minimum acceptable return.
    ///
    /// # Arguments
    ///
    /// * `mar` - Minimum acceptable return **per period** (decimal), e.g. a
    ///   monthly threshold for monthly data. Note this differs from
    ///   [`Self::sharpe`], whose `risk_free_rate` is **annualized**: passing
    ///   the same number to both compares different things. De-annualize an
    ///   annual target with `(1 + annual)^(1/periods_per_year) - 1` first.
    ///
    /// # Returns
    ///
    /// One Sortino ratio per ticker in column order. May return `±∞` for
    /// tickers with zero downside deviation and nonzero mean return.
    pub fn sortino(&self, mar: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::sortino(self.active_returns(i), true, self.ann(), mar))
    }

    /// Calmar ratio for each ticker.
    ///
    /// Computes CAGR over the active date window and divides by the absolute
    /// value of each ticker's worst drawdown over that same window.
    ///
    /// # Returns
    ///
    /// One Calmar ratio per ticker in column order. Returns `0.0` for tickers
    /// with no observed drawdown.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::cagr`] when the active range cannot be
    /// annualized.
    pub fn calmar(&self) -> crate::Result<Vec<f64>> {
        let cagrs = self.cagr()?;
        Ok(self.map_tickers(|i| calmar(cagrs[i], max_drawdown(self.active_drawdown_values(i)))))
    }

    /// Maximum drawdown for each ticker.
    ///
    /// # Returns
    ///
    /// One non-positive maximum drawdown per ticker in column order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use finstack_analytics::Performance;
    /// # use finstack_core::dates::{Date, Month, PeriodKind};
    /// # let dates: Vec<Date> = (1..=4)
    /// #     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
    /// #     .collect();
    /// # let perf = Performance::new(
    /// #     dates,
    /// #     vec![vec![100.0, 105.0, 99.0, 106.0]],
    /// #     vec!["SPY".to_string()],
    /// #     None,
    /// #     PeriodKind::Daily,
    /// # )?;
    /// let max_dd = perf.max_drawdown();
    /// assert!(max_dd[0] <= 0.0);
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    pub fn max_drawdown(&self) -> Vec<f64> {
        self.map_tickers(|i| max_drawdown(self.active_drawdown_values(i)))
    }

    /// Mean drawdown (arithmetic mean of the drawdown path) for each ticker.
    ///
    /// # Returns
    ///
    /// One non-positive mean drawdown per ticker in column order.
    pub fn mean_drawdown(&self) -> Vec<f64> {
        self.map_tickers(|i| mean_drawdown(self.active_drawdown_values(i)))
    }

    /// Historical Value-at-Risk for each ticker (not annualized).
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95` for 95% VaR.
    ///
    /// # Returns
    ///
    /// One VaR value per ticker (non-positive).
    pub fn value_at_risk(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::value_at_risk(self.active_returns(i), confidence))
    }

    /// Expected Shortfall (CVaR) for each ticker (not annualized).
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
    ///
    /// # Returns
    ///
    /// One ES value per ticker (non-positive, always ≤ corresponding VaR).
    pub fn expected_shortfall(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::expected_shortfall(self.active_returns(i), confidence))
    }

    /// Tail ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Quantile level for the upper tail (e.g., `0.95`).
    ///
    /// # Returns
    ///
    /// One tail ratio per ticker.
    pub fn tail_ratio(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::tail_ratio(self.active_returns(i), confidence))
    }

    /// Ulcer Index for each ticker.
    ///
    /// Measures drawdown-based risk from the active drawdown path rather than
    /// return volatility.
    ///
    /// # Returns
    ///
    /// One non-negative Ulcer Index per ticker in column order.
    pub fn ulcer_index(&self) -> Vec<f64> {
        self.map_tickers(|i| ulcer_index(self.active_drawdown_values(i)))
    }

    /// Bias-corrected sample skewness for each ticker.
    ///
    /// # Returns
    ///
    /// One skewness estimate per ticker in column order. Positive values
    /// indicate a heavier right tail.
    pub fn skewness(&self) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::skewness(self.active_returns(i)))
    }

    /// Bias-corrected sample excess kurtosis for each ticker.
    ///
    /// # Returns
    ///
    /// One excess-kurtosis estimate per ticker in column order. Positive
    /// values indicate fatter tails than a normal distribution.
    pub fn kurtosis(&self) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::kurtosis(self.active_returns(i)))
    }

    /// Per-ticker `(skewness, kurtosis)` from one moments pass per ticker.
    ///
    /// Returns two parallel vectors `(skewness_per_ticker, kurtosis_per_ticker)`
    /// in column order. Equivalent to calling [`Self::skewness`] and
    /// [`Self::kurtosis`] but walks each ticker once instead of twice.
    pub fn skew_kurt(&self) -> (Vec<f64>, Vec<f64>) {
        let n = self.ticker_names().len();
        let mut sk = Vec::with_capacity(n);
        let mut ku = Vec::with_capacity(n);
        for i in 0..n {
            let (s, k) = risk_metrics::skew_kurt(self.active_returns(i));
            sk.push(s);
            ku.push(k);
        }
        (sk, ku)
    }

    /// Per-ticker `(value_at_risk, expected_shortfall)` from one tail pass per ticker.
    ///
    /// Returns two parallel vectors `(var_per_ticker, es_per_ticker)` in
    /// column order. Equivalent to calling [`Self::value_at_risk`] and
    /// [`Self::expected_shortfall`] but shares the partition / allocation.
    pub fn value_at_risk_and_es(&self, confidence: f64) -> (Vec<f64>, Vec<f64>) {
        let n = self.ticker_names().len();
        let mut vars = Vec::with_capacity(n);
        let mut ess = Vec::with_capacity(n);
        for i in 0..n {
            let (v, e) = risk_metrics::value_at_risk_and_es(self.active_returns(i), confidence);
            vars.push(v);
            ess.push(e);
        }
        (vars, ess)
    }

    /// Geometric mean return for each ticker.
    ///
    /// # Returns
    ///
    /// One per-period geometric mean return per ticker in column order, using
    /// the active return window.
    pub fn geometric_mean(&self) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::geometric_mean(self.active_returns(i)))
    }

    /// Annualized downside deviation for each ticker.
    ///
    /// # Arguments
    ///
    /// * `mar` - Minimum acceptable per-period return threshold in decimal
    ///   form (for example, `0.0` or `0.001`).
    ///
    /// # Returns
    ///
    /// One downside-deviation value per ticker in column order, annualized
    /// using the configured observation frequency.
    pub fn downside_deviation(&self, mar: f64) -> Vec<f64> {
        self.map_tickers(|i| {
            risk_metrics::downside_deviation(self.active_returns(i), mar, true, self.ann())
        })
    }

    /// Maximum drawdown duration in calendar days for each ticker.
    ///
    /// Duration is measured on the active date grid, so irregular observation
    /// spacing is reflected in the reported day counts.
    ///
    /// # Returns
    ///
    /// One maximum drawdown duration per ticker in column order.
    pub fn max_drawdown_duration(&self) -> Vec<i64> {
        self.map_tickers(|i| dd_max_duration(self.active_drawdown_values(i), self.active_dates()))
    }

    /// Omega ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Per-period threshold return in decimal form, typically `0.0`.
    ///
    /// # Returns
    ///
    /// One Omega ratio per ticker in column order over the active window.
    pub fn omega_ratio(&self, threshold: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::omega_ratio(self.active_returns(i), threshold))
    }

    /// Gain-to-pain ratio for each ticker (sum of returns / sum of |losses|).
    ///
    /// # Returns
    ///
    /// One gain-to-pain ratio per ticker in column order over the active
    /// return window.
    pub fn gain_to_pain(&self) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::gain_to_pain(self.active_returns(i)))
    }

    /// Martin ratio (CAGR divided by Ulcer Index) for each ticker.
    ///
    /// # Returns
    ///
    /// One Martin ratio per ticker in column order. Returns `0.0` for tickers
    /// with zero Ulcer Index.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::cagr`] when the active range cannot be
    /// annualized.
    pub fn martin_ratio(&self) -> crate::Result<Vec<f64>> {
        let cagrs = self.cagr()?;
        Ok(self
            .map_tickers(|i| martin_ratio(cagrs[i], ulcer_index(self.active_drawdown_values(i)))))
    }

    /// Parametric (Gaussian) VaR for each ticker.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
    ///
    /// # Returns
    ///
    /// One non-positive parametric VaR per ticker in column order.
    pub fn parametric_var(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| risk_metrics::parametric_var(self.active_returns(i), confidence, None))
    }

    /// Cornish-Fisher adjusted VaR for each ticker.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
    ///
    /// # Returns
    ///
    /// One non-positive Cornish-Fisher VaR per ticker in column order.
    pub fn cornish_fisher_var(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| {
            risk_metrics::cornish_fisher_var(self.active_returns(i), confidence, None)
        })
    }

    /// Recovery factor for each ticker.
    ///
    /// # Returns
    ///
    /// One recovery factor per ticker in column order, computed as total
    /// compounded return divided by absolute maximum drawdown.
    pub fn recovery_factor(&self) -> Vec<f64> {
        self.map_tickers(|i| {
            let total_ret = comp_total(self.active_returns(i));
            let max_dd = max_drawdown(self.active_drawdown_values(i));
            recovery_factor(total_ret, max_dd)
        })
    }

    /// Sterling ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate.
    /// * `n` - Number of worst drawdowns to average.
    ///
    /// # Returns
    ///
    /// One Sterling ratio per ticker in column order.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::cagr`] when the active range cannot be
    /// annualized.
    pub fn sterling_ratio(&self, risk_free_rate: f64, n: usize) -> crate::Result<Vec<f64>> {
        let cagrs = self.cagr()?;
        Ok(self.map_tickers(|i| {
            let avg = crate::drawdown::mean_episode_drawdown(self.active_drawdown_values(i), n);
            sterling_ratio(cagrs[i], avg, risk_free_rate)
        }))
    }

    /// Burke ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate.
    /// * `n` - Number of worst drawdown episodes to use.
    ///
    /// # Returns
    ///
    /// One Burke ratio per ticker in column order.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::cagr`] when the active range cannot be
    /// annualized.
    pub fn burke_ratio(&self, risk_free_rate: f64, n: usize) -> crate::Result<Vec<f64>> {
        let cagrs = self.cagr()?;
        let dates = self.active_dates();
        Ok(self.map_tickers(|i| {
            let episodes =
                crate::drawdown::drawdown_details(self.active_drawdown_values(i), dates, n);
            let dd_vals: Vec<f64> = episodes.iter().map(|e| e.max_drawdown).collect();
            crate::drawdown::burke_ratio(cagrs[i], &dd_vals, risk_free_rate)
        }))
    }

    /// Pain index for each ticker.
    ///
    /// # Returns
    ///
    /// One non-negative pain index per ticker in column order.
    pub fn pain_index(&self) -> Vec<f64> {
        self.map_tickers(|i| pain_index(self.active_drawdown_values(i)))
    }

    /// Pain ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate.
    ///
    /// # Returns
    ///
    /// One pain ratio per ticker in column order.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::cagr`] when the active range cannot be
    /// annualized.
    pub fn pain_ratio(&self, risk_free_rate: f64) -> crate::Result<Vec<f64>> {
        let cagrs = self.cagr()?;
        Ok(self.map_tickers(|i| {
            let pain = pain_index(self.active_drawdown_values(i));
            pain_ratio(cagrs[i], pain, risk_free_rate)
        }))
    }

    /// Conditional Drawdown at Risk for each ticker.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Confidence level in `(0, 1)`, e.g. `0.95`.
    ///
    /// # Returns
    ///
    /// One non-positive CDaR value per ticker in column order, matching the
    /// sign convention of [`Self::max_drawdown`] / [`Self::mean_drawdown`].
    /// A 95% CDaR of `-0.25` means the average drawdown in the worst 5% tail
    /// is 25%.
    pub fn cdar(&self, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| cdar(self.active_drawdown_values(i), confidence))
    }

    /// Modified Sharpe ratio for each ticker.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Annualized risk-free rate in decimal form.
    /// * `confidence`     - Cornish-Fisher VaR confidence level in `(0, 1)`.
    ///
    /// # Returns
    ///
    /// One modified Sharpe ratio per ticker in column order.
    pub fn modified_sharpe(&self, risk_free_rate: f64, confidence: f64) -> Vec<f64> {
        self.map_tickers(|i| {
            risk_metrics::modified_sharpe(
                self.active_returns(i),
                risk_free_rate,
                confidence,
                self.ann(),
            )
        })
    }
}
