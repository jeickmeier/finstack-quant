//! Serializable, lossless hazard-curve calibration replay data.

use crate::dates::Date;
use serde::Deserialize;
use std::cmp::Ordering;

/// One atomic quote binding retained for hazard calibration replay.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HazardCalibrationInput {
    /// Exact serialized typed CDS quote.
    pub quote: serde_json::Value,
    /// Contractual pillar date resolved from the quote and CDS conventions.
    #[cfg_attr(feature = "json-schema", schemars(with = "crate::wire::DateWire"))]
    pub pillar_date: Date,
    /// Frozen year-fraction pillar time used by the original solve.
    pub pillar_time: f64,
}

/// Exact valuation-layer inputs required to replay a hazard-curve calibration.
///
/// The core market-data crate stores these payloads without interpreting them;
/// the calibration crate deserializes them back into its canonical
/// `HazardCurveParams`, typed CDS quotes, and `CalibrationConfig`. Keeping the
/// complete serde payloads avoids replacing date pillars with rounded tenors or
/// silently substituting current defaults for the original solver policy.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawHazardCalibrationRecipe")]
#[cfg_attr(
    feature = "json-schema",
    derive(schemars::JsonSchema),
    schemars(!try_from, deny_unknown_fields)
)]
pub struct HazardCalibrationRecipe {
    /// Exact serialized `HazardCurveParams` used for the original solve.
    hazard_params: serde_json::Value,
    /// Original par-spread or upfront inputs used to calibrate the curve.
    calibration_inputs: Vec<HazardCalibrationInput>,
    /// Par-spread-only inputs used for quote-space spread risk.
    spread_risk_inputs: Vec<HazardCalibrationInput>,
    /// Exact serialized `CalibrationConfig`, including solver and validation policy.
    calibration_config: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHazardCalibrationRecipe {
    hazard_params: serde_json::Value,
    calibration_inputs: Vec<HazardCalibrationInput>,
    spread_risk_inputs: Vec<HazardCalibrationInput>,
    calibration_config: serde_json::Value,
}

impl TryFrom<RawHazardCalibrationRecipe> for HazardCalibrationRecipe {
    type Error = crate::Error;

    fn try_from(raw: RawHazardCalibrationRecipe) -> crate::Result<Self> {
        Self::new(
            raw.hazard_params,
            raw.calibration_inputs,
            raw.spread_risk_inputs,
            raw.calibration_config,
        )
    }
}

impl HazardCalibrationRecipe {
    /// Construct a deterministically ordered hazard calibration recipe.
    ///
    /// # Arguments
    ///
    /// * `hazard_params` - Exact serialized hazard-curve calibration parameters.
    /// * `calibration_inputs` - Original par-spread or upfront quote bindings.
    /// * `spread_risk_inputs` - Par-spread quote bindings used for spread shocks.
    /// * `calibration_config` - Exact serialized solver and validation policy.
    ///
    /// # Errors
    ///
    /// Returns a validation error when replay inputs are empty, malformed,
    /// unordered, or inconsistent across calibration and spread-risk views.
    pub fn new(
        hazard_params: serde_json::Value,
        calibration_inputs: Vec<HazardCalibrationInput>,
        spread_risk_inputs: Vec<HazardCalibrationInput>,
        calibration_config: serde_json::Value,
    ) -> crate::Result<Self> {
        let mut recipe = Self {
            hazard_params,
            calibration_inputs,
            spread_risk_inputs,
            calibration_config,
        };
        recipe.sort_inputs();
        recipe.validate()?;
        Ok(recipe)
    }

    /// Exact serialized `HazardCurveParams` used for the original solve.
    #[inline]
    pub fn hazard_params(&self) -> &serde_json::Value {
        &self.hazard_params
    }

    /// Original par-spread or upfront inputs used to calibrate the curve.
    #[inline]
    pub fn calibration_inputs(&self) -> &[HazardCalibrationInput] {
        &self.calibration_inputs
    }

    /// Par-spread-only inputs used for quote-space spread risk.
    #[inline]
    pub fn spread_risk_inputs(&self) -> &[HazardCalibrationInput] {
        &self.spread_risk_inputs
    }

    /// Exact serialized `CalibrationConfig`, including solver and validation policy.
    #[inline]
    pub fn calibration_config(&self) -> &serde_json::Value {
        &self.calibration_config
    }

    /// Validate structural invariants required for lossless replay.
    fn validate(&self) -> crate::Result<()> {
        validate_inputs(&self.calibration_inputs, "calibration", false)?;
        validate_inputs(&self.spread_risk_inputs, "spread-risk", true)?;
        if self.calibration_inputs.len() != self.spread_risk_inputs.len() {
            return Err(crate::Error::Validation(format!(
                "hazard replay input counts differ: calibration={}, spread-risk={}",
                self.calibration_inputs.len(),
                self.spread_risk_inputs.len()
            )));
        }
        for (index, (calibration, risk)) in self
            .calibration_inputs
            .iter()
            .zip(&self.spread_risk_inputs)
            .enumerate()
        {
            if quote_id(&calibration.quote).is_empty()
                || quote_id(&calibration.quote) != quote_id(&risk.quote)
            {
                return Err(crate::Error::Validation(format!(
                    "hazard replay quote ID mismatch at index {index}"
                )));
            }
        }
        Ok(())
    }

    fn sort_inputs(&mut self) {
        self.calibration_inputs.sort_by(compare_inputs);
        self.spread_risk_inputs.sort_by(compare_inputs);
    }
}

fn compare_inputs(left: &HazardCalibrationInput, right: &HazardCalibrationInput) -> Ordering {
    left.pillar_time
        .total_cmp(&right.pillar_time)
        .then_with(|| left.pillar_date.cmp(&right.pillar_date))
        .then_with(|| quote_id(&left.quote).cmp(quote_id(&right.quote)))
        .then_with(|| left.quote.to_string().cmp(&right.quote.to_string()))
}

fn validate_inputs(
    inputs: &[HazardCalibrationInput],
    input_kind: &str,
    require_par_spread: bool,
) -> crate::Result<()> {
    if inputs.is_empty() {
        return Err(crate::Error::Validation(format!(
            "hazard {input_kind} replay inputs must not be empty"
        )));
    }
    for (index, input) in inputs.iter().enumerate() {
        if !input.pillar_time.is_finite() || input.pillar_time < 0.0 {
            return Err(crate::Error::Validation(format!(
                "hazard {input_kind} replay pillar time at index {index} must be finite and nonnegative"
            )));
        }
        let id = quote_id(&input.quote);
        if id.is_empty() {
            return Err(crate::Error::Validation(format!(
                "hazard {input_kind} replay quote at index {index} must have a nonempty ID"
            )));
        }
        let quote_type = input
            .quote
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if quote_type != "cds_par_spread" && quote_type != "cds_upfront" {
            return Err(crate::Error::Validation(format!(
                "hazard {input_kind} replay quote '{id}' has unsupported type '{quote_type}'"
            )));
        }
        if require_par_spread && quote_type != "cds_par_spread" {
            return Err(crate::Error::Validation(format!(
                "hazard spread-risk replay quote '{id}' must be a par-spread (cds_par_spread) quote"
            )));
        }
    }
    for pair in inputs.windows(2) {
        if pair[1].pillar_time <= pair[0].pillar_time {
            return Err(crate::Error::Validation(format!(
                "hazard {input_kind} replay pillar times must be strictly increasing"
            )));
        }
    }
    Ok(())
}

fn quote_id(quote: &serde_json::Value) -> &str {
    quote
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{HazardCalibrationInput, HazardCalibrationRecipe};

    fn input(id: &str, pillar_date: &str, pillar_time: f64) -> HazardCalibrationInput {
        HazardCalibrationInput {
            quote: serde_json::json!({"id": id, "type": "cds_par_spread"}),
            pillar_date: crate::dates::parse_iso_date(pillar_date).expect("valid pillar date"),
            pillar_time,
        }
    }

    #[test]
    fn hazard_calibration_recipe_new_canonicalizes_unsorted_inputs_and_serde() {
        let one_year = input("CDS-1Y", "2026-03-20", 1.0);
        let five_year = input("CDS-5Y", "2030-03-20", 5.0);
        let recipe = HazardCalibrationRecipe::new(
            serde_json::json!({"curve_id": "ACME-HZD"}),
            vec![five_year.clone(), one_year.clone()],
            vec![five_year, one_year],
            serde_json::json!({"fail_on_bad_fit": true}),
        )
        .expect("unsorted market inputs must canonicalize");

        let ids = |inputs: &[HazardCalibrationInput]| {
            inputs
                .iter()
                .map(|input| input.quote["id"].as_str().expect("quote ID").to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&recipe.calibration_inputs), ["CDS-1Y", "CDS-5Y"]);
        assert_eq!(ids(&recipe.spread_risk_inputs), ["CDS-1Y", "CDS-5Y"]);

        let first = serde_json::to_string(&recipe).expect("serialize canonical recipe");
        let round_trip: HazardCalibrationRecipe =
            serde_json::from_str(&first).expect("deserialize canonical recipe");
        let second = serde_json::to_string(&round_trip).expect("reserialize canonical recipe");
        assert_eq!(first, second, "canonical recipe serde must be stable");
    }

    #[test]
    fn hazard_calibration_recipe_accepts_strictly_ordered_atomic_inputs() {
        let recipe: HazardCalibrationRecipe = serde_json::from_value(serde_json::json!({
            "hazard_params": {"curve_id": "ACME-HZD"},
            "calibration_inputs": [
                {
                    "quote": {"id": "CDS-1Y-A", "type": "cds_par_spread"},
                    "pillar_date": "2026-03-20",
                    "pillar_time": 1.0
                },
                {
                    "quote": {"id": "CDS-5Y", "type": "cds_par_spread"},
                    "pillar_date": "2030-03-20",
                    "pillar_time": 5.0
                }
            ],
            "spread_risk_inputs": [
                {
                    "quote": {"id": "CDS-1Y-A", "type": "cds_par_spread"},
                    "pillar_date": "2026-03-20",
                    "pillar_time": 1.0
                },
                {
                    "quote": {"id": "CDS-5Y", "type": "cds_par_spread"},
                    "pillar_date": "2030-03-20",
                    "pillar_time": 5.0
                }
            ],
            "calibration_config": {"fail_on_bad_fit": true}
        }))
        .expect("new atomic recipe shape should deserialize");

        let serialized = serde_json::to_value(recipe).expect("recipe should serialize");
        let calibration_ids: Vec<_> = serialized["calibration_inputs"]
            .as_array()
            .expect("calibration inputs")
            .iter()
            .map(|input| input["quote"]["id"].as_str().expect("quote id"))
            .collect();
        assert_eq!(calibration_ids, ["CDS-1Y-A", "CDS-5Y"]);

        let risk_ids: Vec<_> = serialized["spread_risk_inputs"]
            .as_array()
            .expect("spread risk inputs")
            .iter()
            .map(|input| input["quote"]["id"].as_str().expect("quote id"))
            .collect();
        assert_eq!(risk_ids, ["CDS-1Y-A", "CDS-5Y"]);
    }

    #[test]
    fn hazard_calibration_recipe_rejects_obsolete_quote_array() {
        let obsolete = serde_json::json!({
            "hazard_params": {"curve_id": "ACME-HZD"},
            "cds_quotes": [{"id": "CDS-5Y", "type": "cds_par_spread"}],
            "calibration_config": {"fail_on_bad_fit": true}
        });

        assert!(
            serde_json::from_value::<HazardCalibrationRecipe>(obsolete).is_err(),
            "legacy cds_quotes recipes must be rejected"
        );
    }

    #[test]
    fn hazard_calibration_recipe_rejects_mismatched_quote_ids() {
        let malformed = serde_json::json!({
            "hazard_params": {"curve_id": "ACME-HZD"},
            "calibration_inputs": [{
                "quote": {
                    "id": "CDS-5Y",
                    "type": "cds_par_spread",
                    "pillar": {"date": "2030-03-20"}
                },
                "pillar_date": "2030-03-20",
                "pillar_time": 5.0
            }],
            "spread_risk_inputs": [{
                "quote": {
                    "id": "CDS-5Y-RISK",
                    "type": "cds_par_spread",
                    "pillar": {"date": "2030-03-20"}
                },
                "pillar_date": "2030-03-20",
                "pillar_time": 5.0
            }],
            "calibration_config": {"fail_on_bad_fit": true}
        });

        assert!(
            serde_json::from_value::<HazardCalibrationRecipe>(malformed).is_err(),
            "serialized recipes must reject mismatched quote IDs"
        );
    }

    #[test]
    fn hazard_calibration_recipe_rejects_upfront_spread_risk_input() {
        let malformed = serde_json::json!({
            "hazard_params": {"curve_id": "ACME-HZD"},
            "calibration_inputs": [{
                "quote": {
                    "id": "CDS-5Y",
                    "type": "cds_upfront",
                    "pillar": {"date": "2030-03-20"}
                },
                "pillar_date": "2030-03-20",
                "pillar_time": 5.0
            }],
            "spread_risk_inputs": [{
                "quote": {
                    "id": "CDS-5Y",
                    "type": "cds_upfront",
                    "pillar": {"date": "2030-03-20"}
                },
                "pillar_date": "2030-03-20",
                "pillar_time": 5.0
            }],
            "calibration_config": {"fail_on_bad_fit": true}
        });

        assert!(
            serde_json::from_value::<HazardCalibrationRecipe>(malformed).is_err(),
            "spread-risk replay inputs must deserialize as par-spread quotes"
        );
    }
}
