//! Typed `CdsIndex` and `CdsIndexBuilder` WASM bindings.

use crate::api::core::money::JsMoney;
use crate::api::valuations::pricing::parse_market;
use crate::utils::input::from_js_json;
use crate::utils::to_js_err;
use crate::utils::wire::js_date;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `CdsIndex` instrument (CDX / iTraxx style index swap).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const cdsIndex = valuations.instruments.CdsIndex.example();
    /// console.log(cdsIndex.id, cdsIndex.toDict());
    /// cdsIndex.free();
    /// ```
    JsCdsIndex,
    "CdsIndex",
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex
);

instrument_surface!(
    JsCdsIndex,
    "CdsIndex",
    JsCdsIndexBuilder,
    "generated.valuations.CdsIndex"
);

getters!(JsCdsIndex, "CdsIndex", |i| {
        /// Same-day premium and actual upfront cash election.
        /// @returns True when payments dated on the valuation date are still owed.
        include_valuation_date_cashflows as includeValuationDateCashflows: "boolean" => json(i.include_valuation_date_cashflows),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Ticker of the credit index family this contract references.
        /// @returns Index family ticker as supplied at construction, for example `"CDX.NA.IG"` or `"iTraxx Europe"`. The value is stored verbatim and is not normalised or validated against a registry.
        index_name as indexName: "generated.valuations.CdsIndex['index_name']" => json(i.index_name),
        /// Roll series of the credit index, incremented each semi-annual roll.
        /// @returns Series number as an unsigned integer (for example `41` for CDX.NA.IG series 41). Higher numbers denote more recent on-the-run rolls.
        series as series: "generated.valuations.CdsIndex['series']" => json(i.series),
        /// Version within the series.
        /// @returns The version number.
        version as version: "generated.valuations.CdsIndex['version']" => json(i.version),
        /// Traded notional of the index position, carrying its own currency.
        /// @returns Currency-tagged notional in the index deal currency (USD for CDX, EUR for iTraxx). It is the full original notional and is not scaled by the index factor; apply `index_factor` to obtain the current outstanding amount.
        notional as notional: "Money" => money(i.notional),
        /// Fraction of surviving notional.
        /// @returns `1.0` when no constituent has defaulted since inception.
        index_factor as indexFactor: "generated.valuations.CdsIndex['index_factor']" => json(i.index_factor),
        /// Protection perspective.
        /// @returns `"pay"` (buy protection) or `"receive"` (sell protection).
        side as side: "generated.valuations.CdsIndex['side']" => json(i.side),
        /// Regional ISDA convention (serde name).
        /// @returns `"isda_na"`, `"isda_eu"`, `"isda_as"` or `"custom"`.
        convention as convention: "generated.valuations.CdsIndex['convention']" => json(i.convention),
        /// Premium leg specification.
        /// @returns The premium leg.
        premium_leg as premiumLeg: "generated.valuations.CdsIndex['premium_leg']" => json(i.premium_leg),
        /// Protection leg specification.
        /// @returns The protection leg.
        protection_leg as protectionLeg: "generated.valuations.CdsIndex['protection_leg']" => json(i.protection_leg),
        /// Pricing aggregation mode.
        /// @returns `"single_curve"` or `"constituents"`.
        pricing as pricing: "generated.valuations.CdsIndex['pricing']" => json(i.pricing),
        /// Constituent rows.
        /// @returns Typed rows; empty in `single_curve` mode.
        constituents as constituents: "generated.valuations.CdsIndex['constituents']" => json(i.constituents),
        /// Number of names in the pool.
        /// @returns The count, or `null` when unset.
        num_constituents as numConstituents: "NonNullable<generated.valuations.CdsIndex['num_constituents']> | null" => json(i.num_constituents),
        /// Valuation presentation convention (serde name).
        /// @returns `"bloomberg_cdsw_clean"` (default), `"bloomberg_cdsw_clean_full_premium"`, `"isda_dirty"` or `"quant_lib_isda_parity"`.
        valuation_convention as valuationConvention: "NonNullable<generated.valuations.CdsIndex['valuation_convention']>" => json(i.valuation_convention),
        /// Contractual upfront payment as `(payment_date, amount)`; positive means the protection buyer pays.
        /// @returns The upfront pair, or `null` when the trade has no upfront.
        upfront as upfront: "[string, Money] | null" => dated_money(i.upfront),
        /// OTC margin specification in serde form.
        /// @returns The `OtcMarginSpec` plain object, or `null` for unmargined trades.
        margin_spec as marginSpec: "NonNullable<generated.valuations.CdsIndex['margin_spec']> | null" => json(i.margin_spec),
        /// Premium-leg end date as seen by the pricer.
        /// @returns The scheduled maturity, or `null`.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsCdsIndex, "CdsIndex");

factories!(JsCdsIndex, "CdsIndex", finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex, {
        /// Canonical CDX.NA.IG series 42 USD 10,000,000 payer (mirrors Rust `CdsIndex::example`): 60bp running coupon, `single_curve` pricing off `CDX.NA.IG.HAZARD` discounted on `USD-OIS`, premium 2024-03-20 to 2029-12-20, 125 names.
        /// @returns The example index trade.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsCdsIndex, "CdsIndex", {
        /// Par spread of the index in basis points (mirrors Rust `CdsIndex::par_spread`; risky-annuity denominator in `single_curve` mode, weighted constituents otherwise).
        /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param asOf - ISO-8601 valuation date.
        /// @returns Par spread in basis points.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        par_spread as parSpread,
        /// Risky PV01 (risky annuity) of the premium leg (mirrors Rust `CdsIndex::risky_pv01`): PV of 1bp running on the surviving notional.
        /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param asOf - ISO-8601 valuation date.
        /// @returns Risky PV01 in notional currency units per basis point.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        risky_pv01 as riskyPv01,
});

builder_class!(
    /// Fluent builder for `CdsIndex`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.CdsIndex.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsCdsIndexBuilder,
    "CdsIndexBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexBuilder,
    JsCdsIndex,
    |b| b.build()
);

setters!(JsCdsIndexBuilder, "CdsIndexBuilder", {
        /// Elect whether to include payments due on the valuation date.
        /// @param value - True when same-day premium and actual upfront are still owed.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` for non-boolean inputs or `validation` once the builder is consumed.
        include_valuation_date_cashflows as includeValuationDateCashflows: "boolean" => js_bool,
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the index trade.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the index name.
        /// @param value - Index name, e.g. `"CDX.NA.IG"`, `"CDX.NA.HY"`, `"iTraxx Europe"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_name as indexName: "string" => id,
        /// Set the series number.
        /// @param value - Series number, e.g. `42`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        series as series: "number" => js_uint,
        /// Set the version number within the series.
        /// @param value - Version number, e.g. `1`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        version as version: "number" => js_uint,
        /// Set the index factor (fraction of surviving notional).
        /// @param value - Index factor in `[0.0, 1.0]`; `1.0` means no constituent has defaulted since series inception.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        index_factor as indexFactor: "number" => js_f64,
        /// Set the protection buyer/seller perspective.
        /// @param value - `"pay"` to buy protection (pay premium), `"receive"` to sell protection (receive premium).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side: "generated.valuations.CdsIndex['side']" => js_wire,
        /// Set the ISDA regional convention.
        /// @param value - `"isda_na"` is the SNAC / post-Big-Bang North American standard; `"isda_eu"` European; `"isda_as"` Asian; `"custom"` for a manually configured convention.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        convention as convention: "generated.valuations.CdsIndex['convention']" => js_wire,
        /// Set the premium leg specification.
        /// @param value - Premium leg specification (coupon schedule and discounting).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        premium_leg as premiumLeg: "generated.valuations.CdsIndex['premium_leg']" => from_js_json,
        /// Set the protection leg specification.
        /// @param value - Protection leg specification (credit curve and settlement).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_leg as protectionLeg: "generated.valuations.CdsIndex['protection_leg']" => from_js_json,
        /// Set the pricing aggregation mode.
        /// @param value - `"single_curve"` prices the index against a single index hazard curve (synthetic CDS). `"constituents"` prices each issuer separately and aggregates by weight; requires `CdsIndexBuilder.constituents` to be set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        pricing as pricing: "generated.valuations.CdsIndex['pricing']" => js_wire,
        /// Set the index constituents.
        /// @param value - Constituent rows as typed `CdsIndexConstituent` objects, plain objects with `credit` (`reference_entity`, `recovery_rate`, `credit_curve_id`), `weight` and optional `defaulted`, or a JSON array of the same shape.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        constituents as constituents: "generated.valuations.CdsIndex['constituents']" => from_js_json,
        /// Set the number of reference entities in the index pool.
        /// @param value - Number of names in the index pool, e.g. `125` for CDX.NA.IG; required for portfolio-level analytics (e.g. jump-to-default) when `constituents` is empty.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        num_constituents as numConstituents: "number" => js_uint,
        /// Set the valuation presentation convention.
        /// @param value - `"bloomberg_cdsw_clean"` (default) reports Bloomberg CDSW clean principal; `"isda_dirty"` the academic ISDA dirty PV; `"quant_lib_isda_parity"` reproduces QuantLib `IsdaCdsEngine`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        valuation_convention as valuationConvention: "NonNullable<generated.valuations.CdsIndex['valuation_convention']>" => js_wire,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec: "NonNullable<generated.valuations.CdsIndex['margin_spec']>" => from_js_json,
});

common_setters!(JsCdsIndexBuilder, "CdsIndexBuilder");

handle_setters!(JsCdsIndexBuilder, "CdsIndexBuilder", {
        /// Set the notional amount of the index.
        /// @param value - Notional amount of the index, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

#[wasm_bindgen(js_class = CdsIndex)]
impl JsCdsIndex {
    /// Create an index swap from a standard series preset.
    ///
    /// Mirrors Rust `CdsIndex::from_preset`: the preset supplies the index
    /// name, series, version, running coupon and regional ISDA convention;
    /// `params` supplies the trade terms.
    /// @param preset - `CdsIndexParams` preset (plain object or JSON text), e.g. from `valuations.instruments.cdsIndexParamsCdxNaIg`.
    /// @param params - `CdsTradeParams` (plain object or JSON text), every key required: `id`; `notional` - Money wire object `{ amount: "10000000", currency: "USD" }` (decimal string amount) on which both legs are computed; `side` - `"pay"` (buy protection) or `"receive"`; `start` / `end` - ISO-8601 effective and maturity dates; `recovery_rate` - decimal fraction of par (`0.4` = 40%); `discount_curve_id`; `credit_curve_id` - index hazard curve.
    /// @returns The validated index swap.
    /// @throws Error - Throws with kind `validation` if `preset` or `params` is malformed or the instrument fails validation, and kind `invalid_type` if an argument is not an object or JSON text.
    #[wasm_bindgen(js_name = fromPreset)]
    pub fn from_preset(
        #[wasm_bindgen(unchecked_param_type = "generated.valuations.CdsIndexParams | JsonInput")]
        preset: JsValue,
        #[wasm_bindgen(unchecked_param_type = "generated.valuations.CdsTradeParams | JsonInput")]
        params: JsValue,
    ) -> Result<JsCdsIndex, JsValue> {
        let preset: finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndexParams =
            from_js_json(&preset, "preset")?;
        let params: finstack_quant_valuations::instruments::credit_derivatives::cds::CdsTradeParams =
            from_js_json(&params, "params")?;
        finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex::from_preset(
            &preset, params,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Credit spread sensitivity of the index (mirrors Rust `CdsIndex::cs01`).
    ///
    /// Bumps the index par spread by one basis point, recalibrates the hazard
    /// curve and reprices.
    /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the discount and index hazard curves.
    /// @param asOf - ISO-8601 valuation date.
    /// @returns Change in present value for a one basis point spread widening, in the notional currency.
    /// @throws Error - Throws with kind `not_found` if a curve is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if recalibration or pricing fails.
    pub fn cs01(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    ) -> Result<f64, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        let provider =
            finstack_quant_calibration::recalibration::CachedRecalibrationProvider::default();
        self.inner
            .cs01(&market, as_of, &provider)
            .map_err(to_js_err)
    }
}

upfront_setter!(JsCdsIndexBuilder, "CdsIndexBuilder");
