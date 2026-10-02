//! WASM bindings for PD calibration: PIT/TTC conversion, central tendency,
//! the Basel IRB floor and master-scale mapping.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/credit/pd.rs`. The data
//! types `MasterScaleGrade` and `MasterScaleResult` cross the boundary as
//! plain objects in their canonical serde form.

use crate::utils::input::{from_js_json, js_f64, js_f64_seq, js_string};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_models::credit::pd::{
    self, MasterScale, MasterScaleGrade, PdCycleParams, BASEL_IRB_PD_FLOOR,
};
use finstack_quant_models::credit::scoring::ScoringResult;
use wasm_bindgen::prelude::*;

/// Basel IRB regulatory PD floor (0.03%), as a decimal. Twin of the Rust and
/// Python constant `BASEL_IRB_PD_FLOOR`.
/// @returns The floor, `0.0003`.
#[wasm_bindgen(js_name = baselIrbPdFloor)]
pub fn basel_irb_pd_floor() -> f64 {
    BASEL_IRB_PD_FLOOR
}

/// Apply the Basel IRB regulatory PD floor.
/// @param pd - Default probability as a decimal.
/// @returns `max(pd, 0.0003)`.
///
/// # Errors
///
/// Throws a `TypeError` if `pd` is not a number.
#[wasm_bindgen(js_name = applyBaselIrbPdFloor)]
pub fn apply_basel_irb_pd_floor(pd: JsValue) -> Result<f64, JsValue> {
    Ok(pd::apply_basel_irb_pd_floor(js_f64(&pd, "pd")?))
}

/// Convert a point-in-time PD to through-the-cycle under the Vasicek single-factor model.
/// @param pd_pit - Point-in-time default probability, strictly between 0 and 1.
/// @param asset_correlation - Asset correlation with the systematic factor, strictly between 0 and 1.
/// @param cycle_index - Standardized credit-cycle index; positive in benign conditions, negative in stress.
/// @returns The through-the-cycle default probability.
///
/// # Errors
///
/// Throws a `validation` error if the PD or correlation is outside `(0, 1)`
/// or an input is non-finite.
#[wasm_bindgen(js_name = pitToTtc)]
pub fn pit_to_ttc(
    pd_pit: JsValue,
    asset_correlation: JsValue,
    cycle_index: JsValue,
) -> Result<f64, JsValue> {
    let pd_pit = js_f64(&pd_pit, "pdPit")?;
    let params = PdCycleParams {
        asset_correlation: js_f64(&asset_correlation, "assetCorrelation")?,
        cycle_index: js_f64(&cycle_index, "cycleIndex")?,
    };
    pd::pit_to_ttc(pd_pit, &params).map_err(to_js_err)
}

/// Convert a through-the-cycle PD to point-in-time under the Vasicek single-factor model.
/// @param pd_ttc - Through-the-cycle default probability, strictly between 0 and 1.
/// @param asset_correlation - Asset correlation with the systematic factor, strictly between 0 and 1.
/// @param cycle_index - Standardized credit-cycle index; positive in benign conditions, negative in stress.
/// @returns The point-in-time default probability.
///
/// # Errors
///
/// Throws a `validation` error if the PD or correlation is outside `(0, 1)`
/// or an input is non-finite.
#[wasm_bindgen(js_name = ttcToPit)]
pub fn ttc_to_pit(
    pd_ttc: JsValue,
    asset_correlation: JsValue,
    cycle_index: JsValue,
) -> Result<f64, JsValue> {
    let pd_ttc = js_f64(&pd_ttc, "pdTtc")?;
    let params = PdCycleParams {
        asset_correlation: js_f64(&asset_correlation, "assetCorrelation")?,
        cycle_index: js_f64(&cycle_index, "cycleIndex")?,
    };
    pd::ttc_to_pit(pd_ttc, &params).map_err(to_js_err)
}

/// Long-run central-tendency PD from a history of annual default rates.
/// @param annual_default_rates - Annual default rates as decimals in `[0, 1]`, as a `number[]` or `Float64Array`; non-empty.
/// @returns The central-tendency default probability.
///
/// # Errors
///
/// Throws a `validation` error if the series is empty or a rate is non-finite
/// or outside `[0, 1]`.
#[wasm_bindgen(js_name = centralTendency)]
pub fn central_tendency(annual_default_rates: JsValue) -> Result<f64, JsValue> {
    let rates = js_f64_seq(&annual_default_rates, "annualDefaultRates")?;
    pd::central_tendency(&rates).map_err(to_js_err)
}

/// Rating master scale mapping default probabilities to grades.
#[wasm_bindgen(js_name = MasterScale)]
pub struct JsMasterScale {
    pub(crate) inner: MasterScale,
}

json_round_trip!(JsMasterScale, MasterScale);

#[wasm_bindgen(js_class = MasterScale)]
impl JsMasterScale {
    /// Master scale from explicit grades.
    /// @param grades - Array of `MasterScaleGrade` objects (`label`, `upper_pd`, `central_pd`) in ascending `upper_pd` order, or its JSON text.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the list is empty, a PD is out of range,
    /// or the grades are not sorted by `upper_pd`.
    #[wasm_bindgen(constructor)]
    pub fn new(grades: JsValue) -> Result<JsMasterScale, JsValue> {
        let grades: Vec<MasterScaleGrade> = from_js_json(&grades, "grades")?;
        MasterScale::new(grades)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// S&P-style master scale from the embedded credit-assumption registry.
    /// @returns The S&P assumptions master scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry cannot supply the scale.
    #[wasm_bindgen(js_name = spAssumptions)]
    pub fn sp_assumptions() -> Result<JsMasterScale, JsValue> {
        MasterScale::sp_assumptions()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Moody's-style master scale from the embedded credit-assumption registry.
    /// @returns The Moody's assumptions master scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the registry cannot supply the scale.
    #[wasm_bindgen(js_name = moodysAssumptions)]
    pub fn moodys_assumptions() -> Result<JsMasterScale, JsValue> {
        MasterScale::moodys_assumptions()
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Master scale registered under an explicit registry identifier.
    /// @param scale_id - Registry identifier of the master scale.
    /// @returns The registered master scale.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if no scale is registered under `scaleId`.
    #[wasm_bindgen(js_name = fromRegistryId)]
    pub fn from_registry_id(scale_id: JsValue) -> Result<JsMasterScale, JsValue> {
        MasterScale::from_registry_id(&js_string(&scale_id, "scaleId")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Map one PD to its grade: the first grade whose `upper_pd` is at or above it.
    /// @param pd - Default probability as a decimal from 0 through 1.
    /// @returns The `MasterScaleResult` object (`grade`, `central_pd`, `input_pd`, `grade_index`).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `pd` is non-finite or outside `[0, 1]`.
    #[wasm_bindgen(js_name = mapPd)]
    pub fn map_pd(&self, pd: JsValue) -> Result<JsValue, JsValue> {
        let result = self.inner.map_pd(js_f64(&pd, "pd")?).map_err(to_js_err)?;
        to_js_value(&result)
    }

    /// Map a batch of PDs to their grades (Python returns the same rows as a DataFrame).
    /// @param pds - Default probabilities as decimals from 0 through 1, as a `number[]` or `Float64Array`.
    /// @returns One `MasterScaleResult` object per PD, in input order.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if any PD is non-finite or outside `[0, 1]`.
    #[wasm_bindgen(js_name = mapPds)]
    pub fn map_pds(&self, pds: JsValue) -> Result<JsValue, JsValue> {
        let pds = js_f64_seq(&pds, "pds")?;
        let rows = self.inner.map_pds(&pds).map_err(to_js_err)?;
        to_js_value(&rows)
    }

    /// Map a credit-scoring result to a grade through its implied PD.
    /// @param result - `ScoringResult` object or JSON, as returned by the `models.credit` scoring functions.
    /// @returns The `MasterScaleResult` object for the score's implied PD.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the result is malformed, has no implied
    /// PD, or its PD is non-finite.
    #[wasm_bindgen(js_name = mapScore)]
    pub fn map_score(&self, result: JsValue) -> Result<JsValue, JsValue> {
        let result: ScoringResult = from_js_json(&result, "result")?;
        let mapped = self.inner.map_score(&result).map_err(to_js_err)?;
        to_js_value(&mapped)
    }

    /// Number of grades in the scale.
    #[wasm_bindgen(getter, js_name = nGrades)]
    pub fn n_grades(&self) -> usize {
        self.inner.n_grades()
    }

    /// Grades in ascending PD order, as `MasterScaleGrade` objects.
    #[wasm_bindgen(getter)]
    pub fn grades(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.grades())
    }
}
