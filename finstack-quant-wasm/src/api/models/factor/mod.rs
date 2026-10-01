//! WASM bindings for factor, credit-factor, and factor-risk models.
//!
//! Exposes credit hierarchy artifacts and product-independent factor-risk
//! decomposition kernels through the `models.factor` facade.
//!
//! `VolHorizon::Custom` is intentionally **not** exposed — closures do not
//! cross the WASM boundary.
//!
//! Horizon strings accepted by the covariance forecast methods:
//!
//! - `"one_step"` — calibrated annualized variance unchanged.
//! - `"unconditional"` — long-run (identical to `"one_step"` for `Sample` vol
//!   model).
//! - JSON string `'{"n_steps": N}'` — variance scaled by `N`.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_matrix, js_f64_seq, js_opt_bool, js_opt_f64, js_string,
    js_string_seq, js_uint, json_text, opt_json_text,
};
use crate::utils::{to_js_err, to_js_value};

use finstack_quant_models::factor::credit::VolHorizon;
use finstack_quant_models::factor::risk::{DecompositionConfig, PositionRiskDecomposition};
use finstack_quant_models::factor::{FactorCovarianceMatrix, FactorId, FactorModelConfig};
use wasm_bindgen::prelude::*;

// Horizon helper (shared by CreditCalibrator and FactorCovarianceForecast)

/// Parse a horizon descriptor string into a [`VolHorizon`].
///
/// Delegates to the canonical [`VolHorizon::parse`] implementation in
/// `finstack-quant-models`; this wrapper only maps the error to a `JsValue`.
fn parse_vol_horizon(
    s: &str,
) -> Result<finstack_quant_models::factor::credit::VolHorizon, JsValue> {
    finstack_quant_models::factor::credit::VolHorizon::parse(s).map_err(to_js_err)
}

/// Calibrated credit factor hierarchy artifact.
///
/// Produced by [`JsCreditCalibrator`] or loaded from JSON via
/// [`JsCreditFactorModel::from_json`]. Immutable once constructed.
#[wasm_bindgen(js_name = CreditFactorModel)]
pub struct JsCreditFactorModel {
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_quant_models::factor::credit::hierarchy::CreditFactorModel,
}

#[wasm_bindgen(js_class = CreditFactorModel)]
impl JsCreditFactorModel {
    /// Deserialize a `CreditFactorModel` from JSON.
    ///
    /// Validates the required `schema` marker and all structural constraints.
    ///
    /// # Errors
    /// Throws a `validation` error if the JSON is malformed or fails
    /// validation.
    /// @param json - JSON-serialized CreditFactorModel to deserialize.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsCreditFactorModel, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner =
            finstack_quant_models::factor::credit::hierarchy::CreditFactorModel::from_json(json)
                .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Serialize this model to compact wire JSON.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the model cannot be serialized to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        self.inner.to_json().map_err(to_js_err)
    }

    /// Return the exact namespaced contract marker.
    ///
    /// @returns The string `"finstack_quant.credit_factor_model/1"`.
    #[wasm_bindgen(getter)]
    pub fn schema(&self) -> String {
        self.inner.schema.as_str().to_owned()
    }
}

/// Deterministic calibrator that produces a [`JsCreditFactorModel`].
///
/// Configuration and inputs are passed as JSON strings or plain objects.
#[wasm_bindgen(js_name = CreditCalibrator)]
pub struct JsCreditCalibrator {
    inner: finstack_quant_models::factor::credit::calibration::CreditCalibrator,
}

#[wasm_bindgen(js_class = CreditCalibrator)]
impl JsCreditCalibrator {
    /// Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`.
    ///
    /// # Errors
    /// Throws if `config_json` is not a valid `CreditCalibrationConfig`.
    /// @param config_json - Optional credit-factor calibration configuration JSON; omitted or `null` uses the Rust `CreditCalibrationConfig::default()`.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: Option<JsValue>) -> Result<JsCreditCalibrator, JsValue> {
        let config_json = opt_json_text(config_json.as_ref(), "configJson")?;
        let config: finstack_quant_models::factor::credit::calibration::CreditCalibrationConfig =
            match config_json.as_deref() {
                Some(json) => serde_json::from_str(json).map_err(to_js_err)?,
                None => Default::default(),
            };
        Ok(Self {
            inner: finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                config,
            ),
        })
    }

    /// Run the full calibration pipeline and return a `CreditFactorModel`.
    ///
    /// `inputs_json` must be a JSON-serialized `CreditCalibrationInputs`.
    ///
    /// # Errors
    /// Throws if inputs are structurally invalid or calibration fails.
    /// @param inputs_json - Credit-factor calibration input JSON containing issuers, spreads, and observations.
    pub fn calibrate(&self, inputs_json: JsValue) -> Result<JsCreditFactorModel, JsValue> {
        let inputs_json: &str = &json_text(&inputs_json, "inputsJson")?;
        let inputs: finstack_quant_models::factor::credit::calibration::CreditCalibrationInputs =
            serde_json::from_str(inputs_json).map_err(to_js_err)?;
        let model = self.inner.calibrate(inputs).map_err(to_js_err)?;
        Ok(JsCreditFactorModel { inner: model })
    }
}

// LevelsAtDate  (opaque handle — not exposed as a JS class, just passed through)

/// Snapshot of all hierarchy-level factor values at a single date.
///
/// Produced by [`decompose_levels`]. Pass to [`decompose_period`] to compute
/// period-over-period changes.  The full data is available via `toJson`.
#[wasm_bindgen(js_name = LevelsAtDate)]
pub struct JsLevelsAtDate {
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_quant_models::factor::credit::decomposition::LevelsAtDate,
}

#[wasm_bindgen(js_class = LevelsAtDate)]
impl JsLevelsAtDate {
    /// Deserialize a factor-level snapshot from canonical JSON.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `LevelsAtDate` JSON containing `date`, `generic`,
    ///   `by_level`, and `adder` fields.
    ///
    /// # Errors
    ///
    /// Throws when the JSON is malformed or a numeric field is non-finite.
    /// @param json - Canonical `LevelsAtDate` JSON.
    /// @returns A validated `LevelsAtDate` handle.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsLevelsAtDate, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner =
            finstack_quant_models::factor::credit::decomposition::LevelsAtDate::from_json(json)
                .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Serialize the snapshot to JSON.
    ///
    /// # Errors
    /// Throws if any numeric output field is non-finite (NaN/Inf), naming
    /// the offending field instead of silently serializing `null`.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        self.inner.to_json().map_err(to_js_err)
    }

    /// Observation date as an ISO-8601 string.
    #[wasm_bindgen(getter)]
    pub fn date(&self) -> String {
        self.inner.date.to_string()
    }

    /// Generic factor level in basis points.
    #[wasm_bindgen(getter)]
    pub fn generic(&self) -> f64 {
        self.inner.generic
    }

    /// Number of hierarchy levels in this snapshot.
    #[wasm_bindgen(getter, js_name = nLevels)]
    pub fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Return the bucket-value map for one zero-based hierarchy level.
    ///
    /// # Arguments
    ///
    /// * `level_index` - Zero-based hierarchy level index, which must be less
    ///   than `nLevels`.
    ///
    /// # Errors
    ///
    /// Throws when `level_index` is outside the available levels or the map
    /// cannot be converted to a JavaScript object.
    /// @param levelIndex - Zero-based hierarchy level index.
    /// @returns A bucket-name to factor-level mapping in basis points.
    #[wasm_bindgen(js_name = levelValues)]
    pub fn level_values(&self, level_index: JsValue) -> Result<JsValue, JsValue> {
        let level_index: usize = js_uint(&level_index, "levelIndex")?;
        let level = self.inner.level(level_index).map_err(to_js_err)?;
        to_js_value(&level.values)
    }

    /// Return per-issuer residual adders in basis points.
    ///
    /// # Errors
    ///
    /// Throws when the mapping cannot be converted to a JavaScript object.
    /// @returns An issuer-ID to residual-adder mapping.
    #[wasm_bindgen(js_name = adder)]
    pub fn adder(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.adder)
    }
}

/// Component-wise difference between two [`JsLevelsAtDate`] snapshots.
///
/// Produced by [`decompose_period`].
#[wasm_bindgen(js_name = PeriodDecomposition)]
pub struct JsPeriodDecomposition {
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_quant_models::factor::credit::decomposition::PeriodDecomposition,
}

#[wasm_bindgen(js_class = PeriodDecomposition)]
impl JsPeriodDecomposition {
    /// Deserialize a period decomposition from canonical JSON.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical `PeriodDecomposition` JSON containing `from`,
    ///   `to`, `d_generic`, `by_level`, and `d_adder` fields.
    ///
    /// # Errors
    ///
    /// Throws when the JSON is malformed or a numeric field is non-finite.
    /// @param json - Canonical `PeriodDecomposition` JSON.
    /// @returns A validated `PeriodDecomposition` handle.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsPeriodDecomposition, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner =
            finstack_quant_models::factor::credit::decomposition::PeriodDecomposition::from_json(
                json,
            )
            .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Serialize the decomposition to JSON.
    ///
    /// # Errors
    /// Throws if any numeric output field is non-finite (NaN/Inf), naming
    /// the offending field instead of silently serializing `null`.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        self.inner.to_json().map_err(to_js_err)
    }

    /// Earlier snapshot date as an ISO-8601 string.
    #[wasm_bindgen(getter, js_name = fromDate)]
    pub fn from_date(&self) -> String {
        self.inner.from.to_string()
    }

    /// Later snapshot date as an ISO-8601 string.
    #[wasm_bindgen(getter, js_name = toDate)]
    pub fn to_date(&self) -> String {
        self.inner.to.to_string()
    }

    /// Change in the generic factor in basis points.
    #[wasm_bindgen(getter, js_name = dGeneric)]
    pub fn d_generic(&self) -> f64 {
        self.inner.d_generic
    }

    /// Number of hierarchy levels in this decomposition.
    #[wasm_bindgen(getter, js_name = nLevels)]
    pub fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Return the bucket-delta map for one zero-based hierarchy level.
    ///
    /// # Arguments
    ///
    /// * `level_index` - Zero-based hierarchy level index, which must be less
    ///   than `nLevels`.
    ///
    /// # Errors
    ///
    /// Throws when `level_index` is outside the available levels or the map
    /// cannot be converted to a JavaScript object.
    /// @param levelIndex - Zero-based hierarchy level index.
    /// @returns A bucket-name to factor-change mapping in basis points.
    #[wasm_bindgen(js_name = levelDeltas)]
    pub fn level_deltas(&self, level_index: JsValue) -> Result<JsValue, JsValue> {
        let level_index: usize = js_uint(&level_index, "levelIndex")?;
        let level = self.inner.level(level_index).map_err(to_js_err)?;
        to_js_value(&level.deltas)
    }

    /// Return per-issuer residual-adder deltas in basis points.
    ///
    /// # Errors
    ///
    /// Throws when the mapping cannot be converted to a JavaScript object.
    /// @returns An issuer-ID to residual-adder-change mapping.
    #[wasm_bindgen(js_name = dAdder)]
    pub fn d_adder(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.d_adder)
    }
}

// decompose_levels  (free function)

/// Decompose observed issuer spreads at a point in time into per-level factor
/// values and per-issuer residual adders.
///
/// Callers pass **decimal** spreads (`0.012` = 120 bp). Returned factor
/// levels and adders are **bp**.
///
/// # Arguments
///
/// * `model` - Calibrated credit factor hierarchy used for the peel.
/// * `observed_spreads_json` - JSON `{issuer_id: spread}` map in decimal
///   (`0.012` = 120 bp). Values that look like bp (e.g. `100.0`) are rejected.
/// * `observed_generic` - Generic (PC) factor value at `as_of`, same decimal
///   convention as the spreads.
/// * `as_of` - ISO-8601 valuation date for the snapshot.
/// * `runtime_tags_json` - Optional JSON `{issuer_id: {dim_key: tag}}` for
///   issuers not present in the model artifact.
///
/// # Errors
///
/// Throws a `not_found` error if an issuer has no model row and no
/// `runtime_tags` entry, and a `validation` error if `as_of` cannot be parsed
/// or a spread is outside the decimal band.
///
/// @param model - Calibrated CreditFactorModel used for the peel.
/// @param observedSpreadsJson - JSON `{issuer_id: spread}` map in decimal (`0.012` = 120 bp). Returned levels are bp.
/// @param observedGeneric - Observed generic-market spread in decimal, aligned with the model factors.
/// @param asOf - ISO-8601 valuation date used to stamp the snapshot.
/// @param runtimeTagsJson - Optional runtime-tag JSON for issuers missing from the artifact.
#[wasm_bindgen(js_name = decomposeLevels)]
pub fn decompose_levels(
    model: &JsCreditFactorModel,
    observed_spreads_json: JsValue,
    observed_generic: JsValue,
    as_of: JsValue,
    runtime_tags_json: Option<JsValue>,
) -> Result<JsLevelsAtDate, JsValue> {
    let observed_generic = js_f64(&observed_generic, "observedGeneric")?;
    let observed_spreads_json: &str = &json_text(&observed_spreads_json, "observedSpreadsJson")?;
    let as_of: &str = &js_string(&as_of, "asOf")?;
    let runtime_tags_json = opt_json_text(runtime_tags_json.as_ref(), "runtimeTagsJson")?;
    let observed_spreads: std::collections::BTreeMap<finstack_quant_core::types::IssuerId, f64> =
        serde_json::from_str(observed_spreads_json).map_err(to_js_err)?;

    let date = finstack_quant_core::dates::parse_iso_date(as_of).map_err(to_js_err)?;

    let runtime_tags: Option<
        std::collections::BTreeMap<
            finstack_quant_core::types::IssuerId,
            finstack_quant_models::factor::credit::hierarchy::IssuerTags,
        >,
    > = match runtime_tags_json.as_deref() {
        Some(json) => Some(serde_json::from_str(json).map_err(to_js_err)?),
        None => None,
    };

    let inner = finstack_quant_models::factor::credit::decomposition::decompose_levels(
        &model.inner,
        &observed_spreads,
        observed_generic,
        date,
        runtime_tags.as_ref(),
    )
    .map_err(to_js_err)?;

    Ok(JsLevelsAtDate { inner })
}

// decompose_period  (free function)

/// Difference two `LevelsAtDate` snapshots component-wise.
///
/// Output buckets and issuers are restricted to those present in **both**
/// snapshots so the linear reconciliation invariant on `ΔS_i` holds.
///
/// # Errors
/// Throws if `from_levels.date > to_levels.date` or the snapshots disagree
/// on hierarchy depth.
/// @param from_levels - Credit-factor levels at the start of the attribution period.
/// @param to_levels - Credit-factor levels at the end of the attribution period.
#[wasm_bindgen(js_name = decomposePeriod)]
pub fn decompose_period(
    from_levels: &JsLevelsAtDate,
    to_levels: &JsLevelsAtDate,
) -> Result<JsPeriodDecomposition, JsValue> {
    let inner = finstack_quant_models::factor::credit::decomposition::decompose_period(
        &from_levels.inner,
        &to_levels.inner,
    )
    .map_err(to_js_err)?;
    Ok(JsPeriodDecomposition { inner })
}

/// Vol-forecast view over a calibrated `CreditFactorModel`.
///
/// `VolHorizon::Custom` is intentionally **not** exposed.
#[wasm_bindgen(js_name = FactorCovarianceForecast)]
pub struct JsFactorCovarianceForecast {
    /// Store the model by value so `FactorCovarianceForecast<'a>` lifetime
    /// requirements don't escape the WASM boundary.
    model: finstack_quant_models::factor::credit::hierarchy::CreditFactorModel,
}

#[wasm_bindgen(js_class = FactorCovarianceForecast)]
impl JsFactorCovarianceForecast {
    /// Wrap a `CreditFactorModel` for vol forecasting.
    /// @param model - Calibrated CreditFactorModel used to produce the covariance forecast.
    #[wasm_bindgen(constructor)]
    pub fn new(model: &JsCreditFactorModel) -> JsFactorCovarianceForecast {
        Self {
            model: model.inner.clone(),
        }
    }

    /// Build the factor covariance matrix `Σ(t, h) = D · ρ_static · D`.
    ///
    /// Returns a structured `FactorCovarianceMatrix` JavaScript object.
    ///
    /// `horizon_json` accepts `"one_step"`, `"unconditional"`, or
    /// `'{"n_steps": N}'`.
    ///
    /// # Errors
    /// Throws if the horizon string is invalid or the model data is
    /// inconsistent.
    /// @returns Structured covariance matrix with ordered factor axes and row-major data.
    /// @param horizon_json - JSON-serialized forecast horizon defining the future covariance date or period.
    #[wasm_bindgen(js_name = covarianceAt)]
    pub fn covariance_at(&self, horizon_json: JsValue) -> Result<JsValue, JsValue> {
        let horizon_json: &str = &json_text(&horizon_json, "horizonJson")?;
        let h = parse_vol_horizon(horizon_json)?;
        let forecast =
            finstack_quant_models::factor::credit::FactorCovarianceForecast::new(&self.model);
        let cov = forecast.covariance_at(h).map_err(to_js_err)?;
        crate::utils::to_js_value(&cov)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested
    /// horizon.
    ///
    /// # Errors
    /// Throws if the issuer is not present in the model's vol state or the
    /// calibrated variance is negative.
    /// @param issuer_id - Stable issuer identifier used to select the required domain object.
    /// @param horizon_json - JSON-serialized forecast horizon defining the future covariance date or period.
    #[wasm_bindgen(js_name = idiosyncraticVol)]
    pub fn idiosyncratic_vol(
        &self,
        issuer_id: JsValue,
        horizon_json: JsValue,
    ) -> Result<f64, JsValue> {
        let issuer_id: &str = &js_string(&issuer_id, "issuerId")?;
        let horizon_json: &str = &json_text(&horizon_json, "horizonJson")?;
        let h = parse_vol_horizon(horizon_json)?;
        let id = finstack_quant_core::types::IssuerId::new(issuer_id);
        let forecast =
            finstack_quant_models::factor::credit::FactorCovarianceForecast::new(&self.model);
        forecast.idiosyncratic_vol(&id, h).map_err(to_js_err)
    }

    /// Build a structured portfolio-level `FactorModelConfig` using `Σ(t, h)`
    /// at the given horizon and risk measure.
    ///
    /// # Errors
    /// Throws if the horizon or risk measure is invalid, or the model builder
    /// rejects the assembled configuration.
    /// @returns Structured factor-model configuration ready for portfolio risk workflows.
    /// @param horizon_json - JSON-serialized forecast horizon defining the future covariance date or period.
    /// @param risk_measure_json - Optional `RiskMeasure` wire value for the horizon factor model: `"variance"`, `"volatility"`, an object such as `{ var: { confidence: 0.99 } }`, or the same value as JSON text; omitted or `null` uses the Rust `RiskMeasure::default()` (`"variance"`).
    #[wasm_bindgen(js_name = factorModelAt)]
    pub fn factor_model_at(
        &self,
        horizon_json: JsValue,
        risk_measure_json: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let horizon_json: &str = &json_text(&horizon_json, "horizonJson")?;
        let h = parse_vol_horizon(horizon_json)?;
        // `js_opt_wire`: the unit variants are bare wire strings (`"variance"`),
        // which are not JSON text.
        let measure: finstack_quant_models::factor::RiskMeasure =
            crate::utils::wire::js_opt_wire(risk_measure_json.as_ref(), "riskMeasureJson")?
                .unwrap_or_default();
        let forecast =
            finstack_quant_models::factor::credit::FactorCovarianceForecast::new(&self.model);
        let config = forecast
            .factor_model_config_at(h, measure)
            .map_err(to_js_err)?;
        crate::utils::to_js_value(&config)
    }
}

//
// Native tests call underlying Rust APIs directly — WASM wrapper methods that
// invoke `js_sys::Error::new` cannot run on non-wasm32 targets.  The WASM
// surface is exercised end-to-end by `wasm-pack test`.

// Position-level VaR / ES decomposition and risk budgeting

/// Decompose portfolio VaR and ES into position contributions via parametric
/// Euler allocation.
///
/// Returns the canonical `PositionRiskDecomposition` (the object Python's
/// `parametric_var_decomposition` returns): portfolio VaR/ES (losses
/// negative), `method`, and per-position `var_contributions` and
/// `es_contributions` rows.
/// @param position_ids - Position identifiers, one per weight.
/// @param weights - Position weights or exposures in portfolio currency.
/// @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
/// @param compute_incremental - Optional; when `true`, also computes
///   incremental VaR (one full repricing per position). Defaults to `false`.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if identifier, weight, or covariance dimensions
/// disagree; the covariance matrix is not finite, symmetric, and positive
/// semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
#[wasm_bindgen(js_name = parametricVarDecomposition)]
pub fn parametric_var_decomposition(
    position_ids: JsValue,
    weights: JsValue,
    covariance: JsValue,
    confidence: Option<JsValue>,
    compute_incremental: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{
        flatten_square_matrix, DecompositionConfig, ParametricPositionDecomposer,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let weights = js_f64_seq(&weights, "weights")?;
    let covariance = js_f64_matrix(&covariance, "covariance")?;
    let compute_incremental = js_opt_bool(compute_incremental.as_ref(), "computeIncremental")?;

    let cov_flat =
        flatten_square_matrix(covariance, weights.len(), "covariance").map_err(to_js_err)?;
    let mut config = confidence.map_or_else(
        DecompositionConfig::parametric_95,
        DecompositionConfig::parametric,
    );
    if compute_incremental == Some(true) {
        config = config.with_incremental();
    }
    let result = ParametricPositionDecomposer
        .decompose_positions(&weights, &cov_flat, &ids, &config)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Decompose portfolio Expected Shortfall into position contributions via
/// parametric Euler allocation.
///
/// Returns the `ParametricEsDecompositionView` reporting view (the object
/// Python's `parametric_es_decomposition` returns): a top-level
/// `{portfolio_var, portfolio_es, confidence, n_positions, contributions}`
/// object whose `contributions` entries are
/// `{position_id, component_es, marginal_es, pct_contribution}`.
/// @param position_ids - Position identifiers, one per weight.
/// @param weights - Position weights or exposures in portfolio currency.
/// @param covariance - Square position-return covariance matrix as nested rows (`n x n`, row-major).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::parametric_95()` preset (0.95).
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if identifier, weight, or covariance dimensions
/// disagree; the covariance matrix is not finite, symmetric, and positive
/// semidefinite; or `confidence` is not finite and in `(0.5, 1)`.
#[wasm_bindgen(js_name = parametricEsDecomposition)]
pub fn parametric_es_decomposition(
    position_ids: JsValue,
    weights: JsValue,
    covariance: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{
        flatten_square_matrix, parametric_es_decomposition_view, DecompositionConfig,
        ParametricPositionDecomposer,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let weights = js_f64_seq(&weights, "weights")?;
    let covariance = js_f64_matrix(&covariance, "covariance")?;

    let cov_flat =
        flatten_square_matrix(covariance, weights.len(), "covariance").map_err(to_js_err)?;
    let config = confidence.map_or_else(
        DecompositionConfig::parametric_95,
        DecompositionConfig::parametric,
    );
    let decomposition = ParametricPositionDecomposer
        .decompose_positions(&weights, &cov_flat, &ids, &config)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&parametric_es_decomposition_view(&decomposition))
}

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation.
///
/// Returns the canonical `PositionRiskDecomposition`, including the
/// historical `es_contributions` rows (marginal and incremental VaR are
/// `null`).
/// @param position_ids - Position identifiers, one per P&L row.
/// @param position_pnls - Position-major P&L matrix: one row per position, one column per scenario (losses negative).
/// @param confidence - Optional tail confidence as a decimal probability in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::historical_95()` preset (0.95).
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if the matrix does not have one row per position or its
/// rows have different scenario counts, `confidence` is not finite and in
/// `(0.5, 1)`, too few scenarios resolve the requested tail, or a P&L value is
/// non-finite.
#[wasm_bindgen(js_name = historicalVarDecomposition)]
pub fn historical_var_decomposition(
    position_ids: JsValue,
    position_pnls: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?;
    use finstack_quant_models::factor::risk::{
        flatten_position_pnls, DecompositionConfig, HistoricalPositionDecomposer,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let position_pnls = js_f64_matrix(&position_pnls, "positionPnls")?;

    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, ids.len()).map_err(to_js_err)?;
    let config = confidence.map_or_else(
        DecompositionConfig::historical_95,
        DecompositionConfig::historical,
    );
    let result = HistoricalPositionDecomposer
        .decompose_from_pnls(&flat, &ids, n_scenarios, &config)
        .map_err(to_js_err)?;
    crate::utils::to_js_value(&result)
}

/// Evaluate a per-position risk budget against actual component VaRs.
///
/// Returns the canonical `RiskBudgetResult` (the object Python's
/// `evaluate_risk_budget` returns): per-position `positions` rows,
/// `total_overbudget` and `has_breach`. Validation (array-length agreement,
/// duplicate position-id rejection) and the default `utilizationThreshold`
/// live in the canonical Rust `evaluate_risk_budget_arrays` /
/// `DEFAULT_UTILIZATION_THRESHOLD` path shared with the Python binding.
/// @param position_ids - Position identifiers, one per budget row.
/// @param actual_var - Actual component VaR per position, in portfolio currency (loss-signed as the engine reports it).
/// @param target_var_pct - Target share of portfolio VaR per position; non-empty targets must sum to one.
/// @param portfolio_var - Total portfolio VaR used to convert risk-budget shares into absolute amounts.
/// @param utilization_threshold - Optional actual-to-target risk ratio that
///   flags a budget breach; omit for the Rust default of 1.2.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if actual or target arrays do not match the identifier
/// count, a position id is duplicated, non-empty target shares do not sum to
/// one within tolerance, or nonzero component risk is paired with zero
/// `portfolioVar`.
#[wasm_bindgen(js_name = evaluateRiskBudget)]
pub fn evaluate_risk_budget(
    position_ids: JsValue,
    actual_var: JsValue,
    target_var_pct: JsValue,
    portfolio_var: JsValue,
    utilization_threshold: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    let portfolio_var = js_f64(&portfolio_var, "portfolioVar")?;
    let utilization_threshold = js_opt_f64(utilization_threshold.as_ref(), "utilizationThreshold")?;
    use finstack_quant_models::factor::risk::{
        evaluate_risk_budget_arrays, DEFAULT_UTILIZATION_THRESHOLD,
    };
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let actual_var = js_f64_seq(&actual_var, "actualVar")?;
    let target_var_pct = js_f64_seq(&target_var_pct, "targetVarPct")?;
    let threshold = utilization_threshold.unwrap_or(DEFAULT_UTILIZATION_THRESHOLD);

    let result =
        evaluate_risk_budget_arrays(ids, &actual_var, &target_var_pct, portfolio_var, threshold)
            .map_err(to_js_err)?;
    let js = crate::utils::to_js_value(&result)?;
    let positions = js_sys::Reflect::get(&js, &JsValue::from("positions"))?;
    crate::utils::restore_non_finite_rows::<
        finstack_quant_models::factor::risk::PositionBudgetEntry,
    >(&positions)?;
    Ok(js)
}

// Accessors of the calibrated model (twins of the Python getters)

#[wasm_bindgen(js_class = CreditFactorModel)]
impl JsCreditFactorModel {
    /// Calibration date of the model, in ISO-8601 form.
    #[wasm_bindgen(getter, js_name = asOf)]
    pub fn as_of(&self) -> String {
        self.inner.as_of.to_string()
    }

    /// First and last date of the calibration window, as `[start, end]` ISO-8601 strings.
    #[wasm_bindgen(getter, js_name = calibrationWindow)]
    pub fn calibration_window(&self) -> Vec<String> {
        vec![
            self.inner.calibration_window.start.to_string(),
            self.inner.calibration_window.end.to_string(),
        ]
    }

    /// Issuer-beta policy the model was calibrated with: `"dynamic"` or `"globally_off"`.
    #[wasm_bindgen(getter)]
    pub fn policy(&self) -> String {
        self.inner.policy.kind().to_owned()
    }

    /// Sampling frequency of the calibration panel, as its canonical label.
    #[wasm_bindgen(getter, js_name = panelFrequency)]
    pub fn panel_frequency(&self) -> Result<String, JsValue> {
        finstack_quant_core::wire::serde_label(&self.inner.panel_frequency).map_err(to_js_err)
    }

    /// Bucket weighting used for the level returns, as its canonical label.
    #[wasm_bindgen(getter, js_name = bucketWeighting)]
    pub fn bucket_weighting(&self) -> Result<String, JsValue> {
        finstack_quant_core::wire::serde_label(&self.inner.bucket_weighting).map_err(to_js_err)
    }

    /// Factor-model configuration (factors, covariance, matching, risk measure) as a `FactorModelConfig` object.
    #[wasm_bindgen(getter)]
    pub fn config(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.config)
    }

    /// Calibrated factor covariance as a `FactorCovarianceMatrix` object (`factor_ids`, row-major `data`).
    #[wasm_bindgen(getter)]
    pub fn covariance(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.config.covariance)
    }

    /// Calibration diagnostics (fit quality, fold-ups, dropped factors) as a plain object.
    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.diagnostics)
    }

    /// Static factor correlation matrix the covariance forecasts are built on, as a plain object.
    #[wasm_bindgen(getter, js_name = staticCorrelation)]
    pub fn static_correlation(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.static_correlation)
    }

    /// Number of hierarchy levels.
    #[wasm_bindgen(getter, js_name = nLevels)]
    pub fn n_levels(&self) -> usize {
        self.inner.hierarchy.levels.len()
    }

    /// Number of calibrated issuers.
    #[wasm_bindgen(getter, js_name = nIssuers)]
    pub fn n_issuers(&self) -> usize {
        self.inner.issuer_betas.len()
    }

    /// Number of factors in the model configuration.
    #[wasm_bindgen(getter, js_name = nFactors)]
    pub fn n_factors(&self) -> usize {
        self.inner.config.factors.len()
    }

    /// Display labels of the hierarchy levels, broadest first.
    /// @returns Labels such as `"Rating"`, `"Region"`, `"Sector"` or a custom dimension key.
    #[wasm_bindgen(js_name = levelNames)]
    pub fn level_names(&self) -> Vec<String> {
        self.inner.level_names()
    }

    /// Identifiers of the calibrated issuers, in issuer-beta row order.
    /// @returns The issuer identifiers.
    #[wasm_bindgen(js_name = issuerIds)]
    pub fn issuer_ids(&self) -> Vec<String> {
        self.inner.issuer_ids()
    }

    /// Identifiers of the factors, in covariance order.
    /// @returns The factor identifiers.
    #[wasm_bindgen(js_name = factorIds)]
    pub fn factor_ids(&self) -> Vec<String> {
        self.inner.factor_ids()
    }
}

#[wasm_bindgen(js_class = CreditCalibrator)]
impl JsCreditCalibrator {
    /// Calibration configuration in canonical JSON form, as a `CreditCalibrationConfig` object.
    #[wasm_bindgen(getter)]
    pub fn config(&self) -> Result<JsValue, JsValue> {
        to_js_value(self.inner.config())
    }
}

/// Forecast horizon for the covariance and idiosyncratic-vol forecasts.
///
/// The forecast methods take the horizon descriptor; pass `horizon.toString()`.
#[wasm_bindgen(js_name = VolHorizon)]
pub struct JsVolHorizon {
    pub(crate) inner: VolHorizon,
}

#[wasm_bindgen(js_class = VolHorizon)]
impl JsVolHorizon {
    /// One-period horizon: the calibrated annualized variance unchanged.
    /// @returns The one-step horizon.
    #[wasm_bindgen(js_name = oneStep)]
    pub fn one_step() -> JsVolHorizon {
        Self {
            inner: VolHorizon::OneStep,
        }
    }

    /// Long-run horizon: the unconditional variance of the vol model.
    /// @returns The unconditional horizon.
    pub fn unconditional() -> JsVolHorizon {
        Self {
            inner: VolHorizon::Unconditional,
        }
    }

    /// Horizon of `n` annualized model periods; variance scales linearly with `n`.
    /// @param n - Number of model periods; a safe non-negative integer (`0` gives zero variance).
    /// @returns The n-step horizon.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `n` is not a safe non-negative integer.
    #[wasm_bindgen(js_name = nSteps)]
    pub fn n_steps(n: JsValue) -> Result<JsVolHorizon, JsValue> {
        Ok(Self {
            inner: VolHorizon::NSteps(js_uint(&n, "n")?),
        })
    }

    /// Fractional-year horizon, for example `10 / 252` for ten trading days.
    /// @param years - Horizon length in years; finite and non-negative.
    /// @returns The fractional-year horizon.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `years` is non-finite or negative.
    pub fn years(years: JsValue) -> Result<JsVolHorizon, JsValue> {
        VolHorizon::years(js_f64(&years, "years")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Parse a horizon descriptor.
    /// @param s - `"one_step"`, `"unconditional"`, or a JSON object string such as `'{"n_steps": 5}'` or `'{"years": 0.25}'`.
    /// @returns The parsed horizon.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the descriptor is not recognized.
    pub fn parse(s: JsValue) -> Result<JsVolHorizon, JsValue> {
        VolHorizon::parse(&js_string(&s, "s")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Variant label: `"one_step"`, `"unconditional"`, `"n_steps"` or `"years"`.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.inner.kind().to_owned()
    }

    /// Step count when `kind` is `"n_steps"`, otherwise `undefined`.
    #[wasm_bindgen(getter)]
    pub fn n(&self) -> Option<usize> {
        match self.inner {
            VolHorizon::NSteps(n) => Some(n),
            _ => None,
        }
    }

    /// Horizon in years when `kind` is `"years"`, otherwise `undefined`.
    #[wasm_bindgen(getter, js_name = yearsValue)]
    pub fn years_value(&self) -> Option<f64> {
        match self.inner {
            VolHorizon::Years(years) => Some(years),
            _ => None,
        }
    }

    /// Canonical descriptor accepted by `parse` and by the forecast methods.
    /// @returns `"one_step"`, `"unconditional"`, `{"n_steps": N}` or `{"years": Y}`.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.descriptor()
    }
}

// FactorCovarianceMatrix / FactorModelConfig computations. Both types cross
// the boundary as plain objects, so their Rust methods are free functions here.

/// Variance of one factor in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param factor_id - Factor whose diagonal variance is requested; an unknown identifier returns 0.
/// @returns The factor's variance.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorVariance)]
pub fn factor_variance(matrix: JsValue, factor_id: JsValue) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.variance(&FactorId::new(js_string(&factor_id, "factorId")?)))
}

/// Covariance between two factors in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param lhs - First factor identifier; an unknown identifier returns 0.
/// @param rhs - Second factor identifier; an unknown identifier returns 0.
/// @returns The covariance of the two factors.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCovariance)]
pub fn factor_covariance(matrix: JsValue, lhs: JsValue, rhs: JsValue) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.covariance(
        &FactorId::new(js_string(&lhs, "lhs")?),
        &FactorId::new(js_string(&rhs, "rhs")?),
    ))
}

/// Correlation between two factors in a factor covariance matrix.
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @param lhs - First factor identifier; an unknown identifier or a non-positive variance returns 0.
/// @param rhs - Second factor identifier; an unknown identifier or a non-positive variance returns 0.
/// @returns The correlation of the two factors, from -1 through 1.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCorrelation)]
pub fn factor_correlation(matrix: JsValue, lhs: JsValue, rhs: JsValue) -> Result<f64, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    Ok(matrix.correlation(
        &FactorId::new(js_string(&lhs, "lhs")?),
        &FactorId::new(js_string(&rhs, "rhs")?),
    ))
}

/// Factor covariance matrix as one row per factor (the twin of Python `to_numpy`).
/// @param matrix - `FactorCovarianceMatrix` object or JSON (`factor_ids`, row-major `data`).
/// @returns An array of `number[]` rows, `nFactors` by `nFactors`, in `factor_ids` order.
///
/// # Errors
///
/// Throws a `validation` error if `matrix` is malformed or not a valid
/// covariance matrix.
#[wasm_bindgen(js_name = factorCovarianceRows)]
pub fn factor_covariance_rows(matrix: JsValue) -> Result<JsValue, JsValue> {
    let matrix: FactorCovarianceMatrix = from_js_json(&matrix, "matrix")?;
    to_js_value(&matrix.to_rows())
}

/// Validate a factor-model configuration: factor ordering, matching rules and the risk measure.
/// @param config - `FactorModelConfig` object or JSON, as returned by `CreditFactorModel.config` or `factorModelAt`.
///
/// # Errors
///
/// Throws a `validation` error naming the first inconsistency.
#[wasm_bindgen(js_name = validateFactorModelConfig)]
pub fn validate_factor_model_config(config: JsValue) -> Result<(), JsValue> {
    let config: FactorModelConfig = from_js_json(&config, "config")?;
    config.validate().map_err(to_js_err)
}

// Position-risk configuration, stress attribution and lookups

/// Configuration of position-level VaR / ES decomposition.
#[wasm_bindgen(js_name = DecompositionConfig)]
pub struct JsDecompositionConfig {
    pub(crate) inner: DecompositionConfig,
}

json_round_trip!(JsDecompositionConfig, DecompositionConfig);

#[wasm_bindgen(js_class = DecompositionConfig)]
impl JsDecompositionConfig {
    /// Parametric (delta-normal) decomposition at a confidence level.
    /// @param confidence - Tail confidence as a decimal probability in `(0.5, 1)`, such as 0.99.
    /// @returns The parametric configuration.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `confidence` is not a number; the range is
    /// checked when the configuration is used.
    pub fn parametric(confidence: JsValue) -> Result<JsDecompositionConfig, JsValue> {
        Ok(Self {
            inner: DecompositionConfig::parametric(js_f64(&confidence, "confidence")?),
        })
    }

    /// Parametric decomposition at 95% confidence.
    /// @returns The parametric 95% preset.
    #[wasm_bindgen(js_name = parametric95)]
    pub fn parametric_95() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::parametric_95(),
        }
    }

    /// Parametric decomposition at 99% confidence.
    /// @returns The parametric 99% preset.
    #[wasm_bindgen(js_name = parametric99)]
    pub fn parametric_99() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::parametric_99(),
        }
    }

    /// Historical-simulation decomposition at a confidence level.
    /// @param confidence - Tail confidence as a decimal probability in `(0.5, 1)`, such as 0.99.
    /// @returns The historical configuration.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `confidence` is not a number; the range is
    /// checked when the configuration is used.
    pub fn historical(confidence: JsValue) -> Result<JsDecompositionConfig, JsValue> {
        Ok(Self {
            inner: DecompositionConfig::historical(js_f64(&confidence, "confidence")?),
        })
    }

    /// Historical-simulation decomposition at 95% confidence.
    /// @returns The historical 95% preset.
    #[wasm_bindgen(js_name = historical95)]
    pub fn historical_95() -> JsDecompositionConfig {
        Self {
            inner: DecompositionConfig::historical_95(),
        }
    }

    /// Copy of this configuration that also computes incremental VaR (one full repricing per position).
    /// @returns The configuration with `computeIncremental` set.
    #[wasm_bindgen(js_name = withIncremental)]
    pub fn with_incremental(&self) -> JsDecompositionConfig {
        Self {
            inner: self.inner.clone().with_incremental(),
        }
    }

    /// Tail confidence as a decimal probability.
    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Decomposition method: `"parametric"` or `"historical"`.
    #[wasm_bindgen(getter)]
    pub fn method(&self) -> Result<String, JsValue> {
        finstack_quant_core::wire::serde_label(&self.inner.method).map_err(to_js_err)
    }

    /// Whether leave-one-out incremental VaR is computed.
    #[wasm_bindgen(getter, js_name = computeIncremental)]
    pub fn compute_incremental(&self) -> bool {
        self.inner.compute_incremental
    }
}

/// Default utilization threshold of `evaluateRiskBudget`. Twin of the Rust and
/// Python constant `DEFAULT_UTILIZATION_THRESHOLD`.
/// @returns The threshold as a fraction of the risk budget.
#[wasm_bindgen(js_name = defaultUtilizationThreshold)]
pub fn default_utilization_threshold() -> f64 {
    finstack_quant_models::factor::risk::DEFAULT_UTILIZATION_THRESHOLD
}

/// Attribute the portfolio loss in tail scenarios to positions.
///
/// Returns the canonical `StressAttribution` (the object Python's
/// `build_stress_attribution` returns): the VaR threshold, the tail scenarios
/// and each position's average tail P&L and share of the tail loss.
/// @param position_ids - Position identifiers, one per row of `positionPnls`.
/// @param position_pnls - Position-major P&L matrix as nested rows: one row per position, one column per scenario, in reporting-currency amounts.
/// @param confidence - Optional tail confidence in `(0.5, 1)`; omitted or `null` uses the Rust `DecompositionConfig::historical_95()` preset (0.95).
/// @returns The `StressAttribution` object.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type, and a
/// `validation` error if the dimensions disagree, a P&L is non-finite,
/// `confidence` is outside `(0.5, 1)`, or the tail holds no scenario.
#[wasm_bindgen(js_name = buildStressAttribution)]
pub fn build_stress_attribution(
    position_ids: JsValue,
    position_pnls: JsValue,
    confidence: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    use finstack_quant_models::factor::risk::{build_stress_attribution, flatten_position_pnls};
    let ids = js_string_seq(&position_ids, "positionIds")?;
    let position_pnls = js_f64_matrix(&position_pnls, "positionPnls")?;
    let confidence = js_opt_f64(confidence.as_ref(), "confidence")?
        .unwrap_or_else(|| DecompositionConfig::historical_95().confidence);
    let (flat, n_scenarios) = flatten_position_pnls(position_pnls, ids.len()).map_err(to_js_err)?;
    let result =
        build_stress_attribution(&ids, &flat, n_scenarios, confidence).map_err(to_js_err)?;
    to_js_value(&result)
}

/// One position's component VaR from a position risk decomposition.
/// @param decomp - `PositionRiskDecomposition` object or JSON, as returned by `parametricVarDecomposition` or `historicalVarDecomposition`.
/// @param position_id - Position identifier exactly as it appears in the decomposition.
/// @returns The position's Euler-allocated component VaR (losses negative).
///
/// # Errors
///
/// Throws a `validation` error if `decomp` is malformed, and a `not_found`
/// error if the position is not in the decomposition.
#[wasm_bindgen(js_name = positionComponentVar)]
pub fn position_component_var(decomp: JsValue, position_id: JsValue) -> Result<f64, JsValue> {
    let decomp: PositionRiskDecomposition = from_js_json(&decomp, "decomp")?;
    let position_id = js_string(&position_id, "positionId")?;
    decomp.try_component_var(&position_id).map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::types::IssuerId;
    use finstack_quant_models::factor::credit::calibration::{
        BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig, CreditCalibrationInputs,
        CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel, PanelSpace,
        VolModelChoice,
    };
    use finstack_quant_models::factor::credit::hierarchy::{
        CreditFactorModel, CreditFactorModelSchema, CreditHierarchySpec, GenericFactorSpec,
        HierarchyDimension, IssuerTags,
    };
    use std::collections::BTreeMap;
    use time::Month;

    // Fixture helpers

    fn d(year: i32, month: Month, day: u8) -> finstack_quant_core::dates::Date {
        create_date(year, month, day).expect("valid date")
    }

    fn monthly_dates(
        n: usize,
        end: finstack_quant_core::dates::Date,
    ) -> Vec<finstack_quant_core::dates::Date> {
        use finstack_quant_core::dates::DateExt;
        let mut out = Vec::with_capacity(n);
        let mut current = end;
        for _ in 0..n {
            out.push(current);
            current = if current == current.end_of_month() {
                current.add_months(-1).end_of_month()
            } else {
                current.add_months(-1)
            };
        }
        out.reverse();
        out
    }

    fn fixture_config() -> CreditCalibrationConfig {
        CreditCalibrationConfig {
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
            },
            min_bucket_size_per_level: BucketSizeThresholds {
                per_level: vec![1, 1],
            },
            vol_model: VolModelChoice::Sample,
            covariance_strategy: CovarianceStrategy::Diagonal,
            use_returns_or_levels: PanelSpace::Returns,
            panel_frequency:
                finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
            bucket_weighting:
                finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
            ..Default::default()
        }
    }

    fn fixture_inputs() -> CreditCalibrationInputs {
        let n = 24usize;
        let as_of = d(2024, Month::March, 31);
        let dates = monthly_dates(n, as_of);

        let generic_values: Vec<f64> = (0..n)
            .map(|i| 0.0100 + 0.00005 * (i as f64).sin())
            .collect();

        let issuer_specs = [
            ("ISSUER-A", "IG", "EU"),
            ("ISSUER-B", "IG", "NA"),
            ("ISSUER-C", "HY", "EU"),
        ];

        let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
        let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
        let mut as_of_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

        for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
            let issuer_id = IssuerId::new(*id);
            let base = 0.0100 + (idx as f64) * 0.0025;
            let series: Vec<Option<f64>> = (0..n)
                .map(|i| {
                    Some(
                        base + 0.0050 * generic_values[i] / 0.0100
                            + 0.0005 * (i as f64 + idx as f64).sin(),
                    )
                })
                .collect();
            as_of_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
            spreads.insert(issuer_id.clone(), series);
            let mut t = BTreeMap::new();
            t.insert("rating".to_owned(), rating.to_string());
            t.insert("region".to_owned(), region.to_string());
            tags.insert(issuer_id, IssuerTags(t));
        }

        CreditCalibrationInputs {
            history_panel: HistoryPanel { dates, spreads },
            issuer_tags: IssuerTagPanel { tags },
            generic_factor: GenericFactorSeries {
                spec: GenericFactorSpec {
                    name: "CDX IG 5Y".to_owned(),
                    series_id: "cdx.ig.5y".to_owned(),
                },
                values: generic_values,
            },
            as_of,
            as_of_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
            spread_durations: BTreeMap::new(),
        }
    }

    // Smoke test: calibrate → serialize → deserialize round-trip

    #[test]
    fn calibrate_serialize_deserialize_roundtrip() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        // Serialize to JSON.
        let json = serde_json::to_string_pretty(&model).expect("serialize");
        assert!(!json.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(
            parsed["schema"].as_str().unwrap(),
            CreditFactorModelSchema::CreditFactorModel.as_str()
        );

        // Deserialize and validate — structural round-trip.
        let model2: CreditFactorModel = serde_json::from_str(&json).expect("deserialize");
        model2.validate().expect("validate round-tripped model");

        // Key structural properties are preserved.
        assert_eq!(model.as_of, model2.as_of);
        assert_eq!(model.hierarchy.levels.len(), model2.hierarchy.levels.len());
        assert_eq!(model.issuer_betas.len(), model2.issuer_betas.len());

        // Re-serialize the deserialized model — must also be valid JSON.
        let json2 = serde_json::to_string_pretty(&model2).expect("re-serialize");
        let parsed2: serde_json::Value = serde_json::from_str(&json2).expect("valid JSON 2");
        assert_eq!(
            parsed2["schema"].as_str().unwrap(),
            CreditFactorModelSchema::CreditFactorModel.as_str()
        );
    }

    #[test]
    fn decompose_levels_and_period_smoke() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        let spreads_t0: BTreeMap<IssuerId, f64> = [
            (IssuerId::new("ISSUER-A"), 0.0150_f64),
            (IssuerId::new("ISSUER-B"), 0.0175_f64),
        ]
        .into_iter()
        .collect();
        let spreads_t1: BTreeMap<IssuerId, f64> = [
            (IssuerId::new("ISSUER-A"), 0.0155_f64),
            (IssuerId::new("ISSUER-B"), 0.0170_f64),
        ]
        .into_iter()
        .collect();

        let levels_t0 = finstack_quant_models::factor::credit::decomposition::decompose_levels(
            &model,
            &spreads_t0,
            0.0100,
            d(2024, Month::March, 28),
            None,
        )
        .expect("decompose_levels t0");

        let levels_t1 = finstack_quant_models::factor::credit::decomposition::decompose_levels(
            &model,
            &spreads_t1,
            0.01005,
            d(2024, Month::March, 29),
            None,
        )
        .expect("decompose_levels t1");

        // Serde serialization must produce valid JSON.
        let l0_val = serde_json::to_value(&levels_t0).expect("LevelsAtDate serializes");
        assert!(l0_val.is_object());
        assert_eq!(l0_val["date"].as_str().unwrap(), "2024-03-28");

        // decompose_period.
        let period = finstack_quant_models::factor::credit::decomposition::decompose_period(
            &levels_t0, &levels_t1,
        )
        .expect("decompose_period");
        let p_val = serde_json::to_value(&period).expect("PeriodDecomposition serializes");
        assert!(p_val.is_object());
        assert!(p_val["d_generic"].as_f64().is_some());
    }

    #[test]
    fn factor_covariance_forecast_covariance_at_one_step() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        let forecast = finstack_quant_models::factor::credit::FactorCovarianceForecast::new(&model);
        let cov = forecast
            .covariance_at(finstack_quant_models::factor::credit::VolHorizon::OneStep)
            .expect("covariance_at");
        let cov_json = serde_json::to_string_pretty(&cov).expect("serialize");
        let cov_val: serde_json::Value = serde_json::from_str(&cov_json).expect("valid json");
        assert!(cov_val.is_object());
    }

    /// `HierarchyDimension` serde must emit the binding's public snake-case
    /// JSON convention.
    #[test]
    fn levels_at_date_dimension_matches_serde_convention() {
        use finstack_quant_models::factor::credit::hierarchy::HierarchyDimension;
        use serde_json::json;

        // Unit-level checks against serde round-trip.
        let cases: &[(HierarchyDimension, serde_json::Value)] = &[
            (HierarchyDimension::Rating, json!("rating")),
            (HierarchyDimension::Region, json!("region")),
            (HierarchyDimension::Sector, json!("sector")),
            (
                HierarchyDimension::Custom("Currency".to_owned()),
                json!({"custom": "Currency"}),
            ),
        ];

        for (dim, expected) in cases {
            let serde_got = serde_json::to_value(dim).expect("serde serializes HierarchyDimension");
            assert_eq!(
                serde_got, *expected,
                "serde({dim:?}) mismatch: got {serde_got}, want {expected}"
            );
        }
    }

    /// Full integration: `decompose_levels` serde emits
    /// dimension keys that match serde convention in a real calibrated model.
    #[test]
    fn decompose_levels_dimension_keys_match_serde() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        let spreads: std::collections::BTreeMap<IssuerId, f64> = [
            (IssuerId::new("ISSUER-A"), 0.0150_f64),
            (IssuerId::new("ISSUER-B"), 0.0175_f64),
        ]
        .into_iter()
        .collect();

        let levels = finstack_quant_models::factor::credit::decomposition::decompose_levels(
            &model,
            &spreads,
            0.0100,
            d(2024, Month::March, 28),
            None,
        )
        .expect("decompose_levels");

        let val = serde_json::to_value(&levels).expect("LevelsAtDate serializes");
        let by_level = val["by_level"].as_array().expect("by_level is array");
        for entry in by_level {
            let dim = &entry["dimension"];
            // Must be a lowercase string (Rating/Region/Sector) or an object
            // with a single "custom" key — never a PascalCase string.
            match dim {
                serde_json::Value::String(s) => {
                    assert_eq!(
                        *s,
                        s.to_lowercase(),
                        "dimension string must be lowercase, got {s:?}"
                    );
                }
                serde_json::Value::Object(obj) => {
                    assert!(
                        obj.contains_key("custom"),
                        "object dimension must have 'custom' key, got {obj:?}"
                    );
                }
                other => panic!("unexpected dimension JSON: {other:?}"),
            }
        }
    }

    /// Verify parse_vol_horizon recognizes valid forms without triggering
    /// `js_sys` (which only works on wasm32 targets).
    #[test]
    fn parse_vol_horizon_valid_forms() {
        use finstack_quant_models::factor::credit::VolHorizon;
        // OneStep and Unconditional match early without calling to_js_err.
        assert!(matches!(
            super::parse_vol_horizon("one_step").unwrap(),
            VolHorizon::OneStep
        ));
        assert!(matches!(
            super::parse_vol_horizon("unconditional").unwrap(),
            VolHorizon::Unconditional
        ));
        // NSteps parses a valid JSON object — also no to_js_err call on this path.
        let h = super::parse_vol_horizon(r#"{"n_steps": 5}"#).unwrap();
        assert!(matches!(h, VolHorizon::NSteps(5)));
    }
}
