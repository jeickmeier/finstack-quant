//! WASM bindings for the volatility surfaces of
//! `finstack_quant_core::market_data::surfaces`: `VolSurface`, `VolCube`,
//! `VolCubeExpirySlice` and `FxDeltaVolSurface`.

use crate::utils::input::{
    from_js_json, js_f64, js_f64_seq, js_opt_f64, js_opt_f64_seq, js_opt_string, js_string, js_uint,
};
use crate::utils::wire::{label, parse_label};
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_core::market_data::surfaces::{
    FxDeltaVolSurface as RustFxDeltaVolSurface, SabrParameterData, VolCube as RustVolCube,
    VolCubeExpirySlice as RustVolCubeExpirySlice, VolGridOpts, VolInterpolationMode,
    VolSurface as RustVolSurface,
};
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
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const node = core.sabrParameterDataNew(0.2, 1, 0, -0.5);
/// console.log(node.nu); // 0
/// ```
#[wasm_bindgen(js_name = sabrParameterDataNew)]
pub fn sabr_parameter_data_new(
    alpha: JsValue,
    beta: JsValue,
    nu: JsValue,
    rho: JsValue,
    shift: Option<JsValue>,
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
    /// @param interpolation_mode - Interpolation across the expiry axis: `"vol"` or
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiries: JsValue,
        tenors: JsValue,
        params_row_major: JsValue,
        forwards_row_major: JsValue,
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
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsVolCube, JsValue> {
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
    #[wasm_bindgen(getter, js_name = params)]
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
    #[wasm_bindgen(js_name = paramsAt)]
    pub fn params_at(&self, exp_idx: JsValue, tenor_idx: JsValue) -> Result<JsValue, JsValue> {
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
    #[wasm_bindgen(js_name = forwardAt)]
    pub fn forward_at(&self, exp_idx: JsValue, tenor_idx: JsValue) -> Result<f64, JsValue> {
        let exp_idx: usize = js_uint(&exp_idx, "expIdx")?;
        let tenor_idx: usize = js_uint(&tenor_idx, "tenorIdx")?;
        self.inner.forward_at(exp_idx, tenor_idx).map_err(to_js_err)
    }
}

/// FX vol surface quoted in **delta space** (ATM, 25-delta RR/BF, optional
/// 10-delta wings).
///
/// Stores market-standard FX delta quotes (Wystup 2006, Clark 2011). Use
/// `models.volatility` (`getFxDeltaVol`, `getFxDeltaPillarVols`) for
/// evaluation; this type does not convert quotes to strikes itself.
/// The delta convention is **forward delta (premium-unadjusted)**.
#[wasm_bindgen(js_name = FxDeltaVolSurface)]
pub struct JsFxDeltaVolSurface {
    pub(crate) inner: Arc<RustFxDeltaVolSurface>,
}

#[wasm_bindgen(js_class = FxDeltaVolSurface)]
impl JsFxDeltaVolSurface {
    /// Construct an FX delta-quoted vol surface with 25-delta wings.
    ///
    /// Optional `rr10d` / `bf10d` add 10-delta wings for richer wing
    /// interpolation. Omit both (`undefined`/`null`) for a three-point smile;
    /// the Rust constructor rejects one without the other.
    ///
    /// # Arguments
    /// * `id`        - Stable surface identifier.
    /// * `expiries`  - Strictly increasing positive expiry times (years).
    /// * `atm_vols`  - ATM delta-neutral straddle vols per expiry.
    /// * `rr25d`     - 25-delta risk reversal per expiry (call vol − put vol).
    /// * `bf25d`     - 25-delta butterfly per expiry (wing avg − ATM).
    /// * `rr10d`     - Optional 10-delta risk reversal per expiry.
    /// * `bf10d`     - Optional 10-delta butterfly per expiry.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if `rr10d` and `bf10d` are not both present
    /// or both absent; quote arrays are empty or have mismatched lengths;
    /// expiries are not finite, positive, and strictly increasing; ATM vols are
    /// not finite and positive; or any risk reversal or butterfly is non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiries: JsValue,
        atm_vols: JsValue,
        rr25d: JsValue,
        bf25d: JsValue,
        rr10d: Option<JsValue>,
        bf10d: Option<JsValue>,
    ) -> Result<JsFxDeltaVolSurface, JsValue> {
        let expiries = js_f64_seq(&expiries, "expiries")?;
        let atm_vols = js_f64_seq(&atm_vols, "atmVols")?;
        let rr25d = js_f64_seq(&rr25d, "rr25d")?;
        let bf25d = js_f64_seq(&bf25d, "bf25d")?;
        let rr10d = js_opt_f64_seq(rr10d.as_ref(), "rr10d")?;
        let bf10d = js_opt_f64_seq(bf10d.as_ref(), "bf10d")?;
        let id: &str = &js_string(&id, "id")?;
        let surface =
            RustFxDeltaVolSurface::new(id, expiries, atm_vols, rr25d, bf25d, rr10d, bf10d)
                .map_err(to_js_err)?;
        Ok(Self {
            inner: Arc::new(surface),
        })
    }

    /// Deserialize canonical FX delta quotes without reconstructing positional arrays.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical FxDeltaVolSurface JSON with expiries in years and
    ///   annualized decimal ATM, risk-reversal, and butterfly quotes. Optional
    ///   10-delta wings must occur together; unknown fields are rejected.
    /// @returns A validated FxDeltaVolSurface handle owned by the caller; release it with free().
    /// @throws Error - Throws when JSON is malformed, fields are unknown, or native expiry, quote, or wing validation fails.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsFxDeltaVolSurface, JsValue> {
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
}

/// Implied-volatility surface on an expiry × strike (or expiry × tenor) grid.
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

/// Optional keys of the `VolSurface` constructor's options bag.
#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VolSurfaceOptions {
    secondary_axis: Option<String>,
    interpolation_mode: Option<String>,
    quote_type: Option<String>,
    displacements: Option<Vec<f64>>,
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
    /// * `strikes` - Secondary-axis coordinates (strikes, or swap tenors in
    ///   years), strictly increasing.
    /// * `vols` - Decimal volatilities as a flat row-major (expiry-major) grid
    ///   of `expiries.length * strikes.length` entries, finite and non-negative.
    /// * `options` - Optional `{ secondaryAxis, interpolationMode, quoteType,
    ///   displacements }`: `secondaryAxis` is `"strike"` or `"tenor"`,
    ///   `interpolationMode` is `"vol"` or `"total_variance"`, `quoteType` is
    ///   `"black_lognormal"`, `"shifted_black_lognormal"` or `"normal"`, and
    ///   `displacements` holds one shifted-Black displacement per expiry
    ///   (required for `"shifted_black_lognormal"` quotes). Every omitted key
    ///   takes the Rust default.
    ///
    /// @returns The validated `VolSurface`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an empty or unsorted axis, a
    /// grid of the wrong length, a negative or non-finite volatility, an
    /// unknown axis, mode or quote-type name, or displacements that do not
    /// match the quote convention and the expiry count.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiries: JsValue,
        strikes: JsValue,
        vols: JsValue,
        options: Option<JsValue>,
    ) -> Result<JsVolSurface, JsValue> {
        let id = js_string(&id, "id")?;
        let expiries = js_f64_seq(&expiries, "expiries")?;
        let strikes = js_f64_seq(&strikes, "strikes")?;
        let options = match options
            .as_ref()
            .filter(|value| !value.is_undefined() && !value.is_null())
        {
            Some(value) => from_js_json::<VolSurfaceOptions>(value, "options")?,
            None => VolSurfaceOptions::default(),
        };
        let mut opts = VolGridOpts::default();
        if let Some(axis) = options.secondary_axis.as_deref() {
            opts.secondary_axis = parse_label(axis)?;
        }
        if let Some(mode) = options.interpolation_mode.as_deref() {
            opts.interpolation_mode = parse_label(mode)?;
        }
        if let Some(quote) = options.quote_type.as_deref() {
            opts.quote_type = quote.parse().map_err(to_js_err)?;
        }
        let vols = js_f64_seq(&vols, "vols")?;
        let surface = RustVolSurface::from_grid(
            id,
            &expiries,
            &strikes,
            &vols,
            opts,
            options.displacements.as_deref(),
        );
        surface
            .map(|surface| Self::from_inner(Arc::new(surface)))
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
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsVolSurface, JsValue> {
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
    #[wasm_bindgen(getter, js_name = strikes)]
    pub fn strikes(&self) -> Box<[f64]> {
        self.inner.strikes().into()
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

    /// A shifted-Black copy of this surface carrying one displacement per
    /// expiry (Rust `VolSurface::with_displacements`).
    ///
    /// # Arguments
    ///
    /// * `displacements` - Finite additive shifts in strike/rate units, one
    ///   per expiry, in expiry-axis order.
    ///
    /// @returns A new validated `VolSurface` whose quote type is
    /// `"shifted_black_lognormal"`; this surface is unchanged.
    /// @throws `TypeError` (kind `invalid_type`) if `displacements` is not a
    /// numeric array; `FinstackError` (kind `validation`) if the count differs
    /// from the expiry count or a shift is non-finite.
    #[wasm_bindgen(js_name = withDisplacements)]
    pub fn with_displacements(&self, displacements: JsValue) -> Result<JsVolSurface, JsValue> {
        self.inner
            .as_ref()
            .clone()
            .with_displacements(&js_f64_seq(&displacements, "displacements")?)
            .map(|surface| Self::from_inner(Arc::new(surface)))
            .map_err(to_js_err)
    }

    /// Interpolation across the expiry axis: `"vol"` or `"total_variance"`.
    #[wasm_bindgen(getter, js_name = interpolationMode)]
    pub fn interpolation_mode(&self) -> Result<String, JsValue> {
        label(&self.inner.interpolation_mode())
    }

    /// Grid dimensions as `[expiryCount, strikeCount]`.
    #[wasm_bindgen(getter, js_name = gridShape)]
    pub fn grid_shape(&self) -> Box<[usize]> {
        let (expiries, strikes) = self.inner.grid_shape();
        Box::new([expiries, strikes])
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
    /// * `quote_type` - `"black_lognormal"`, `"shifted_black_lognormal"` or
    ///   `"normal"`; omitted uses `"black_lognormal"`.
    /// * `displacements` - Shifted-Black displacements in rate units, one per
    ///   tenor; required for shifted quotes and omitted otherwise.
    ///
    /// @returns The validated `VolCubeExpirySlice`.
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an invalid expiry or axis, a
    /// grid of the wrong length, an invalid volatility, an unknown quote type,
    /// or displacements that do not match the quote convention and tenor count.
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: JsValue,
        expiry: JsValue,
        tenors: JsValue,
        strikes: JsValue,
        vols: JsValue,
        quote_type: Option<JsValue>,
        displacements: Option<JsValue>,
    ) -> Result<JsVolCubeExpirySlice, JsValue> {
        let quote_type = js_opt_string(quote_type.as_ref(), "quoteType")?
            .map(|quote| quote.parse().map_err(to_js_err))
            .transpose()?
            .unwrap_or_default();
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
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsVolCubeExpirySlice, JsValue> {
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
