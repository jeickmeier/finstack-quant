//! Historical scenario replay for portfolios.
//!
//! Replays a static portfolio through a sequence of dated market snapshots,
//! producing configurable P&L and attribution output at each step.
//!
//! This module is only available when the `scenarios` feature is enabled.

use crate::attribution::PortfolioAttribution;
use crate::error::{Error, Result};
use crate::valuation::PortfolioValuation;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

/// What to compute at each replay step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReplayMode {
    /// Just portfolio PV at each date.
    PvOnly,
    /// PV + daily/cumulative P&L.
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
pub struct ReplayConfig {
    /// What to compute at each step.
    pub mode: ReplayMode,
    /// Attribution method (only used in `FullAttribution` mode).
    #[serde(default)]
    pub attribution_method: finstack_quant_attribution::AttributionMethod,
    /// Valuation options forwarded to `value_portfolio`.
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
struct JsonSnapshot {
    date: String,
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
        let format = time::format_description::well_known::Iso8601::DEFAULT;
        let raw: Vec<JsonSnapshot> = serde_json::from_str(json)
            .map_err(|e| Error::InvalidInput(format!("invalid snapshots JSON: {e}")))?;
        let mut snapshots = Vec::with_capacity(raw.len());
        for entry in raw {
            let date = Date::parse(&entry.date, &format).map_err(|e| {
                Error::InvalidInput(format!("invalid snapshot date '{}': {e}", entry.date))
            })?;
            snapshots.push((date, entry.market));
        }
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
    /// Daily P&L (this step minus prior step). `None` at step 0.
    pub daily_pnl: Option<Money>,
    /// Cumulative P&L (this step minus step 0). `None` at step 0.
    pub cumulative_pnl: Option<Money>,
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
    /// Total P&L (end value minus start value).
    pub total_pnl: Money,
    /// Maximum drawdown from peak to trough.
    pub max_drawdown: Money,
    /// Maximum drawdown as a percentage of peak value.
    pub max_drawdown_pct: f64,
    /// Date of the peak before the maximum drawdown.
    pub max_drawdown_peak_date: Date,
    /// Date of the trough of the maximum drawdown.
    pub max_drawdown_trough_date: Date,
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
use crate::valuation::{value_portfolio_at, value_portfolio_serial_at};
use finstack_quant_core::config::FinstackConfig;

/// Portfolios below this position count benefit more from parallelizing the
/// outer replay loop (one Rayon task per timeline snapshot) than from the
/// inner per-position parallelism inside [`value_portfolio`]. Above the
/// threshold the inner parallelism is left in place to avoid oversubscribing
/// the Rayon thread pool.
const REPLAY_OUTER_PARALLEL_POSITION_THRESHOLD: usize = 256;

/// Minimum number of replay snapshots at which outer-loop parallelism is
/// considered; for very short timelines the serial path is clearer and the
/// work per date is amortized by the benchmark loop anyway.
const REPLAY_OUTER_PARALLEL_SNAPSHOT_THRESHOLD: usize = 8;

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

    // Decide where to spend Rayon parallelism: small portfolios benefit from
    // parallelizing the outer (per-date) loop and running each valuation
    // serially; large portfolios already saturate the thread pool via
    // per-position parallelism inside `value_portfolio`, so the outer loop
    // runs serially to avoid nested dispatch overhead.
    let use_outer_parallel = portfolio.positions.len() < REPLAY_OUTER_PARALLEL_POSITION_THRESHOLD
        && timeline.snapshots.len() >= REPLAY_OUTER_PARALLEL_SNAPSHOT_THRESHOLD;

    // Phase A: value the portfolio at every snapshot date. Per-snapshot
    // results are kept as `Result<_>` so the strict / best-effort branch
    // below can decide whether a single failure aborts the run.
    let valuation_results: Vec<Result<PortfolioValuation>> = if use_outer_parallel {
        use rayon::prelude::*;
        timeline
            .snapshots
            .par_iter()
            .map(|(date, market)| {
                value_portfolio_serial_at(
                    portfolio,
                    market,
                    finstack_config,
                    &config.valuation_options,
                    *date,
                )
            })
            .collect()
    } else {
        timeline
            .snapshots
            .iter()
            .map(|(date, market)| {
                value_portfolio_at(
                    portfolio,
                    market,
                    finstack_config,
                    &config.valuation_options,
                    *date,
                )
            })
            .collect()
    };

    // Pair each result with its dated snapshot so best-effort skipping can
    // record which dates dropped out without losing the alignment.
    let mut skipped_dates: Vec<(Date, String)> = Vec::new();
    let mut surviving: Vec<(Date, &MarketContext, PortfolioValuation)> =
        Vec::with_capacity(timeline.len());
    for ((date, market), result) in timeline.snapshots.iter().zip(valuation_results) {
        match result {
            Ok(v) => surviving.push((*date, market, v)),
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

    // Phase B: assemble ReplayStep entries with P&L and (optionally)
    // attribution. Runs serially — the per-step work is cheap (subtractions
    // and one attribution call) and serial ordering keeps tracing output
    // deterministic. Attribution itself already fans out over positions via
    // `attribute_portfolio_pnl`.
    let mut steps = Vec::with_capacity(surviving.len());

    let mut surviving_iter = surviving.into_iter();
    let (first_date, mut prev_market, val_0) = surviving_iter.next().ok_or_else(|| {
        Error::InvalidInput("Replay must have at least one valid step (unreachable)".into())
    })?;
    steps.push(ReplayStep {
        date: first_date,
        valuation: val_0,
        daily_pnl: None,
        cumulative_pnl: None,
        attribution: None,
    });

    for (date, market, val_i) in surviving_iter {
        let prev_step = &steps[steps.len() - 1];

        let daily_pnl = if compute_pnl {
            Some(
                val_i
                    .total_base_ccy
                    .checked_sub(prev_step.valuation.total_base_ccy)
                    .map_err(|e| {
                        Error::InvalidInput(format!(
                            "daily P&L overflow computing {date} minus {} \
                             (base {}): {e}",
                            prev_step.date,
                            val_i.total_base_ccy.currency()
                        ))
                    })?,
            )
        } else {
            None
        };

        let cumulative_pnl = if compute_pnl {
            Some(
                val_i
                    .total_base_ccy
                    .checked_sub(steps[0].valuation.total_base_ccy)
                    .map_err(|e| {
                        Error::InvalidInput(format!(
                            "cumulative P&L overflow computing {date} minus {} \
                             (base {}): {e}",
                            steps[0].date,
                            val_i.total_base_ccy.currency()
                        ))
                    })?,
            )
        } else {
            None
        };

        let attribution = if compute_attribution {
            // Attribute step-over-step using the *previous surviving*
            // market, not `timeline.snapshots[steps.len() - 1]`. The two
            // diverge under best-effort policy when intermediate snapshots
            // were skipped: skipping must collapse the prev/curr pair to
            // the latest pair that actually produced valuations.
            let attr = crate::attribution::attribute_portfolio_pnl(
                portfolio,
                prev_market,
                market,
                prev_step.date,
                date,
                finstack_config,
                config.attribution_method.clone(),
            )?;
            Some(attr)
        } else {
            None
        };

        steps.push(ReplayStep {
            date,
            valuation: val_i,
            daily_pnl,
            cumulative_pnl,
            attribution,
        });

        // Advance the prev-market reference for the next iteration; this is
        // what keeps best-effort attribution aligned even when intermediate
        // snapshots were skipped.
        prev_market = market;
    }

    let summary = compute_summary(&steps);
    Ok(ReplayResult {
        steps,
        summary,
        skipped_dates,
    })
}

fn compute_summary(steps: &[ReplayStep]) -> ReplaySummary {
    let start_value = steps[0].valuation.total_base_ccy;
    let end_value = steps[steps.len() - 1].valuation.total_base_ccy;
    let total_pnl = Money::new(
        end_value.amount() - start_value.amount(),
        start_value.currency(),
    );

    // Max drawdown via high-water mark
    let mut peak_value = start_value.amount();
    let mut peak_date = steps[0].date;
    let mut max_dd = 0.0_f64;
    let mut max_dd_peak_date = steps[0].date;
    let mut max_dd_trough_date = steps[0].date;

    for step in steps {
        let val = step.valuation.total_base_ccy.amount();
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
    }

    let max_drawdown_pct = if peak_value.abs() > f64::EPSILON {
        max_dd / peak_value.abs()
    } else {
        0.0
    };

    ReplaySummary {
        start_date: steps[0].date,
        end_date: steps[steps.len() - 1].date,
        num_steps: steps.len(),
        start_value,
        end_value,
        total_pnl,
        max_drawdown: Money::new(max_dd, start_value.currency()),
        max_drawdown_pct,
        max_drawdown_peak_date: max_dd_peak_date,
        max_drawdown_trough_date: max_dd_trough_date,
    }
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
                total_base_ccy: Money::new(value, Currency::USD),
                by_entity: IndexMap::new(),
                degraded_positions: Vec::new(),
                fx_collapse_policy: FxConversionPolicy::CashflowDate,
            },
            daily_pnl: None,
            cumulative_pnl: None,
            attribution: None,
        }
    }

    #[test]
    fn minor16_drawdown_pct_is_positive_for_negative_peak_values() {
        let steps = vec![
            synthetic_step(date!(2024 - 01 - 01), -100.0),
            synthetic_step(date!(2024 - 01 - 02), -150.0),
        ];

        let summary = compute_summary(&steps);

        assert_eq!(summary.max_drawdown.amount(), 50.0);
        assert_eq!(summary.max_drawdown_pct, 0.5);
    }
}
