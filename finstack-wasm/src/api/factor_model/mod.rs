//! WASM bindings for the credit factor hierarchy.
//!
//! Exposes [`WasmCreditFactorModel`], [`WasmCreditCalibrator`], the free
//! functions [`decompose_levels`] and [`decompose_period`], and
//! [`WasmFactorCovarianceForecast`].
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

use crate::utils::to_js_err;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Horizon helper (shared by CreditCalibrator and FactorCovarianceForecast)
// ---------------------------------------------------------------------------

/// Parse a horizon descriptor string into a [`VolHorizon`].
///
/// Delegates to the canonical [`VolHorizon::parse`] implementation in
/// `finstack-portfolio`; this wrapper only maps the error to a `JsValue`.
fn parse_vol_horizon(s: &str) -> Result<finstack_portfolio::factor_model::VolHorizon, JsValue> {
    finstack_portfolio::factor_model::VolHorizon::parse(s).map_err(to_js_err)
}

// ---------------------------------------------------------------------------
// Output finiteness validation
//
// `serde_json` silently serializes NaN/Inf as `null`, so risk outputs are
// checked for finiteness at the boundary and rejected with an error naming
// the offending field instead of emitting `null`.
// ---------------------------------------------------------------------------

/// Reject a non-finite numeric output, naming the field in the error.
fn ensure_finite(field: &str, v: f64) -> Result<(), JsValue> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(JsValue::from_str(&format!(
            "non-finite value ({v}) in output field '{field}'"
        )))
    }
}

/// Validate every entry of a factor covariance matrix is finite.
fn ensure_covariance_finite(
    cov: &finstack_factor_model::covariance::FactorCovarianceMatrix,
) -> Result<(), JsValue> {
    for (i, v) in cov.as_slice().iter().enumerate() {
        ensure_finite(&format!("covariance.data[{i}]"), *v)?;
    }
    Ok(())
}

/// Validate every numeric field of a `LevelsAtDate` snapshot is finite.
fn ensure_levels_finite(levels: &finstack_factor_model::LevelsAtDate) -> Result<(), JsValue> {
    ensure_finite("generic", levels.generic)?;
    for lev in &levels.by_level {
        for (bucket, v) in &lev.values {
            ensure_finite(
                &format!("by_level[{}].values[{bucket}]", lev.level_index),
                *v,
            )?;
        }
    }
    for (issuer, v) in &levels.adder {
        ensure_finite(&format!("adder[{}]", issuer.as_str()), *v)?;
    }
    Ok(())
}

/// Validate every numeric field of a `PeriodDecomposition` is finite.
fn ensure_period_finite(
    period: &finstack_factor_model::PeriodDecomposition,
) -> Result<(), JsValue> {
    ensure_finite("d_generic", period.d_generic)?;
    for lev in &period.by_level {
        for (bucket, v) in &lev.deltas {
            ensure_finite(
                &format!("by_level[{}].deltas[{bucket}]", lev.level_index),
                *v,
            )?;
        }
    }
    for (issuer, v) in &period.d_adder {
        ensure_finite(&format!("d_adder[{}]", issuer.as_str()), *v)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CreditFactorModel
// ---------------------------------------------------------------------------

/// Calibrated credit factor hierarchy artifact.
///
/// Produced by [`WasmCreditCalibrator`] or loaded from JSON via
/// [`WasmCreditFactorModel::from_json`]. Immutable once constructed.
#[wasm_bindgen(js_name = CreditFactorModel)]
pub struct WasmCreditFactorModel {
    #[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_factor_model::credit::hierarchy::CreditFactorModel,
}

#[wasm_bindgen(js_class = CreditFactorModel)]
impl WasmCreditFactorModel {
    /// Deserialize a `CreditFactorModel` from JSON.
    ///
    /// Validates the `schema_version` field and all structural constraints.
    ///
    /// # Errors
    /// Throws if the JSON is malformed or fails validation.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(s: &str) -> Result<WasmCreditFactorModel, JsValue> {
        let inner: finstack_factor_model::credit::hierarchy::CreditFactorModel =
            serde_json::from_str(s).map_err(to_js_err)?;
        inner.validate().map_err(to_js_err)?;
        Ok(Self { inner })
    }

    /// Serialize this model to pretty-printed JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string_pretty(&self.inner).map_err(to_js_err)
    }
}

// ---------------------------------------------------------------------------
// CreditCalibrator
// ---------------------------------------------------------------------------

/// Deterministic calibrator that produces a [`WasmCreditFactorModel`].
///
/// Configuration and inputs are passed as JSON strings.
#[wasm_bindgen(js_name = CreditCalibrator)]
pub struct WasmCreditCalibrator {
    inner: finstack_factor_model::CreditCalibrator,
}

#[wasm_bindgen(js_class = CreditCalibrator)]
impl WasmCreditCalibrator {
    /// Construct a calibrator from a JSON-serialized `CreditCalibrationConfig`.
    ///
    /// # Errors
    /// Throws if `config_json` is not a valid `CreditCalibrationConfig`.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmCreditCalibrator, JsValue> {
        let config: finstack_factor_model::CreditCalibrationConfig =
            serde_json::from_str(config_json).map_err(to_js_err)?;
        Ok(Self {
            inner: finstack_factor_model::CreditCalibrator::new(config),
        })
    }

    /// Run the full calibration pipeline and return a `CreditFactorModel`.
    ///
    /// `inputs_json` must be a JSON-serialized `CreditCalibrationInputs`.
    ///
    /// # Errors
    /// Throws if inputs are structurally invalid or calibration fails.
    pub fn calibrate(&self, inputs_json: &str) -> Result<WasmCreditFactorModel, JsValue> {
        let inputs: finstack_factor_model::CreditCalibrationInputs =
            serde_json::from_str(inputs_json).map_err(to_js_err)?;
        let model = self.inner.calibrate(inputs).map_err(to_js_err)?;
        Ok(WasmCreditFactorModel { inner: model })
    }
}

// ---------------------------------------------------------------------------
// LevelsAtDate  (opaque handle — not exposed as a JS class, just passed through)
// ---------------------------------------------------------------------------

/// Snapshot of all hierarchy-level factor values at a single date.
///
/// Produced by [`decompose_levels`]. Pass to [`decompose_period`] to compute
/// period-over-period changes.  The full data is available via `toJson`.
#[wasm_bindgen(js_name = LevelsAtDate)]
pub struct WasmLevelsAtDate {
    #[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_factor_model::LevelsAtDate,
}

#[wasm_bindgen(js_class = LevelsAtDate)]
impl WasmLevelsAtDate {
    /// Serialize the snapshot to JSON.
    ///
    /// # Errors
    /// Throws if any numeric output field is non-finite (NaN/Inf), naming
    /// the offending field instead of silently serializing `null`.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        ensure_levels_finite(&self.inner)?;
        serde_json::to_string_pretty(&self.inner).map_err(to_js_err)
    }
}

// ---------------------------------------------------------------------------
// PeriodDecomposition  (opaque handle)
// ---------------------------------------------------------------------------

/// Component-wise difference between two [`WasmLevelsAtDate`] snapshots.
///
/// Produced by [`decompose_period`].
#[wasm_bindgen(js_name = PeriodDecomposition)]
pub struct WasmPeriodDecomposition {
    #[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub(crate) inner: finstack_factor_model::PeriodDecomposition,
}

#[wasm_bindgen(js_class = PeriodDecomposition)]
impl WasmPeriodDecomposition {
    /// Serialize the decomposition to JSON.
    ///
    /// # Errors
    /// Throws if any numeric output field is non-finite (NaN/Inf), naming
    /// the offending field instead of silently serializing `null`.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        ensure_period_finite(&self.inner)?;
        serde_json::to_string_pretty(&self.inner).map_err(to_js_err)
    }
}

// ---------------------------------------------------------------------------
// decompose_levels  (free function)
// ---------------------------------------------------------------------------

/// Decompose observed issuer spreads at a point in time into per-level factor
/// values and per-issuer residual adders.
///
/// - `model` — calibrated `CreditFactorModel`.
/// - `observed_spreads_json` — JSON `{issuer_id: spread}` map.
/// - `observed_generic` — generic (PC) factor value at `as_of`.
/// - `as_of` — ISO 8601 date string.
/// - `runtime_tags_json` — optional JSON `{issuer_id: {dim_key: tag}}` for
///   issuers not present in the model artifact.
///
/// Returns a `LevelsAtDate` handle.
///
/// # Errors
/// Throws if an issuer has no model row and no `runtime_tags` entry, or if
/// `as_of` cannot be parsed.
#[wasm_bindgen(js_name = decomposeLevels)]
pub fn decompose_levels(
    model: &WasmCreditFactorModel,
    observed_spreads_json: &str,
    observed_generic: f64,
    as_of: &str,
    runtime_tags_json: Option<String>,
) -> Result<WasmLevelsAtDate, JsValue> {
    let observed_spreads: std::collections::BTreeMap<finstack_core::types::IssuerId, f64> =
        serde_json::from_str(observed_spreads_json).map_err(to_js_err)?;

    let date = finstack_core::dates::parse_iso_date(as_of).map_err(to_js_err)?;

    let runtime_tags: Option<
        std::collections::BTreeMap<
            finstack_core::types::IssuerId,
            finstack_factor_model::credit::hierarchy::IssuerTags,
        >,
    > = match runtime_tags_json.as_deref() {
        Some(json) => Some(serde_json::from_str(json).map_err(to_js_err)?),
        None => None,
    };

    let inner = finstack_factor_model::decompose_levels(
        &model.inner,
        &observed_spreads,
        observed_generic,
        date,
        runtime_tags.as_ref(),
    )
    .map_err(to_js_err)?;

    Ok(WasmLevelsAtDate { inner })
}

// ---------------------------------------------------------------------------
// decompose_period  (free function)
// ---------------------------------------------------------------------------

/// Difference two `LevelsAtDate` snapshots component-wise.
///
/// Output buckets and issuers are restricted to those present in **both**
/// snapshots so the linear reconciliation invariant on `ΔS_i` holds.
///
/// # Errors
/// Throws if `from_levels.date > to_levels.date` or the snapshots disagree
/// on hierarchy depth.
#[wasm_bindgen(js_name = decomposePeriod)]
pub fn decompose_period(
    from_levels: &WasmLevelsAtDate,
    to_levels: &WasmLevelsAtDate,
) -> Result<WasmPeriodDecomposition, JsValue> {
    let inner = finstack_factor_model::decompose_period(&from_levels.inner, &to_levels.inner)
        .map_err(to_js_err)?;
    Ok(WasmPeriodDecomposition { inner })
}

// ---------------------------------------------------------------------------
// FactorCovarianceForecast
// ---------------------------------------------------------------------------

/// Vol-forecast view over a calibrated `CreditFactorModel`.
///
/// `VolHorizon::Custom` is intentionally **not** exposed.
#[wasm_bindgen(js_name = FactorCovarianceForecast)]
pub struct WasmFactorCovarianceForecast {
    /// Store the model by value so `FactorCovarianceForecast<'a>` lifetime
    /// requirements don't escape the WASM boundary.
    model: finstack_factor_model::credit::hierarchy::CreditFactorModel,
}

#[wasm_bindgen(js_class = FactorCovarianceForecast)]
impl WasmFactorCovarianceForecast {
    /// Wrap a `CreditFactorModel` for vol forecasting.
    #[wasm_bindgen(constructor)]
    pub fn new(model: &WasmCreditFactorModel) -> WasmFactorCovarianceForecast {
        Self {
            model: model.inner.clone(),
        }
    }

    /// Build the factor covariance matrix `Σ(t, h) = D · ρ_static · D`.
    ///
    /// Returns pretty-printed JSON of a `FactorCovarianceMatrix`.
    ///
    /// `horizon_json` accepts `"one_step"`, `"unconditional"`, or
    /// `'{"n_steps": N}'`.
    ///
    /// # Errors
    /// Throws if the horizon string is invalid or the model data is
    /// inconsistent.
    #[wasm_bindgen(js_name = covarianceAt)]
    pub fn covariance_at(&self, horizon_json: &str) -> Result<String, JsValue> {
        let h = parse_vol_horizon(horizon_json)?;
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        let cov = forecast.covariance_at(h).map_err(to_js_err)?;
        ensure_covariance_finite(&cov)?;
        serde_json::to_string_pretty(&cov).map_err(to_js_err)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested
    /// horizon.
    ///
    /// # Errors
    /// Throws if the issuer is not present in the model's vol state or the
    /// calibrated variance is negative.
    #[wasm_bindgen(js_name = idiosyncraticVol)]
    pub fn idiosyncratic_vol(&self, issuer_id: &str, horizon_json: &str) -> Result<f64, JsValue> {
        let h = parse_vol_horizon(horizon_json)?;
        let id = finstack_core::types::IssuerId::new(issuer_id);
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        let vol = forecast.idiosyncratic_vol(&id, h).map_err(to_js_err)?;
        ensure_finite("idiosyncratic_vol", vol)?;
        Ok(vol)
    }

    /// Build a portfolio-level `FactorModel` JSON using `Σ(t, h)` at the
    /// given horizon and risk measure.
    ///
    /// Returns pretty-printed JSON of the assembled `FactorModelConfig`.
    ///
    /// # Errors
    /// Throws if the horizon or risk measure is invalid, or the model builder
    /// rejects the assembled configuration.
    #[wasm_bindgen(js_name = factorModelAt)]
    pub fn factor_model_at(
        &self,
        horizon_json: &str,
        risk_measure_json: &str,
    ) -> Result<String, JsValue> {
        let h = parse_vol_horizon(horizon_json)?;
        let measure: finstack_factor_model::RiskMeasure =
            serde_json::from_str(risk_measure_json).map_err(to_js_err)?;
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        let config = forecast
            .factor_model_config_at(h, measure)
            .map_err(to_js_err)?;
        ensure_covariance_finite(&config.covariance)?;
        serde_json::to_string_pretty(&config).map_err(to_js_err)
    }
}

// ---------------------------------------------------------------------------
// Tests
//
// Native tests call underlying Rust APIs directly — WASM wrapper methods that
// invoke `js_sys::Error::new` cannot run on non-wasm32 targets.  The WASM
// surface is exercised end-to-end by `wasm-pack test`.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use finstack_core::dates::create_date;
    use finstack_core::types::IssuerId;
    use finstack_factor_model::credit::hierarchy::{
        CreditFactorModel, CreditHierarchySpec, GenericFactorSpec, HierarchyDimension, IssuerTags,
    };
    use finstack_factor_model::{
        BucketSizeThresholds, CovarianceStrategy, CreditCalibrationConfig, CreditCalibrationInputs,
        CreditCalibrator, GenericFactorSeries, HistoryPanel, IssuerTagPanel, PanelSpace,
        VolModelChoice,
    };
    use std::collections::BTreeMap;
    use time::Month;

    // -----------------------------------------------------------------------
    // Fixture helpers
    // -----------------------------------------------------------------------

    fn d(year: i32, month: Month, day: u8) -> finstack_core::dates::Date {
        create_date(year, month, day).expect("valid date")
    }

    fn monthly_dates(n: usize, end: finstack_core::dates::Date) -> Vec<finstack_core::dates::Date> {
        let mut out = Vec::with_capacity(n);
        let mut current = end;
        for _ in 0..n {
            out.push(current);
            for _ in 0..30 {
                current = current.previous_day().expect("in range");
            }
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
            annualization_factor: 12.0,
            ..Default::default()
        }
    }

    fn fixture_inputs() -> CreditCalibrationInputs {
        let n = 24usize;
        let as_of = d(2024, Month::March, 31);
        let dates = monthly_dates(n, as_of);

        let generic_values: Vec<f64> = (0..n).map(|i| 100.0 + 0.5 * (i as f64).sin()).collect();

        let issuer_specs = [
            ("ISSUER-A", "IG", "EU"),
            ("ISSUER-B", "IG", "NA"),
            ("ISSUER-C", "HY", "EU"),
        ];

        let mut spreads: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
        let mut tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
        let mut asof_spreads: BTreeMap<IssuerId, f64> = BTreeMap::new();

        for (idx, (id, rating, region)) in issuer_specs.iter().enumerate() {
            let issuer_id = IssuerId::new(*id);
            let base = 100.0 + (idx as f64) * 25.0;
            let series: Vec<Option<f64>> = (0..n)
                .map(|i| {
                    Some(
                        base + 50.0 * generic_values[i] / 100.0
                            + 5.0 * (i as f64 + idx as f64).sin(),
                    )
                })
                .collect();
            asof_spreads.insert(issuer_id.clone(), series[n - 1].unwrap());
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
            asof_spreads,
            idiosyncratic_overrides: BTreeMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Smoke test: calibrate → serialize → deserialize round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn calibrate_serialize_deserialize_roundtrip() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        // Serialize to JSON.
        let json = serde_json::to_string_pretty(&model).expect("serialize");
        assert!(!json.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(
            parsed["schema_version"].as_str().unwrap(),
            CreditFactorModel::SCHEMA_VERSION
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
            parsed2["schema_version"].as_str().unwrap(),
            CreditFactorModel::SCHEMA_VERSION
        );
    }

    #[test]
    fn decompose_levels_and_period_smoke() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        let spreads_t0: BTreeMap<IssuerId, f64> = [
            (IssuerId::new("ISSUER-A"), 150.0_f64),
            (IssuerId::new("ISSUER-B"), 175.0_f64),
        ]
        .into_iter()
        .collect();
        let spreads_t1: BTreeMap<IssuerId, f64> = [
            (IssuerId::new("ISSUER-A"), 155.0_f64),
            (IssuerId::new("ISSUER-B"), 170.0_f64),
        ]
        .into_iter()
        .collect();

        let levels_t0 = finstack_factor_model::decompose_levels(
            &model,
            &spreads_t0,
            100.0,
            d(2024, Month::March, 28),
            None,
        )
        .expect("decompose_levels t0");

        let levels_t1 = finstack_factor_model::decompose_levels(
            &model,
            &spreads_t1,
            100.5,
            d(2024, Month::March, 29),
            None,
        )
        .expect("decompose_levels t1");

        // Serde serialization must produce valid JSON.
        let l0_val = serde_json::to_value(&levels_t0).expect("LevelsAtDate serializes");
        assert!(l0_val.is_object());
        assert_eq!(l0_val["date"].as_str().unwrap(), "2024-03-28");

        // decompose_period.
        let period = finstack_factor_model::decompose_period(&levels_t0, &levels_t1)
            .expect("decompose_period");
        let p_val = serde_json::to_value(&period).expect("PeriodDecomposition serializes");
        assert!(p_val.is_object());
        assert!(p_val["d_generic"].as_f64().is_some());
    }

    #[test]
    fn factor_covariance_forecast_covariance_at_one_step() {
        let cal = CreditCalibrator::new(fixture_config());
        let model = cal.calibrate(fixture_inputs()).expect("calibrate");

        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&model);
        let cov = forecast
            .covariance_at(finstack_portfolio::factor_model::VolHorizon::OneStep)
            .expect("covariance_at");
        let cov_json = serde_json::to_string_pretty(&cov).expect("serialize");
        let cov_val: serde_json::Value = serde_json::from_str(&cov_json).expect("valid json");
        assert!(cov_val.is_object());
    }

    /// `HierarchyDimension` serde must emit the binding's public snake-case
    /// JSON convention.
    #[test]
    fn levels_at_date_dimension_matches_serde_convention() {
        use finstack_factor_model::credit::hierarchy::HierarchyDimension;
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
            (IssuerId::new("ISSUER-A"), 150.0_f64),
            (IssuerId::new("ISSUER-B"), 175.0_f64),
        ]
        .into_iter()
        .collect();

        let levels = finstack_factor_model::decompose_levels(
            &model,
            &spreads,
            100.0,
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
        use finstack_portfolio::factor_model::VolHorizon;
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
