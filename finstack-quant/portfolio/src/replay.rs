//! Historical scenario replay for portfolios.
//!
//! Replays a static portfolio through a sequence of dated market snapshots,
//! producing configurable P&L and attribution output at each step.
//!
//! `daily_mtm_pnl`, `cumulative_mtm_pnl`, `total_mtm_pnl`, and
//! `max_mtm_drawdown*` describe changes in marked holdings and exclude paid
//! cashflows. In full-attribution mode, `attribution.total_pnl` includes those
//! payments. A coupon or redemption therefore creates an expected difference
//! between these explicitly distinct measures; cash is not reinvested here.
//!
//! This module is only available when the `scenarios` feature is enabled.

use crate::attribution::{
    attribution_endpoint_profile, reduce_method_owned_prepared, reduce_metrics_based_prepared,
    PortfolioAttribution,
};
use crate::error::{Error, Result};
use crate::evaluation::{EvaluationMetricProfile, EvaluationProfile, PortfolioEvaluationPlan};
use crate::valuation::{PortfolioValuation, RequestedMetrics};
use finstack_quant_attribution::{default_attribution_metrics, AttributionMethod};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

const STRICT_ENDPOINT_BATCH_SIZE: usize = 8;

/// What to compute at each replay step.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    /// Just portfolio PV at each date.
    PvOnly,
    /// PV + daily/cumulative mark-to-market P&L.
    PvAndPnl,
    /// PV + P&L + per-position factor decomposition.
    FullAttribution,
}

/// What to do when a single snapshot fails to revalue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayErrorPolicy {
    /// Fail the entire replay on the first valuation error. This is the
    /// historical behaviour and the right default for hedge-fund risk
    /// reporting where a missing snapshot must surface, not be silently
    /// skipped. Default.
    #[default]
    Strict,
    /// Skip snapshots that fail to revalue and continue. Failed dates are
    /// reported on `ReplayResult::skipped_dates` so callers can surface them
    /// to ops without losing the rest of the timeline. Use this when
    /// running ad-hoc backfills where a single bad day shouldn't discard
    /// weeks of computed steps.
    BestEffort,
}

/// Configuration for a replay run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayConfig {
    /// What to compute at each step.
    pub mode: ReplayMode,
    /// Attribution method (only used in `FullAttribution` mode).
    #[serde(default)]
    pub attribution_method: finstack_quant_attribution::AttributionMethod,
    /// Valuation options compiled into each replay evaluation profile.
    #[serde(default)]
    pub valuation_options: crate::valuation::PortfolioValuationOptions,
    /// Strict-vs-best-effort handling of per-snapshot failures.
    #[serde(default)]
    pub on_error: ReplayErrorPolicy,
}

/// A dated snapshot in the JSON wire format used by bindings.
///
/// Shape: `{"date": "YYYY-MM-DD", "market": <MarketContext JSON>}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonSnapshot {
    #[serde(with = "finstack_quant_core::wire::date")]
    date: Date,
    market: MarketContext,
}

/// A dated sequence of market snapshots.
///
/// Invariants enforced by [`ReplayTimeline::new`]:
/// - Non-empty
/// - Sorted by date ascending
/// - No duplicate dates
pub struct ReplayTimeline {
    snapshots: Vec<(Date, MarketContext)>,
}

impl ReplayTimeline {
    /// Parse a JSON array of `{"date": ..., "market": ...}` snapshots
    /// and construct a validated timeline.
    ///
    /// This is the canonical entry point used by the Python and WASM bindings;
    /// they do not parse snapshots themselves.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] when the JSON or any ISO-8601 date is
    /// invalid, or when the decoded snapshots violate timeline ordering rules.
    pub fn from_json_snapshots(json: &str) -> Result<Self> {
        let raw: Vec<JsonSnapshot> = serde_json::from_str(json)
            .map_err(|e| Error::InvalidInput(format!("invalid snapshots JSON: {e}")))?;
        let snapshots = raw
            .into_iter()
            .map(|entry| (entry.date, entry.market))
            .collect();
        Self::new(snapshots)
    }

    /// Create a new timeline from a vector of `(date, market)` pairs.
    ///
    /// Returns an error if the vector is empty, not sorted by date, or
    /// contains duplicate dates.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] for an empty timeline or dates that are
    /// not strictly ascending.
    ///
    /// # Arguments
    ///
    /// * `snapshots` - Chronologically ordered portfolio snapshots to replay.
    pub fn new(snapshots: Vec<(Date, MarketContext)>) -> Result<Self> {
        if snapshots.is_empty() {
            return Err(Error::InvalidInput(
                "ReplayTimeline must be non-empty".into(),
            ));
        }
        for window in snapshots.windows(2) {
            let (d0, _) = &window[0];
            let (d1, _) = &window[1];
            if d1 <= d0 {
                return Err(Error::InvalidInput(format!(
                    "ReplayTimeline dates must be strictly ascending, found {d0} >= {d1}"
                )));
            }
        }
        Ok(Self { snapshots })
    }

    /// Number of snapshots.
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Whether the timeline is empty (always false after construction).
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// First and last dates in the timeline.
    pub fn date_range(&self) -> (Date, Date) {
        // Indexing is safe: new() enforces non-empty.
        (
            self.snapshots[0].0,
            self.snapshots[self.snapshots.len() - 1].0,
        )
    }

    /// Iterate over `(date, market)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = &(Date, MarketContext)> {
        self.snapshots.iter()
    }
}

/// Output for a single replay step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayStep {
    /// Valuation date.
    pub date: Date,
    /// Full portfolio valuation at this date.
    pub valuation: PortfolioValuation,
    /// Daily mark-to-market P&L, excluding paid cashflows (this PV minus prior PV). `None` at step 0.
    pub daily_mtm_pnl: Option<Money>,
    /// Cumulative mark-to-market P&L, excluding paid cashflows (this PV minus initial PV). `None` at step 0.
    pub cumulative_mtm_pnl: Option<Money>,
    /// Factor attribution between prior step and this step. `None` at step 0
    /// and in non-attribution modes.
    pub attribution: Option<PortfolioAttribution>,
}

/// Aggregate statistics across the full replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplaySummary {
    /// First date in the timeline.
    pub start_date: Date,
    /// Last date in the timeline.
    pub end_date: Date,
    /// Number of steps (including step 0).
    pub num_steps: usize,
    /// Portfolio value at step 0.
    pub start_value: Money,
    /// Portfolio value at the last step.
    pub end_value: Money,
    /// Total mark-to-market P&L (end PV minus start PV), excluding paid cashflows.
    pub total_mtm_pnl: Money,
    /// Maximum mark-to-market drawdown from peak to trough, selected on the largest
    /// base-currency (dollar) decline from a running high-water mark.
    pub max_mtm_drawdown: Money,
    /// Maximum percentage mark-to-market drawdown, selected independently of
    /// [`max_mtm_drawdown`](Self::max_mtm_drawdown) as the largest `decline / peak`
    /// ratio over positive peaks. The dollar-largest and percentage-largest
    /// drawdowns can come from different peak/trough pairs (a small early
    /// peak can host the deepest relative loss). `0.0` when no positive peak
    /// ever existed: a percentage decline from a non-positive portfolio value
    /// is not meaningful.
    pub max_mtm_drawdown_pct: f64,
    /// Date of the peak before the maximum (dollar-selected) drawdown.
    pub max_mtm_drawdown_peak_date: Date,
    /// Date of the trough of the maximum (dollar-selected) drawdown.
    pub max_mtm_drawdown_trough_date: Date,
    /// Date of the peak before the maximum percentage-selected drawdown.
    /// `None` when no positive peak ever produced a decline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mtm_drawdown_pct_peak_date: Option<Date>,
    /// Date of the trough of the maximum percentage-selected drawdown.
    /// `None` when no positive peak ever produced a decline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_mtm_drawdown_pct_trough_date: Option<Date>,
}

/// Full output of a replay run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayResult {
    /// Per-step output.
    pub steps: Vec<ReplayStep>,
    /// Aggregate statistics.
    pub summary: ReplaySummary,
    /// Snapshots that were skipped because their valuation failed and the
    /// run was configured for [`ReplayErrorPolicy::BestEffort`]. Empty in
    /// strict mode (the run would have aborted instead).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_dates: Vec<(Date, String)>,
}

use crate::portfolio::Portfolio;
use finstack_quant_core::config::FinstackConfig;

fn replay_phase_profile(config: &ReplayConfig, metrics_attribution: bool) -> EvaluationProfile {
    let mut options = config.valuation_options.clone();
    if metrics_attribution {
        match &mut options.metrics {
            RequestedMetrics::Standard => {
                options.metrics = RequestedMetrics::StandardPlus(default_attribution_metrics());
            }
            RequestedMetrics::StandardPlus(extra) => {
                extra.extend(default_attribution_metrics());
            }
            RequestedMetrics::Only(_) => {}
        }
    }
    EvaluationProfile::from_options(&options)
}

fn phase_a_results(
    portfolio: &Portfolio,
    timeline: &ReplayTimeline,
    profile: EvaluationProfile,
    config: &FinstackConfig,
) -> Result<Vec<Result<PortfolioValuation>>> {
    let mut plan = PortfolioEvaluationPlan::new(config);
    let portfolio_state = plan.register_portfolio(portfolio);
    let jobs = timeline
        .snapshots
        .iter()
        .map(|(date, market)| {
            let market_state = plan.register_market(market, *date);
            plan.register_evaluation(
                market_state,
                portfolio_state,
                profile.clone(),
                crate::evaluation::PositionExecution::Auto,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let mut outcome = plan.execute();
    Ok(jobs
        .into_iter()
        .map(|job| outcome.take_valuation(job))
        .collect())
}

fn attribution_endpoint_batch(
    portfolio: &Portfolio,
    surviving: &[(Date, &MarketContext, Option<PortfolioValuation>)],
    endpoint_needed: &[bool],
    indices: RangeInclusive<usize>,
    profile: &EvaluationProfile,
    config: &FinstackConfig,
) -> Result<IndexMap<usize, Result<PortfolioValuation>>> {
    let needed: Vec<usize> = indices
        .filter(|&index| endpoint_needed.get(index).copied().unwrap_or(false))
        .collect();
    if needed.is_empty() {
        return Ok(IndexMap::new());
    }

    let mut plan = PortfolioEvaluationPlan::new(config);
    let portfolio_state = plan.register_portfolio(portfolio);
    let jobs = needed
        .iter()
        .map(|&index| {
            let (date, market, _) = &surviving[index];
            let market_state = plan.register_market(market, *date);
            plan.register_evaluation(
                market_state,
                portfolio_state,
                profile.clone(),
                crate::evaluation::PositionExecution::Auto,
            )
            .map(|job| (index, job))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut outcome = plan.execute();
    let mut endpoints = IndexMap::with_capacity(jobs.len());
    for (index, job) in jobs {
        endpoints.insert(index, outcome.take_valuation(job));
    }
    Ok(endpoints)
}

fn valuation_matches_endpoint_profile(
    portfolio: &Portfolio,
    valuation: &PortfolioValuation,
    profile: &EvaluationProfile,
    allow_complete_metric_superset: bool,
) -> bool {
    valuation.provenance.as_ref().is_some_and(|provenance| {
        let same_state = provenance.portfolio_state_id == portfolio.evaluation_state_id
            && provenance.base_currency == portfolio.base_currency;
        if !same_state {
            return false;
        }
        if provenance.profile == *profile {
            return true;
        }
        if !allow_complete_metric_superset
            || !valuation
                .position_values
                .values()
                .all(|value| value.risk_metrics_complete)
        {
            return false;
        }
        match (&provenance.profile.metrics, &profile.metrics) {
            (
                EvaluationMetricProfile::Metrics(actual),
                EvaluationMetricProfile::Metrics(required),
            ) => required.iter().all(|metric| actual.contains(metric)),
            _ => false,
        }
    })
}

fn endpoint_or_fallback<'a>(
    endpoint: Option<&'a Result<PortfolioValuation>>,
    fallback: &'a PortfolioValuation,
) -> Result<&'a PortfolioValuation> {
    match endpoint {
        Some(Ok(valuation)) => Ok(valuation),
        Some(Err(error)) => Err(error.clone()),
        None => Ok(fallback),
    }
}

/// Replay a portfolio through a sequence of dated market snapshots.
///
/// For each date in the timeline the portfolio is re-valued using the
/// corresponding [`MarketContext`].  Depending on [`ReplayMode`]:
///
/// * **`PvOnly`** -- only portfolio PV is recorded at each step.
/// * **`PvAndPnl`** -- daily and cumulative P&L are computed as well.
/// * **`FullAttribution`** -- P&L plus per-position factor decomposition.
///
/// Returns a [`ReplayResult`] containing the per-step detail and an
/// aggregate [`ReplaySummary`].
///
/// Under [`ReplayErrorPolicy::Strict`], the first failed valuation or
/// attribution aborts the replay. Under `BestEffort`, failed valuations are
/// recorded in `skipped_dates` and P&L is measured between consecutive
/// surviving dates; the call still fails if no snapshot can be valued.
///
/// # Errors
///
/// Returns valuation or attribution errors under the strict policy, failures
/// from an empty best-effort result, and errors while computing daily or
/// cumulative base-currency P&L (for example, an incompatible currency or
/// amount overflow).
///
/// # Arguments
///
/// * `portfolio` - Static portfolio definition valued at every timeline
///   snapshot; its base currency is used for replay P&L.
/// * `timeline` - Ordered dated market snapshots to replay; each date is
///   passed as the explicit valuation date for its snapshot.
/// * `config` - Replay mode, error policy, attribution settings, and
///   portfolio-valuation options.
/// * `finstack_config` - Library configuration for pricing conventions and
///   market-data resolution at each replay step.
pub fn replay_portfolio(
    portfolio: &Portfolio,
    timeline: &ReplayTimeline,
    config: &ReplayConfig,
    finstack_config: &FinstackConfig,
) -> Result<ReplayResult> {
    let compute_pnl = matches!(
        config.mode,
        ReplayMode::PvAndPnl | ReplayMode::FullAttribution
    );
    let compute_attribution = matches!(config.mode, ReplayMode::FullAttribution);
    let metrics_attribution =
        compute_attribution && matches!(config.attribution_method, AttributionMethod::MetricsBased);
    let phase_profile = replay_phase_profile(config, metrics_attribution);

    let valuation_results = phase_a_results(portfolio, timeline, phase_profile, finstack_config)?;

    let mut skipped_dates: Vec<(Date, String)> = Vec::new();
    let mut surviving: Vec<(Date, &MarketContext, Option<PortfolioValuation>)> =
        Vec::with_capacity(timeline.len());
    for ((date, market), result) in timeline.snapshots.iter().zip(valuation_results) {
        match result {
            Ok(v) => surviving.push((*date, market, Some(v))),
            Err(e) => match config.on_error {
                ReplayErrorPolicy::Strict => return Err(e),
                ReplayErrorPolicy::BestEffort => {
                    tracing::warn!(
                        date = %date,
                        error = %e,
                        "Replay snapshot skipped under best-effort policy"
                    );
                    skipped_dates.push((*date, e.to_string()));
                }
            },
        }
    }

    if surviving.is_empty() {
        return Err(Error::InvalidInput(format!(
            "Replay produced no valid steps: {} of {} snapshots failed under \
             best-effort policy. Inspect skipped_dates on the result for \
             the originating error messages.",
            skipped_dates.len(),
            timeline.len()
        )));
    }

    let endpoint_profile =
        compute_attribution.then(|| attribution_endpoint_profile(&config.attribution_method));
    let allow_complete_metric_superset = metrics_attribution
        && matches!(
            config.valuation_options.metrics,
            RequestedMetrics::Standard | RequestedMetrics::StandardPlus(_)
        );
    let endpoint_needed: Vec<bool> = surviving
        .iter()
        .map(|(_, _, valuation)| {
            endpoint_profile.as_ref().is_some_and(|required| {
                valuation.as_ref().is_none_or(|valuation| {
                    !valuation_matches_endpoint_profile(
                        portfolio,
                        valuation,
                        required,
                        allow_complete_metric_superset,
                    )
                })
            })
        })
        .collect();

    let mut steps = Vec::with_capacity(surviving.len());
    let first_date = surviving[0].0;
    let mut prev_market = surviving[0].1;
    let val_0 = surviving[0].2.take().ok_or_else(|| {
        Error::InvalidInput("Replay must have at least one valid step (unreachable)".into())
    })?;
    steps.push(ReplayStep {
        date: first_date,
        valuation: val_0,
        daily_mtm_pnl: None,
        cumulative_mtm_pnl: None,
        attribution: None,
    });

    let mut next_index = 1;
    let mut previous_endpoint: Option<Result<PortfolioValuation>> = None;
    while next_index < surviving.len() {
        let batch_end = if compute_attribution {
            next_index
                .saturating_add(STRICT_ENDPOINT_BATCH_SIZE - 1)
                .min(surviving.len() - 1)
        } else {
            surviving.len() - 1
        };
        let preparation_start = if next_index == 1 { 0 } else { next_index };
        let mut endpoint_batch = if let Some(profile) = endpoint_profile.as_ref() {
            attribution_endpoint_batch(
                portfolio,
                &surviving,
                &endpoint_needed,
                preparation_start..=batch_end,
                profile,
                finstack_config,
            )?
        } else {
            IndexMap::new()
        };
        if next_index == 1 {
            previous_endpoint = endpoint_batch.shift_remove(&0);
        }

        for (offset, (date, market, valuation)) in
            surviving[next_index..=batch_end].iter_mut().enumerate()
        {
            let index = next_index + offset;
            let date = *date;
            let market = *market;
            let val_i = valuation.take().ok_or_else(|| {
                Error::InvalidInput(format!(
                    "Replay valuation at surviving index {index} was consumed twice"
                ))
            })?;
            let prev_step = &steps[steps.len() - 1];

            let daily_mtm_pnl = if compute_pnl {
                Some(
                    val_i
                        .total_base_currency
                        .checked_sub(prev_step.valuation.total_base_currency)
                        .map_err(|e| {
                            Error::InvalidInput(format!(
                                "daily P&L overflow computing {date} minus {} \
                                 (base {}): {e}",
                                prev_step.date,
                                val_i.total_base_currency.currency()
                            ))
                        })?,
                )
            } else {
                None
            };

            let cumulative_mtm_pnl = if compute_pnl {
                Some(
                    val_i
                        .total_base_currency
                        .checked_sub(steps[0].valuation.total_base_currency)
                        .map_err(|e| {
                            Error::InvalidInput(format!(
                                "cumulative P&L overflow computing {date} minus {} \
                                 (base {}): {e}",
                                steps[0].date,
                                val_i.total_base_currency.currency()
                            ))
                        })?,
                )
            } else {
                None
            };

            let attribution = if compute_attribution {
                // Attribute step-over-step using the previous surviving
                // market. Best-effort skips therefore collapse to the latest
                // pair that actually produced valuations.
                let prev_endpoint =
                    endpoint_or_fallback(previous_endpoint.as_ref(), &prev_step.valuation)?;
                let endpoint = endpoint_or_fallback(endpoint_batch.get(&index), &val_i)?;
                let attr = if metrics_attribution {
                    reduce_metrics_based_prepared(
                        portfolio,
                        (prev_market, market),
                        (prev_step.date, date),
                        finstack_config,
                        prev_endpoint,
                        endpoint,
                    )?
                } else {
                    reduce_method_owned_prepared(
                        portfolio,
                        prev_market,
                        market,
                        prev_step.date,
                        date,
                        finstack_config,
                        &config.attribution_method,
                        prev_endpoint,
                        endpoint,
                    )?
                };
                Some(attr)
            } else {
                None
            };

            let current_endpoint = endpoint_batch.shift_remove(&index);
            steps.push(ReplayStep {
                date,
                valuation: val_i,
                daily_mtm_pnl,
                cumulative_mtm_pnl,
                attribution,
            });
            prev_market = market;
            previous_endpoint = current_endpoint;
        }
        next_index = batch_end + 1;
    }

    let summary = compute_summary(&steps)?;
    Ok(ReplayResult {
        steps,
        summary,
        skipped_dates,
    })
}

/// Compute the aggregate replay summary.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when the end-minus-start total MTM P&L is not
/// representable (the same `checked_sub` discipline used for the per-step
/// daily and cumulative P&L).
fn compute_summary(steps: &[ReplayStep]) -> Result<ReplaySummary> {
    let start_value = steps[0].valuation.total_base_currency;
    let end_value = steps[steps.len() - 1].valuation.total_base_currency;
    let total_mtm_pnl = end_value.checked_sub(start_value).map_err(|e| {
        Error::InvalidInput(format!(
            "total MTM P&L overflow computing {} minus {} (base {}): {e}",
            steps[steps.len() - 1].date,
            steps[0].date,
            end_value.currency()
        ))
    })?;

    // Max drawdown via high-water mark. The dollar-selected and the
    // percentage-selected drawdowns are tracked independently in one pass:
    // the largest dollar decline and the largest relative decline can fall
    // from different peaks (a small early peak can host the deepest relative
    // loss while a later, larger peak hosts the biggest dollar loss).
    let mut peak_value = start_value.amount();
    let mut peak_date = steps[0].date;
    // Dollar-selected drawdown.
    let mut max_dd = 0.0_f64;
    let mut max_dd_peak_date = steps[0].date;
    let mut max_dd_trough_date = steps[0].date;
    // Percentage-selected drawdown. Each percentage is relative to the
    // contemporaneous peak the decline fell from, and only positive peaks
    // participate: a "percentage of a non-positive value" is not meaningful,
    // so if no positive peak ever existed the percentage reports 0.0.
    let mut max_dd_pct = 0.0_f64;
    let mut max_dd_pct_peak_date: Option<Date> = None;
    let mut max_dd_pct_trough_date: Option<Date> = None;

    for step in steps {
        let val = step.valuation.total_base_currency.amount();
        if val > peak_value {
            peak_value = val;
            peak_date = step.date;
        }
        let dd = peak_value - val;
        if dd > max_dd {
            max_dd = dd;
            max_dd_peak_date = peak_date;
            max_dd_trough_date = step.date;
        }
        if peak_value > 0.0 {
            let dd_pct = dd / peak_value;
            if dd_pct > max_dd_pct {
                max_dd_pct = dd_pct;
                max_dd_pct_peak_date = Some(peak_date);
                max_dd_pct_trough_date = Some(step.date);
            }
        }
    }

    Ok(ReplaySummary {
        start_date: steps[0].date,
        end_date: steps[steps.len() - 1].date,
        num_steps: steps.len(),
        start_value,
        end_value,
        total_mtm_pnl,
        max_mtm_drawdown: Money::new(max_dd, start_value.currency())?,
        max_mtm_drawdown_pct: max_dd_pct,
        max_mtm_drawdown_peak_date: max_dd_peak_date,
        max_mtm_drawdown_trough_date: max_dd_trough_date,
        max_mtm_drawdown_pct_peak_date: max_dd_pct_peak_date,
        max_mtm_drawdown_pct_trough_date: max_dd_pct_trough_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::fx::FxConversionPolicy;
    use indexmap::IndexMap;
    use time::macros::date;

    fn synthetic_step(date: Date, value: f64) -> ReplayStep {
        ReplayStep {
            date,
            valuation: PortfolioValuation {
                as_of: date,
                position_values: IndexMap::new(),
                total_base_currency: Money::new(value, Currency::USD).expect("valid money fixture"),
                by_entity: IndexMap::new(),
                degraded_positions: Vec::new(),
                fx_collapse_policy: FxConversionPolicy::CashflowDate,
                provenance: None,
            },
            daily_mtm_pnl: None,
            cumulative_mtm_pnl: None,
            attribution: None,
        }
    }

    /// The percentage drawdown must be measured against the peak the
    /// drawdown fell from, not the final global high-water mark. With a
    /// higher peak *after* the trough, dividing by the global peak
    /// understates the drawdown (Bacon, *Practical Portfolio Performance
    /// Measurement*, drawdown is peak-relative).
    #[test]
    fn drawdown_pct_uses_contemporaneous_peak_not_final_global_peak() {
        let steps = vec![
            synthetic_step(date!(2024 - 01 - 01), 100.0),
            synthetic_step(date!(2024 - 01 - 02), 50.0),
            synthetic_step(date!(2024 - 01 - 03), 200.0),
            synthetic_step(date!(2024 - 01 - 04), 190.0),
        ];

        let summary = compute_summary(&steps).expect("summary computes");

        // Largest dollar decline is 100 -> 50 = 50.
        assert_eq!(summary.max_mtm_drawdown.amount(), 50.0);
        // Its percentage is 50/100 = 50%, NOT 50/200 = 25%.
        assert_eq!(summary.max_mtm_drawdown_pct, 0.5);
        assert_eq!(summary.max_mtm_drawdown_peak_date, date!(2024 - 01 - 01));
        assert_eq!(summary.max_mtm_drawdown_trough_date, date!(2024 - 01 - 02));
    }

    /// The percentage drawdown must be selected independently of the dollar
    /// drawdown: with 100 -> 50 -> 1000 -> 900 the largest dollar decline is
    /// 100 (from the 1000 peak, 10%) while the largest percentage decline is
    /// 50% (from the 100 peak). Selecting the trough on dollar decline and
    /// then reporting its ratio understates the percentage drawdown.
    #[test]
    fn max_mtm_drawdown_pct_is_selected_independently_of_dollar_drawdown() {
        let steps = vec![
            synthetic_step(date!(2024 - 01 - 01), 100.0),
            synthetic_step(date!(2024 - 01 - 02), 50.0),
            synthetic_step(date!(2024 - 01 - 03), 1000.0),
            synthetic_step(date!(2024 - 01 - 04), 900.0),
        ];

        let summary = compute_summary(&steps).expect("summary computes");

        // Dollar-selected drawdown: 1000 -> 900 = 100.
        assert_eq!(summary.max_mtm_drawdown.amount(), 100.0);
        assert_eq!(summary.max_mtm_drawdown_peak_date, date!(2024 - 01 - 03));
        assert_eq!(summary.max_mtm_drawdown_trough_date, date!(2024 - 01 - 04));
        // Percentage-selected drawdown: 100 -> 50 = 50%.
        assert_eq!(summary.max_mtm_drawdown_pct, 0.5);
        assert_eq!(
            summary.max_mtm_drawdown_pct_peak_date,
            Some(date!(2024 - 01 - 01))
        );
        assert_eq!(
            summary.max_mtm_drawdown_pct_trough_date,
            Some(date!(2024 - 01 - 02))
        );
    }

    /// `total_pnl` must use the same `checked_sub` discipline as the daily
    /// and cumulative P&L computations: an end-minus-start amount that
    /// overflows the Decimal representation propagates an error instead of
    /// panicking inside `Money::new`.
    #[test]
    fn total_pnl_overflow_propagates_an_error() {
        // Decimal max is ~7.92e28; end - start = ~1.58e29 overflows.
        let steps = vec![
            synthetic_step(date!(2024 - 01 - 01), -7.9e28),
            synthetic_step(date!(2024 - 01 - 02), 7.9e28),
        ];

        let error = compute_summary(&steps).expect_err("overflow must surface as an error");
        assert!(
            error.to_string().contains("total P&L"),
            "unexpected error: {error}"
        );
    }

    /// Percentage drawdown is 0.0 when no positive peak exists; dollar
    /// drawdown is still reported.
    #[test]
    fn minor16_drawdown_pct_is_zero_when_no_positive_peak_exists() {
        let steps = vec![
            synthetic_step(date!(2024 - 01 - 01), -100.0),
            synthetic_step(date!(2024 - 01 - 02), -150.0),
        ];

        let summary = compute_summary(&steps).expect("summary computes");

        assert_eq!(summary.max_mtm_drawdown.amount(), 50.0);
        assert_eq!(summary.max_mtm_drawdown_pct, 0.0);
        assert_eq!(summary.max_mtm_drawdown_pct_peak_date, None);
        assert_eq!(summary.max_mtm_drawdown_pct_trough_date, None);
    }
}
