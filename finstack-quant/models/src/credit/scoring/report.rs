//! One-call credit score: a scoring model's result and its default master-scale grade.
//!
//! [`credit_score_report`] owns the grading default, so every host gets the
//! same grade: an implied PD is mapped on [`MasterScale::default_calibration`].

use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

use super::{
    AltmanZDoublePrimeInput, AltmanZPrimeInput, AltmanZScoreInput, OhlsonOScoreInput,
    ScoringResult, ZmijewskiInput, altman_em_score, altman_z_double_prime, altman_z_prime,
    altman_z_score, ohlson_o_score, zmijewski_score,
};
use crate::credit::pd::{MasterScale, MasterScaleResult};

/// One scoring model and its input ratios.
///
/// The `model` tag names the scoring function; `input` carries that model's
/// input struct. `altman_em_score` takes the Z''-Score ratios.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "model",
    content = "input",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CreditScoreRequest {
    /// [`altman_z_score`]: original Altman Z-Score for public manufacturers.
    AltmanZScore(AltmanZScoreInput),
    /// [`altman_z_prime`]: Altman Z'-Score for private firms.
    AltmanZPrime(AltmanZPrimeInput),
    /// [`altman_z_double_prime`]: Altman Z''-Score for non-manufacturers.
    AltmanZDoublePrime(AltmanZDoublePrimeInput),
    /// [`altman_em_score`]: Altman EM-Score for emerging-market corporates.
    AltmanEmScore(AltmanZDoublePrimeInput),
    /// [`ohlson_o_score`]: Ohlson O-Score logistic model.
    OhlsonOScore(OhlsonOScoreInput),
    /// [`zmijewski_score`]: Zmijewski probit model.
    ZmijewskiScore(ZmijewskiInput),
}

/// The result of one [`credit_score_report`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreditScoreReport {
    /// The scoring model's result.
    pub result: ScoringResult,
    /// Default-calibration master-scale grade of `result.implied_pd`; `None`
    /// when the model has no native implied PD (the Altman family).
    pub grade: Option<MasterScaleResult>,
}

/// Score one model's input ratios and grade the implied PD.
///
/// The implied PD of a model that has one (Ohlson, Zmijewski) is graded on
/// [`MasterScale::default_calibration`]; the Altman family has no implied PD,
/// so its grade is `None`.
///
/// # Arguments
///
/// * `request` - The scoring model and its finite input ratios (decimal
///   ratios; Ohlson's two indicators exactly `0` or `1`).
///
/// # Returns
///
/// The model's [`ScoringResult`] and the default master-scale grade of its
/// implied PD.
///
/// # Errors
///
/// Returns a validation error when an input ratio is non-finite or an Ohlson
/// indicator is not exactly `0` or `1`, or when the embedded default master
/// scale cannot load.
///
/// # Examples
///
/// ```
/// use finstack_quant_models::credit::scoring::{
///     credit_score_report, CreditScoreRequest, ZmijewskiInput,
/// };
///
/// let report = credit_score_report(&CreditScoreRequest::ZmijewskiScore(ZmijewskiInput {
///     net_income_to_total_assets: 0.1,
///     total_liabilities_to_total_assets: 0.4,
///     current_assets_to_current_liabilities: 2.0,
/// }))?;
/// assert!(report.grade.is_some());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn credit_score_report(request: &CreditScoreRequest) -> Result<CreditScoreReport> {
    let result = match request {
        CreditScoreRequest::AltmanZScore(input) => altman_z_score(input),
        CreditScoreRequest::AltmanZPrime(input) => altman_z_prime(input),
        CreditScoreRequest::AltmanZDoublePrime(input) => altman_z_double_prime(input),
        CreditScoreRequest::AltmanEmScore(input) => altman_em_score(input),
        CreditScoreRequest::OhlsonOScore(input) => ohlson_o_score(input),
        CreditScoreRequest::ZmijewskiScore(input) => zmijewski_score(input),
    }
    .map_err(|error| Error::Validation(error.to_string()))?;
    let grade = match result.implied_pd {
        Some(pd) => Some(
            MasterScale::default_calibration()?
                .map_pd(pd)
                .map_err(|error| Error::Validation(error.to_string()))?,
        ),
        None => None,
    };
    Ok(CreditScoreReport { result, grade })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zmijewski() -> CreditScoreRequest {
        CreditScoreRequest::ZmijewskiScore(ZmijewskiInput {
            net_income_to_total_assets: 0.1,
            total_liabilities_to_total_assets: 0.4,
            current_assets_to_current_liabilities: 2.0,
        })
    }

    #[test]
    fn implied_pd_is_graded_on_the_default_master_scale() {
        let report = credit_score_report(&zmijewski()).expect("report");
        let expected = MasterScale::default_calibration()
            .expect("scale")
            .map_score(&report.result)
            .expect("grade");
        assert_eq!(report.grade, Some(expected));
    }

    #[test]
    fn altman_has_no_grade() {
        let report = credit_score_report(&CreditScoreRequest::AltmanZScore(AltmanZScoreInput {
            working_capital_to_total_assets: 0.1,
            retained_earnings_to_total_assets: 0.2,
            ebit_to_total_assets: 0.15,
            market_equity_to_total_liabilities: 1.5,
            sales_to_total_assets: 1.8,
        }))
        .expect("report");
        assert_eq!(report.result.implied_pd, None);
        assert_eq!(report.grade, None);
    }

    #[test]
    fn em_score_uses_the_z_double_prime_ratios() {
        let input = AltmanZDoublePrimeInput {
            working_capital_to_total_assets: 0.2,
            retained_earnings_to_total_assets: 0.3,
            ebit_to_total_assets: 0.15,
            book_equity_to_total_liabilities: 1.2,
        };
        let report =
            credit_score_report(&CreditScoreRequest::AltmanEmScore(input)).expect("report");
        assert_eq!(report.result, altman_em_score(&input).expect("score"));
    }

    #[test]
    fn non_finite_ratio_is_a_validation_error() {
        let request = CreditScoreRequest::ZmijewskiScore(ZmijewskiInput {
            net_income_to_total_assets: f64::NAN,
            total_liabilities_to_total_assets: 0.4,
            current_assets_to_current_liabilities: 2.0,
        });
        assert!(matches!(
            credit_score_report(&request),
            Err(Error::Validation(_))
        ));
    }

    #[test]
    fn request_wire_form_is_model_tagged_and_strict() {
        let json = serde_json::to_value(zmijewski()).expect("serialize");
        assert_eq!(json["model"], "zmijewski_score");
        assert_eq!(json["input"]["net_income_to_total_assets"], 0.1);
        let back: CreditScoreRequest = serde_json::from_value(json.clone()).expect("deserialize");
        assert_eq!(back, zmijewski());
        let mut extra = json.clone();
        extra["input"]["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CreditScoreRequest>(extra).is_err());
        let mut unknown = json;
        unknown["model"] = serde_json::json!("beaver");
        assert!(serde_json::from_value::<CreditScoreRequest>(unknown).is_err());
    }
}
