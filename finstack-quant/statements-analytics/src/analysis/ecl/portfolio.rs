//! Portfolio-level ECL aggregation and provision waterfall.
//!
//! This module aggregates individual exposure ECL results into portfolio-level
//! metrics, including:
//!
//! - Total ECL and ECL by stage
//! - EAD and exposure count by stage
//! - ECL by segment (product, geography, rating, etc.)
//! - Stage migration matrix (previous stage -> current stage)
//! - Provision movement waterfall between reporting dates

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::engine::ExposureEclResult;
use super::types::Stage;

// ---------------------------------------------------------------------------
// Portfolio ECL result
// ---------------------------------------------------------------------------

/// Portfolio-level ECL result with stage migration and segment breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioEclResult {
    /// Total ECL across all exposures.
    pub total_ecl: f64,

    /// ECL by stage.
    pub ecl_by_stage: IndexMap<Stage, f64>,

    /// Exposure count by stage.
    pub count_by_stage: IndexMap<Stage, usize>,

    /// EAD by stage (total outstanding balance per stage).
    pub ead_by_stage: IndexMap<Stage, f64>,

    /// ECL by segment (product, geography, rating, etc.).
    pub ecl_by_segment: IndexMap<String, f64>,

    /// Stage migration matrix: previous_stage -> current_stage -> count.
    /// Only populated for exposures that have a `previous_stage`.
    pub migration_matrix: IndexMap<Stage, IndexMap<Stage, usize>>,

    /// Per-exposure results.
    pub exposure_results: Vec<ExposureEclResult>,

    /// Exposure ids in `exposure_results` that could not be matched against
    /// the exposure slice passed to
    /// [`PortfolioEclResult::from_results_with_exposures`]. Unmatched
    /// results still contribute their ECL and EAD to the stage totals, but
    /// carry no segment or migration data. Empty when constructed via
    /// [`PortfolioEclResult::from_results`].
    #[serde(default)]
    pub unmatched_exposure_ids: Vec<String>,
}

impl PortfolioEclResult {
    /// Construct from individual exposure results.
    pub fn from_results(results: Vec<ExposureEclResult>) -> Self {
        let mut total_ecl = 0.0;
        let mut ecl_by_stage: IndexMap<Stage, f64> = IndexMap::new();
        let mut count_by_stage: IndexMap<Stage, usize> = IndexMap::new();
        let mut ead_by_stage: IndexMap<Stage, f64> = IndexMap::new();
        let ecl_by_segment: IndexMap<String, f64> = IndexMap::new();
        let migration_matrix: IndexMap<Stage, IndexMap<Stage, usize>> = IndexMap::new();

        // Initialize all stages to ensure consistent ordering
        for stage in &[Stage::Stage1, Stage::Stage2, Stage::Stage3] {
            ecl_by_stage.entry(*stage).or_insert(0.0);
            count_by_stage.entry(*stage).or_insert(0);
            ead_by_stage.entry(*stage).or_insert(0.0);
        }

        for result in &results {
            let stage = result.stage_result.stage;
            let ecl = result.ecl_result.ecl;

            total_ecl += ecl;
            *ecl_by_stage.entry(stage).or_insert(0.0) += ecl;
            *count_by_stage.entry(stage).or_insert(0) += 1;
            *ead_by_stage.entry(stage).or_insert(0.0) += result.ead;
        }

        Self {
            total_ecl,
            ecl_by_stage,
            count_by_stage,
            ead_by_stage,
            ecl_by_segment,
            migration_matrix,
            exposure_results: results,
            unmatched_exposure_ids: Vec::new(),
        }
    }

    /// Construct from exposure results with access to the original exposures
    /// for segment and migration data.
    pub fn from_results_with_exposures(
        results: Vec<ExposureEclResult>,
        exposures: &[super::types::Exposure],
    ) -> Self {
        let mut total_ecl = 0.0;
        let mut ecl_by_stage: IndexMap<Stage, f64> = IndexMap::new();
        let mut count_by_stage: IndexMap<Stage, usize> = IndexMap::new();
        let mut ead_by_stage: IndexMap<Stage, f64> = IndexMap::new();
        let mut ecl_by_segment: IndexMap<String, f64> = IndexMap::new();
        let mut migration_matrix: IndexMap<Stage, IndexMap<Stage, usize>> = IndexMap::new();

        // Initialize all stages
        for stage in &[Stage::Stage1, Stage::Stage2, Stage::Stage3] {
            ecl_by_stage.entry(*stage).or_insert(0.0);
            count_by_stage.entry(*stage).or_insert(0);
            ead_by_stage.entry(*stage).or_insert(0.0);
        }

        // Build exposure lookup by id
        let exposure_map: IndexMap<&str, &super::types::Exposure> =
            exposures.iter().map(|e| (e.id.as_str(), e)).collect();

        let mut unmatched_exposure_ids = Vec::new();

        for result in &results {
            let stage = result.stage_result.stage;
            let ecl = result.ecl_result.ecl;
            let exp_id = result.ecl_result.exposure_id.as_str();

            total_ecl += ecl;
            *ecl_by_stage.entry(stage).or_insert(0.0) += ecl;
            *count_by_stage.entry(stage).or_insert(0) += 1;

            if let Some(exposure) = exposure_map.get(exp_id) {
                *ead_by_stage.entry(stage).or_insert(0.0) += exposure.ead;

                // Segment aggregation
                for segment in &exposure.segments {
                    *ecl_by_segment.entry(segment.clone()).or_insert(0.0) += ecl;
                }

                // Migration matrix
                if let Some(prev_stage) = &exposure.previous_stage {
                    let inner = migration_matrix.entry(*prev_stage).or_default();
                    *inner.entry(stage).or_insert(0) += 1;
                }
            } else {
                // Surface unmatched ids instead of silently zero-filling
                // their EAD (which would inflate the coverage ratio). The
                // result-level EAD still feeds the stage totals.
                unmatched_exposure_ids.push(exp_id.to_string());
                *ead_by_stage.entry(stage).or_insert(0.0) += result.ead;
            }
        }

        Self {
            total_ecl,
            ecl_by_stage,
            count_by_stage,
            ead_by_stage,
            ecl_by_segment,
            migration_matrix,
            exposure_results: results,
            unmatched_exposure_ids,
        }
    }

    /// Coverage ratio: total ECL / total EAD.
    pub fn coverage_ratio(&self) -> f64 {
        let total_ead: f64 = self.ead_by_stage.values().sum();
        if total_ead > 0.0 {
            self.total_ecl / total_ead
        } else {
            0.0
        }
    }

    /// Stage 3 ratio: Stage 3 EAD / total EAD.
    pub fn stage3_ratio(&self) -> f64 {
        let total_ead: f64 = self.ead_by_stage.values().sum();
        let s3_ead = self
            .ead_by_stage
            .get(&Stage::Stage3)
            .copied()
            .unwrap_or(0.0);
        if total_ead > 0.0 {
            s3_ead / total_ead
        } else {
            0.0
        }
    }

    /// Number of exposures in the portfolio.
    pub fn exposure_count(&self) -> usize {
        self.exposure_results.len()
    }
}

// ---------------------------------------------------------------------------
// Provision waterfall
// ---------------------------------------------------------------------------

/// Provision movement waterfall between two reporting dates.
///
/// Tracks how the ECL provision balance moves from opening to closing,
/// decomposed into business drivers (new originations, stage transfers,
/// write-offs, etc.).
///
/// The accounting identity is:
/// closing = opening + new_originations + transfers_to_stage2
///         + transfers_to_stage3 + cured_to_stage1 + cured_to_stage2
///         + derecognitions + write_offs + remeasurement
///
/// Cure movements are split into `cured_to_stage1` (Stage 2 → Stage 1,
/// and Stage 3 → Stage 1 "direct" cures) and `cured_to_stage2`
/// (Stage 3 → Stage 2 partial cures / probation). This matches the
/// IFRS 9 disclosure convention which treats partial cures out of
/// credit-impaired separately from full cures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionWaterfall {
    /// Opening ECL balance (previous period closing).
    pub opening: f64,
    /// New originations (new exposures added this period).
    pub new_originations: f64,
    /// Transfers to Stage 2 (net ECL change from stage upgrades).
    pub transfers_to_stage2: f64,
    /// Transfers to Stage 3 (net ECL change from stage downgrades).
    pub transfers_to_stage3: f64,
    /// Cured back to Stage 1 (net ECL release) — both Stage 2 → 1 and
    /// Stage 3 → 1.
    pub cured_to_stage1: f64,
    /// Partial cure Stage 3 → Stage 2 (net ECL change, typically a
    /// release as Stage 3 lifetime ECL is de-recognised, partly offset
    /// by Stage 2 lifetime ECL).
    pub cured_to_stage2: f64,
    /// Derecognitions (maturities, repayments, sales).
    pub derecognitions: f64,
    /// Write-offs.
    pub write_offs: f64,
    /// Model/parameter changes (remeasurement, i.e., the residual).
    pub remeasurement: f64,
    /// Closing ECL balance.
    pub closing: f64,
}

/// Compute the provision waterfall between two portfolio snapshots.
///
/// Matches exposures by ID across periods to track stage movements and
/// identify new originations and derecognitions. All disappearing exposures
/// are reported as derecognitions (`write_offs` stays 0.0); use
/// [`compute_waterfall_with_write_offs`] to split write-offs onto their own
/// line.
///
/// # Arguments
///
/// * `previous` -- Previous period portfolio result
/// * `current` -- Current period portfolio result
pub fn compute_waterfall(
    previous: &PortfolioEclResult,
    current: &PortfolioEclResult,
) -> ProvisionWaterfall {
    compute_waterfall_with_write_offs(previous, current, &[])
}

/// Compute the provision waterfall, splitting write-offs out of derecognitions.
///
/// Identical to [`compute_waterfall`], except that exposures listed in
/// `written_off_ids` which disappear between the two snapshots are reported
/// on the `write_offs` line (IFRS 7.35I) instead of being folded into
/// `derecognitions`. Ids that do not correspond to a disappearing exposure
/// are ignored.
///
/// # Arguments
///
/// * `previous` -- Previous period portfolio result
/// * `current` -- Current period portfolio result
/// * `written_off_ids` -- Exposure ids written off during the period
pub fn compute_waterfall_with_write_offs(
    previous: &PortfolioEclResult,
    current: &PortfolioEclResult,
    written_off_ids: &[String],
) -> ProvisionWaterfall {
    let opening = previous.total_ecl;
    let closing = current.total_ecl;

    // Build lookup maps by exposure_id
    let prev_map: IndexMap<&str, &ExposureEclResult> = previous
        .exposure_results
        .iter()
        .map(|r| (r.ecl_result.exposure_id.as_str(), r))
        .collect();

    let curr_map: IndexMap<&str, &ExposureEclResult> = current
        .exposure_results
        .iter()
        .map(|r| (r.ecl_result.exposure_id.as_str(), r))
        .collect();

    let mut new_originations = 0.0;
    let mut derecognitions = 0.0;
    let mut write_offs = 0.0;
    let mut transfers_to_stage2 = 0.0;
    let mut transfers_to_stage3 = 0.0;
    let mut cured_to_stage1 = 0.0;
    let mut cured_to_stage2 = 0.0;
    let mut remeasurement = 0.0;

    // New originations: in current but not in previous
    for (id, curr_result) in &curr_map {
        if !prev_map.contains_key(id) {
            new_originations += curr_result.ecl_result.ecl;
        }
    }

    // Derecognitions / write-offs: in previous but not in current
    for (id, prev_result) in &prev_map {
        if !curr_map.contains_key(id) {
            if written_off_ids.iter().any(|w| w == id) {
                write_offs -= prev_result.ecl_result.ecl; // Negative = utilisation
            } else {
                derecognitions -= prev_result.ecl_result.ecl; // Negative = release
            }
        }
    }

    // Stage transfers and remeasurement for matched exposures
    for (id, curr_result) in &curr_map {
        if let Some(prev_result) = prev_map.get(id) {
            let prev_stage = prev_result.stage_result.stage;
            let curr_stage = curr_result.stage_result.stage;
            let ecl_delta = curr_result.ecl_result.ecl - prev_result.ecl_result.ecl;

            match (prev_stage, curr_stage) {
                (Stage::Stage1, Stage::Stage2) => {
                    transfers_to_stage2 += ecl_delta;
                }
                (Stage::Stage1, Stage::Stage3) | (Stage::Stage2, Stage::Stage3) => {
                    transfers_to_stage3 += ecl_delta;
                }
                (Stage::Stage2, Stage::Stage1) | (Stage::Stage3, Stage::Stage1) => {
                    // Direct cure back to Stage 1 (including Stage 3
                    // "full cure" after probation). Negative delta = release.
                    cured_to_stage1 += ecl_delta;
                }
                (Stage::Stage3, Stage::Stage2) => {
                    // Partial cure out of credit-impaired into Stage 2.
                    // Kept on its own line so disclosure ties back to
                    // the IFRS 7.35H credit-impaired movement table.
                    cured_to_stage2 += ecl_delta;
                }
                _ => {
                    // Same stage (Stage1→1, Stage2→2, Stage3→3) or any
                    // residual combination is treated as remeasurement.
                    remeasurement += ecl_delta;
                }
            }
        }
    }

    ProvisionWaterfall {
        opening,
        new_originations,
        transfers_to_stage2,
        transfers_to_stage3,
        cured_to_stage1,
        cured_to_stage2,
        derecognitions,
        write_offs,
        remeasurement,
        closing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ecl::engine::{EclBucket, EclResult, WeightedEclResult};
    use crate::analysis::ecl::staging::{StageResult, StagingTrigger};

    fn make_exposure_result(id: &str, stage: Stage, ecl: f64, ead: f64) -> ExposureEclResult {
        ExposureEclResult {
            ead,
            stage_result: StageResult {
                stage,
                triggers: vec![StagingTrigger::NoTrigger],
                cured: false,
            },
            ecl_result: WeightedEclResult {
                exposure_id: id.to_string(),
                stage,
                ecl,
                scenario_breakdown: vec![(
                    "base".to_string(),
                    1.0,
                    EclResult {
                        exposure_id: id.to_string(),
                        stage,
                        ecl,
                        horizon: 1.0,
                        buckets: vec![EclBucket {
                            t_start: 0.0,
                            t_end: 1.0,
                            marginal_pd: 0.02,
                            lgd: 0.45,
                            ead,
                            discount_factor: 1.0,
                            ecl,
                        }],
                    },
                )],
            },
        }
    }

    #[test]
    fn test_portfolio_aggregation_sums() {
        let results = vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0),
            make_exposure_result("B", Stage::Stage1, 200.0, 20_000.0),
            make_exposure_result("C", Stage::Stage2, 500.0, 15_000.0),
            make_exposure_result("D", Stage::Stage3, 800.0, 5_000.0),
        ];

        let portfolio = PortfolioEclResult::from_results(results);

        assert!((portfolio.total_ecl - 1600.0).abs() < 1e-10);
        assert_eq!(portfolio.count_by_stage[&Stage::Stage1], 2);
        assert_eq!(portfolio.count_by_stage[&Stage::Stage2], 1);
        assert_eq!(portfolio.count_by_stage[&Stage::Stage3], 1);
        assert!((portfolio.ecl_by_stage[&Stage::Stage1] - 300.0).abs() < 1e-10);
        assert!((portfolio.ecl_by_stage[&Stage::Stage2] - 500.0).abs() < 1e-10);
        assert!((portfolio.ecl_by_stage[&Stage::Stage3] - 800.0).abs() < 1e-10);
    }

    #[test]
    fn test_coverage_ratio() {
        let results = vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0),
            make_exposure_result("B", Stage::Stage2, 500.0, 10_000.0),
        ];

        let portfolio = PortfolioEclResult::from_results(results);

        // total ECL = 600, total EAD = 20000
        let expected = 600.0 / 20_000.0;
        assert!(
            (portfolio.coverage_ratio() - expected).abs() < 1e-10,
            "Coverage ratio: {} vs expected {}",
            portfolio.coverage_ratio(),
            expected
        );
    }

    #[test]
    fn test_stage3_ratio() {
        let results = vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 80_000.0),
            make_exposure_result("B", Stage::Stage3, 500.0, 20_000.0),
        ];

        let portfolio = PortfolioEclResult::from_results(results);

        // Stage 3 EAD = 20000, total EAD = 100000
        assert!(
            (portfolio.stage3_ratio() - 0.2).abs() < 1e-10,
            "Stage 3 ratio: {} vs expected 0.2",
            portfolio.stage3_ratio()
        );
    }

    #[test]
    fn test_provision_waterfall_new_originations() {
        let previous = PortfolioEclResult::from_results(vec![make_exposure_result(
            "A",
            Stage::Stage1,
            100.0,
            10_000.0,
        )]);
        let current = PortfolioEclResult::from_results(vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0),
            make_exposure_result("B", Stage::Stage1, 200.0, 20_000.0),
        ]);

        let waterfall = compute_waterfall(&previous, &current);

        assert!((waterfall.opening - 100.0).abs() < 1e-10);
        assert!((waterfall.closing - 300.0).abs() < 1e-10);
        assert!((waterfall.new_originations - 200.0).abs() < 1e-10);
    }

    #[test]
    fn test_provision_waterfall_derecognitions() {
        let previous = PortfolioEclResult::from_results(vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0),
            make_exposure_result("B", Stage::Stage1, 200.0, 20_000.0),
        ]);
        let current = PortfolioEclResult::from_results(vec![make_exposure_result(
            "A",
            Stage::Stage1,
            100.0,
            10_000.0,
        )]);

        let waterfall = compute_waterfall(&previous, &current);

        assert!((waterfall.opening - 300.0).abs() < 1e-10);
        assert!((waterfall.closing - 100.0).abs() < 1e-10);
        assert!((waterfall.derecognitions - (-200.0)).abs() < 1e-10);
    }

    #[test]
    fn test_provision_waterfall_write_offs_split_from_derecognitions() {
        let previous = PortfolioEclResult::from_results(vec![
            make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0),
            make_exposure_result("B", Stage::Stage3, 400.0, 20_000.0),
            make_exposure_result("C", Stage::Stage1, 50.0, 5_000.0),
        ]);
        let current = PortfolioEclResult::from_results(vec![make_exposure_result(
            "A",
            Stage::Stage1,
            100.0,
            10_000.0,
        )]);

        let waterfall = compute_waterfall_with_write_offs(&previous, &current, &["B".to_string()]);

        assert!((waterfall.write_offs - (-400.0)).abs() < 1e-10);
        assert!((waterfall.derecognitions - (-50.0)).abs() < 1e-10);
    }

    #[test]
    fn test_unmatched_exposure_ids_surfaced() {
        let results = vec![make_exposure_result("A", Stage::Stage1, 100.0, 10_000.0)];
        let portfolio = PortfolioEclResult::from_results_with_exposures(results, &[]);

        assert_eq!(portfolio.unmatched_exposure_ids, vec!["A".to_string()]);
        // Result-level EAD still feeds the stage totals.
        assert!((portfolio.ead_by_stage[&Stage::Stage1] - 10_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_provision_waterfall_stage_transfer() {
        let previous = PortfolioEclResult::from_results(vec![make_exposure_result(
            "A",
            Stage::Stage1,
            100.0,
            10_000.0,
        )]);
        let current = PortfolioEclResult::from_results(vec![make_exposure_result(
            "A",
            Stage::Stage2,
            400.0,
            10_000.0,
        )]);

        let waterfall = compute_waterfall(&previous, &current);

        assert!((waterfall.opening - 100.0).abs() < 1e-10);
        assert!((waterfall.closing - 400.0).abs() < 1e-10);
        assert!((waterfall.transfers_to_stage2 - 300.0).abs() < 1e-10);
    }
}
