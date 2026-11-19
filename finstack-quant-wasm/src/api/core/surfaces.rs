//! WASM bindings for the volatility surfaces of
//! `finstack_quant_core::market_data::surfaces`: `VolSurface`, `VolCube`,
//! `VolCubeExpirySlice` and `FxDeltaVolSurface`.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::{JsVolatilityClock, js_opt_volatility_clock};
use crate::utils::input::{
    from_js_json, invalid_type, is_plain_object, js_f64, js_f64_seq, js_opt_f64, js_opt_f64_seq,
    js_opt_string, js_string, js_uint, wasm_handle_string,
};
use crate::utils::wire::{label, parse_label};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::currency::Currency as RustCurrency;
use finstack_quant_core::market_data::surfaces::{
    FxAtmConvention, FxButterflyConvention, FxDeltaConvention,
    FxDeltaVolSurface as RustFxDeltaVolSurface, FxDeltaVolSurfaceParams, SabrParameterData,
    VolCube as RustVolCube, VolCubeExpirySlice as RustVolCubeExpirySlice, VolGridOpts,
    VolInterpolationMode, VolQuoteType, VolSurface as RustVolSurface, VolSurfaceAxis,
};
use finstack_quant_core::money::fx::CurrencyPair;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Construct validated SABR parameter data for a volatility-cube node.
///
/// # Arguments
///
/// * `alpha` - Finite, strictly positive initial volatility scale.
/// * `beta` - Finite CEV elasticity in the inclusive range `[0, 1]`.
/// * `nu` - Finite, nonnegative volatility of volatility; zero is deterministic.
/// * `rho` - Finite forward-volatility correlation strictly between `-1` and `1`.
/// * `shift` - Optional finite displacement in forward/strike units; `null` or
///   omission records an unshifted node, and finite negative shifts are allowed.
///
/// @returns A `SabrParameterData` plain object; an absent shift is omitted.
/// @throws `TypeError` (kind `invalid_type`) for a non-number argument;
/// `FinstackError` (kind `validation`) for a non-finite or out-of-range parameter.
/// @param alpha - Finite, strictly positive initial volatility scale.
/// @param beta - Finite CEV elasticity in the inclusive range `[0, 1]`.
/// @param nu - Finite, nonnegative volatility of volatility; zero is deterministic.
/// @param rho - Finite forward-volatility correlation strictly between `-1` and `1`.
/// @param shift - Optional finite displacement in forward/strike units; `null` or omission records an unshifted node, and finite negative shifts are allowed.
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const node = core.sabrParameterDataNew(0.2, 1, 0, -0.5);
/// console.log(node.nu); // 0
/// ```
#[wasm_bindgen(js_name = sabrParameterDataNew, unchecked_return_type = "SabrParameterData")]
pub fn sabr_parameter_data_new(
    #[wasm_bindgen(unchecked_param_type = "number")] alpha: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] beta: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] nu: JsValue,
    #[wasm_bindgen(unchecked_param_type = "number")] rho: JsValue,
    #[wasm_bindgen(unchecked_optional_param_type = "number | null")] shift: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    to_js_value(
        &SabrParameterData::new(
            js_f64(&alpha, "alpha")?,
            js_f64(&beta, "beta")?,
            js_f64(&nu, "nu")?,
            js_f64(&rho, "rho")?,
            js_opt_f64(shift.as_ref(), "shift")?,
        )
        .map_err(to_js_err)?,
    )
}

/// SABR volatility cube for swaption pricing.
///
/// Stores calibrated SABR parameters on an expiry × tenor grid and evaluates
/// implied volatilities via bilinear parameter interpolation followed by the
/// Hagan (2002) approximation.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const cube = new core.VolCube(
///   "USD-SWAPTION",
///   [1],
///   [5],
///   [{ alpha: 0.02, beta: 0.5, rho: -0.2, nu: 0.4 }],
///   [0.03]
/// );
/// console.log(cube.id, cube.interpolationMode);
/// cube.free();
/// ```
#[wasm_bindgen(js_name = VolCube)]
pub struct JsVolCube {
    pub(crate) inner: Arc<RustVolCube>,
}

#[wasm_bindgen(js_class = VolCube)]
impl JsVolCube {
    /// Construct a vol cube from SABR parameter nodes (Rust `VolCube::from_grid`).
    ///
    /// # Arguments
    /// * `id` - Curve identifier.
    /// * `expiries` - Option expiry axis in years (strictly increasing).
    /// * `tenors` - Swap tenor axis in years (strictly increasing).
    /// * `params_row_major` - Row-major SABR nodes (expiry-major): an array of
    ///   `{ alpha, beta, rho, nu, shift? }` objects (the `SabrParameterData`
    ///   wire form) of length `expiries.length * tenors.length`; an omitted
    ///   or `null` `shift` means an unshifted node. The `nu` component is
    ///   nonnegative; zero gives deterministic volatility.
    /// * `forwards_row_major` - Row-major forward rates, one per grid node.
    /// @param interpolationMode - Interpolation across the expiry axis: `"vol"` or
    /// `"total_variance"`; omitted keeps the Rust `VolCube::from_grid` default (`"vol"`).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if an axis is empty, non-finite,
    /// non-positive, or not strictly increasing; the parameter or forward array
    /// has the wrong length; a node has unknown fields; a forward is
    /// non-finite; any SABR node has invalid
    /// alpha, beta, rho, nu, or shift; or `interpolationMode` is neither `vol`
    /// nor `total_variance`.
    /// @param id - Curve identifier.
    /// @param expiries - Option expiry axis in years (strictly increasing).
    /// @param tenors - Swap tenor axis in years (strictly increasing).
    /// @param paramsRowMajor - Row-major SABR nodes (expiry-major): an array of `{ alpha, beta, rho, nu, shift? }` objects (the `SabrParameterData` wire form) of length `expiries.length * tenors.length`; an omitted or `null` `shift` means an unshifted node. The `nu` component is nonnegative; zero gives deterministic volatility.
    /// @param forwardsRowMajor - Row-major forward rates, one per grid node.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] tenors: JsValue,
        #[wasm_bindgen(
            js_name = "paramsRowMajor",
            unchecked_param_type = "SabrParameterData[] | string"
        )]
        params_row_major: JsValue,
        #[wasm_bindgen(js_name = "forwardsRowMajor", unchecked_param_type = "NumericArray")]
        forwards_row_major: JsValue,
        #[wasm_bindgen(
            js_name = "interpolationMode",
            unchecked_optional_param_type = "string"
        )]
        interpolation_mode: Option<JsValue>,
    ) -> Result<JsVolCube, JsValue> {
        let id = js_string(&id, "id")?;
        let expiries = js_f64_seq(&expiries, "expiries")?;
        let tenors = js_f64_seq(&tenors, "tenors")?;
        let sabr_params =
            from_js_json::<Vec<SabrParameterData>>(&params_row_major, "paramsRowMajor")?;
        let forwards = js_f64_seq(&forwards_row_major, "forwardsRowMajor")?;
        let interpolation_mode = js_opt_string(interpolation_mode.as_ref(), "interpolationMode")?;
        let mut cube = RustVolCube::from_grid(&id, &expiries, &tenors, &sabr_params, &forwards)
            .map_err(to_js_err)?;
        if let Some(mode) = interpolation_mode.as_deref() {
            cube = cube.with_interpolation_mode(parse_label::<VolInterpolationMode>(mode)?);
        }
        Ok(Self {
            inner: Arc::new(cube),
        })
    }

    /// Deserialize a canonical SABR cube state without flattening parameter nodes.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical VolCube JSON containing id, expiry and tenor axes in years,
    ///   row-major SABR nodes and decimal-rate forwards, and interpolation_mode.
    ///   Missing or null node shifts remain absent; unknown fields are rejected.
    /// @returns A validated VolCube handle owned by the caller; release it with free().
    /// @throws Error - Throws when JSON is malformed, fields are unknown, or native axis, parameter, or forward validation fails.
    /// @param json - Canonical VolCube JSON containing id, expiry and tenor axes in years, row-major SABR nodes and decimal-rate forwards, and interpolation_mode. Missing or null node shifts remain absent; unknown fields are rejected.
    ///
    /// @example
    /// ```typescript
    /// import init, { core } from "finstack-quant-wasm";
    /// await init();
    /// const cube = core.VolCube.fromJson(
    ///   JSON.stringify({
    ///     id: "USD-SWAPTION",
    ///     expiries: [1],
    ///     tenors: [5],
    ///     params: [{ alpha: 0.03, beta: 0.5, rho: -0.2, nu: 0.4, shift: null }],
    ///     forwards: [0.03],
    ///     interpolation_mode: "vol",
    ///   })
    /// );
    /// console.log(cube.id);
    /// cube.free();
    /// ```
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsVolCube, JsValue> {
        let inner = from_js_json::<RustVolCube>(&json, "json")?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Interpolation contract used across the expiry axis.
    #[wasm_bindgen(getter, js_name = interpolationMode)]
    pub fn interpolation_mode(&self) -> Result<String, JsValue> {
        label(&self.inner.interpolation_mode())
    }

    /// Cube identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid cube).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Option expiry axis in years.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Box<[f64]> {
        self.inner.expiries().into()
    }

    /// Underlying swap tenor axis in years.
    #[wasm_bindgen(getter, js_name = tenors)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors().into()
    }

    /// Grid shape as `[nExpiries, nTenors]`.
    #[wasm_bindgen(getter, js_name = gridShape)]
    pub fn grid_shape(&self) -> Box<[usize]> {
        let (n_exp, n_ten) = self.inner.grid_shape();
        Box::new([n_exp, n_ten])
    }

    /// Row-major forward rates (decimals), one per grid node.
    #[wasm_bindgen(getter, js_name = forwards)]
    pub fn forwards(&self) -> Box<[f64]> {
        self.inner.forwards().into()
    }

    /// Row-major SABR nodes as plain `{alpha, beta, rho, nu, shift?}` objects.
    ///
    /// @returns One object per grid node in row-major (expiry, tenor) order.
    /// @throws If serialization fails (not expected for a valid cube).
    #[wasm_bindgen(getter, js_name = params, unchecked_return_type = "SabrParameterData[]")]
    pub fn params(&self) -> Result<JsValue, JsValue> {
        crate::utils::to_js_value(&self.inner.params())
    }

    /// SABR parameters at grid indices, as a plain `{alpha, beta, rho, nu, shift?}` object.
    ///
    /// # Arguments
    ///
    /// * `exp_idx` - Zero-based index into `expiries`.
    /// * `tenor_idx` - Zero-based index into `tenors`.
    ///
    /// @returns The node's SABR parameters.
    /// @throws `TypeError` if an index is not a non-negative integer;
    /// `FinstackError` (kind `validation`) if it lies outside `gridShape`.
    /// @param expIdx - Zero-based index into `expiries`.
    /// @param tenorIdx - Zero-based index into `tenors`.
    #[wasm_bindgen(js_name = paramsAt, unchecked_return_type = "SabrParameterData")]
    pub fn params_at(
        &self,
        #[wasm_bindgen(js_name = "expIdx", unchecked_param_type = "number")] exp_idx: JsValue,
        #[wasm_bindgen(js_name = "tenorIdx", unchecked_param_type = "number")] tenor_idx: JsValue,
    ) -> Result<JsValue, JsValue> {
        let exp_idx: usize = js_uint(&exp_idx, "expIdx")?;
        let tenor_idx: usize = js_uint(&tenor_idx, "tenorIdx")?;
        let params = self
            .inner
            .params_at(exp_idx, tenor_idx)
            .map_err(to_js_err)?;
        crate::utils::to_js_value(params)
    }

    /// Forward rate (decimal) at grid indices.
    ///
    /// # Arguments
    ///
    /// * `exp_idx` - Zero-based index into `expiries`.
    /// * `tenor_idx` - Zero-based index into `tenors`.
    ///
    /// @returns The node's forward rate.
    /// @throws `TypeError` if an index is not a non-negative integer;
    /// `FinstackError` (kind `validation`) if it lies outside `gridShape`.
    /// @param expIdx - Zero-based index into `expiries`.
    /// @param tenorIdx - Zero-based index into `tenors`.
    #[wasm_bindgen(js_name = forwardAt)]
    pub fn forward_at(
        &self,
        #[wasm_bindgen(js_name = "expIdx", unchecked_param_type = "number")] exp_idx: JsValue,
        #[wasm_bindgen(js_name = "tenorIdx", unchecked_param_type = "number")] tenor_idx: JsValue,
    ) -> Result<f64, JsValue> {
        let exp_idx: usize = js_uint(&exp_idx, "expIdx")?;
        let tenor_idx: usize = js_uint(&tenor_idx, "tenorIdx")?;
        self.inner.forward_at(exp_idx, tenor_idx).map_err(to_js_err)
    }
}

thread_local! {
    static CURRENCY_PROTOTYPE: std::cell::OnceCell<js_sys::Object> = const { std::cell::OnceCell::new() };
}

fn currency_arg(value: &JsValue, label_name: &str) -> Result<RustCurrency, JsValue> {
    if let Some(code) = value.as_string() {
        return code.parse::<RustCurrency>().map_err(to_js_err);
    }
    if let Some(code) = wasm_handle_string(
        value,
        label_name,
        &CURRENCY_PROTOTYPE,
        || {
            Ok(JsCurrency {
                inner: RustCurrency::USD,
            })
        },
        "toString",
    )? {
        return code.parse::<RustCurrency>().map_err(to_js_err);
    }
    Err(invalid_type(
        label_name,
        "expected a Currency handle or ISO-4217 currency code string",
    ))
}

fn parse_currency_pair(value: &JsValue) -> Result<CurrencyPair, JsValue> {
    if let Some(text) = value.as_string() {
        let parsed = text.parse::<CurrencyPair>().map_err(to_js_err)?;
        return CurrencyPair::try_new(parsed.base, parsed.quote).map_err(to_js_err);
    }
    if js_sys::Array::is_array(value) {
        let array = js_sys::Array::from(value);
        if array.length() != 2 {
            return Err(invalid_type(
                "pair",
                "expected a [base, quote] array of length 2",
            ));
        }
        let base = currency_arg(&array.get(0), "pair[0]")?;
        let quote = currency_arg(&array.get(1), "pair[1]")?;
        return CurrencyPair::try_new(base, quote).map_err(to_js_err);
    }
    if is_plain_object(value) {
        let parsed = from_js_json::<CurrencyPair>(value, "pair")?;
        return CurrencyPair::try_new(parsed.base, parsed.quote).map_err(to_js_err);
    }
    Err(invalid_type(
        "pair",
        "expected a currency pair string (\"EUR/USD\"), [base, quote] pair, or CurrencyPair object",
    ))
}

struct ParsedFxSurfaceOptions {
    rr10d: Option<Vec<f64>>,
    bf10d: Option<Vec<f64>>,
    delta_convention: FxDeltaConvention,
    atm_convention: FxAtmConvention,
    butterfly_convention: FxButterflyConvention,
    expiry_clock: Option<finstack_quant_core::dates::VolatilityClock>,
}

fn parse_fx_surface_options(opts_val: &JsValue) -> Result<ParsedFxSurfaceOptions, JsValue> {
    const ALLOWED_KEYS: [&str; 6] = [
        "rr10d",
        "bf10d",
        "deltaConvention",
        "atmConvention",
        "butterflyConvention",
        "expiryClock",
    ];
    if !is_plain_object(opts_val) {
        return Err(invalid_type(
            "options",
            "expected a plain FxDeltaVolSurfaceOptions object",
        ));
    }
    let obj = js_sys::Object::from(opts_val.clone());
    for key_val in js_sys::Object::keys(&obj).iter() {
        let Some(key) = key_val.as_string() else {
            continue;
        };
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(to_js_err(finstack_quant_core::Error::Validation(format!(
                "unknown FxDeltaVolSurface option `{key}`; allowed keys are {}",
                ALLOWED_KEYS.join(", ")
            ))));
        }
    }
    let field = |name: &str| {
        js_sys::Reflect::get(&obj, &JsValue::from(name))
            .map_err(|_| invalid_type(&format!("options.{name}"), "could not read property"))
    };
    let label = |name: &str| js_string(&field(name)?, &format!("options.{name}"));
    Ok(ParsedFxSurfaceOptions {
        delta_convention: parse_label(&label("deltaConvention")?)?,
        atm_convention: parse_label(&label("atmConvention")?)?,
        butterfly_convention: parse_label(&label("butterflyConvention")?)?,
        rr10d: js_opt_f64_seq(Some(&field("rr10d")?), "options.rr10d")?,
        bf10d: js_opt_f64_seq(Some(&field("bf10d")?), "options.bf10d")?,
        expiry_clock: js_opt_volatility_clock(Some(&field("expiryClock")?), "options.expiryClock")?,
    })
}

/// FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
/// 10-delta wings).
///
/// Stores market-standard FX delta quotes (Wystup 2006, Clark 2011) on an
/// explicit `BASE/QUOTE` currency pair under the closed profile
/// (`forward_unadjusted`, `delta_neutral_straddle`, `smile`). Use
/// `models.volatility` (`getFxDeltaVol`, `getFxDeltaPillarVols`,
/// `materializeFxDeltaSurface`) for evaluation; this type does not convert
/// quotes to strikes itself.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const surface = new core.FxDeltaVolSurface(
///   "EURUSD-VOL",
///   "EUR/USD",
///   [1],
///   [0.12],
///   [0.01],
///   [0.002],
///   { deltaConvention: "forward_unadjusted", atmConvention: "delta_neutral_straddle", butterflyConvention: "smile" }
/// );
/// console.log(surface.id, surface.pair, surface.quoteType, surface.expiries.length);
/// surface.free();
/// ```
#[wasm_bindgen(js_name = FxDeltaVolSurface)]
pub struct JsFxDeltaVolSurface {
    pub(crate) inner: Arc<RustFxDeltaVolSurface>,
}

#[wasm_bindgen(js_class = FxDeltaVolSurface)]
impl JsFxDeltaVolSurface {
    /// Construct an FX delta-quoted vol surface with 25-delta wings and
    /// explicit pair and convention options (Rust `FxDeltaVolSurface::from_params`).
    ///
    /// The required `options` object supplies all three convention labels.
    /// It also accepts paired 10-delta wings (`rr10d` and `bf10d`) and an
    /// optional `expiryClock`. Unknown option keys are rejected.
    ///
    /// # Arguments
    /// * `id`       - Non-empty stable surface identifier.
    /// * `pair`     - Currency pair `BASE/QUOTE` (string such as `"EUR/USD"`, `[base, quote]` pair, or `{ base, quote }` object; `base != quote`).
    /// * `expiries` - Strictly increasing positive expiry times (years).
    /// * `atm_vols` - Positive ATM delta-neutral straddle vols per expiry (decimal).
    /// * `rr25d`    - 25-delta risk reversal per expiry (call vol − put vol, decimal).
    /// * `bf25d`    - 25-delta butterfly per expiry (wing avg − ATM, decimal).
    /// * `options`  - Required `FxDeltaVolSurfaceOptions` bag (`rr10d`, `bf10d`, `deltaConvention`, `atmConvention`, `butterflyConvention`, `expiryClock`).
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` for mistyped arguments or a `FinstackError` (kind
    /// `validation`) if `id` is empty, `pair` has identical currencies or an
    /// invalid ISO code, `options` contains unknown keys or unpaired
    /// `rr10d`/`bf10d`, expiry/quote lengths mismatch, or any value violates
    /// domain constraints.
    /// @param id - Non-empty stable surface identifier.
    /// @param pair - Ordered currency pair (`"EUR/USD"`, `[base, quote]`, or `CurrencyPair`).
    /// @param expiries - Strictly increasing positive expiry times (years).
    /// @param atmVols - ATM delta-neutral straddle vols per expiry (decimal).
    /// @param rr25d - 25-delta risk reversal per expiry (call vol − put vol, decimal).
    /// @param bf25d - 25-delta butterfly per expiry (wing avg − ATM, decimal).
    /// @param options - Three required FX convention labels, optional paired 10-delta wings, and an optional expiry clock.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "CurrencyPairInput")] pair: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
        #[wasm_bindgen(js_name = "atmVols", unchecked_param_type = "NumericArray")]
        atm_vols: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] rr25d: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] bf25d: JsValue,
        #[wasm_bindgen(unchecked_param_type = "FxDeltaVolSurfaceOptions")] options: JsValue,
    ) -> Result<JsFxDeltaVolSurface, JsValue> {
        let id = js_string(&id, "id")?;
        let pair = parse_currency_pair(&pair)?;
        let expiries = js_f64_seq(&expiries, "expiries")?;
        let atm_vols = js_f64_seq(&atm_vols, "atmVols")?;
        let rr25d = js_f64_seq(&rr25d, "rr25d")?;
        let bf25d = js_f64_seq(&bf25d, "bf25d")?;
        let opts = parse_fx_surface_options(&options)?;
        let surface = RustFxDeltaVolSurface::from_params(FxDeltaVolSurfaceParams {
            id: CurveId::new(id),
            pair,
            delta_convention: opts.delta_convention,
            atm_convention: opts.atm_convention,
            butterfly_convention: opts.butterfly_convention,
            expiries,
            atm_vols,
            rr_25d: rr25d,
            bf_25d: bf25d,
            rr_10d: opts.rr10d,
            bf_10d: opts.bf10d,
            expiry_clock: opts.expiry_clock,
        })
        .map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Deserialize canonical FX delta quotes without reconstructing positional arrays.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical FxDeltaVolSurface JSON with `pair`, conventions,
    ///   expiries in years, and annualized decimal ATM, risk-reversal, and
    ///   butterfly quotes. Optional 10-delta wings must occur together; unknown
    ///   fields are rejected.
    /// @returns A validated FxDeltaVolSurface handle owned by the caller; release it with free().
    /// @throws Error - Throws when JSON is malformed, fields are unknown, or native expiry, quote, or wing validation fails.
    /// @param json - Canonical FxDeltaVolSurface JSON with `pair`, conventions, expiries in years, and annualized decimal ATM, risk-reversal, and butterfly quotes. Optional 10-delta wings must occur together; unknown fields are rejected.
    ///
    /// @example
    /// ```typescript
    /// import init, { core } from "finstack-quant-wasm";
    /// await init();
    /// const surface = core.FxDeltaVolSurface.fromJson(
    ///   JSON.stringify({
    ///     id: "EURUSD",
    ///     pair: { base: "EUR", quote: "USD" },
    ///     delta_convention: "forward_unadjusted",
    ///     atm_convention: "delta_neutral_straddle",
    ///     butterfly_convention: "smile",
    ///     expiries: [1],
    ///     atm_vols: [0.12],
    ///     rr_25d: [0.01],
    ///     bf_25d: [0.002],
    ///     rr_10d: null,
    ///     bf_10d: null,
    ///     expiry_clock: null,
    ///   })
    /// );
    /// console.log(surface.id, surface.pair);
    /// surface.free();
    /// ```
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsFxDeltaVolSurface, JsValue> {
        let inner = from_js_json::<RustFxDeltaVolSurface>(&json, "json")?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Surface identifier.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Ordered currency pair (`"BASE/QUOTE"`).
    #[wasm_bindgen(getter, js_name = pair)]
    pub fn pair(&self) -> String {
        self.inner.pair().to_string()
    }

    /// Expiry axis in years.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Box<[f64]> {
        self.inner.expiries().into()
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid surface).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// ATM delta-neutral straddle vols per expiry (decimals).
    #[wasm_bindgen(getter, js_name = atmVols)]
    pub fn atm_vols(&self) -> Box<[f64]> {
        self.inner.atm_vols().into()
    }

    /// 25-delta risk reversals per expiry (call vol minus put vol, decimals).
    #[wasm_bindgen(getter, js_name = rr25d)]
    pub fn rr_25d(&self) -> Box<[f64]> {
        self.inner.rr_25d().into()
    }

    /// 25-delta butterflies per expiry (wing average minus ATM, decimals).
    #[wasm_bindgen(getter, js_name = bf25d)]
    pub fn bf_25d(&self) -> Box<[f64]> {
        self.inner.bf_25d().into()
    }

    /// 10-delta risk reversals per expiry, or `undefined` without 10-delta wings.
    #[wasm_bindgen(getter, js_name = rr10d)]
    pub fn rr_10d(&self) -> Option<Box<[f64]>> {
        self.inner.rr_10d().map(Into::into)
    }

    /// 10-delta butterflies per expiry, or `undefined` without 10-delta wings.
    #[wasm_bindgen(getter, js_name = bf10d)]
    pub fn bf_10d(&self) -> Option<Box<[f64]>> {
        self.inner.bf_10d().map(Into::into)
    }

    /// Forward delta convention (`"forward_unadjusted"`).
    #[wasm_bindgen(getter, js_name = deltaConvention)]
    pub fn delta_convention(&self) -> Result<String, JsValue> {
        label(&self.inner.delta_convention())
    }

    /// ATM strike convention (`"delta_neutral_straddle"`).
    #[wasm_bindgen(getter, js_name = atmConvention)]
    pub fn atm_convention(&self) -> Result<String, JsValue> {
        label(&self.inner.atm_convention())
    }

    /// Butterfly quote convention (`"smile"`).
    #[wasm_bindgen(getter, js_name = butterflyConvention)]
    pub fn butterfly_convention(&self) -> Result<String, JsValue> {
        label(&self.inner.butterfly_convention())
    }

    /// Output volatility quote type (`"black_lognormal"`).
    #[wasm_bindgen(getter, js_name = quoteType)]
    pub fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Optional volatility expiry clock, or `undefined` when not configured.
    #[wasm_bindgen(getter, js_name = expiryClock)]
    pub fn expiry_clock(&self) -> Option<JsVolatilityClock> {
        self.inner
            .expiry_clock()
            .cloned()
            .map(|inner| JsVolatilityClock { inner })
    }
}

/// Implied-volatility surface on an expiry × secondary-coordinate (strike or tenor) grid.
///
/// Volatilities are decimals (`0.20` is 20%); expiries are year fractions.
/// Lookups inside the grid interpolate bilinearly in volatility or in total
/// variance, per `interpolationMode`.
///
/// @example
/// ```typescript
/// import init, { core, models } from "finstack-quant-wasm";
/// await init();
/// const surface = new core.VolSurface(
///   "SPX-VOL",
///   [0.5, 1.0],
///   [90, 100, 110],
///   [0.24, 0.2, 0.22, 0.23, 0.21, 0.22],
///   {
///     secondaryAxis: "strike",
///     quoteType: "black_lognormal",
///     interpolationMode: "vol",
///   },
/// );
/// models.volatility.getSurfaceVol(surface, 0.75, 100); // 0.205
/// surface.gridShape; // [2, 3]
/// ```
#[wasm_bindgen(js_name = VolSurface)]
pub struct JsVolSurface {
    pub(crate) inner: Arc<RustVolSurface>,
}

impl JsVolSurface {
    pub(crate) fn from_inner(inner: Arc<RustVolSurface>) -> Self {
        Self { inner }
    }
}

struct ParsedVolSurfaceOptions {
    secondary_axis: VolSurfaceAxis,
    quote_type: VolQuoteType,
    interpolation_mode: VolInterpolationMode,
    displacements: Option<Vec<f64>>,
    expiry_clock: Option<finstack_quant_core::dates::VolatilityClock>,
}

fn parse_vol_surface_options(value: &JsValue) -> Result<ParsedVolSurfaceOptions, JsValue> {
    const ALLOWED_KEYS: [&str; 5] = [
        "secondaryAxis",
        "quoteType",
        "interpolationMode",
        "displacements",
        "expiryClock",
    ];
    if !is_plain_object(value) {
        return Err(invalid_type(
            "options",
            "expected a plain VolSurfaceOptions object",
        ));
    }
    let obj = js_sys::Object::from(value.clone());
    for key_val in js_sys::Object::keys(&obj).iter() {
        let Some(key) = key_val.as_string() else {
            continue;
        };
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(to_js_err(finstack_quant_core::Error::Validation(format!(
                "unknown VolSurface option `{key}`; allowed keys are {}",
                ALLOWED_KEYS.join(", ")
            ))));
        }
    }
    let field = |name: &str| {
        js_sys::Reflect::get(&obj, &JsValue::from(name))
            .map_err(|_| invalid_type(&format!("options.{name}"), "could not read property"))
    };
    let secondary_axis = parse_label::<VolSurfaceAxis>(&js_string(
        &field("secondaryAxis")?,
        "options.secondaryAxis",
    )?)?;
    let quote_type = js_string(&field("quoteType")?, "options.quoteType")?
        .parse::<VolQuoteType>()
        .map_err(to_js_err)?;
    let interpolation_mode = parse_label::<VolInterpolationMode>(&js_string(
        &field("interpolationMode")?,
        "options.interpolationMode",
    )?)?;
    let displacements = js_opt_f64_seq(Some(&field("displacements")?), "options.displacements")?;
    let expiry_clock =
        js_opt_volatility_clock(Some(&field("expiryClock")?), "options.expiryClock")?;
    Ok(ParsedVolSurfaceOptions {
        secondary_axis,
        quote_type,
        interpolation_mode,
        displacements,
        expiry_clock,
    })
}

#[wasm_bindgen(js_class = VolSurface)]
impl JsVolSurface {
    /// Construct a surface from a row-major volatility grid (Rust
    /// `VolSurface::from_grid`).
    ///
    /// # Arguments
    ///
    /// * `id` - Surface identifier, the `MarketContext` lookup key.
    /// * `expiries` - Option expiries in years, strictly increasing.
    /// * `secondary_values` - Secondary-axis coordinates (strikes, or swap
    ///   tenors in years), strictly increasing.
    /// * `vols` - Decimal volatilities as a flat row-major (expiry-major) grid
    ///   of `expiries.length * secondaryValues.length` entries, finite and
    ///   non-negative.
    /// * `options` - `{ secondaryAxis, quoteType, interpolationMode,
    ///   displacements?, expiryClock? }`: required `secondaryAxis` (`"strike"`
    ///   or `"tenor"`), `quoteType` (`"black_lognormal"`,
    ///   `"shifted_black_lognormal"` or `"normal"`), and `interpolationMode`
    ///   (`"vol"` or `"total_variance"`); optional `displacements` (one
    ///   shifted-Black shift per expiry) and `expiryClock` (optional
    ///   calendar/day-count clock).
    ///
    /// @returns The validated `VolSurface`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument or
    /// missing required option; `FinstackError` (kind `validation`) for an
    /// empty or unsorted axis, a grid of the wrong length, a negative or
    /// non-finite volatility, an unknown option key, an unknown axis, mode or
    /// quote-type name, or displacements that do not match the quote convention
    /// and the expiry count.
    /// @param id - Surface identifier, the `MarketContext` lookup key.
    /// @param expiries - Option expiries in years, strictly increasing.
    /// @param secondaryValues - Secondary-axis coordinates (strikes, or swap tenors in years), strictly increasing.
    /// @param vols - Decimal volatilities as a flat row-major (expiry-major) grid of `expiries.length * secondaryValues.length` entries, finite and non-negative.
    /// @param options - `{ secondaryAxis, quoteType, interpolationMode, displacements?, expiryClock? }` with required `secondaryAxis`, `quoteType`, and `interpolationMode`, plus optional `displacements` and `expiryClock`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] expiries: JsValue,
        #[wasm_bindgen(js_name = "secondaryValues", unchecked_param_type = "NumericArray")]
        secondary_values: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] vols: JsValue,
        #[wasm_bindgen(unchecked_param_type = "VolSurfaceOptions")] options: JsValue,
    ) -> Result<JsVolSurface, JsValue> {
        let id = js_string(&id, "id")?;
        let expiries = js_f64_seq(&expiries, "expiries")?;
        let secondary_values = js_f64_seq(&secondary_values, "secondaryValues")?;
        let options = parse_vol_surface_options(&options)?;
        let vols = js_f64_seq(&vols, "vols")?;
        let opts = VolGridOpts {
            secondary_axis: options.secondary_axis,
            quote_type: options.quote_type,
            interpolation_mode: options.interpolation_mode,
        };
        RustVolSurface::from_grid(
            id,
            &expiries,
            &secondary_values,
            &vols,
            opts,
            options.displacements.as_deref(),
        )
        .map(|surface| match options.expiry_clock {
            Some(clock) => surface.with_expiry_clock(clock),
            None => surface,
        })
        .map(Arc::new)
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form shared with Python `VolSurface.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical VolSurface JSON text or plain object; unknown
    ///   fields are rejected and the grid is re-validated.
    ///
    /// @returns The validated `VolSurface`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails grid validation.
    /// @param json - Canonical VolSurface JSON text or plain object; unknown fields are rejected and the grid is re-validated.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsVolSurface, JsValue> {
        from_js_json::<RustVolSurface>(&json, "json")
            .map(|surface| Self::from_inner(Arc::new(surface)))
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid surface).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Surface identifier (the `MarketContext` lookup key).
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Expiry axis in years, strictly increasing.
    #[wasm_bindgen(getter, js_name = expiries)]
    pub fn expiries(&self) -> Box<[f64]> {
        self.inner.expiries().into()
    }

    /// Secondary axis (strikes, or swap tenors in years), strictly increasing.
    #[wasm_bindgen(getter, js_name = secondaryValues)]
    pub fn secondary_values(&self) -> Box<[f64]> {
        self.inner.secondary_values().into()
    }

    /// Flat row-major grid of decimal volatilities (`gridShape[0]` rows of `gridShape[1]`).
    #[wasm_bindgen(getter, js_name = vols)]
    pub fn vols(&self) -> Box<[f64]> {
        self.inner.vols().into()
    }

    /// What the secondary axis holds: `"strike"` or `"tenor"`.
    #[wasm_bindgen(getter, js_name = secondaryAxis)]
    pub fn secondary_axis(&self) -> String {
        self.inner.secondary_axis().to_string()
    }

    /// Volatility quote convention: `"black_lognormal"`, `"shifted_black_lognormal"` or `"normal"`.
    #[wasm_bindgen(getter, js_name = quoteType)]
    pub fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Per-expiry shifted-Black displacements (Rust `VolSurface::displacements`).
    ///
    /// @returns One displacement per expiry in strike/rate units, or
    /// `undefined` for unshifted and normal quotes.
    #[wasm_bindgen(getter, js_name = displacements)]
    pub fn displacements(&self) -> Option<Box<[f64]>> {
        self.inner.displacements().map(Into::into)
    }

    /// Optional volatility expiry clock, or `undefined` when not configured.
    #[wasm_bindgen(getter, js_name = expiryClock)]
    pub fn expiry_clock(&self) -> Option<JsVolatilityClock> {
        self.inner
            .expiry_clock()
            .cloned()
            .map(|inner| JsVolatilityClock { inner })
    }

    /// Interpolation across the expiry axis: `"vol"` or `"total_variance"`.
    #[wasm_bindgen(getter, js_name = interpolationMode)]
    pub fn interpolation_mode(&self) -> Result<String, JsValue> {
        label(&self.inner.interpolation_mode())
    }

    /// Grid dimensions as `[expiryCount, secondaryValueCount]`.
    #[wasm_bindgen(getter, js_name = gridShape)]
    pub fn grid_shape(&self) -> Box<[usize]> {
        let (expiries, secondary_values) = self.inner.grid_shape();
        Box::new([expiries, secondary_values])
    }
}

/// Volatility grid at one fixed option expiry, indexed by underlying tenor and strike.
///
/// The fixed-expiry slice of a SABR cube: `models.volatility.materializeCubeExpirySlice`
/// returns this handle, and `fromJson` loads one from its canonical JSON.
/// Shifted-Black quotes carry one displacement per tenor, in strike units.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const slice = new core.VolCubeExpirySlice(
///   "USD-SWAPTION-1Y",
///   1.0,
///   [2.0, 5.0],
///   [0.02, 0.03, 0.04],
///   [0.3, 0.25, 0.27, 0.28, 0.24, 0.26],
///   "black_lognormal",
/// );
/// slice.gridShape; // [2, 3]
/// slice.quoteType; // "black_lognormal"
/// ```
#[wasm_bindgen(js_name = VolCubeExpirySlice)]
pub struct JsVolCubeExpirySlice {
    pub(crate) inner: Arc<RustVolCubeExpirySlice>,
}

#[wasm_bindgen(js_class = VolCubeExpirySlice)]
impl JsVolCubeExpirySlice {
    /// Construct a fixed-expiry tenor-by-strike grid (Rust
    /// `VolCubeExpirySlice::from_grid`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of the materialized grid.
    /// * `expiry` - Fixed option expiry in years; finite and positive.
    /// * `tenors` - Underlying tenors in years, positive and strictly increasing.
    /// * `strikes` - Strike coordinates in forward-rate units, strictly increasing.
    /// * `vols` - Flat tenor-major grid, `tenors.length * strikes.length`
    ///   entries: decimal Black volatilities or absolute normal volatilities.
    /// * `quote_type` - Required convention: `"black_lognormal"`,
    ///   `"shifted_black_lognormal"`, or `"normal"`.
    /// * `displacements` - Shifted-Black displacements in rate units, one per
    ///   tenor; required for shifted quotes and omitted otherwise.
    ///
    /// @returns The validated `VolCubeExpirySlice`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an invalid expiry or axis, a
    /// grid of the wrong length, an invalid volatility, an unknown quote type,
    /// or displacements that do not match the quote convention and tenor count.
    /// @param id - Identifier of the materialized grid.
    /// @param expiry - Fixed option expiry in years; finite and positive.
    /// @param tenors - Underlying tenors in years, positive and strictly increasing.
    /// @param strikes - Strike coordinates in forward-rate units, strictly increasing.
    /// @param vols - Flat tenor-major grid, `tenors.length * strikes.length` entries: decimal Black volatilities or absolute normal volatilities.
    /// @param quoteType - Required convention: `"black_lognormal"`, `"shifted_black_lognormal"`, or `"normal"`.
    /// @param displacements - Shifted-Black displacements in rate units, one per tenor; required for shifted quotes and omitted otherwise.
    #[wasm_bindgen(constructor)]
    pub fn new(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] expiry: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] tenors: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] strikes: JsValue,
        #[wasm_bindgen(unchecked_param_type = "NumericArray")] vols: JsValue,
        #[wasm_bindgen(js_name = "quoteType", unchecked_param_type = "string")] quote_type: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "NumericArray | null")]
        displacements: Option<JsValue>,
    ) -> Result<JsVolCubeExpirySlice, JsValue> {
        let quote_type = js_string(&quote_type, "quoteType")?
            .parse()
            .map_err(to_js_err)?;
        let displacements = js_opt_f64_seq(displacements.as_ref(), "displacements")?;
        RustVolCubeExpirySlice::from_grid(
            js_string(&id, "id")?,
            js_f64(&expiry, "expiry")?,
            &js_f64_seq(&tenors, "tenors")?,
            &js_f64_seq(&strikes, "strikes")?,
            &js_f64_seq(&vols, "vols")?,
            quote_type,
            displacements.as_deref(),
        )
        .map(|slice| Self {
            inner: Arc::new(slice),
        })
        .map_err(to_js_err)
    }

    /// Deserialize from the canonical JSON wire form shared with Python
    /// `VolCubeExpirySlice.to_json`.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical VolCubeExpirySlice JSON text or plain object, such
    ///   as `toJson()` output; unknown fields are rejected and the grid is
    ///   re-validated.
    ///
    /// @returns The validated `VolCubeExpirySlice`.
    /// @throws `TypeError` if `json` is not a JSON string or plain object;
    /// `FinstackError` (kind `validation`) if it does not match the schema or
    /// fails grid validation.
    /// @param json - Canonical VolCubeExpirySlice JSON text or plain object, such as `toJson()` output; unknown fields are rejected and the grid is re-validated.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(
        #[wasm_bindgen(unchecked_param_type = "JsonInput")] json: JsValue,
    ) -> Result<JsVolCubeExpirySlice, JsValue> {
        from_js_json::<RustVolCubeExpirySlice>(&json, "json").map(|slice| Self {
            inner: Arc::new(slice),
        })
    }

    /// Serialize to the canonical JSON wire form accepted by `fromJson` and Python.
    ///
    /// @returns Compact JSON text.
    /// @throws If serialization fails (not expected for a valid slice).
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&*self.inner).map_err(to_js_err)
    }

    /// Identifier of the materialized grid.
    #[wasm_bindgen(getter, js_name = id)]
    pub fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Fixed option expiry of the slice in years.
    #[wasm_bindgen(getter, js_name = expiry)]
    pub fn expiry(&self) -> f64 {
        self.inner.expiry()
    }

    /// Underlying-tenor axis in years, strictly increasing.
    #[wasm_bindgen(getter, js_name = tenors)]
    pub fn tenors(&self) -> Box<[f64]> {
        self.inner.tenors().into()
    }

    /// Strike axis in forward-rate units, strictly increasing.
    #[wasm_bindgen(getter, js_name = strikes)]
    pub fn strikes(&self) -> Box<[f64]> {
        self.inner.strikes().into()
    }

    /// Flat tenor-major volatility grid in the declared quote convention.
    #[wasm_bindgen(getter, js_name = vols)]
    pub fn vols(&self) -> Box<[f64]> {
        self.inner.vols().into()
    }

    /// Quote convention: `"black_lognormal"`, `"shifted_black_lognormal"` or `"normal"`.
    #[wasm_bindgen(getter, js_name = quoteType)]
    pub fn quote_type(&self) -> String {
        self.inner.quote_type().to_string()
    }

    /// Per-tenor shifted-Black displacements in rate units, or `undefined` for unshifted quotes.
    #[wasm_bindgen(getter, js_name = displacements)]
    pub fn displacements(&self) -> Option<Box<[f64]>> {
        self.inner.displacements().map(Into::into)
    }

    /// Grid dimensions as `[tenorCount, strikeCount]`.
    #[wasm_bindgen(getter, js_name = gridShape)]
    pub fn grid_shape(&self) -> Box<[usize]> {
        let (tenors, strikes) = self.inner.grid_shape();
        Box::new([tenors, strikes])
    }
}
