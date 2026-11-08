//! Per-ticker metrics and the canonical fixed performance summary.

use finstack_quant_core::dates::DayCountContext;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use finstack_quant_core::{Error, Result};

use super::Performance;
use crate::drawdown::{
    deepest, max_drawdown_duration, mean_drawdown, mean_episode_drawdown, pain_index, ulcer_index,
    worst_episode_depths,
};
use crate::returns::{comp_total, validate_cash};
use crate::risk_metrics::{self, CagrDayCount};

/// Divide available finite reward by measured nonnegative risk without a cutoff.
fn reward_ratio(reward: f64, risk: f64) -> Result<f64> {
    if reward.is_nan() || risk.is_nan() {
        return Ok(f64::NAN);
    }
    if !reward.is_finite() || !risk.is_finite() || risk < 0.0 {
        return Err(Error::Internal(
            "drawdown reward or risk is not finite and valid".into(),
        ));
    }
    if risk == 0.0 {
        return Ok(if reward == 0.0 {
            f64::NAN
        } else {
            f64::INFINITY.copysign(reward)
        });
    }
    let value = reward / risk;
    if !value.is_finite() {
        return Err(Error::Internal(
            "drawdown reward/risk ratio is not representable".into(),
        ));
    }
    Ok(value)
}

fn excess_reward(growth: f64, cash: f64) -> Result<f64> {
    if growth.is_nan() {
        return Ok(f64::NAN);
    }
    let reward = growth - cash;
    if !reward.is_finite() {
        return Err(Error::Internal(
            "dated excess reward is not representable".into(),
        ));
    }
    Ok(reward)
}

impl Performance {
    fn scalar_map(&self, mut calculate: impl FnMut(usize) -> Result<f64>) -> Result<Vec<f64>> {
        (0..self.ticker_names.len()).map(&mut calculate).collect()
    }

    /// Dated compounded annual growth for each ticker's active holding interval.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Explicit annual clock; the canonical default is ACT/365F.
    /// * `context` - Complete convention-specific context, including calendar,
    ///   coupon frequency/reference boundaries and termination status when needed.
    ///
    /// # Returns
    ///
    /// Decimal dated CAGR in ticker order; empty or zero-year histories are NaN.
    ///
    /// # Errors
    ///
    /// Rejects missing day-count context and unrepresentable calculated growth.
    pub fn cagr(&self, day_count: CagrDayCount, context: DayCountContext<'_>) -> Result<Vec<f64>> {
        // Validate required context even when the selected window is empty.
        risk_metrics::validate_cagr_context(day_count, context)?;
        self.scalar_map(|i| {
            let Some((start, end)) = self.active_holding_period_for_ticker(i) else {
                return Ok(f64::NAN);
            };
            risk_metrics::cagr(self.active_returns(i), start, end, day_count, context)
        })
    }

    /// Arithmetic mean return in explicitly selected observation or annual units.
    ///
    /// # Arguments
    ///
    /// * `annualize` - True multiplies the mean by the required stored frequency;
    ///   false returns the unannualized observation-period mean.
    ///
    /// # Returns
    ///
    /// Decimal means in ticker order; empty histories are NaN.
    ///
    /// # Errors
    ///
    /// Rejects absent required frequency or unrepresentable calculated means.
    pub fn mean_return(&self, annualize: bool) -> Result<Vec<f64>> {
        let ann = if annualize { self.ann()? } else { 1.0 };
        self.scalar_map(|i| risk_metrics::mean_return(self.active_returns(i), annualize, ann))
    }

    /// Sample standard deviation in explicitly selected observation or annual units.
    ///
    /// # Arguments
    ///
    /// * `annualize` - True uses square-root stored-frequency scaling; false
    ///   returns observation-period sample standard deviation without frequency.
    ///
    /// # Returns
    ///
    /// Nonnegative risks in ticker order; fewer than two returns are NaN.
    ///
    /// # Errors
    ///
    /// Rejects absent required frequency and unrepresentable calculated risk.
    pub fn volatility(&self, annualize: bool) -> Result<Vec<f64>> {
        let ann = if annualize { self.ann()? } else { 1.0 };
        self.scalar_map(|i| crate::volatility(self.active_returns(i), ann))
    }

    /// Annualized arithmetic excess-return Sharpe using sample standard deviation.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   strictly greater than -1, geometrically converted to one observation.
    ///
    /// # Returns
    ///
    /// Ratios in ticker order; insufficient history and 0/0 are NaN, while
    /// finite nonzero reward over exactly zero measured risk is signed infinity.
    ///
    /// # Errors
    ///
    /// Rejects invalid cash, missing frequency and numerical calculation failures.
    pub fn sharpe(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        let ann = self.ann()?;
        self.scalar_map(|i| crate::sharpe(self.active_returns(i), risk_free_rate, ann))
    }

    /// Annualized Sortino with all observations in its downside denominator.
    ///
    /// # Arguments
    ///
    /// * `mar` - Explicit finite decimal minimum acceptable return per observation;
    ///   accepts any finite threshold and receives no annual cash conversion.
    ///
    /// # Returns
    ///
    /// Ratios in ticker order with owner-defined sample and zero-downside outcomes.
    ///
    /// # Errors
    ///
    /// Rejects invalid MAR, missing frequency and numerical failures.
    pub fn sortino(&self, mar: f64) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.scalar_map(|i| crate::sortino(self.active_returns(i), mar, ann))
    }

    /// ACT/365F CAGR divided by maximum drawdown magnitude over the active window.
    ///
    /// # Returns
    ///
    /// Ratios in ticker order, NaN for empty history or 0/0, signed infinity for
    /// nonzero reward over measured zero drawdown; no automatic 36-month cutoff.
    ///
    /// # Errors
    ///
    /// Propagates growth and requested-ratio calculation failures.
    pub fn calmar(&self) -> Result<Vec<f64>> {
        let growth = self.cagr(CagrDayCount::default(), DayCountContext::default())?;
        self.scalar_map(|i| reward_ratio(growth[i], deepest(self.active_drawdown_values(i)).abs()))
    }

    /// Deepest signed decimal drawdown per ticker; empty histories are NaN.
    ///
    /// # Returns
    ///
    /// Values in `[-1, 0]` in ticker order, with zero for a valid loss-free path.
    pub fn max_drawdown(&self) -> Result<Vec<f64>> {
        Ok(self.map_tickers(|i| deepest(self.active_drawdown_values(i))))
    }

    /// Mean signed endpoint drawdown, including observed zeros equally.
    ///
    /// # Returns
    ///
    /// Nonpositive decimal means in ticker order; empty histories are NaN.
    pub fn mean_drawdown(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| mean_drawdown(self.active_drawdown_values(i)))
    }

    /// Signed historical R-7 return VaR in native observation-period units.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite confidence strictly between 0.5 and 1.
    ///
    /// # Returns
    ///
    /// Decimal return quantiles in ticker order; empty histories are NaN.
    ///
    /// # Errors
    ///
    /// Rejects invalid confidence and unrepresentable requested estimates.
    pub fn value_at_risk(&self, confidence: f64) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| risk_metrics::value_at_risk(self.active_returns(i), confidence))
    }

    /// Signed exact fractional empirical lower-tail expected shortfall.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite confidence strictly between 0.5 and 1.
    ///
    /// # Returns
    ///
    /// Mean decimal tail returns in ticker order; ties do not expand selected
    /// mass and empty histories are NaN.
    ///
    /// # Errors
    ///
    /// Rejects invalid confidence and unrepresentable requested tail averages.
    pub fn expected_shortfall(&self, confidence: f64) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| risk_metrics::expected_shortfall(self.active_returns(i), confidence))
    }

    /// Ratio of absolute upper and lower empirical R-7 quantiles.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite upper quantile strictly between 0.5 and 1.
    ///
    /// # Returns
    ///
    /// Observation-period magnitude ratios in ticker order; empty and 0/0
    /// results are NaN, and positive-over-zero is infinity.
    ///
    /// # Errors
    ///
    /// Rejects invalid confidence and numerical calculation failures.
    pub fn tail_ratio(&self, confidence: f64) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| risk_metrics::tail_ratio(self.active_returns(i), confidence))
    }

    /// Root-mean-square endpoint drawdown, excluding the opening baseline.
    ///
    /// # Returns
    ///
    /// Nonnegative decimal Ulcer indices in ticker order; empty histories are NaN.
    pub fn ulcer_index(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| ulcer_index(self.active_drawdown_values(i)))
    }

    /// Fisher-adjusted Type-2 sample skewness for each ticker.
    ///
    /// # Returns
    ///
    /// Shape estimates in ticker order; fewer than three observations or zero
    /// variance is undefined (NaN).
    ///
    /// # Errors
    ///
    /// Propagates numerical failures while evaluating the requested statistic.
    pub fn skewness(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| risk_metrics::skewness(self.active_returns(i)))
    }

    /// Fisher-adjusted Type-2 sample excess kurtosis for each ticker.
    ///
    /// # Returns
    ///
    /// Excess shape estimates in ticker order; fewer than four observations or
    /// zero variance is undefined (NaN).
    ///
    /// # Errors
    ///
    /// Propagates numerical failures while evaluating the requested statistic.
    pub fn kurtosis(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| risk_metrics::kurtosis(self.active_returns(i)))
    }

    /// Geometric mean simple investment return per observation.
    ///
    /// # Returns
    ///
    /// Decimal geometric means in ticker order; terminal loss is -1 and empty
    /// histories are undefined (NaN). No annualization is applied.
    ///
    /// # Errors
    ///
    /// Propagates unrepresentable requested growth calculations.
    pub fn geometric_mean(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| risk_metrics::geometric_mean(self.active_returns(i)))
    }

    /// Annualized RMS shortfall from the explicit observation-period MAR.
    ///
    /// # Arguments
    ///
    /// * `mar` - Any finite decimal hurdle per observation, including values
    ///   below -1; all observations remain in the denominator.
    ///
    /// # Returns
    ///
    /// Nonnegative annualized downside deviations in ticker order; empty is NaN.
    ///
    /// # Errors
    ///
    /// Rejects invalid MAR, missing frequency and unrepresentable risk.
    pub fn downside_deviation(&self, mar: f64) -> Result<Vec<f64>> {
        let ann = self.ann()?;
        self.scalar_map(|i| {
            risk_metrics::downside_deviation(self.active_returns(i), mar, true, ann)
        })
    }

    /// Longest drawdown episode in actual calendar days over all episodes.
    ///
    /// # Returns
    ///
    /// Nullable day counts in ticker order; empty histories are absent and
    /// valid histories without a decline have zero duration.
    ///
    /// # Errors
    ///
    /// Rejects malformed date alignment or invalid cached paths.
    pub fn max_drawdown_duration(&self) -> Result<Vec<Option<i64>>> {
        (0..self.ticker_names.len())
            .map(|i| {
                let (path, dates) = self.active_drawdown_path(i);
                max_drawdown_duration(&path, dates)
            })
            .collect()
    }

    /// Empirical gain/shortfall Omega around a supplied observation hurdle.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Any finite decimal threshold per observation; no
    ///   annualization or cash conversion is applied.
    ///
    /// # Returns
    ///
    /// Ratios in ticker order; empty or 0/0 is NaN, positive-over-zero is infinity.
    ///
    /// # Errors
    ///
    /// Rejects invalid thresholds and numerical calculation failures.
    pub fn omega_ratio(&self, threshold: f64) -> Result<Vec<f64>> {
        self.scalar_map(|i| risk_metrics::omega_ratio(self.active_returns(i), threshold))
    }

    /// Arithmetic net return sum divided by negative-return magnitudes.
    ///
    /// # Returns
    ///
    /// Unannualized ratios in ticker order; empty and 0/0 are NaN.
    ///
    /// # Errors
    ///
    /// Propagates unrepresentable requested ratio calculations.
    pub fn gain_to_pain(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| risk_metrics::gain_to_pain(self.active_returns(i)))
    }

    /// Cash-adjusted dated CAGR divided by endpoint Ulcer index.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash rate
    ///   strictly greater than -1, subtracted directly from ACT/365F CAGR.
    ///
    /// # Returns
    ///
    /// Martin ratios in ticker order with undefined and exact-zero-risk outcomes.
    ///
    /// # Errors
    ///
    /// Rejects invalid cash and propagates growth or ratio calculation failures.
    pub fn martin_ratio(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        let growth = self.cagr(CagrDayCount::default(), DayCountContext::default())?;
        self.scalar_map(|i| {
            reward_ratio(
                excess_reward(growth[i], risk_free_rate)?,
                ulcer_index(self.active_drawdown_values(i))?,
            )
        })
    }

    /// Additive i.i.d. Gaussian return VaR over whole observation periods.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite confidence strictly between 0.5 and 1.
    /// * `horizon_periods` - Positive exact integer observation count no larger
    ///   than 2^53-1; absent means one observation. No frequency is required.
    ///
    /// # Returns
    ///
    /// Signed decimal model quantiles in ticker order; fewer than two returns is
    /// unavailable. Additive model outputs need not lie in the investment domain.
    ///
    /// # Errors
    ///
    /// Rejects invalid requests and unrepresentable requested model quantiles.
    pub fn parametric_var(
        &self,
        confidence: f64,
        horizon_periods: Option<u64>,
    ) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| {
            risk_metrics::parametric_var(self.active_returns(i), confidence, horizon_periods)
        })
    }

    /// Classical plug-in Cornish-Fisher VaR with a global quantile-ordering gate.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite confidence strictly between 0.5 and 1.
    /// * `horizon_periods` - Positive exact integer observation count no larger
    ///   than 2^53-1; absent means one. Shapes scale with the same additive horizon.
    ///
    /// # Returns
    ///
    /// Signed decimal model estimates in ticker order; unavailable moment
    /// histories are NaN. No automatic Gaussian substitution is performed.
    ///
    /// # Errors
    ///
    /// Rejects invalid requests, a globally nonordered CF polynomial and
    /// unrepresentable requested estimates.
    pub fn cornish_fisher_var(
        &self,
        confidence: f64,
        horizon_periods: Option<u64>,
    ) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| {
            risk_metrics::cornish_fisher_var(self.active_returns(i), confidence, horizon_periods)
        })
    }

    /// Compounded total investment return per unit of maximum drawdown.
    ///
    /// # Returns
    ///
    /// Unannualized recovery factors in ticker order; empty and 0/0 are NaN.
    ///
    /// # Errors
    ///
    /// Propagates growth and requested-ratio calculation failures.
    pub fn recovery_factor(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| {
            reward_ratio(
                comp_total(self.active_returns(i))?,
                deepest(self.active_drawdown_values(i)).abs(),
            )
        })
    }

    /// Cash-adjusted ACT/365F CAGR per mean magnitude of the worst episodes.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash greater
    ///   than -1, subtracted directly from dated CAGR.
    /// * `n` - Positive number of worst episode valleys; use all available when
    ///   fewer exist, including open episodes, with no zero padding or penalty.
    ///
    /// # Returns
    ///
    /// Sterling ratios in ticker order with metric-specific unavailability and
    /// exact-zero-risk outcomes.
    ///
    /// # Errors
    ///
    /// Rejects invalid cash or zero count and propagates calculation failures.
    pub fn sterling_ratio(&self, risk_free_rate: f64, n: usize) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        if n == 0 {
            return Err(Error::Validation(
                "Sterling episode count must be positive".into(),
            ));
        }
        let growth = self.cagr(CagrDayCount::default(), DayCountContext::default())?;
        self.scalar_map(|i| {
            if self.active_returns(i).is_empty() {
                return Ok(f64::NAN);
            }
            reward_ratio(
                excess_reward(growth[i], risk_free_rate)?,
                mean_episode_drawdown(self.active_drawdown_values(i), n).abs(),
            )
        })
    }

    /// Modified Burke using every episode and normalization by return count.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash greater
    ///   than -1, subtracted directly from ACT/365F CAGR.
    ///
    /// # Returns
    ///
    /// `(CAGR-cash)/sqrt(sum(episode_depth^2)/return_count)` in ticker order;
    /// empty and 0/0 are NaN, and nonzero-over-zero is signed infinity.
    ///
    /// # Errors
    ///
    /// Rejects invalid cash and propagates growth or ratio calculation failures.
    pub fn burke_ratio(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        let growth = self.cagr(CagrDayCount::default(), DayCountContext::default())?;
        self.scalar_map(|i| {
            let path = self.active_drawdown_values(i);
            if path.is_empty() {
                return Ok(f64::NAN);
            }
            let depths = worst_episode_depths(path, usize::MAX);
            let scale = depths.iter().copied().map(f64::abs).fold(0.0, f64::max);
            let risk = if scale == 0.0 {
                0.0
            } else {
                let mut normalized =
                    finstack_quant_core::math::summation::NeumaierAccumulator::new();
                for depth in depths {
                    normalized.add((depth / scale).powi(2));
                }
                scale * (normalized.total() / path.len() as f64).sqrt()
            };
            if scale > 0.0 && risk == 0.0 {
                return Err(Error::Internal(
                    "nonzero Burke risk is not representable".into(),
                ));
            }
            reward_ratio(excess_reward(growth[i], risk_free_rate)?, risk)
        })
    }

    /// Mean absolute observed endpoint drawdown, with observed zeros included.
    ///
    /// # Returns
    ///
    /// Nonnegative decimal Pain indices in ticker order; empty is NaN.
    pub fn pain_index(&self) -> Result<Vec<f64>> {
        self.scalar_map(|i| pain_index(self.active_drawdown_values(i)))
    }

    /// Cash-adjusted ACT/365F CAGR per mean absolute endpoint drawdown.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash greater
    ///   than -1, subtracted directly from dated CAGR.
    ///
    /// # Returns
    ///
    /// Pain ratios in ticker order with undefined and exact-zero-risk outcomes.
    ///
    /// # Errors
    ///
    /// Rejects invalid cash and propagates growth or ratio calculation failures.
    pub fn pain_ratio(&self, risk_free_rate: f64) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        let growth = self.cagr(CagrDayCount::default(), DayCountContext::default())?;
        self.scalar_map(|i| {
            reward_ratio(
                excess_reward(growth[i], risk_free_rate)?,
                pain_index(self.active_drawdown_values(i))?,
            )
        })
    }

    /// Signed fractional empirical lower-tail average of endpoint drawdowns.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Explicit finite confidence strictly between 0.5 and 1;
    ///   includes all endpoints and excludes the additional opening zero.
    ///
    /// # Returns
    ///
    /// Nonpositive decimal CDaR in ticker order; empty is NaN and valid
    /// loss-free histories give zero, without an annualization multiplier.
    ///
    /// # Errors
    ///
    /// Rejects invalid confidence and propagates requested-estimator failures.
    pub fn cdar(&self, confidence: f64) -> Result<Vec<f64>> {
        Self::ensure_confidence(confidence)?;
        self.scalar_map(|i| {
            let mut path = self.active_drawdown_values(i).to_vec();
            if path.is_empty() {
                return Ok(f64::NAN);
            }
            // A displayed -1 drawdown may be rounded positive wealth, so it
            // is not an investment-return terminal event for path validation.
            path.sort_unstable_by(f64::total_cmp);
            finstack_quant_core::math::stats::empirical_tail_mean(&path, 1.0 - confidence)
        })
    }

    /// Observation-period arithmetic excess return divided by total-return CF loss.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Explicit finite annual-effective decimal cash greater
    ///   than -1, converted geometrically using the required stored frequency.
    /// * `confidence` - Explicit finite CF confidence strictly between 0.5 and 1;
    ///   the denominator uses one observation and receives no annualization.
    ///
    /// # Returns
    ///
    /// Modified Sharpe ratios in ticker order with owner-defined availability.
    ///
    /// # Errors
    ///
    /// Rejects invalid requests, missing frequency, invalid CF ordering and
    /// numerical failures without substituting another tail model.
    pub fn modified_sharpe(&self, risk_free_rate: f64, confidence: f64) -> Result<Vec<f64>> {
        validate_cash(risk_free_rate)?;
        Self::ensure_confidence(confidence)?;
        let ann = self.ann()?;
        self.scalar_map(|i| {
            risk_metrics::modified_sharpe(self.active_returns(i), risk_free_rate, confidence, ann)
        })
    }

    /// Fixed 22-metric report with owner-specific samples and explicit context.
    ///
    /// # Arguments
    ///
    /// * `risk_free_rate` - Finite annual-effective decimal cash greater than -1;
    ///   affects Sharpe and is retained in report metadata.
    /// * `mar` - Finite decimal hurdle per observation for Sortino and downside.
    /// * `omega_threshold` - Independent finite decimal Omega hurdle per observation.
    /// * `confidence` - Finite confidence strictly between 0.5 and 1 for historical
    ///   R-7 VaR, fractional empirical ES and tail ratio.
    ///
    /// # Returns
    ///
    /// One row per supplied ticker with fixed ordered metric keys, explicit
    /// units/context and each row's own and paired observation provenance.
    /// Undefined and intentional signed-infinite cells preserve their meanings.
    ///
    /// # Errors
    ///
    /// Rejects invalid inputs, absent frequency or benchmark and every request,
    /// metric calculation or table-construction failure atomically.
    pub fn summary(
        &self,
        risk_free_rate: f64,
        mar: f64,
        omega_threshold: f64,
        confidence: f64,
    ) -> Result<TableEnvelope> {
        validate_cash(risk_free_rate)?;
        Self::ensure_confidence(confidence)?;
        if !mar.is_finite() || !omega_threshold.is_finite() {
            return Err(Error::Validation(
                "summary MAR and Omega threshold must be finite".into(),
            ));
        }
        self.ann()?;
        let benchmark = self.benchmark()?;
        let (var, es): (Vec<f64>, Vec<f64>) = (0..self.ticker_names.len())
            .map(|i| risk_metrics::value_at_risk_and_es(self.active_returns(i), confidence))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();
        let metrics: [(&str, Vec<f64>); 22] = [
            (
                "cagr",
                self.cagr(CagrDayCount::default(), DayCountContext::default())?,
            ),
            ("mean_return", self.mean_return(true)?),
            ("volatility", self.volatility(true)?),
            ("sharpe", self.sharpe(risk_free_rate)?),
            ("sortino", self.sortino(mar)?),
            ("calmar", self.calmar()?),
            ("max_drawdown", self.max_drawdown()?),
            ("value_at_risk", var),
            ("expected_shortfall", es),
            ("tracking_error", self.tracking_error()?),
            ("information_ratio", self.information_ratio()?),
            ("skewness", self.skewness()?),
            ("kurtosis", self.kurtosis()?),
            ("geometric_mean", self.geometric_mean()?),
            ("downside_deviation", self.downside_deviation(mar)?),
            ("omega_ratio", self.omega_ratio(omega_threshold)?),
            ("gain_to_pain", self.gain_to_pain()?),
            ("ulcer_index", self.ulcer_index()?),
            ("pain_index", self.pain_index()?),
            ("recovery_factor", self.recovery_factor()?),
            ("tail_ratio", self.tail_ratio(confidence)?),
            ("r_squared", self.r_squared()?),
        ];
        let mut columns = Vec::with_capacity(23);
        columns.push(
            TableColumn::new("ticker", TableColumnData::String(self.ticker_names.clone()))
                .with_role(TableColumnRole::Dimension),
        );
        for (name, values) in metrics {
            columns.push(
                TableColumn::new(name, TableColumnData::Float64(values))
                    .with_role(TableColumnRole::Measure),
            );
        }
        let mut table = TableEnvelope::new(columns, Default::default())?;
        table
            .metadata
            .insert("kind".into(), "analytics_summary".into());
        table
            .metadata
            .insert("risk_free_rate".into(), risk_free_rate.into());
        table.metadata.insert("mar".into(), mar.into());
        table
            .metadata
            .insert("omega_threshold".into(), omega_threshold.into());
        table
            .metadata
            .insert("confidence".into(), confidence.into());
        table.metadata.insert(
            "frequency".into(),
            self.frequency.map(|f| f.to_string()).into(),
        );
        table.metadata.insert(
            "benchmark".into(),
            self.ticker_names[benchmark].clone().into(),
        );
        table
            .metadata
            .insert("cagr_day_count".into(), "act_365f".into());
        table.metadata.insert("units".into(), serde_json::json!({
            "mean_return": "annual_arithmetic_decimal", "volatility": "annual_decimal",
            "downside_deviation": "annual_decimal", "sharpe": "annual_ratio", "sortino": "annual_ratio",
            "geometric_mean": "observation_decimal", "value_at_risk": "observation_signed_decimal",
            "expected_shortfall": "observation_signed_decimal", "drawdown": "signed_decimal",
            "kurtosis": "excess"
        }));
        let rows = (0..self.ticker_names.len()).map(|i| {
            let own = self.active_span_for_ticker(i);
            let pair = self.active_two_ticker_span(i, benchmark);
            let own_dates = self.dates_for_span(own);
            let pair_dates = self.dates_for_span(pair);
            serde_json::json!({
                "ticker": self.ticker_names[i],
                "own_sample_count": own.len(),
                "own_start": own_dates.first().map(ToString::to_string),
                "own_end": own_dates.last().map(ToString::to_string),
                "holding_start": (!own.is_empty()).then(|| self.price_dates[own.start].to_string()),
                "paired_sample_count": pair.len(),
                "paired_start": pair_dates.first().map(ToString::to_string),
                "paired_end": pair_dates.last().map(ToString::to_string),
                "paired_holding_start": (!pair.is_empty()).then(|| self.price_dates[pair.start].to_string())
            })
        }).collect::<Vec<_>>();
        table.metadata.insert("rows".into(), rows.into());
        validate_summary(&table)?;
        Ok(table)
    }
}

/// Validate an analytics summary after restoring its shared table envelope.
///
/// Generic table deserialization validates columns and row counts; this check
/// additionally admits only the canonical metric layout and financial metadata.
/// It verifies reported structure and availability, not source-data accuracy.
///
/// # Arguments
///
/// * `table` - Restored or constructed fixed analytics summary with numeric
///   result sentinels retained and complete per-row observation provenance.
///
/// # Errors
///
/// Returns [`Error::Validation`] for wrong keys/types/order, invalid required
/// inputs, inconsistent ticker/sample/date metadata or impossible metric results.
pub fn validate_summary(table: &TableEnvelope) -> Result<()> {
    use finstack_quant_core::dates::{Date, PeriodKind, parse_iso_date};
    use serde_json::Value;
    const METRICS: [&str; 22] = [
        "cagr",
        "mean_return",
        "volatility",
        "sharpe",
        "sortino",
        "calmar",
        "max_drawdown",
        "value_at_risk",
        "expected_shortfall",
        "tracking_error",
        "information_ratio",
        "skewness",
        "kurtosis",
        "geometric_mean",
        "downside_deviation",
        "omega_ratio",
        "gain_to_pain",
        "ulcer_index",
        "pain_index",
        "recovery_factor",
        "tail_ratio",
        "r_squared",
    ];
    let bad = || Error::Validation("malformed canonical analytics summary".into());
    table.validate()?;
    if table.columns.len() != 23 || table.row_count == 0 {
        return Err(bad());
    }
    let ticker_column = &table.columns[0];
    let TableColumnData::String(tickers) = &ticker_column.data else {
        return Err(bad());
    };
    if ticker_column.name != "ticker" || ticker_column.role != Some(TableColumnRole::Dimension) {
        return Err(bad());
    }
    let mut seen = std::collections::HashSet::new();
    if tickers
        .iter()
        .any(|ticker| ticker.trim().is_empty() || !seen.insert(ticker))
    {
        return Err(bad());
    }
    let mut metric_values = Vec::with_capacity(22);
    for (column, name) in table.columns[1..].iter().zip(METRICS) {
        if column.name != name || column.role != Some(TableColumnRole::Measure) {
            return Err(bad());
        }
        let TableColumnData::Float64(values) = &column.data else {
            return Err(bad());
        };
        metric_values.push(values);
    }
    let metadata = &table.metadata;
    const KEYS: [&str; 10] = [
        "kind",
        "risk_free_rate",
        "mar",
        "omega_threshold",
        "confidence",
        "frequency",
        "benchmark",
        "cagr_day_count",
        "units",
        "rows",
    ];
    if metadata.len() != KEYS.len() || KEYS.iter().any(|key| !metadata.contains_key(*key)) {
        return Err(bad());
    }
    if metadata["kind"].as_str() != Some("analytics_summary")
        || metadata["cagr_day_count"].as_str() != Some("act_365f")
    {
        return Err(bad());
    }
    validate_cash(metadata["risk_free_rate"].as_f64().ok_or_else(bad)?)?;
    for key in ["mar", "omega_threshold"] {
        if !metadata[key].as_f64().is_some_and(f64::is_finite) {
            return Err(bad());
        }
    }
    Performance::ensure_confidence(metadata["confidence"].as_f64().ok_or_else(bad)?)?;
    metadata["frequency"]
        .as_str()
        .ok_or_else(bad)?
        .parse::<PeriodKind>()?;
    let benchmark = metadata["benchmark"].as_str().ok_or_else(bad)?;
    if !tickers.iter().any(|ticker| ticker == benchmark) {
        return Err(bad());
    }
    let units = serde_json::json!({
        "mean_return": "annual_arithmetic_decimal", "volatility": "annual_decimal",
        "downside_deviation": "annual_decimal", "sharpe": "annual_ratio", "sortino": "annual_ratio",
        "geometric_mean": "observation_decimal", "value_at_risk": "observation_signed_decimal",
        "expected_shortfall": "observation_signed_decimal", "drawdown": "signed_decimal", "kurtosis": "excess"
    });
    if metadata["units"] != units {
        return Err(bad());
    }
    let rows = metadata["rows"].as_array().ok_or_else(bad)?;
    if rows.len() != tickers.len() {
        return Err(bad());
    }
    let date = |value: &Value| -> Result<Option<Date>> {
        if value.is_null() {
            return Ok(None);
        }
        parse_iso_date(value.as_str().ok_or_else(bad)?).map(Some)
    };
    for (i, row) in rows.iter().enumerate() {
        let row = row.as_object().ok_or_else(bad)?;
        const ROW_KEYS: [&str; 9] = [
            "ticker",
            "own_sample_count",
            "own_start",
            "own_end",
            "holding_start",
            "paired_sample_count",
            "paired_start",
            "paired_end",
            "paired_holding_start",
        ];
        if row.len() != ROW_KEYS.len()
            || ROW_KEYS.iter().any(|key| !row.contains_key(*key))
            || row["ticker"].as_str() != Some(&tickers[i])
        {
            return Err(bad());
        }
        let count = row["own_sample_count"].as_u64().ok_or_else(bad)?;
        let paired = row["paired_sample_count"].as_u64().ok_or_else(bad)?;
        if paired > count {
            return Err(bad());
        }
        let own_bounds = [
            date(&row["holding_start"])?,
            date(&row["own_start"])?,
            date(&row["own_end"])?,
        ];
        let pair_bounds = [
            date(&row["paired_holding_start"])?,
            date(&row["paired_start"])?,
            date(&row["paired_end"])?,
        ];
        for (n, bounds) in [(count, own_bounds), (paired, pair_bounds)] {
            match bounds {
                [None, None, None] if n == 0 => {}
                [Some(open), Some(start), Some(end)]
                    if n > 0
                        && open < start
                        && start <= end
                        && (n == 1) == (start == end)
                        && n <= (end - start).whole_days() as u64 + 1 => {}
                _ => return Err(bad()),
            }
        }
        if paired > 0
            && (pair_bounds[0] < own_bounds[0]
                || pair_bounds[1] < own_bounds[1]
                || pair_bounds[2] > own_bounds[2])
        {
            return Err(bad());
        }
        if tickers[i] == benchmark && (paired != count || pair_bounds != own_bounds) {
            return Err(bad());
        }
        for (m, values) in metric_values.iter().enumerate() {
            let value = values[i];
            let permitted_infinity = matches!(m, 3 | 4 | 5 | 10 | 15 | 16 | 19 | 20);
            if value.is_infinite() && !permitted_infinity {
                return Err(bad());
            }
            if (count == 0
                || matches!(m, 2 | 3) && count < 2
                || m == 11 && count < 3
                || m == 12 && count < 4
                || matches!(m, 9 | 10 | 21) && paired < 2)
                && !value.is_nan()
            {
                return Err(bad());
            }
            if value.is_nan() {
                continue;
            }
            if matches!(m, 0 | 7 | 8 | 13) && value < -1.0
                || m == 6 && !(-1.0..=0.0).contains(&value)
                || matches!(m, 2 | 9 | 14 | 17 | 18 | 21) && value < 0.0
                || matches!(m, 17 | 18 | 21) && value > 1.0
                || matches!(m, 15 | 20) && value < 0.0
            {
                return Err(bad());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perf() -> Performance {
        use finstack_quant_core::dates::{Date, PeriodKind};
        use time::Month;
        let mut dates = (1..=7)
            .map(|day| Date::from_calendar_date(2025, Month::January, day).unwrap())
            .collect::<Vec<_>>();
        dates.push(Date::from_calendar_date(2026, Month::January, 1).unwrap());
        Performance::new(&crate::PerformancePanel {
            dates,
            series: vec![vec![-0.5, 0.0, 1.0, -0.75, 0.0, 3.0, 0.1]],
            kind: crate::PanelSeriesKind::Returns,
            ticker_names: vec!["A".into()],
            benchmark_ticker: Some("A".into()),
            frequency: Some(PeriodKind::Daily),
        })
        .unwrap()
    }

    #[test]
    fn drawdown_reward_population_and_cash_match_independent_arithmetic() {
        let perf = perf();
        let reward = 0.1 - 0.02;
        let burke_risk = ((0.25_f64 + 0.5625) / 7.0).sqrt();
        let martin_risk = ((0.5_f64 + 1.125) / 7.0).sqrt();
        assert!((perf.burke_ratio(0.02).unwrap()[0] - reward / burke_risk).abs() < 1e-14);
        assert!((perf.martin_ratio(0.02).unwrap()[0] - reward / martin_risk).abs() < 1e-14);
        assert!((perf.sterling_ratio(0.02, 1).unwrap()[0] - reward / 0.75).abs() < 1e-14);
        assert!((perf.sterling_ratio(0.02, 5).unwrap()[0] - reward / 0.625).abs() < 1e-14);
        assert!(perf.sterling_ratio(0.02, 0).is_err());
        assert!(perf.martin_ratio(-1.0).is_err());
        assert!(perf.burke_ratio(f64::INFINITY).is_err());
    }

    #[test]
    fn cdar_does_not_mistake_rounded_loss_for_terminal_event() {
        use finstack_quant_core::dates::parse_iso_date;
        let perf = Performance::new(&crate::PerformancePanel {
            dates: ["2025-01-01", "2025-01-02", "2025-01-03", "2025-01-04"]
                .iter()
                .map(|s| parse_iso_date(s).unwrap())
                .collect(),
            series: vec![vec![-0.9999999999999999, -0.9999999999999999, 0.0]],
            kind: crate::PanelSeriesKind::Returns,
            ticker_names: vec!["A".into()],
            benchmark_ticker: None,
            frequency: None,
        })
        .unwrap();
        assert_eq!(perf.cdar(0.95).unwrap(), [-1.0]);
        assert!(perf.summary(0.0, 0.0, 0.0, 0.95).is_err());
    }

    #[test]
    fn summary_retains_inputs_provenance_and_lossless_undefined_cells() {
        let mut perf = perf();
        let table = perf.summary(0.02, -0.01, 0.005, 0.95).unwrap();
        assert_eq!(table.metadata["mar"], -0.01);
        assert_eq!(table.metadata["omega_threshold"], 0.005);
        assert_eq!(table.metadata["rows"][0]["own_sample_count"], 7);
        assert_eq!(table.metadata["rows"][0]["paired_sample_count"], 7);
        assert_eq!(table.metadata["rows"][0]["holding_start"], "2025-01-01");
        let mut invalid = table;
        invalid.metadata.get_mut("rows").unwrap()[0]["paired_sample_count"] = 8.into();
        assert!(validate_summary(&invalid).is_err());
        let later = finstack_quant_core::dates::parse_iso_date("2027-01-01").unwrap();
        perf.reset_date_range(later, later).unwrap();
        let empty = perf.summary(0.02, 0.0, 0.0, 0.95).unwrap();
        assert_eq!(empty.row_count, 1);
        let wire = serde_json::to_string(&empty).unwrap();
        assert!(wire.contains("\"nan\""));
        let restored: TableEnvelope = serde_json::from_str(&wire).unwrap();
        validate_summary(&restored).unwrap();
        assert_eq!(perf.max_drawdown_duration().unwrap(), [None]);
        assert!(perf.summary(0.02, f64::NAN, 0.0, 0.95).is_err());
    }

    #[test]
    fn drawdown_reward_zero_and_numerical_failures_are_distinct() {
        assert!(reward_ratio(0.0, 0.0).unwrap().is_nan());
        assert_eq!(reward_ratio(0.1, 0.0).unwrap(), f64::INFINITY);
        assert_eq!(reward_ratio(-0.1, 0.0).unwrap(), f64::NEG_INFINITY);
        assert!(reward_ratio(f64::MAX, f64::MIN_POSITIVE).is_err());
        assert!(excess_reward(f64::MAX, -f64::MAX).is_err());
        assert!(reward_ratio(f64::NAN, 0.0).unwrap().is_nan());
    }
}
