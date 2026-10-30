//! One-call Merton report: the model, its term structure, grades and CDS terms.
//!
//! [`merton_report`] owns the two defaults a report depends on, so every host
//! gets the same numbers: an equity-sourced model is calibrated to the KMV
//! default point ([`MertonModel::kmv_default_point`]), and default
//! probabilities are graded on [`MasterScale::default_calibration`].

use std::collections::BTreeMap;

use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::MertonModel;
use crate::credit::pd::{MasterScale, MasterScaleResult};

/// Where a [`merton_report`] takes its model from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MertonReportSource {
    /// A model already specified; it is used as supplied.
    Model {
        /// The canonical model.
        model: MertonModel,
    },
    /// A model calibrated from equity with [`MertonModel::from_equity`], using
    /// the KMV default point as the debt barrier.
    Equity {
        /// Market value of equity, in the firm's monetary unit.
        equity_value: f64,
        /// Annualized equity-return volatility, as a decimal.
        equity_vol: f64,
        /// Liabilities due within one year; a total-debt barrier is this with
        /// no long-term debt.
        short_term_debt: f64,
        /// Liabilities maturing beyond one year.
        long_term_debt: f64,
        /// Continuously compounded risk-free rate, as a decimal.
        risk_free_rate: f64,
        /// Continuous payout yield on assets, as a decimal.
        payout_rate: f64,
        /// Calibration horizon in years.
        maturity: f64,
    },
}

/// Inputs of one [`merton_report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MertonReportRequest {
    /// Where the model comes from.
    pub source: MertonReportSource,
    /// Horizons in years for the term structure, in display order; each must be
    /// finite and strictly positive.
    pub horizons: Vec<f64>,
    /// CDS and implied-equity maturity in years.
    pub maturity: f64,
    /// Recovery at default as a decimal fraction of par, for the implied and
    /// CDS spreads.
    pub recovery: f64,
}

/// Implied equity of a Merton model at the report's maturity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MertonImpliedEquity {
    /// Implied market value of equity, in the firm's monetary unit.
    pub value: f64,
    /// Implied annualized equity volatility, as a decimal.
    pub vol: f64,
}

/// The result of one [`merton_report`].
///
/// Every per-horizon field aligns with `horizons`. A first-passage model has no
/// endogenous debt spread and a CreditGrades model fixes its own CDS recovery, so
/// a measure Rust rejects for the model is `None`, with its message in `errors`,
/// and the rest of the report stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MertonReport {
    /// The model the report was computed from.
    pub model: MertonModel,
    /// KMV default point of an equity source; `None` for a supplied model.
    pub default_point: Option<f64>,
    /// The requested horizons in years, in request order.
    pub horizons: Vec<f64>,
    /// Default probability per horizon, as a decimal.
    pub default_probabilities: Vec<f64>,
    /// Distance to default per horizon, in standard deviations.
    pub distance_to_default: Vec<f64>,
    /// Default-calibration master-scale grade of each default probability.
    pub grades: Vec<MasterScaleResult>,
    /// [`MertonModel::implied_spread`] per horizon, decimal per year.
    pub implied_spread: Option<Vec<f64>>,
    /// [`MertonModel::debt_spread`] per horizon, decimal per year.
    pub debt_spread: Option<Vec<f64>>,
    /// The requested CDS and implied-equity maturity in years.
    pub maturity: f64,
    /// The requested recovery, as a decimal fraction of par.
    pub recovery: f64,
    /// [`MertonModel::cds_par_spread`] at `maturity`, decimal per year.
    pub cds_par_spread: Option<f64>,
    /// [`MertonModel::implied_equity`] at `maturity`.
    pub implied_equity: Option<MertonImpliedEquity>,
    /// Message of each rejected measure, by field name; empty when all computed.
    pub errors: BTreeMap<String, String>,
}

/// The measure's value, or `None` with its message recorded under `name`.
fn recorded<T>(errors: &mut BTreeMap<String, String>, name: &str, result: Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            errors.insert(name.to_string(), error.to_string());
            None
        }
    }
}

/// Compute a Merton report from a supplied model or an equity calibration.
///
/// An equity source takes the KMV default point (short-term debt plus half of
/// long-term debt) as its barrier, and grades come from
/// [`MasterScale::default_calibration`].
///
/// # Arguments
///
/// * `request` - The model source, the horizons in years, the CDS and
///   implied-equity maturity in years, and the recovery as a decimal fraction
///   of par.
///
/// # Returns
///
/// The full report. A spread or implied-equity measure Rust rejects for this
/// model is `None`, with its message under its field name in
/// [`MertonReport::errors`].
///
/// # Errors
///
/// Returns a validation error when a horizon is non-finite or not strictly
/// positive, when the equity calibration or KMV default point rejects its
/// inputs, or when the embedded default master scale cannot load.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::credit::{
///     merton_report, MertonModel, MertonReportRequest, MertonReportSource,
/// };
///
/// let report = merton_report(&MertonReportRequest {
///     source: MertonReportSource::Model {
///         model: MertonModel::new(100.0, 0.25, 60.0, 0.03)?,
///     },
///     horizons: vec![1.0, 5.0],
///     maturity: 5.0,
///     recovery: 0.4,
/// })?;
/// assert_eq!(report.grades.len(), 2);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn merton_report(request: &MertonReportRequest) -> Result<MertonReport> {
    if let Some(horizon) = request
        .horizons
        .iter()
        .find(|horizon| !(horizon.is_finite() && **horizon > 0.0))
    {
        return Err(Error::Validation(format!(
            "merton_report: horizons must be finite and > 0, got {horizon}"
        )));
    }
    let (model, default_point) = match &request.source {
        MertonReportSource::Model { model } => (model.clone(), None),
        MertonReportSource::Equity {
            equity_value,
            equity_vol,
            short_term_debt,
            long_term_debt,
            risk_free_rate,
            payout_rate,
            maturity,
        } => {
            let default_point = MertonModel::kmv_default_point(*short_term_debt, *long_term_debt)?;
            let model = MertonModel::from_equity(
                *equity_value,
                *equity_vol,
                default_point,
                *risk_free_rate,
                *payout_rate,
                *maturity,
            )?;
            (model, Some(default_point))
        }
    };
    let horizons = &request.horizons;
    let default_probabilities = model.default_probabilities(horizons);
    let grades = MasterScale::default_calibration()?
        .map_pds(&default_probabilities)
        .map_err(|error| Error::Validation(error.to_string()))?;
    let (maturity, recovery) = (request.maturity, request.recovery);
    let mut errors = BTreeMap::new();
    let implied_spread = recorded(
        &mut errors,
        "implied_spread",
        horizons
            .iter()
            .map(|horizon| model.implied_spread(*horizon, recovery))
            .collect(),
    );
    let debt_spread = recorded(
        &mut errors,
        "debt_spread",
        horizons
            .iter()
            .map(|horizon| model.debt_spread(*horizon))
            .collect(),
    );
    let cds_par_spread = recorded(
        &mut errors,
        "cds_par_spread",
        model.cds_par_spread(maturity, recovery),
    );
    let implied_equity = recorded(
        &mut errors,
        "implied_equity",
        model
            .implied_equity(maturity)
            .map(|(value, vol)| MertonImpliedEquity { value, vol }),
    );
    Ok(MertonReport {
        default_point,
        horizons: horizons.clone(),
        default_probabilities,
        distance_to_default: horizons
            .iter()
            .map(|horizon| model.distance_to_default(*horizon))
            .collect(),
        grades,
        implied_spread,
        debt_spread,
        maturity,
        recovery,
        cds_par_spread,
        implied_equity,
        errors,
        model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credit::{AssetDynamics, MertonBarrierType};

    fn terms(source: MertonReportSource) -> MertonReportRequest {
        MertonReportRequest {
            source,
            horizons: vec![1.0, 5.0],
            maturity: 5.0,
            recovery: 0.4,
        }
    }

    fn firm() -> MertonModel {
        MertonModel::new(100.0, 0.25, 60.0, 0.03).expect("valid model")
    }

    fn equity() -> MertonReportSource {
        MertonReportSource::Equity {
            equity_value: 50.0,
            equity_vol: 0.4,
            short_term_debt: 30.0,
            long_term_debt: 40.0,
            risk_free_rate: 0.03,
            payout_rate: 0.0,
            maturity: 1.0,
        }
    }

    #[test]
    fn equity_source_uses_the_kmv_default_point_as_its_barrier() {
        let report = merton_report(&terms(equity())).expect("report");
        assert_eq!(report.default_point, Some(50.0));
        assert_eq!(report.model.debt_barrier(), 50.0);
        let direct = MertonModel::from_equity(50.0, 0.4, 50.0, 0.03, 0.0, 1.0).expect("model");
        assert_eq!(report.model, direct);
    }

    #[test]
    fn supplied_model_has_no_default_point_and_matches_the_model_measures() {
        let model = firm();
        let report = merton_report(&terms(MertonReportSource::Model {
            model: model.clone(),
        }))
        .expect("report");
        assert_eq!(report.default_point, None);
        assert_eq!(report.model, model);
        assert_eq!(
            report.default_probabilities,
            model.default_probabilities(&[1.0, 5.0])
        );
        assert_eq!(
            report.distance_to_default,
            vec![
                model.distance_to_default(1.0),
                model.distance_to_default(5.0)
            ]
        );
        assert_eq!(
            report.cds_par_spread,
            Some(model.cds_par_spread(5.0, 0.4).expect("spread"))
        );
    }

    #[test]
    fn grades_come_from_the_default_master_scale() {
        let report = merton_report(&terms(equity())).expect("report");
        let expected = MasterScale::default_calibration()
            .expect("scale")
            .map_pds(&report.default_probabilities)
            .expect("grades");
        assert_eq!(report.grades, expected);
    }

    #[test]
    fn a_measure_rejected_for_the_model_keeps_its_error_beside_the_rest() {
        let model = MertonModel::new_with_dynamics(
            100.0,
            0.25,
            60.0,
            0.03,
            0.0,
            MertonBarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("model");
        let report = merton_report(&terms(MertonReportSource::Model { model })).expect("report");
        assert_eq!(report.debt_spread, None);
        assert!(report.errors["debt_spread"].contains("Terminal only"));
        assert_eq!(report.errors.len(), 1);
        assert!(report.implied_spread.is_some());
        assert!(report.cds_par_spread.is_some());
    }

    #[test]
    fn rejects_a_non_positive_horizon_and_a_rejected_equity_source() {
        let mut request = terms(equity());
        request.horizons = vec![1.0, 0.0];
        assert!(merton_report(&request).is_err());
        let bad = MertonReportSource::Equity {
            equity_value: 50.0,
            equity_vol: 0.4,
            short_term_debt: 0.0,
            long_term_debt: 0.0,
            risk_free_rate: 0.03,
            payout_rate: 0.0,
            maturity: 1.0,
        };
        assert!(merton_report(&terms(bad)).is_err());
    }

    #[test]
    fn report_round_trips_and_request_rejects_unknown_fields() {
        let report = merton_report(&terms(equity())).expect("report");
        let json = serde_json::to_string(&report).expect("serialize");
        let back: MertonReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, report);
        let mut request = serde_json::to_value(terms(equity())).expect("serialize");
        request["source"]["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<MertonReportRequest>(request).is_err());
    }
}
