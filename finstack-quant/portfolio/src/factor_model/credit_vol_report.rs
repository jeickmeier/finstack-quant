//! Portfolio reporting adapters for credit factor-risk decompositions.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{Error, Result};
use crate::types::PositionId;
use finstack_quant_factor::credit::hierarchy::{dimension_key, CreditFactorModel};
use finstack_quant_factor::parse_bucket_factor_id;
use finstack_quant_factor::risk::RiskDecomposition;
use finstack_quant_factor::RiskMeasure;
use finstack_quant_factor::CREDIT_GENERIC_FACTOR_ID;

/// Aggregated credit risk grouped by hierarchy level.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CreditVolReport {
    /// Total risk under the selected measure.
    pub total: f64,
    /// Risk measure used by the underlying decomposition.
    pub measure: RiskMeasure,
    /// Contribution from the generic credit factor.
    pub generic: f64,
    /// Per-hierarchy-level rollups.
    pub by_level: Vec<LevelVolContribution>,
    /// Portfolio idiosyncratic contribution.
    pub idiosyncratic_total: f64,
    /// Position-level breakdown; `None` unless requested.
    pub by_position: Option<Vec<PositionVolContribution>>,
}

/// Aggregated risk contribution for one hierarchy level.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct LevelVolContribution {
    /// Hierarchy level name: the level's dimension key (`"rating"`,
    /// `"region"`, `"sector"` or a custom dimension's own key), matching
    /// `CreditFactorModel::level_names` and the issuer-tag keys.
    pub level_name: String,
    /// Total contribution across the level's buckets.
    pub total: f64,
    /// Contributions keyed by canonical bucket path.
    pub by_bucket: BTreeMap<String, f64>,
}

/// Position-level credit risk breakdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PositionVolContribution {
    /// Portfolio position identifier.
    pub position_id: PositionId,
    /// Systematic factor contribution.
    pub factor_total: f64,
    /// Idiosyncratic contribution.
    pub idiosyncratic: f64,
    /// Sum of systematic and idiosyncratic contributions.
    pub total: f64,
}

/// Build a portfolio credit report from a models-owned risk decomposition.
///
/// # Arguments
///
/// * `decomposition` - Factor and residual contributions to aggregate.
/// * `model` - Credit factor model whose hierarchy supplies level names.
/// * `by_position` - Whether to include position-level contribution rows.
///
/// # Returns
///
/// The [`CreditVolReport`]; `generic` plus every level total accounts for all
/// of the decomposition's factor contributions.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when a factor contribution is neither the
/// generic credit factor nor a credit bucket factor of one of `model`'s
/// hierarchy levels (for example a non-credit factor id, or a bucket id whose
/// level index the hierarchy does not have), since its risk could not be
/// reported.
pub fn build_credit_vol_report(
    decomposition: &RiskDecomposition,
    model: &CreditFactorModel,
    by_position: bool,
) -> Result<CreditVolReport> {
    let mut by_level: Vec<LevelVolContribution> = model
        .hierarchy
        .levels
        .iter()
        .map(|level| LevelVolContribution {
            level_name: dimension_key(level).to_owned(),
            total: 0.0,
            by_bucket: BTreeMap::new(),
        })
        .collect();

    let mut generic = 0.0;
    for contribution in &decomposition.factor_contributions {
        let id = contribution.factor_id.as_str();
        if id == CREDIT_GENERIC_FACTOR_ID {
            generic += contribution.absolute_risk;
            continue;
        }
        let output = parse_bucket_factor_id(&contribution.factor_id)
            .and_then(|(level, bucket)| by_level.get_mut(level).map(|output| (output, bucket)));
        let Some((output, bucket)) = output else {
            return Err(Error::invalid_input(format!(
                "credit vol report cannot attribute factor '{id}': it is neither the generic \
                 credit factor nor a bucket of the model's {} hierarchy levels",
                model.hierarchy.levels.len()
            )));
        };
        output.total += contribution.absolute_risk;
        *output.by_bucket.entry(bucket.to_owned()).or_insert(0.0) += contribution.absolute_risk;
    }

    let residual_variance: f64 = decomposition
        .position_residual_contributions
        .iter()
        .map(|contribution| contribution.residual_variance)
        .sum();
    let residual_scale = if residual_variance > 0.0 {
        decomposition.residual_risk / residual_variance
    } else {
        0.0
    };

    let position_rows = by_position.then(|| {
        let mut factor_by_position: BTreeMap<String, f64> = BTreeMap::new();
        for contribution in &decomposition.position_factor_contributions {
            *factor_by_position
                .entry(contribution.position_id.clone())
                .or_insert(0.0) += contribution.risk_contribution;
        }
        let mut residual_by_position: BTreeMap<String, f64> = BTreeMap::new();
        for contribution in &decomposition.position_residual_contributions {
            *residual_by_position
                .entry(contribution.position_id.clone())
                .or_insert(0.0) += contribution.residual_variance * residual_scale;
        }
        let keys: BTreeSet<String> = factor_by_position
            .keys()
            .chain(residual_by_position.keys())
            .cloned()
            .collect();
        keys.into_iter()
            .map(|id| {
                let factor_total = factor_by_position.get(&id).copied().unwrap_or(0.0);
                let idiosyncratic = residual_by_position.get(&id).copied().unwrap_or(0.0);
                PositionVolContribution {
                    position_id: PositionId::new(id),
                    factor_total,
                    idiosyncratic,
                    total: factor_total + idiosyncratic,
                }
            })
            .collect()
    });

    Ok(CreditVolReport {
        total: decomposition.total_risk,
        measure: decomposition.measure,
        generic,
        by_level,
        idiosyncratic_total: decomposition.residual_risk,
        by_position: position_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_level_model() -> CreditFactorModel {
        serde_json::from_value(serde_json::json!({
            "schema": "finstack_quant.credit_factor_model/1",
            "as_of": "2024-03-29",
            "calibration_window": {"start": "2022-03-29", "end": "2024-03-29"},
            "policy": "globally_off",
            "generic_factor": {"name": "CDX IG", "series_id": "cdx.ig.5y"},
            "hierarchy": {"levels": ["rating", "region"]},
            "panel_frequency": "monthly",
            "use_returns_or_levels": "returns",
            "bucket_weighting": "equal",
            "config": {
                "factors": [],
                "covariance": {"n": 0, "factor_ids": [], "data": []},
                "matching": {"mapping_table": []}
            },
            "issuer_betas": [],
            "anchor_state": {"pc": 0.0, "by_level": []},
            "static_correlation": {"factor_ids": [], "data": []},
            "vol_state": {"factors": {}, "idiosyncratic": {}},
            "factor_histories": null,
            "diagnostics": {
                "mode_counts": {},
                "bucket_sizes_per_level": [],
                "fold_ups": [],
                "r_squared_histogram": null,
                "tag_taxonomy": {}
            }
        }))
        .expect("two-level credit factor model")
    }

    fn decomposition_with(factor_id: &str) -> RiskDecomposition {
        serde_json::from_value(serde_json::json!({
            "total_risk": 1.0,
            "measure": "variance",
            "factor_contributions": [
                {"factor_id": "credit::generic", "absolute_risk": 0.4,
                 "relative_risk": 0.4, "marginal_risk": 0.0},
                {"factor_id": factor_id, "absolute_risk": 0.6,
                 "relative_risk": 0.6, "marginal_risk": 0.0}
            ],
            "residual_risk": 0.0,
            "position_factor_contributions": [],
            "position_residual_contributions": []
        }))
        .expect("risk decomposition")
    }

    #[test]
    fn credit_vol_report_rejects_contributions_it_cannot_attribute() {
        // Dropping these silently made `generic + by_level` fall short of the
        // decomposition's factor risk with no signal to the caller.
        let model = two_level_model();
        for factor_id in [
            "usd_rates",
            "credit::levelX::rating::IG",
            "credit::level2::a.b.c::x.y.z",
        ] {
            let error = build_credit_vol_report(&decomposition_with(factor_id), &model, false)
                .expect_err("unattributable credit factor must be rejected");
            assert!(
                error.to_string().contains(factor_id),
                "error must name '{factor_id}', got: {error}"
            );
        }

        let report = build_credit_vol_report(
            &decomposition_with("credit::level1::rating.region::IG.EU"),
            &model,
            false,
        )
        .expect("known credit bucket");
        assert_eq!(report.by_level[1].by_bucket["IG.EU"], 0.6);
    }

    #[test]
    fn credit_vol_report_round_trips_through_json() {
        let report = CreditVolReport {
            total: 1.0,
            measure: RiskMeasure::Variance,
            generic: 0.1,
            by_level: vec![LevelVolContribution {
                level_name: "rating".to_owned(),
                total: 0.2,
                by_bucket: BTreeMap::from([("IG".to_owned(), 0.2)]),
            }],
            idiosyncratic_total: 0.4,
            by_position: Some(vec![PositionVolContribution {
                position_id: PositionId::new("POS1"),
                factor_total: 0.3,
                idiosyncratic: 0.0,
                total: 0.3,
            }]),
        };

        let json = serde_json::to_value(&report).expect("serialize");
        assert_eq!(json["measure"], "variance");
        assert_eq!(json["by_level"][0]["by_bucket"]["IG"], 0.2);
        assert_eq!(json["by_position"][0]["position_id"], "POS1");
        let restored: CreditVolReport = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, report);
    }
}
