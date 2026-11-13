//! Endpoint-assigned calendar/fiscal returns and descriptive occupied-bucket statistics.

use finstack_quant_core::dates::{Date, FiscalConfig, PeriodId, PeriodKind};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::wire::NonFiniteFields;
use serde::{Deserialize, Serialize};

use super::returns::{comp_total, validate_history};

fn invalid(message: impl Into<String>) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Validation(message.into())
}

/// One occupied calendar bucket, labeled by its last actual return endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "PeriodicReturnWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct PeriodicReturn {
    /// Last actual observed return endpoint in this bucket.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Finite compounded decimal simple return, at least `-1`.
    pub value: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PeriodicReturnWire {
    date: Date,
    value: f64,
}

impl TryFrom<PeriodicReturnWire> for PeriodicReturn {
    type Error = finstack_quant_core::Error;
    fn try_from(wire: PeriodicReturnWire) -> Result<Self, Self::Error> {
        if !wire.value.is_finite() || wire.value < -1.0 {
            return Err(invalid("periodic return must be finite and at least -1"));
        }
        Ok(Self {
            date: wire.date,
            value: wire.value,
        })
    }
}

/// Compact actual holding coverage for one selected lookback sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "LookbackSampleWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct LookbackSample {
    /// Number of original return intervals selected, rather than calendar buckets.
    pub n_obs: usize,
    /// Actual valuation immediately before the first selected return; absent when empty.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub holding_start: Option<Date>,
    /// Actual final selected return endpoint; absent when empty.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub holding_end: Option<Date>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookbackSampleWire {
    n_obs: usize,
    holding_start: Option<Date>,
    holding_end: Option<Date>,
}

impl TryFrom<LookbackSampleWire> for LookbackSample {
    type Error = finstack_quant_core::Error;
    fn try_from(wire: LookbackSampleWire) -> Result<Self, Self::Error> {
        let sample = Self {
            n_obs: wire.n_obs,
            holding_start: wire.holding_start,
            holding_end: wire.holding_end,
        };
        sample.validate()?;
        Ok(sample)
    }
}

impl LookbackSample {
    pub(crate) fn validate(&self) -> finstack_quant_core::Result<()> {
        match (self.n_obs, self.holding_start, self.holding_end) {
            (0, None, None) => Ok(()),
            (n, Some(start), Some(end)) if n > 0 && start < end => {
                let days = (end - start).whole_days();
                if n as u128 <= days as u128 {
                    Ok(())
                } else {
                    Err(invalid(
                        "lookback observation count exceeds its actual holding days",
                    ))
                }
            }
            _ => Err(invalid(
                "lookback count and actual holding bounds are inconsistent",
            )),
        }
    }
}

/// Equal-weight statistics of occupied aggregation buckets, including partial buckets.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "PeriodStatsAdmission")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct PeriodStats {
    /// Number of occupied aggregation buckets, including neutral returns.
    pub n_obs: usize,
    /// Number of strictly positive bucket returns.
    pub wins: usize,
    /// Number of strictly negative bucket returns.
    pub losses: usize,
    /// Largest bucket return, or undefined for no buckets.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub best: f64,
    /// Smallest bucket return, or undefined for no buckets.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub worst: f64,
    /// Longest strictly positive run among successive occupied buckets; missing buckets do not interrupt it.
    pub consecutive_wins: usize,
    /// Longest strictly negative run among successive occupied buckets; observed zero interrupts it.
    pub consecutive_losses: usize,
    /// Positive bucket count divided by all occupied buckets; undefined when empty.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub win_rate: f64,
    /// Arithmetic mean over all occupied bucket returns; undefined when empty.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub avg_return: f64,
    /// Conditional positive-return mean; undefined when no positive bucket exists.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub avg_win: f64,
    /// Conditional negative-return mean, retaining its negative sign; undefined when absent.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub avg_loss: f64,
    /// Conditional mean gain divided by mean loss magnitude; undefined unless both groups exist.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub payoff_ratio: f64,
    /// Sum of gains divided by loss magnitudes; positive infinity for gains without losses.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub profit_factor: f64,
    /// Cardinal Profitability Construct adaptation `profit_factor * win_rate * payoff_ratio`; requires both groups.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub cpc_index: f64,
    /// Signed loss-normalized binary risk fraction `p - q/payoff`, with decisive-bucket probabilities; requires both groups.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub kelly_risk_fraction: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "PeriodStats", deny_unknown_fields)]
struct PeriodStatsWire {
    n_obs: usize,
    wins: usize,
    losses: usize,
    consecutive_wins: usize,
    consecutive_losses: usize,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    best: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    worst: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    win_rate: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    avg_return: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    avg_win: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    avg_loss: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    payoff_ratio: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    profit_factor: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    cpc_index: f64,
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    kelly_risk_fraction: f64,
}

impl Serialize for PeriodStats {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate().map_err(serde::ser::Error::custom)?;
        PeriodStatsWire::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for PeriodStatsAdmission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        PeriodStatsWire::deserialize(deserializer).map(Self)
    }
}

struct PeriodStatsAdmission(PeriodStats);

impl TryFrom<PeriodStatsAdmission> for PeriodStats {
    type Error = finstack_quant_core::Error;
    fn try_from(value: PeriodStatsAdmission) -> Result<Self, Self::Error> {
        value.0.validate()?;
        Ok(value.0)
    }
}

impl NonFiniteFields for PeriodStats {
    const NON_FINITE_FIELDS: &'static [&'static str] = &[
        "best",
        "worst",
        "win_rate",
        "avg_return",
        "avg_win",
        "avg_loss",
        "payoff_ratio",
        "profit_factor",
        "cpc_index",
        "kelly_risk_fraction",
    ];
}

impl PeriodStats {
    fn empty() -> Self {
        Self {
            n_obs: 0,
            wins: 0,
            losses: 0,
            best: f64::NAN,
            worst: f64::NAN,
            consecutive_wins: 0,
            consecutive_losses: 0,
            win_rate: f64::NAN,
            avg_return: f64::NAN,
            avg_win: f64::NAN,
            avg_loss: f64::NAN,
            payoff_ratio: f64::NAN,
            profit_factor: f64::NAN,
            cpc_index: f64::NAN,
            kelly_risk_fraction: f64::NAN,
        }
    }

    fn validate(&self) -> finstack_quant_core::Result<()> {
        if self
            .wins
            .checked_add(self.losses)
            .is_none_or(|n| n > self.n_obs)
            || self.consecutive_wins > self.wins
            || self.consecutive_losses > self.losses
            || (self.wins == 0) != (self.consecutive_wins == 0)
            || (self.losses == 0) != (self.consecutive_losses == 0)
        {
            return Err(invalid(
                "period statistics counts and streaks are inconsistent",
            ));
        }
        let min_winning_streak = if self.wins > 0 {
            self.wins.div_ceil(self.n_obs - self.wins + 1)
        } else {
            0
        };
        let min_losing_streak = if self.losses > 0 {
            self.losses.div_ceil(self.n_obs - self.losses + 1)
        } else {
            0
        };
        if self.consecutive_wins < min_winning_streak || self.consecutive_losses < min_losing_streak
        {
            return Err(invalid("period streak is shorter than its counts permit"));
        }
        if (self.wins == self.n_obs
            && self.n_obs > 0
            && (self.consecutive_wins != self.n_obs || self.worst <= 0.0))
            || (self.losses == self.n_obs
                && self.n_obs > 0
                && (self.consecutive_losses != self.n_obs || self.best >= 0.0))
        {
            return Err(invalid(
                "all-win/all-loss period statistics require their full occupied streak and strict extrema",
            ));
        }
        if self.n_obs == 0 {
            if [
                self.best,
                self.worst,
                self.win_rate,
                self.avg_return,
                self.avg_win,
                self.avg_loss,
                self.payoff_ratio,
                self.profit_factor,
                self.cpc_index,
                self.kelly_risk_fraction,
            ]
            .iter()
            .any(|value| !value.is_nan())
            {
                return Err(invalid(
                    "empty period statistics must have undefined floating estimates",
                ));
            }
            return Ok(());
        }
        if !self.best.is_finite()
            || !self.worst.is_finite()
            || self.worst < -1.0
            || self.best < self.worst
            || !self.avg_return.is_finite()
            || self.avg_return < self.worst
            || self.avg_return > self.best
            || self
                .win_rate
                .partial_cmp(&(self.wins as f64 / self.n_obs as f64))
                != Some(std::cmp::Ordering::Equal)
            || (self.wins == 0 && self.best > 0.0)
            || (self.losses == 0 && self.worst < 0.0)
            || (self.wins > 0 && self.best <= 0.0)
            || (self.losses > 0 && self.worst >= 0.0)
            || (self.n_obs - self.wins - self.losses > 0 && (self.best < 0.0 || self.worst > 0.0))
        {
            return Err(invalid(
                "period statistics extrema, mean or win rate are inconsistent",
            ));
        }
        if (self.wins == 0 && !self.avg_win.is_nan())
            || (self.wins > 0
                && (!self.avg_win.is_finite() || self.avg_win <= 0.0 || self.avg_win > self.best))
            || (self.losses == 0 && !self.avg_loss.is_nan())
            || (self.losses > 0
                && (!self.avg_loss.is_finite()
                    || self.avg_loss >= 0.0
                    || self.avg_loss < self.worst))
        {
            return Err(invalid(
                "period statistics conditional mean availability is inconsistent",
            ));
        }
        if self.wins > 0 && self.losses > 0 {
            if !self.payoff_ratio.is_finite()
                || self.payoff_ratio <= 0.0
                || !self.profit_factor.is_finite()
                || self.profit_factor <= 0.0
                || !self.cpc_index.is_finite()
                || self.cpc_index <= 0.0
                || !self.kelly_risk_fraction.is_finite()
                || self.kelly_risk_fraction > 1.0
            {
                return Err(invalid(
                    "two-sided period statistics require finite representable ratios",
                ));
            }
        } else {
            if !self.payoff_ratio.is_nan()
                || !self.cpc_index.is_nan()
                || !self.kelly_risk_fraction.is_nan()
            {
                return Err(invalid(
                    "two-sided period estimates are undefined without both groups",
                ));
            }
            let profit_ok = if self.wins > 0 {
                self.profit_factor == f64::INFINITY
            } else if self.losses > 0 {
                self.profit_factor == 0.0
            } else {
                self.profit_factor.is_nan()
            };
            if !profit_ok {
                return Err(invalid("profit-factor availability is inconsistent"));
            }
        }
        Ok(())
    }
}

/// Compound each occupied bucket and label it by its actual last endpoint.
pub(crate) fn group_period_buckets(
    dates: &[Date],
    returns: &[f64],
    frequency: PeriodKind,
    fiscal_config: Option<FiscalConfig>,
) -> finstack_quant_core::Result<Vec<PeriodicReturn>> {
    if dates.len() != returns.len() || dates.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid(
            "period grouping requires equal date/return lengths and strictly ascending dates",
        ));
    }
    validate_history(returns)?;
    let Some(&first_date) = dates.first() else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    let mut current_pid = PeriodId::from_date(first_date, frequency, fiscal_config)?;
    let mut start = 0;
    for i in 1..dates.len() {
        let pid = PeriodId::from_date(dates[i], frequency, fiscal_config)?;
        if pid != current_pid {
            result.push(PeriodicReturn {
                date: dates[i - 1],
                value: comp_total(&returns[start..i])?,
            });
            start = i;
            current_pid = pid;
        }
    }
    result.push(PeriodicReturn {
        date: dates[dates.len() - 1],
        value: comp_total(&returns[start..])?,
    });
    Ok(result)
}

fn checked_positive_exp(log_value: f64, label: &str) -> finstack_quant_core::Result<f64> {
    let value = log_value.exp();
    if !value.is_finite() || value == 0.0 {
        Err(finstack_quant_core::Error::Internal(format!(
            "{label} is not representable"
        )))
    } else {
        Ok(value)
    }
}

pub(crate) fn period_stats_inner(
    returns: impl Iterator<Item = f64>,
) -> finstack_quant_core::Result<PeriodStats> {
    let returns: Vec<f64> = returns.collect();
    // Bucket reports can round positive wealth to -1 without terminating later source buckets.
    if returns
        .iter()
        .any(|value| !value.is_finite() || *value < -1.0)
    {
        return Err(invalid(
            "period statistics require finite bucket returns at least -1",
        ));
    }
    if returns.is_empty() {
        return Ok(PeriodStats::empty());
    }
    let scale = returns.iter().copied().map(f64::abs).fold(0.0, f64::max);
    let mut stats = PeriodStats::empty();
    stats.n_obs = returns.len();
    stats.best = returns.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    stats.worst = returns.iter().copied().fold(f64::INFINITY, f64::min);
    let mut total_sum = NeumaierAccumulator::new();
    let mut gains = NeumaierAccumulator::new();
    let mut losses = NeumaierAccumulator::new();
    let mut raw_total = NeumaierAccumulator::new();
    let mut raw_gains = NeumaierAccumulator::new();
    let mut raw_losses = NeumaierAccumulator::new();
    let gain_scale = stats.best.max(0.0);
    let loss_scale = (-stats.worst).max(0.0);
    let mut win_streak = 0;
    let mut loss_streak = 0;
    for &value in &returns {
        raw_total.add(value);
        if scale > 0.0 {
            total_sum.add(value / scale);
        }
        if value > 0.0 {
            stats.wins += 1;
            win_streak += 1;
            loss_streak = 0;
            stats.consecutive_wins = stats.consecutive_wins.max(win_streak);
            gains.add(value / gain_scale);
            raw_gains.add(value);
        } else if value < 0.0 {
            stats.losses += 1;
            loss_streak += 1;
            win_streak = 0;
            stats.consecutive_losses = stats.consecutive_losses.max(loss_streak);
            losses.add(-value / loss_scale);
            raw_losses.add(-value);
        } else {
            win_streak = 0;
            loss_streak = 0;
        }
    }
    let net = raw_total.total();
    stats.avg_return = if net.is_finite() {
        net / stats.n_obs as f64
    } else {
        scale * (total_sum.total() / stats.n_obs as f64)
    };
    if stats.avg_return == 0.0
        && (net.is_finite() && net != 0.0 || !net.is_finite() && total_sum.total() != 0.0)
    {
        return Err(finstack_quant_core::Error::Internal(
            "mean bucket return underflowed".into(),
        ));
    }
    stats.win_rate = stats.wins as f64 / stats.n_obs as f64;
    if stats.wins > 0 {
        stats.avg_win = if raw_gains.total().is_finite() {
            raw_gains.total() / stats.wins as f64
        } else {
            gain_scale * (gains.total() / stats.wins as f64)
        };
    }
    if stats.losses > 0 {
        stats.avg_loss = -raw_losses.total() / stats.losses as f64;
    }
    if (stats.wins > 0 && stats.avg_win == 0.0) || (stats.losses > 0 && stats.avg_loss == 0.0) {
        return Err(finstack_quant_core::Error::Internal(
            "conditional mean is not representable".into(),
        ));
    }
    if stats.wins > 0 && stats.losses > 0 {
        let raw_gain_sum = raw_gains.total();
        let raw_loss_sum = raw_losses.total();
        let direct_pf = raw_gain_sum / raw_loss_sum;
        stats.profit_factor = if direct_pf.is_finite() && direct_pf > 0.0 {
            direct_pf
        } else {
            // Scale only when raw gross sums or intermediate quotients exceed
            // binary64 range; ordinary direct division preserves adjacent ratios.
            let direct = (gain_scale / loss_scale) * (gains.total() / losses.total());
            if direct.is_finite() && direct > 0.0 {
                direct
            } else {
                checked_positive_exp(
                    gain_scale.ln() - loss_scale.ln() + gains.total().ln() - losses.total().ln(),
                    "profit factor",
                )?
            }
        };
        stats.payoff_ratio = stats.avg_win / -stats.avg_loss;
        if !stats.payoff_ratio.is_finite() || stats.payoff_ratio == 0.0 {
            return Err(finstack_quant_core::Error::Internal(
                "payoff ratio is not representable".into(),
            ));
        }
        // The inner factor L/N is at most one, so this order cannot overflow
        // an intermediate when the requested final CPC is representable.
        stats.cpc_index = stats.profit_factor
            * (stats.profit_factor * (stats.losses as f64 / stats.n_obs as f64));
        if !stats.cpc_index.is_finite() || stats.cpc_index == 0.0 {
            return Err(finstack_quant_core::Error::Internal(
                "CPC index is not representable".into(),
            ));
        }
        let p = stats.wins as f64 / (stats.wins + stats.losses) as f64;
        // K = p * (G-D)/G. A compensated direct net retains a tiny signed
        // Kelly estimate when gains and losses nearly cancel.
        stats.kelly_risk_fraction = if net.is_finite() && raw_gain_sum.is_finite() {
            p * (net / raw_gain_sum)
        } else {
            p * ((total_sum.total() / gains.total()) * (scale / gain_scale))
        };
        if !stats.kelly_risk_fraction.is_finite()
            || (stats.kelly_risk_fraction == 0.0
                && ((net.is_finite() && net != 0.0)
                    || (!net.is_finite() && total_sum.total() != 0.0)))
        {
            return Err(finstack_quant_core::Error::Internal(
                "Kelly risk fraction is not representable".into(),
            ));
        }
    } else if stats.wins > 0 {
        stats.profit_factor = f64::INFINITY;
    } else if stats.losses > 0 {
        stats.profit_factor = 0.0;
    }
    stats.validate()?;
    Ok(stats)
}
#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodKind;
    use time::Month;

    fn d(y: i32, m: u8, day: u8) -> Date {
        finstack_quant_core::dates::create_date(y, Month::try_from(m).expect("valid month"), day)
            .expect("valid date")
    }

    #[test]
    fn group_period_buckets_monthly_reconciles() {
        let dates = vec![
            d(2021, 1, 5),
            d(2021, 1, 20),
            d(2021, 2, 10),
            d(2021, 2, 25),
        ];
        let rets = vec![0.01, 0.02, -0.01, 0.03];
        let out = group_period_buckets(&dates, &rets, PeriodKind::Monthly, None)
            .expect("representable calendar periods");

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].date, d(2021, 1, 20));
        assert_eq!(out[1].date, d(2021, 2, 25));
        assert!((out[0].value - (1.01 * 1.02 - 1.0)).abs() < 1e-12);
        assert!((out[1].value - (0.99 * 1.03 - 1.0)).abs() < 1e-12);
        let total = (1.0 + out[0].value) * (1.0 + out[1].value) - 1.0;
        let expected = 1.01 * 1.02 * 0.99 * 1.03 - 1.0;
        assert!((total - expected).abs() < 1e-12);
    }

    #[test]
    fn group_period_buckets_empty_is_empty() {
        let out = group_period_buckets(&[], &[], PeriodKind::Monthly, None)
            .expect("representable calendar periods");
        assert!(out.is_empty());
    }

    #[test]
    fn period_stats_non_finite_fields_match_serde_sentinels() {
        let stats = PeriodStats {
            n_obs: 0,
            wins: 0,
            losses: 0,
            best: f64::NAN,
            worst: f64::NAN,
            consecutive_wins: 0,
            consecutive_losses: 0,
            win_rate: f64::NAN,
            avg_return: f64::NAN,
            avg_win: f64::NAN,
            avg_loss: f64::NAN,
            payoff_ratio: f64::NAN,
            profit_factor: f64::NAN,
            cpc_index: f64::NAN,
            kelly_risk_fraction: f64::NAN,
        };
        crate::test_support::assert_non_finite_fields(&stats);
    }

    #[test]
    fn group_by_monthly() {
        let dates = vec![d(2025, 1, 2), d(2025, 1, 3), d(2025, 2, 3), d(2025, 2, 4)];
        let returns = vec![0.01, 0.02, -0.01, 0.03];
        let grouped = group_period_buckets(&dates, &returns, PeriodKind::Monthly, None)
            .expect("representable calendar periods");
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].date, d(2025, 1, 3));
        assert_eq!(grouped[1].date, d(2025, 2, 4));
    }

    #[test]
    fn period_stats_basic() {
        let stats = period_stats_inner([0.05, -0.02, 0.03, 0.01].into_iter()).unwrap();
        assert!((stats.best - 0.05).abs() < 1e-12);
        assert!((stats.worst - (-0.02)).abs() < 1e-12);
        assert!((stats.win_rate - 0.75).abs() < 1e-12);
    }

    #[test]
    fn period_stats_empty() {
        let stats = period_stats_inner(std::iter::empty()).unwrap();
        assert!(stats.win_rate.is_nan());
    }

    #[test]
    fn period_stats_all_winning_has_only_defined_empirical_profit_factor() {
        let stats = period_stats_inner([0.02, 0.01, 0.03].into_iter()).unwrap();
        assert!(stats.payoff_ratio.is_nan());
        assert!(stats.profit_factor.is_infinite());
        assert!(stats.cpc_index.is_nan());
        assert!(stats.kelly_risk_fraction.is_nan());
    }

    #[test]
    fn period_stats_without_gains_or_losses_has_undefined_ratios() {
        let stats = period_stats_inner([0.0, 0.0].into_iter()).unwrap();
        assert!(stats.payoff_ratio.is_nan());
        assert!(stats.profit_factor.is_nan());
        assert!(stats.cpc_index.is_nan());
        assert!(stats.kelly_risk_fraction.is_nan());
    }

    #[test]
    fn neutral_bucket_changes_cpc_but_not_loss_normalized_kelly() {
        let three = period_stats_inner([0.02, -0.01, 0.0].into_iter()).unwrap();
        let two = period_stats_inner([0.02, -0.01].into_iter()).unwrap();
        assert_eq!((three.n_obs, three.wins, three.losses), (3, 1, 1));
        assert!((three.payoff_ratio - 2.0).abs() < 1e-14);
        assert!((three.profit_factor - 2.0).abs() < 1e-14);
        assert!((three.cpc_index - 4.0 / 3.0).abs() < 1e-14);
        assert!((two.cpc_index - 2.0).abs() < 1e-14);
        assert!((three.kelly_risk_fraction - 0.25).abs() < 1e-14);
        assert!((two.kelly_risk_fraction - three.kelly_risk_fraction).abs() < 1e-14);
    }

    #[test]
    fn losses_without_gains_preserve_defined_zero_profit_factor() {
        let stats = period_stats_inner([-0.01, -0.03].into_iter()).unwrap();
        assert_eq!(stats.profit_factor, 0.0);
        assert!(stats.avg_win.is_nan());
        assert!((stats.avg_loss + 0.02).abs() < 1e-16);
        assert!(stats.kelly_risk_fraction.is_nan());
    }

    #[test]
    fn period_stat_wire_preserves_all_undefined_fields_and_rejects_bad_counts() {
        let stats = period_stats_inner(std::iter::empty()).unwrap();
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["best"], "nan");
        let restored: PeriodStats = serde_json::from_value(json.clone()).unwrap();
        assert!(restored.avg_return.is_nan());
        let mut invalid = json;
        invalid["wins"] = serde_json::json!(1);
        assert!(serde_json::from_value::<PeriodStats>(invalid).is_err());
    }

    #[test]
    fn period_grouping_never_truncates_or_reorders_input() {
        let dates = [d(2025, 1, 2), d(2025, 1, 3)];
        assert!(group_period_buckets(&dates, &[0.01], PeriodKind::Monthly, None).is_err());
        assert!(
            group_period_buckets(
                &[dates[1], dates[0]],
                &[0.01, 0.02],
                PeriodKind::Monthly,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn rounded_bucket_loss_does_not_terminate_original_history() {
        let dates = [d(2025, 1, 2), d(2025, 1, 3), d(2025, 2, 3)];
        let returns = [-0.999999999, -0.999999999, 0.02];
        let buckets = group_period_buckets(&dates, &returns, PeriodKind::Monthly, None).unwrap();
        assert_eq!(buckets[0].value, -1.0);
        assert!((buckets[1].value - 0.02).abs() < 1e-16);
        assert!(period_stats_inner(buckets.into_iter().map(|bucket| bucket.value)).is_ok());
    }

    #[test]
    fn scaled_means_do_not_overflow_defined_one_sided_statistics() {
        let stats = period_stats_inner([f64::MAX, f64::MAX].into_iter()).unwrap();
        assert_eq!(stats.avg_return, f64::MAX);
        assert_eq!(stats.avg_win, f64::MAX);
        assert_eq!(stats.profit_factor, f64::INFINITY);
        assert!(stats.cpc_index.is_nan());
    }

    #[test]
    fn missing_calendar_bucket_does_not_interrupt_occupied_streak() {
        let buckets = group_period_buckets(
            &[d(2025, 1, 3), d(2025, 3, 3)],
            &[0.01, 0.02],
            PeriodKind::Monthly,
            None,
        )
        .unwrap();
        let stats = period_stats_inner(buckets.into_iter().map(|bucket| bucket.value)).unwrap();
        assert_eq!(stats.n_obs, 2);
        assert_eq!(stats.consecutive_wins, 2);
    }

    #[test]
    fn unrepresentable_nonzero_bucket_mean_is_a_calculation_failure() {
        let error = period_stats_inner([f64::from_bits(1), 0.0].into_iter()).unwrap_err();
        assert!(matches!(error, finstack_quant_core::Error::Internal(_)));
    }

    #[test]
    fn adjacent_gain_loss_ratio_retains_signed_kelly_difference() {
        let loss = 0.01_f64;
        let gain = loss.next_up();
        let stats = period_stats_inner([gain, -loss].into_iter()).unwrap();
        assert_eq!(stats.profit_factor, gain / loss);
        assert_eq!(stats.payoff_ratio, gain / loss);
        let expected = 0.5 * ((gain - loss) / gain);
        assert_eq!(stats.kelly_risk_fraction, expected);
        assert!(stats.kelly_risk_fraction > 0.0);
    }

    #[test]
    fn restoration_rejects_impossible_unanimous_streak() {
        let stats = period_stats_inner([0.02, 0.03].into_iter()).unwrap();
        let mut wire = serde_json::to_value(stats).unwrap();
        wire["consecutive_wins"] = serde_json::json!(1);
        assert!(serde_json::from_value::<PeriodStats>(wire).is_err());
    }

    #[test]
    fn restoration_enforces_streak_count_feasibility_with_mixed_outcomes() {
        let stats = period_stats_inner([0.02, 0.03, -0.01, 0.04].into_iter()).unwrap();
        let mut wire = serde_json::to_value(stats).unwrap();
        wire["consecutive_wins"] = serde_json::json!(1);
        assert!(serde_json::from_value::<PeriodStats>(wire).is_err());
    }

    #[test]
    fn weekly_grouping_uses_iso_week_boundaries_across_year_end() {
        let dates = vec![d(2024, 12, 30), d(2024, 12, 31), d(2025, 1, 2)];
        let returns = vec![0.01, 0.02, 0.03];

        let grouped = group_period_buckets(&dates, &returns, PeriodKind::Weekly, None)
            .expect("representable calendar periods");

        assert_eq!(
            grouped.len(),
            1,
            "all observations should fall in one ISO week"
        );
        assert_eq!(grouped[0].date, d(2025, 1, 2));
        assert!((grouped[0].value - (1.01 * 1.02 * 1.03 - 1.0)).abs() < 1e-12);
    }
}

#[cfg(test)]
mod fiscal_reconciliation {
    use super::*;
    use finstack_quant_core::dates::parse_iso_date;

    #[test]
    fn fiscal_buckets_reconcile_at_every_frequency() {
        let dates = [
            "2024-04-05",
            "2024-04-06",
            "2024-05-05",
            "2024-05-06",
            "2024-10-06",
        ]
        .map(|value| parse_iso_date(value).unwrap());
        let returns = [0.01, -0.02, 0.03, 0.04, -0.01];
        for kind in [
            PeriodKind::Daily,
            PeriodKind::Weekly,
            PeriodKind::Monthly,
            PeriodKind::Quarterly,
            PeriodKind::SemiAnnual,
            PeriodKind::Annual,
        ] {
            let grouped = group_period_buckets(
                &dates,
                &returns,
                kind,
                Some(FiscalConfig::new(4, 6).expect("valid fiscal start")),
            )
            .unwrap();
            let linked: Vec<_> = grouped.iter().map(|point| point.value).collect();
            assert!(
                (comp_total(&linked).unwrap() - comp_total(&returns).unwrap()).abs() < 1e-12,
                "{kind}"
            );
            assert_eq!(grouped.last().unwrap().date, *dates.last().unwrap());
        }
    }
}
