//! One position risk report: a VaR/ES decomposition from a parametric or
//! historical source, with an optional risk budget and, for scenario P&L, the
//! tail stress attribution.
//!
//! [`position_risk_report`] runs the existing kernels
//! ([`parametric_var_decomposition`], [`historical_var_decomposition`],
//! [`evaluate_risk_budget_from_decomposition`], [`build_stress_attribution`])
//! from one [`PositionRiskRequest`], so a host does not reshape matrices or
//! decide which analyses apply to which source.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_factor::risk::{
//!     PositionRiskRequest, PositionRiskSource, position_risk_report,
//! };
//!
//! let report = position_risk_report(&PositionRiskRequest {
//!     position_ids: vec!["A".into(), "B".into()],
//!     confidence: Some(0.99),
//!     source: PositionRiskSource::Parametric {
//!         weights: vec![0.6, 0.4],
//!         covariance: vec![vec![0.04, 0.01], vec![0.01, 0.09]],
//!         compute_incremental: false,
//!     },
//!     budget: None,
//! })?;
//! assert!(report.decomposition.portfolio_var < 0.0);
//! assert!(report.stress.is_none());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use serde::{Deserialize, Serialize};

use finstack_quant_core::math::linalg::flatten_square_matrix;

use super::{
    PositionRiskDecomposition, RiskBudgetResult, StressAttribution, build_stress_attribution,
    evaluate_risk_budget_from_decomposition, flatten_position_pnls, historical_var_decomposition,
    parametric_var_decomposition,
};

/// Where a position VaR/ES decomposition comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PositionRiskSource {
    /// Euler allocation under normal returns.
    Parametric {
        /// Position weights or exposures in portfolio currency, one per
        /// position id.
        weights: Vec<f64>,
        /// Position-return covariance: `n x n` rows in position order,
        /// symmetric positive semidefinite.
        covariance: Vec<Vec<f64>>,
        /// Also compute leave-one-out incremental VaR (one repricing per
        /// position); omitted is `false`.
        #[serde(default)]
        compute_incremental: bool,
    },
    /// Historical simulation over scenario P&L; also yields the tail stress
    /// attribution.
    Historical {
        /// Position-major scenario P&L: one row per position, one column per
        /// scenario, losses negative.
        pnls: Vec<Vec<f64>>,
    },
}

/// Target shares of portfolio VaR checked against the decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PositionRiskBudget {
    /// Target fraction of portfolio VaR per position, in `[0, 1]` and summing
    /// to ~1.0, in position order.
    pub targets: Vec<f64>,
    /// Actual-to-target ratio above which a breach is flagged; omitted uses
    /// [`super::DEFAULT_UTILIZATION_THRESHOLD`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utilization_threshold: Option<f64>,
}

/// One position risk report request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PositionRiskRequest {
    /// Position identifiers, in the order of the source's rows.
    pub position_ids: Vec<String>,
    /// Tail confidence as a decimal strictly inside `(0.5, 1)`; omitted is
    /// `0.95`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Parametric covariance or historical scenario P&L.
    pub source: PositionRiskSource,
    /// Risk budget to evaluate against the decomposition; omitted skips it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<PositionRiskBudget>,
}

/// Result of a [`PositionRiskRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct PositionRiskReport {
    /// Portfolio and per-position VaR/ES (losses negative).
    pub decomposition: PositionRiskDecomposition,
    /// Budget evaluation of the decomposition; `None` without a budget.
    pub budget: Option<RiskBudgetResult>,
    /// Tail stress attribution of a historical source's P&L; `None` for a
    /// parametric source.
    pub stress: Option<StressAttribution>,
}

/// Decompose position VaR/ES from one source, then evaluate the requested
/// budget and, for scenario P&L, the tail stress attribution.
///
/// # Arguments
///
/// * `request` - Position ids, confidence, the parametric or historical
///   source and an optional risk budget.
///
/// # Errors
///
/// Returns a validation error when the source's dimensions disagree with the
/// position ids, the covariance is invalid, too few scenarios resolve the
/// tail, the confidence is outside `(0.5, 1)`, or the budget is malformed.
pub fn position_risk_report(
    request: &PositionRiskRequest,
) -> finstack_quant_core::Result<PositionRiskReport> {
    let ids = &request.position_ids;
    let (decomposition, stress) = match &request.source {
        PositionRiskSource::Parametric {
            weights,
            covariance,
            compute_incremental,
        } => {
            let covariance =
                flatten_square_matrix(covariance.clone(), weights.len(), "covariance")?;
            let decomposition = parametric_var_decomposition(
                ids,
                weights,
                &covariance,
                request.confidence,
                *compute_incremental,
            )?;
            (decomposition, None)
        }
        PositionRiskSource::Historical { pnls } => {
            let (flat, n_scenarios) = flatten_position_pnls(pnls.clone(), ids.len())?;
            let decomposition =
                historical_var_decomposition(ids, &flat, n_scenarios, request.confidence)?;
            let stress = build_stress_attribution(ids, &flat, n_scenarios, request.confidence)?;
            (decomposition, Some(stress))
        }
    };
    let budget = request
        .budget
        .as_ref()
        .map(|budget| {
            evaluate_risk_budget_from_decomposition(
                &decomposition,
                &budget.targets,
                budget.utilization_threshold,
            )
        })
        .transpose()?;
    Ok(PositionRiskReport {
        decomposition,
        budget,
        stress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<String> {
        vec!["A".into(), "B".into()]
    }

    #[test]
    fn parametric_report_matches_the_kernel_and_has_no_stress() -> finstack_quant_core::Result<()> {
        let report = position_risk_report(&PositionRiskRequest {
            position_ids: ids(),
            confidence: Some(0.99),
            source: PositionRiskSource::Parametric {
                weights: vec![0.6, 0.4],
                covariance: vec![vec![0.04, 0.01], vec![0.01, 0.09]],
                compute_incremental: true,
            },
            budget: Some(PositionRiskBudget {
                targets: vec![0.5, 0.5],
                utilization_threshold: None,
            }),
        })?;
        let direct = parametric_var_decomposition(
            &ids(),
            &[0.6, 0.4],
            &[0.04, 0.01, 0.01, 0.09],
            Some(0.99),
            true,
        )?;
        assert_eq!(report.decomposition, direct);
        assert!(report.stress.is_none());
        assert_eq!(report.budget.map(|b| b.positions.len()), Some(2));
        Ok(())
    }

    #[test]
    fn historical_report_adds_the_stress_attribution() -> finstack_quant_core::Result<()> {
        let pnls = vec![
            vec![-5.0, 1.0, 2.0, -1.0, 0.5, 3.0, -2.0, 1.5, -0.5, 2.5],
            vec![-1.0, 0.5, -3.0, 1.0, 0.2, -0.5, 1.0, -2.0, 0.5, 1.0],
        ];
        let report = position_risk_report(&PositionRiskRequest {
            position_ids: ids(),
            confidence: Some(0.8),
            source: PositionRiskSource::Historical { pnls: pnls.clone() },
            budget: None,
        })?;
        let (flat, n) = flatten_position_pnls(pnls, 2)?;
        let stress = build_stress_attribution(&ids(), &flat, n, Some(0.8))?;
        assert_eq!(
            report.stress.map(|s| s.var_threshold),
            Some(stress.var_threshold)
        );
        assert!(report.budget.is_none());
        Ok(())
    }

    #[test]
    fn wire_form_is_tagged_and_strict() {
        let json = r#"{"position_ids":["A"],"source":{"kind":"historical","pnls":[[-1.0,2.0]]}}"#;
        let request: Result<PositionRiskRequest, _> = serde_json::from_str(json);
        assert!(request.is_ok());
        let unknown = json.replace(r#""pnls""#, r#""weights":[1.0],"pnls""#);
        assert!(serde_json::from_str::<PositionRiskRequest>(&unknown).is_err());
    }

    #[test]
    fn mismatched_dimensions_are_rejected() {
        let result = position_risk_report(&PositionRiskRequest {
            position_ids: ids(),
            confidence: None,
            source: PositionRiskSource::Parametric {
                weights: vec![1.0, 0.0],
                covariance: vec![vec![0.04]],
                compute_incremental: false,
            },
            budget: None,
        });
        assert!(result.is_err());
    }
}
