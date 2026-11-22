//! Historical tail-scenario stress attribution for position P&L matrices.

use serde::{Deserialize, Serialize};

use super::{MinTail, PreparedHistoricalScenarios};

/// Per-position attribution of portfolio losses in tail scenarios.
///
/// For each scenario that breaches the VaR threshold, reports which
/// positions contributed the most to the portfolio loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StressAttribution {
    /// Portfolio VaR threshold (scenarios with P&L at or below this are
    /// "tail events"). Loss convention: reported as a negative number.
    pub var_threshold: f64,

    /// Number of tail scenarios analyzed.
    pub n_tail_scenarios: usize,

    /// Canonical position ordering shared by every [`tail_scenarios`] entry.
    ///
    /// `tail_scenarios[k].position_pnls[i]` is the P&L for `position_ids[i]`.
    /// Storing the id list once here (rather than re-attaching it to every tail
    /// scenario) avoids duplicating it `n_tail_scenarios` times.
    ///
    /// [`tail_scenarios`]: Self::tail_scenarios
    pub position_ids: Vec<String>,

    /// Per-position average contribution to tail losses.
    ///
    /// Sorted by absolute contribution (largest risk driver first).
    pub position_contributions: Vec<StressPositionEntry>,

    /// Individual tail scenario breakdowns.
    ///
    /// Contains `n_tail_scenarios` entries, each with per-position P&L
    /// index-aligned to [`position_ids`](Self::position_ids).
    /// Sorted by portfolio loss (worst first).
    pub tail_scenarios: Vec<TailScenarioBreakdown>,
}

/// Single position's contribution to tail stress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StressPositionEntry {
    /// Position identifier.
    pub position_id: String,

    /// Average P&L contribution in tail scenarios.
    pub avg_tail_pnl: f64,

    /// Fraction of total portfolio tail loss attributable to this position.
    pub pct_of_tail_loss: f64,

    /// Worst single-scenario P&L for this position.
    pub worst_scenario_pnl: f64,
}

/// Breakdown of a single tail scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailScenarioBreakdown {
    /// Scenario index in the original history.
    pub scenario_index: usize,

    /// Total portfolio P&L for this scenario.
    pub portfolio_pnl: f64,

    /// Per-position P&L contributions, index-aligned to
    /// [`StressAttribution::position_ids`]. Entry `i` is the P&L for
    /// `StressAttribution::position_ids[i]`.
    pub position_pnls: Vec<f64>,
}

/// Build tail-scenario stress attribution from position-level historical P&Ls.
///
/// The `position_pnls` buffer is row-major with shape
/// `n_scenarios × position_ids.len()`: for scenario `s` and position `i`, the
/// P&L is stored at `position_pnls[s * n_positions + i]`. Tail scenarios are
/// selected using the same boundary convention as
/// [`super::historical_var_decomposition`]: sort portfolio P&Ls
/// ascending, take the shared snapped-ceil tail count of scenarios, and set
/// `var_threshold` to the signed P&L of the least-bad tail scenario
/// (losses-negative convention).
///
/// # Errors
///
/// Returns a validation error when dimensions do not match, confidence is not
/// in `(0.5, 1)`, the requested tail has fewer than one scenario, or any P&L is
/// non-finite.
///
/// # Arguments
///
/// * `position_ids` - Position identifiers in the same column order as each
///   scenario row in `position_pnls`.
/// * `position_pnls` - Scenario-major flat P&L buffer with shape
///   `n_scenarios * position_ids.len()`; entries are reporting-currency P&L
///   amounts.
/// * `n_scenarios` - Number of scenario rows encoded in `position_pnls`.
/// * `confidence` - Tail confidence level strictly between 0.5 and 1.0, such
///   as `0.99` for a 99% stress-tail attribution. `None` defaults to `0.95`.
pub fn build_stress_attribution(
    position_ids: &[String],
    position_pnls: &[f64],
    n_scenarios: usize,
    confidence: Option<f64>,
) -> finstack_quant_core::Result<StressAttribution> {
    let prepared = PreparedHistoricalScenarios::prepare(
        position_ids,
        position_pnls,
        n_scenarios,
        confidence,
        MinTail::AtLeastOne,
    )?;
    Ok(build_stress_attribution_prepared(&prepared))
}

pub(super) fn build_stress_attribution_prepared(
    prepared: &PreparedHistoricalScenarios<'_>,
) -> StressAttribution {
    let position_ids = prepared.position_ids;
    let position_pnls = prepared.position_pnls;
    let n_scenarios = prepared.n_scenarios;
    let n_positions = position_ids.len();

    if n_positions == 0 || n_scenarios == 0 {
        return StressAttribution {
            var_threshold: 0.0,
            n_tail_scenarios: 0,
            position_ids: Vec::new(),
            position_contributions: Vec::new(),
            tail_scenarios: Vec::new(),
        };
    }

    let n_tail = super::tail_scenario_count(prepared.confidence, n_scenarios);
    let portfolio_pnls = &prepared.portfolio_pnls;

    // Loss convention (workspace-wide): the threshold follows the P&L sign,
    // so tail losses are negative. Clamp to zero only if the boundary
    // scenario is actually a gain.
    let var_idx = (n_tail - 1).min(n_scenarios - 1);
    let var_threshold = portfolio_pnls[var_idx].1.min(0.0);

    let tail = &portfolio_pnls[..n_tail];
    let avg_tail_loss = -tail.iter().map(|(_, pnl)| *pnl).sum::<f64>() / n_tail as f64;

    let mut pnl_sums = vec![0.0; n_positions];
    let mut worst_pnls = vec![f64::INFINITY; n_positions];
    let mut tail_scenarios = Vec::with_capacity(n_tail);

    for &(scenario_index, portfolio_pnl) in tail {
        let row_start = scenario_index * n_positions;
        let row = &position_pnls[row_start..row_start + n_positions];
        for (idx, pnl) in row.iter().copied().enumerate() {
            pnl_sums[idx] += pnl;
            worst_pnls[idx] = worst_pnls[idx].min(pnl);
        }
        // Store only the `f64` row; the position id for column `i` is
        // `StressAttribution::position_ids[i]`, carried once on the parent
        // result instead of cloned into every tail scenario.
        tail_scenarios.push(TailScenarioBreakdown {
            scenario_index,
            portfolio_pnl,
            position_pnls: row.to_vec(),
        });
    }

    let mut position_contributions: Vec<StressPositionEntry> = position_ids
        .iter()
        .enumerate()
        .map(|(idx, position_id)| {
            let avg_tail_pnl = pnl_sums[idx] / n_tail as f64;
            let pct_of_tail_loss = if avg_tail_loss.abs() > f64::EPSILON {
                -avg_tail_pnl / avg_tail_loss
            } else {
                0.0
            };
            StressPositionEntry {
                position_id: position_id.clone(),
                avg_tail_pnl,
                pct_of_tail_loss,
                worst_scenario_pnl: worst_pnls[idx],
            }
        })
        .collect();

    position_contributions.sort_by(|a, b| {
        b.avg_tail_pnl
            .abs()
            .total_cmp(&a.avg_tail_pnl.abs())
            .then_with(|| a.position_id.as_str().cmp(b.position_id.as_str()))
    });

    StressAttribution {
        var_threshold,
        n_tail_scenarios: n_tail,
        position_ids: position_ids.to_vec(),
        position_contributions,
        tail_scenarios,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = finstack_quant_core::Result<()>;

    #[test]
    fn stress_attribution_defaults_to_the_historical_95_confidence() -> TestResult {
        let (ids, pnls, n_scenarios) = crate::risk::historical_sample_pnls();
        let default = build_stress_attribution(&ids, &pnls, n_scenarios, None)?;
        let explicit = build_stress_attribution(&ids, &pnls, n_scenarios, Some(0.95))?;
        assert_eq!(
            default.var_threshold.to_bits(),
            explicit.var_threshold.to_bits()
        );
        assert_eq!(default.n_tail_scenarios, explicit.n_tail_scenarios);
        assert_eq!(default.n_tail_scenarios, 10);
        Ok(())
    }

    #[test]
    fn stress_attribution_uses_historical_tail_boundary() -> TestResult {
        let ids = [String::from("A"), String::from("B")];
        let n_scenarios = 20;
        let mut pnls = Vec::with_capacity(n_scenarios * ids.len());
        for scenario in 0..n_scenarios {
            match scenario {
                0 => {
                    pnls.push(-8.0);
                    pnls.push(-2.0);
                }
                1 => {
                    pnls.push(-3.0);
                    pnls.push(-1.0);
                }
                _ => {
                    pnls.push(1.0);
                    pnls.push(0.5);
                }
            }
        }

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, Some(0.95))?;

        assert_eq!(attr.n_tail_scenarios, 1);
        assert!((attr.var_threshold - (-10.0)).abs() < 1e-12);
        assert_eq!(attr.tail_scenarios[0].scenario_index, 0);
        assert_eq!(attr.tail_scenarios[0].portfolio_pnl, -10.0);
        // Per-position P&L is index-aligned to the shared `position_ids` list
        // carried once on the parent (no per-scenario id duplication).
        assert_eq!(
            attr.position_ids,
            vec![String::from("A"), String::from("B")]
        );
        assert_eq!(attr.tail_scenarios[0].position_pnls, vec![-8.0, -2.0]);
        assert_eq!(
            attr.position_contributions[0].position_id,
            String::from("A")
        );
        assert!((attr.position_contributions[0].avg_tail_pnl + 8.0).abs() < 1e-12);
        assert!((attr.position_contributions[0].pct_of_tail_loss - 0.8).abs() < 1e-12);
        assert!((attr.position_contributions[1].pct_of_tail_loss - 0.2).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn stress_attribution_averages_multiple_tail_scenarios() -> TestResult {
        let ids = [String::from("A"), String::from("B")];
        let n_scenarios = 40;
        let mut pnls = Vec::with_capacity(n_scenarios * ids.len());
        for scenario in 0..n_scenarios {
            match scenario {
                0 => {
                    pnls.push(-8.0);
                    pnls.push(-2.0);
                }
                1 => {
                    pnls.push(-2.0);
                    pnls.push(-4.0);
                }
                _ => {
                    pnls.push(0.5);
                    pnls.push(0.5);
                }
            }
        }

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, Some(0.95))?;

        assert_eq!(attr.n_tail_scenarios, 2);
        assert!((attr.var_threshold - (-6.0)).abs() < 1e-12);
        assert_eq!(attr.tail_scenarios[0].scenario_index, 0);
        assert_eq!(attr.tail_scenarios[1].scenario_index, 1);

        let contrib_a = attr
            .position_contributions
            .iter()
            .find(|entry| entry.position_id == "A")
            .expect("A contribution should exist");
        let contrib_b = attr
            .position_contributions
            .iter()
            .find(|entry| entry.position_id == "B")
            .expect("B contribution should exist");

        assert!((contrib_a.avg_tail_pnl + 5.0).abs() < 1e-12);
        assert!((contrib_b.avg_tail_pnl + 3.0).abs() < 1e-12);
        assert!((contrib_a.pct_of_tail_loss - 0.625).abs() < 1e-12);
        assert!((contrib_b.pct_of_tail_loss - 0.375).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn stress_attribution_rejects_underspecified_tail() {
        let ids = [String::from("A")];
        let pnls = vec![0.0; 50];
        let result = build_stress_attribution(&ids, &pnls, 50, Some(0.99));
        assert!(result.is_err());
    }

    #[test]
    fn stress_attribution_tail_count_is_exact_at_rational_boundaries() -> TestResult {
        // Same exact-rational boundary as the historical engine: the tail
        // at 90% of 1000 scenarios is exactly 100 scenarios.
        let n_scenarios = 1000;
        let pnls: Vec<f64> = (0..n_scenarios).map(|s| s as f64 - 500.0).collect();
        let ids = [String::from("A")];

        let attr = build_stress_attribution(&ids, &pnls, n_scenarios, Some(0.90))?;
        assert_eq!(attr.n_tail_scenarios, 100);
        Ok(())
    }

    #[test]
    fn stress_attribution_validates_confidence_on_empty_inputs() {
        let result = build_stress_attribution(&[], &[], 0, Some(1.5));
        assert!(
            result.is_err(),
            "invalid confidence must fail even when empty"
        );
    }

    #[test]
    fn stress_attribution_rejects_scenario_size_overflow() {
        let ids = [String::from("A"), String::from("B")];
        let result = build_stress_attribution(&ids, &[], usize::MAX, Some(0.95));
        assert!(
            result.is_err(),
            "n_scenarios * n_positions overflow must fail"
        );
    }
}
