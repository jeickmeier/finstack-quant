//! SABR (Stochastic Alpha Beta Rho) volatility bindings for WASM.
//!
//! Exposes `SabrParameters`, `SabrModel`, `SabrSmile`, and `SabrCalibrator` to
//! JS/TS. Naming follows the Python binding convention (PascalCase with
//! lower-cased acronym, e.g. `SabrParameters` rather than the Rust-native
//! `SABRParameters`).

use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::models::volatility::sabr::{
    SABRCalibrator, SABRModel, SABRParameters, SABRSmile,
};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// SabrParameters
// ---------------------------------------------------------------------------

/// SABR model parameters `(alpha, beta, nu, rho)` with optional `shift`.
#[wasm_bindgen(js_name = SabrParameters)]
pub struct WasmSabrParameters {
    #[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub inner: SABRParameters,
}

#[wasm_bindgen(js_class = SabrParameters)]
impl WasmSabrParameters {
    #[wasm_bindgen(constructor)]
    /// Create the object from its inputs.
    pub fn new(
        alpha: f64,
        beta: f64,
        nu: f64,
        rho: f64,
        shift: Option<f64>,
    ) -> Result<WasmSabrParameters, JsValue> {
        let inner = match shift {
            Some(s) => SABRParameters::new_with_shift(alpha, beta, nu, rho, s),
            None => SABRParameters::new(alpha, beta, nu, rho),
        }
        .map_err(to_js_err)?;
        Ok(Self { inner })
    }

    #[wasm_bindgen(js_name = equityDefault)]
    /// Default SABR parameters for equity underlyings.
    pub fn equity_default() -> WasmSabrParameters {
        Self {
            inner: SABRParameters::equity_default(),
        }
    }

    #[wasm_bindgen(js_name = ratesDefault)]
    /// Default SABR parameters for rates underlyings.
    pub fn rates_default() -> WasmSabrParameters {
        Self {
            inner: SABRParameters::rates_default(),
        }
    }

    #[wasm_bindgen(getter)]
    /// SABR `alpha` (ATM volatility level).
    pub fn alpha(&self) -> f64 {
        self.inner.alpha
    }

    #[wasm_bindgen(getter)]
    /// SABR `beta` (backbone exponent).
    pub fn beta(&self) -> f64 {
        self.inner.beta
    }

    #[wasm_bindgen(getter)]
    /// SABR `nu` (vol-of-vol).
    pub fn nu(&self) -> f64 {
        self.inner.nu
    }

    #[wasm_bindgen(getter)]
    /// SABR `rho` (spot/vol correlation).
    pub fn rho(&self) -> f64 {
        self.inner.rho
    }

    #[wasm_bindgen(getter)]
    /// Displacement applied for shifted SABR, if any.
    pub fn shift(&self) -> Option<f64> {
        self.inner.shift
    }

    #[wasm_bindgen(js_name = isShifted)]
    /// Whether a displacement (shift) is configured.
    pub fn is_shifted(&self) -> bool {
        self.inner.is_shifted()
    }
}

impl WasmSabrParameters {
    fn clone_inner(&self) -> SABRParameters {
        self.inner.clone()
    }
}

// ---------------------------------------------------------------------------
// SabrModel
// ---------------------------------------------------------------------------

/// Hagan-2002 SABR volatility model.
#[wasm_bindgen(js_name = SabrModel)]
pub struct WasmSabrModel {
    inner: SABRModel,
}

#[wasm_bindgen(js_class = SabrModel)]
impl WasmSabrModel {
    #[wasm_bindgen(constructor)]
    /// Create the object from its inputs.
    pub fn new(params: &WasmSabrParameters) -> WasmSabrModel {
        Self {
            inner: SABRModel::new(params.clone_inner()),
        }
    }

    #[wasm_bindgen(js_name = impliedVol)]
    /// Black implied volatility for the given strike.
    pub fn implied_vol(&self, forward: f64, strike: f64, t: f64) -> Result<f64, JsValue> {
        self.inner
            .implied_volatility(forward, strike, t)
            .map_err(to_js_err)
    }

    /// Parameters used by this model.
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> WasmSabrParameters {
        WasmSabrParameters {
            inner: self.inner.parameters().clone(),
        }
    }

    #[wasm_bindgen(js_name = supportsNegativeRates)]
    /// Whether the parameterization admits negative forwards.
    pub fn supports_negative_rates(&self) -> bool {
        self.inner.supports_negative_rates()
    }
}

// ---------------------------------------------------------------------------
// SabrSmile
// ---------------------------------------------------------------------------

/// Volatility smile generator for a fixed `(forward, t)` pair.
#[wasm_bindgen(js_name = SabrSmile)]
pub struct WasmSabrSmile {
    inner: SABRSmile,
}

#[wasm_bindgen(js_class = SabrSmile)]
impl WasmSabrSmile {
    #[wasm_bindgen(constructor)]
    /// Create the object from its inputs.
    pub fn new(params: &WasmSabrParameters, forward: f64, t: f64) -> WasmSabrSmile {
        let model = SABRModel::new(params.clone_inner());
        Self {
            inner: SABRSmile::new(model, forward, t),
        }
    }

    #[wasm_bindgen(js_name = atmVol)]
    /// At-the-money implied volatility.
    pub fn atm_vol(&self) -> Result<f64, JsValue> {
        self.inner.atm_vol().map_err(to_js_err)
    }

    #[wasm_bindgen(js_name = impliedVol)]
    /// Black implied volatility for the given strike.
    pub fn implied_vol(&self, strike: f64) -> Result<f64, JsValue> {
        self.inner
            .generate_smile(&[strike])
            .map(|v| v[0])
            .map_err(to_js_err)
    }

    #[wasm_bindgen(js_name = generateSmile)]
    /// Implied volatilities for a strike grid.
    pub fn generate_smile(&self, strikes: Vec<f64>) -> Result<Vec<f64>, JsValue> {
        self.inner.generate_smile(&strikes).map_err(to_js_err)
    }

    /// Butterfly + monotonicity arbitrage diagnostics.
    ///
    /// Returns a JSON object with `arbitrage_free`, `butterfly_violations`,
    /// and `monotonicity_violations` arrays (snake_case keys matching the Rust
    /// canonical fields and the Python binding).
    #[wasm_bindgen(js_name = arbitrageDiagnostics)]
    pub fn arbitrage_diagnostics(
        &self,
        strikes: Vec<f64>,
        r: Option<f64>,
        q: Option<f64>,
    ) -> Result<JsValue, JsValue> {
        let result = self
            .inner
            .validate_no_arbitrage(&strikes, r.unwrap_or(0.0), q.unwrap_or(0.0))
            .map_err(to_js_err)?;
        // Keep snake_case keys matching the Rust canonical fields and the Python
        // binding so cross-binding consumers and parity tests read the same
        // names (the earlier camelCase remap diverged from Python).
        let butterfly: Vec<serde_json::Value> = result
            .butterfly_violations
            .iter()
            .map(|v| {
                serde_json::json!({
                    "strike": v.strike,
                    "butterfly_value": v.butterfly_value,
                    "severity_pct": v.severity_pct,
                })
            })
            .collect();
        let monotonicity: Vec<serde_json::Value> = result
            .monotonicity_violations
            .iter()
            .map(|v| {
                serde_json::json!({
                    "strike_low": v.strike_low,
                    "strike_high": v.strike_high,
                    "price_low": v.price_low,
                    "price_high": v.price_high,
                })
            })
            .collect();
        let out = serde_json::json!({
            "arbitrage_free": result.is_arbitrage_free(),
            "butterfly_violations": butterfly,
            "monotonicity_violations": monotonicity,
        });
        to_js_value(&out)
    }
}

// ---------------------------------------------------------------------------
// SabrCalibrator
// ---------------------------------------------------------------------------

/// SABR calibrator (Levenberg-Marquardt with beta fixed).
#[wasm_bindgen(js_name = SabrCalibrator)]
pub struct WasmSabrCalibrator {
    inner: SABRCalibrator,
}

#[wasm_bindgen(js_class = SabrCalibrator)]
impl WasmSabrCalibrator {
    #[wasm_bindgen(constructor)]
    /// Create the object from its inputs.
    pub fn new() -> WasmSabrCalibrator {
        Self {
            inner: SABRCalibrator::new(),
        }
    }

    #[wasm_bindgen(js_name = highPrecision)]
    /// Calibrator preset with tighter convergence tolerances.
    pub fn high_precision() -> WasmSabrCalibrator {
        Self {
            inner: SABRCalibrator::high_precision(),
        }
    }

    /// Return a copy of this calibrator with an overridden convergence
    /// tolerance, preserving all other settings (e.g. the iteration cap from
    /// `highPrecision`).
    #[wasm_bindgen(js_name = withTolerance)]
    pub fn with_tolerance(&self, tolerance: f64) -> WasmSabrCalibrator {
        Self {
            inner: self.inner.clone().with_tolerance(tolerance),
        }
    }

    /// Calibrate `(alpha, nu, rho)` to market vols with `beta` fixed.
    pub fn calibrate(
        &self,
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> Result<WasmSabrParameters, JsValue> {
        check_smile_lengths(&strikes, &market_vols)?;
        self.inner
            .calibrate(forward, &strikes, &market_vols, t, beta)
            .map(|inner| WasmSabrParameters { inner })
            .map_err(to_js_err)
    }

    /// Calibrate with automatic shift selection for negative-rate smiles.
    ///
    /// When the forward or any strike is negative, a shifted-SABR fit is
    /// performed with an automatically chosen shift; otherwise this behaves
    /// like `calibrate`.
    #[wasm_bindgen(js_name = calibrateAutoShift)]
    pub fn calibrate_auto_shift(
        &self,
        forward: f64,
        strikes: Vec<f64>,
        market_vols: Vec<f64>,
        t: f64,
        beta: f64,
    ) -> Result<WasmSabrParameters, JsValue> {
        check_smile_lengths(&strikes, &market_vols)?;
        self.inner
            .calibrate_auto_shift(forward, &strikes, &market_vols, t, beta)
            .map(|inner| WasmSabrParameters { inner })
            .map_err(to_js_err)
    }
}

fn check_smile_lengths(strikes: &[f64], market_vols: &[f64]) -> Result<(), JsValue> {
    if strikes.len() != market_vols.len() {
        return Err(to_js_err(format!(
            "strikes length ({}) must match market_vols length ({})",
            strikes.len(),
            market_vols.len()
        )));
    }
    Ok(())
}

impl Default for WasmSabrCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabr_params_equity_default_roundtrip() {
        let p = WasmSabrParameters::equity_default();
        assert!((p.alpha() - 0.20).abs() < 1e-12);
        assert!((p.beta() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn sabr_model_computes_atm_vol() {
        let p = WasmSabrParameters::new(0.2, 1.0, 0.3, -0.2, None).expect("params");
        let smile = WasmSabrSmile::new(&p, 100.0, 1.0);
        let atm = smile.atm_vol().expect("atm_vol");
        assert!(atm > 0.0 && atm < 1.0);
    }

    #[test]
    fn sabr_model_exposes_params_getter() {
        let p = WasmSabrParameters::new(0.2, 0.5, 0.3, -0.2, None).expect("params");
        let model = WasmSabrModel::new(&p);
        let roundtrip = model.params();
        assert!((roundtrip.alpha() - 0.2).abs() < 1e-12);
        assert!((roundtrip.beta() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn sabr_calibrator_with_tolerance_calibrates() {
        let p = WasmSabrParameters::new(0.05, 0.5, 0.4, -0.1, None).expect("params");
        let strikes = vec![0.01, 0.02, 0.03, 0.04, 0.05];
        let smile = WasmSabrSmile::new(&p, 0.03, 1.0);
        let vols = smile.generate_smile(strikes.clone()).expect("smile");

        // 1e-6 on the vega-weighted SSE objective is attainable within the
        // default iteration budget; tighter tolerances fail loudly under the
        // strict non-convergence semantics of core `minimize` because rho is
        // weakly identified on this near-symmetric strike set.
        let calibrator = WasmSabrCalibrator::new().with_tolerance(1e-6);
        let fitted = calibrator
            .calibrate(0.03, strikes, vols, 1.0, 0.5)
            .expect("calibrate");
        assert!((fitted.beta() - 0.5).abs() < 1e-12);
        assert!(fitted.alpha() > 0.0);
    }

    #[test]
    fn sabr_calibrate_auto_shift_fits_negative_rate_smile() {
        let p = WasmSabrParameters::new(0.05, 0.5, 0.4, -0.1, Some(0.03)).expect("params");
        let forward = -0.005;
        let strikes = vec![-0.015, -0.01, -0.005, 0.0, 0.005];
        let smile = WasmSabrSmile::new(&p, forward, 1.0);
        let vols = smile.generate_smile(strikes.clone()).expect("smile");

        let fitted = WasmSabrCalibrator::new()
            .calibrate_auto_shift(forward, strikes, vols, 1.0, 0.5)
            .expect("calibrate_auto_shift");
        let shift = fitted
            .shift()
            .expect("negative-rate fit must carry a shift");
        assert!(shift > 0.0);
        assert!(fitted.is_shifted());
    }
}
