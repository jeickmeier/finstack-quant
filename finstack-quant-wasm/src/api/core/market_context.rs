//! WASM handle for `finstack_quant_core::market_data::context::MarketContext`.
//!
//! Parse a market context once with `MarketContext.fromJson`, then pass the
//! handle to every `*WithMarket` entry point instead of re-parsing the same
//! JSON on each pricing or sensitivity call.

use crate::api::core::currency::JsCurrency;
use crate::api::core::market_curves::{
    JsBaseCorrelationCurve, JsCreditIndexData, JsInflationCurve, JsPriceCurve,
};
use crate::api::core::market_data::{JsDiscountCurve, JsForwardCurve, JsFxMatrix, JsHazardCurve};
use crate::api::core::market_scalars::{JsInflationIndex, JsScalarTimeSeries};
use crate::api::core::money::JsMoney;
use crate::api::core::surfaces::{JsFxDeltaVolSurface, JsVolCube, JsVolSurface};
use crate::utils::input::{js_f64, js_int, js_opt_string, js_string, json_text};
use crate::utils::{parse_iso_date, to_js_err, to_js_value};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Market data container: curves, surfaces, prices, series and FX for one
/// valuation date.
///
/// Build one with `new MarketContext()` plus the `insert*` methods, or parse
/// a persisted snapshot with `MarketContext.fromJson`; then pass the handle
/// to `priceInstrumentWithMarket` and the other `*WithMarket` entry points so
/// the market is not re-parsed on every pricing call. The `insert*` methods
/// change the context in place and return nothing.
///
/// @example
/// ```typescript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const market = new core.MarketContext();
/// market.insert(core.DiscountCurve.flat("USD-OIS", "2025-01-02", 0.04));
/// market.insertPrice("SPX", 5900.0);
/// market.curveIds(); // ["USD-OIS"]
/// market.getDiscount("USD-OIS").df(1.0); // exp(-0.04)
/// const copy = core.MarketContext.fromJson(market.toJson());
/// copy.contains("USD-OIS"); // true
/// ```
#[wasm_bindgen(js_name = MarketContext)]
#[derive(Clone, Default)]
pub struct JsMarketContext {
    inner: Arc<MarketContext>,
}

impl JsMarketContext {
    /// Access the inner MarketContext (crate-internal).
    pub(crate) fn inner(&self) -> &MarketContext {
        self.inner.as_ref()
    }

    /// Mutable access; copies the context first if a pricing call still shares it.
    fn context_mut(&mut self) -> &mut MarketContext {
        Arc::make_mut(&mut self.inner)
    }
}

#[wasm_bindgen(js_class = MarketContext)]
impl JsMarketContext {
    /// Create an empty market context.
    ///
    /// @returns A `MarketContext` with no curves, surfaces, prices, series or FX.
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsMarketContext {
        Self::default()
    }

    /// Parse a market context from its canonical JSON representation.
    ///
    /// # Arguments
    ///
    /// * `json` - Canonical MarketContext JSON (string or plain object), the
    ///   same payload accepted by pricing `marketJson` arguments. Unknown fields
    ///   are rejected.
    /// @returns A `MarketContext` handle that can be reused across pricing calls; release it with free().
    /// @throws Error - Throws with kind `validation` when the JSON is malformed or does not match the MarketContext schema, and a `TypeError` when `json` is neither a string nor a plain object.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: JsValue) -> Result<JsMarketContext, JsValue> {
        let json: &str = &json_text(&json, "json")?;
        let inner: MarketContext = serde_json::from_str(json).map_err(to_js_err)?;
        Ok(JsMarketContext {
            inner: Arc::new(inner),
        })
    }

    /// Serialize the wrapped MarketContext back to canonical JSON.
    ///
    /// @returns Canonical MarketContext JSON accepted by `MarketContext.fromJson` and every `marketJson` argument.
    /// @throws Error - Throws if the market context cannot be serialized to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(to_js_err)
    }

    /// Store a discount curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `DiscountCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertDiscountCurve)]
    pub fn insert_discount_curve(&mut self, curve: &JsDiscountCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a forward curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `ForwardCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertForwardCurve)]
    pub fn insert_forward_curve(&mut self, curve: &JsForwardCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a hazard curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `HazardCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertHazardCurve)]
    pub fn insert_hazard_curve(&mut self, curve: &JsHazardCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a inflation curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `InflationCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertInflationCurve)]
    pub fn insert_inflation_curve(&mut self, curve: &JsInflationCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a price or volatility-index curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `PriceCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertPriceCurve)]
    pub fn insert_price_curve(&mut self, curve: &JsPriceCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a base-correlation curve under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `BaseCorrelationCurve` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertBaseCorrelationCurve)]
    pub fn insert_base_correlation_curve(&mut self, curve: &JsBaseCorrelationCurve) {
        self.context_mut().insert_mut(Arc::clone(&curve.inner));
    }

    /// Store a volatility surface under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `VolSurface` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertVolSurface)]
    pub fn insert_vol_surface(&mut self, curve: &JsVolSurface) {
        self.context_mut()
            .insert_surface_mut(Arc::clone(&curve.inner));
    }

    /// Store a FX delta-quoted volatility surface under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `FxDeltaVolSurface` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertFxDeltaVolSurface)]
    pub fn insert_fx_delta_vol_surface(&mut self, curve: &JsFxDeltaVolSurface) {
        self.context_mut()
            .insert_fx_delta_vol_surface_mut(Arc::clone(&curve.inner));
    }

    /// Store a SABR volatility cube under its own id; `insert` dispatches here.
    ///
    /// # Arguments
    ///
    /// * `curve` - The `VolCube` to store; an item with the same id is replaced.
    #[wasm_bindgen(js_name = insertVolCube)]
    pub fn insert_vol_cube(&mut self, curve: &JsVolCube) {
        self.context_mut()
            .insert_vol_cube_mut(Arc::clone(&curve.inner));
    }

    /// Attach the FX matrix used for currency conversion (Rust `MarketContext::insert_fx_mut`).
    ///
    /// # Arguments
    ///
    /// * `fx` - FX matrix to attach; it replaces any matrix already attached
    ///   and is shared, so later `setQuote` calls on it are visible here.
    #[wasm_bindgen(js_name = insertFx)]
    pub fn insert_fx(&mut self, fx: &JsFxMatrix) {
        self.context_mut().insert_fx_mut(Arc::clone(&fx.inner));
    }

    /// Store a market scalar: a unitless number, or a price in a currency
    /// (Rust `MarketContext::insert_price_mut`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier to store the scalar under.
    /// * `value` - Finite scalar value: an index level, a spot price, a
    ///   recovery assumption and so on.
    /// * `currency` - ISO-4217 code that makes the scalar a monetary price;
    ///   omitted stores a unitless number.
    ///
    /// @throws `TypeError` (kind `invalid_type`) for a mistyped argument;
    /// `FinstackError` (kind `validation`) for an unknown currency or a
    /// monetary value that is not finite.
    #[wasm_bindgen(js_name = insertPrice)]
    pub fn insert_price(
        &mut self,
        id: JsValue,
        value: JsValue,
        currency: Option<JsValue>,
    ) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        let value = js_f64(&value, "value")?;
        let scalar = match js_opt_string(currency.as_ref(), "currency")? {
            Some(code) => {
                let currency: Currency = code.parse().map_err(to_js_err)?;
                MarketScalar::Price(Money::new(value, currency).map_err(to_js_err)?)
            }
            None => MarketScalar::Unitless(value),
        };
        self.context_mut().insert_price_mut(id, scalar);
        Ok(())
    }

    /// Store credit-index market data (Rust `MarketContext::insert_credit_index_mut`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier of the index, such as `"CDX-IG-43"`.
    /// * `data` - Constituent count, recovery, index hazard curve and
    ///   base-correlation curve of the index.
    ///
    /// @throws `TypeError` if `id` is not a string.
    #[wasm_bindgen(js_name = insertCreditIndex)]
    pub fn insert_credit_index(
        &mut self,
        id: JsValue,
        data: &JsCreditIndexData,
    ) -> Result<(), JsValue> {
        let id = js_string(&id, "id")?;
        self.context_mut()
            .insert_credit_index_mut(id, (*data.inner).clone());
        Ok(())
    }

    /// Store a scalar time series under its own id (Rust `MarketContext::insert_series_mut`).
    ///
    /// # Arguments
    ///
    /// * `series` - Series to store; one with the same id is replaced.
    #[wasm_bindgen(js_name = insertSeries)]
    pub fn insert_series(&mut self, series: &JsScalarTimeSeries) {
        self.context_mut().insert_series_mut(series.inner.clone());
    }

    /// Store an inflation index under its own id (Rust
    /// `MarketContext::insert_inflation_index_mut`).
    ///
    /// # Arguments
    ///
    /// * `index` - Inflation index to store; one with the same id is replaced.
    #[wasm_bindgen(js_name = insertInflationIndex)]
    pub fn insert_inflation_index(&mut self, index: &JsInflationIndex) {
        let id = index.inner.id.clone();
        self.context_mut()
            .insert_inflation_index_mut(id, Arc::clone(&index.inner));
    }

    /// Map a CSA code to the discount curve used for collateralised trades
    /// (Rust `MarketContext::map_collateral_mut`).
    ///
    /// # Arguments
    ///
    /// * `csa_code` - Credit-support-annex code, such as `"USD-CSA"`.
    /// * `discount_id` - Identifier of the discount curve to use for that
    ///   CSA; it is resolved when a collateral curve is requested.
    ///
    /// @throws `TypeError` if an argument is not a string.
    #[wasm_bindgen(js_name = mapCollateral)]
    pub fn map_collateral(
        &mut self,
        csa_code: JsValue,
        discount_id: JsValue,
    ) -> Result<(), JsValue> {
        let csa_code = js_string(&csa_code, "csaCode")?;
        let discount_id = CurveId::from(js_string(&discount_id, "discountId")?);
        self.context_mut().map_collateral_mut(csa_code, discount_id);
        Ok(())
    }

    /// Look up a discount curve by id (Rust `MarketContext::get_discount`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `DiscountCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getDiscount)]
    pub fn get_discount(&self, id: JsValue) -> Result<JsDiscountCurve, JsValue> {
        self.inner
            .get_discount(js_string(&id, "id")?)
            .map(|inner| JsDiscountCurve { inner })
            .map_err(to_js_err)
    }

    /// Look up a forward curve by id (Rust `MarketContext::get_forward`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `ForwardCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getForward)]
    pub fn get_forward(&self, id: JsValue) -> Result<JsForwardCurve, JsValue> {
        self.inner
            .get_forward(js_string(&id, "id")?)
            .map(|inner| JsForwardCurve { inner })
            .map_err(to_js_err)
    }

    /// Look up a hazard curve by id (Rust `MarketContext::get_hazard`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `HazardCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getHazard)]
    pub fn get_hazard(&self, id: JsValue) -> Result<JsHazardCurve, JsValue> {
        self.inner
            .get_hazard(js_string(&id, "id")?)
            .map(|inner| JsHazardCurve { inner })
            .map_err(to_js_err)
    }

    /// Look up a base-correlation curve by id (Rust `MarketContext::get_base_correlation`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `BaseCorrelationCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getBaseCorrelation)]
    pub fn get_base_correlation(&self, id: JsValue) -> Result<JsBaseCorrelationCurve, JsValue> {
        self.inner
            .get_base_correlation(js_string(&id, "id")?)
            .map(JsBaseCorrelationCurve::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a inflation curve by id (Rust `MarketContext::get_inflation_curve`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `InflationCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getInflationCurve)]
    pub fn get_inflation_curve(&self, id: JsValue) -> Result<JsInflationCurve, JsValue> {
        self.inner
            .get_inflation_curve(js_string(&id, "id")?)
            .map(JsInflationCurve::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a price curve (kind `"price"`) by id (Rust `MarketContext::get_price_curve`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `PriceCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getPriceCurve)]
    pub fn get_price_curve(&self, id: JsValue) -> Result<JsPriceCurve, JsValue> {
        self.inner
            .get_price_curve(js_string(&id, "id")?)
            .map(JsPriceCurve::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a volatility-index curve (a `PriceCurve` of kind `"vol_index"`) by id (Rust `MarketContext::get_vol_index_curve`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `PriceCurve`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getVolIndexCurve)]
    pub fn get_vol_index_curve(&self, id: JsValue) -> Result<JsPriceCurve, JsValue> {
        self.inner
            .get_vol_index_curve(js_string(&id, "id")?)
            .map(JsPriceCurve::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a inflation index by id (Rust `MarketContext::get_inflation_index`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `InflationIndex`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getInflationIndex)]
    pub fn get_inflation_index(&self, id: JsValue) -> Result<JsInflationIndex, JsValue> {
        self.inner
            .get_inflation_index(js_string(&id, "id")?)
            .map(JsInflationIndex::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a volatility surface by id (Rust `MarketContext::get_surface`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `VolSurface`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getSurface)]
    pub fn get_surface(&self, id: JsValue) -> Result<JsVolSurface, JsValue> {
        self.inner
            .get_surface(js_string(&id, "id")?)
            .map(JsVolSurface::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a FX delta-quoted volatility surface by id (Rust `MarketContext::get_fx_delta_vol_surface`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `FxDeltaVolSurface`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getFxDeltaVolSurface)]
    pub fn get_fx_delta_vol_surface(&self, id: JsValue) -> Result<JsFxDeltaVolSurface, JsValue> {
        self.inner
            .get_fx_delta_vol_surface(js_string(&id, "id")?)
            .map(|inner| JsFxDeltaVolSurface { inner })
            .map_err(to_js_err)
    }

    /// Look up a SABR volatility cube by id (Rust `MarketContext::get_vol_cube`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `VolCube`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getVolCube)]
    pub fn get_vol_cube(&self, id: JsValue) -> Result<JsVolCube, JsValue> {
        self.inner
            .get_vol_cube(js_string(&id, "id")?)
            .map(|inner| JsVolCube { inner })
            .map_err(to_js_err)
    }

    /// Look up a credit-index data by id (Rust `MarketContext::get_credit_index`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the item was inserted under.
    ///
    /// @returns The stored `CreditIndexData`; it shares the context's data.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no such item is stored under `id`.
    #[wasm_bindgen(js_name = getCreditIndex)]
    pub fn get_credit_index(&self, id: JsValue) -> Result<JsCreditIndexData, JsValue> {
        self.inner
            .get_credit_index(js_string(&id, "id")?)
            .map(JsCreditIndexData::from_inner)
            .map_err(to_js_err)
    }

    /// Look up a market scalar by id (Rust `MarketContext::get_price`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the scalar was inserted under.
    ///
    /// @returns A plain `MarketScalar` object: `{ unitless: number }`, or
    /// `{ price: { amount, currency } }` for a monetary price.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no scalar is stored under `id`.
    #[wasm_bindgen(js_name = getPrice)]
    pub fn get_price(&self, id: JsValue) -> Result<JsValue, JsValue> {
        let scalar = self
            .inner
            .get_price(js_string(&id, "id")?)
            .map_err(to_js_err)?;
        to_js_value(scalar)
    }

    /// Look up a scalar time series by id (Rust `MarketContext::get_series`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier the series was inserted under.
    ///
    /// @returns A copy of the stored `ScalarTimeSeries`.
    /// @throws `TypeError` if `id` is not a string; `FinstackError` (kind
    /// `not_found`) if no series is stored under `id`.
    #[wasm_bindgen(js_name = getSeries)]
    pub fn get_series(&self, id: JsValue) -> Result<JsScalarTimeSeries, JsValue> {
        self.inner
            .get_series(js_string(&id, "id")?)
            .map(|series| JsScalarTimeSeries {
                inner: series.clone(),
            })
            .map_err(to_js_err)
    }

    /// The attached FX matrix, or `undefined` when none is attached.
    #[wasm_bindgen(getter, js_name = fx)]
    pub fn fx(&self) -> Option<JsFxMatrix> {
        self.inner.fx().map(|fx| JsFxMatrix {
            inner: Arc::clone(fx),
        })
    }

    /// The attached FX matrix, required (Rust `MarketContext::fx_required`).
    ///
    /// @returns The attached `FxMatrix`; it shares the context's quotes.
    /// @throws `FinstackError` (kind `not_found`) if no FX matrix is attached.
    #[wasm_bindgen(js_name = fxRequired)]
    pub fn fx_required(&self) -> Result<JsFxMatrix, JsValue> {
        self.inner
            .fx_required()
            .map(|fx| JsFxMatrix {
                inner: Arc::clone(fx),
            })
            .map_err(to_js_err)
    }

    /// Convert an amount into another currency with the attached FX matrix
    /// (Rust `MarketContext::convert_money`).
    ///
    /// # Arguments
    ///
    /// * `amount` - Monetary amount to convert.
    /// * `target_currency` - Destination `Currency` object; an amount already
    ///   in it is returned unchanged.
    /// * `as_of` - ISO-8601 date of the FX rate lookup.
    ///
    /// @returns The converted `Money` in `targetCurrency`.
    /// @throws `TypeError` for a mistyped argument; `FinstackError` (kind
    /// `not_found`) if no FX matrix is attached or the pair has no rate, or
    /// kind `validation` for a malformed date.
    #[wasm_bindgen(js_name = convertMoney)]
    pub fn convert_money(
        &self,
        amount: &JsMoney,
        target_currency: &JsCurrency,
        as_of: JsValue,
    ) -> Result<JsMoney, JsValue> {
        let as_of = parse_iso_date(&js_string(&as_of, "asOf")?)?;
        self.inner
            .convert_money(amount.inner, target_currency.inner, as_of)
            .map(|inner| JsMoney { inner })
            .map_err(to_js_err)
    }

    /// Whether any market data is stored under an id (Rust `MarketContext::contains`).
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier to look for.
    ///
    /// @returns `true` when a curve, surface, price, series, index, dividend
    /// schedule or collateral mapping is registered under `id`.
    /// @throws `TypeError` if `id` is not a string.
    #[wasm_bindgen(js_name = contains)]
    pub fn contains(&self, id: JsValue) -> Result<bool, JsValue> {
        Ok(self.inner.contains(js_string(&id, "id")?))
    }

    /// Identifiers of every stored curve.
    ///
    /// @returns Curve ids in sorted order; surfaces, prices and series are not listed.
    #[wasm_bindgen(js_name = curveIds)]
    pub fn curve_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .inner
            .curve_ids()
            .map(|id| id.as_str().to_owned())
            .collect();
        ids.sort();
        ids
    }

    /// Whether the context holds no market data at all.
    ///
    /// @returns `true` for a context with nothing inserted.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Counts of the stored market data (Rust `MarketContext::stats`).
    ///
    /// @returns A plain object with `curve_counts` (count per curve type),
    /// `total_curves`, `has_fx`, `surface_count`, `vol_cube_count`,
    /// `price_count`, `series_count`, `inflation_index_count`,
    /// `credit_index_count`, `dividend_schedule_count`,
    /// `fx_delta_vol_surface_count` and `collateral_mapping_count`.
    /// @throws If the counts cannot be converted (not expected).
    #[wasm_bindgen(js_name = stats)]
    pub fn stats(&self) -> Result<JsValue, JsValue> {
        to_js_value(&self.inner.stats())
    }

    /// A copy of the context with every curve rolled forward in time (Rust
    /// `MarketContext::roll_forward`).
    ///
    /// # Arguments
    ///
    /// * `days` - Calendar days to advance each curve's base date; the curves
    ///   keep their shape and drop expired pillars.
    ///
    /// @returns A new `MarketContext`; this context is unchanged.
    /// @throws `TypeError` if `days` is not an integer; `FinstackError` if a
    /// curve cannot be rolled (for example no pillar remains).
    #[wasm_bindgen(js_name = rollForward)]
    pub fn roll_forward(&self, days: JsValue) -> Result<JsMarketContext, JsValue> {
        let days: i64 = js_int(&days, "days")?;
        self.inner
            .roll_forward(days)
            .map(|context| JsMarketContext {
                inner: Arc::new(context),
            })
            .map_err(to_js_err)
    }
}

impl JsMarketContext {
    /// Wrap a Rust `MarketContext` produced by a binding (crate-internal).
    pub(crate) fn from_inner(inner: MarketContext) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}
