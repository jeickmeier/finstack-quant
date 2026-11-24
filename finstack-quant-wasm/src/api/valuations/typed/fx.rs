//! Typed FX instrument classes and their fluent builders.
//!
//! Every FX class exposes `fromJson` / `toJson`, `price`, `metric` and
//! Greeks; `FxForward` and `FxOption` add one getter per public Rust field,
//! the `Instrument` trait surface, Rust presets and the `FinancialBuilder`
//! setters. A bare spec becomes a class through
//! `fromJson(instrumentEnvelopeFromSpec(type, spec))`.
//!
//! # Monte-Carlo determinism
//!
//! The `price` method accepts Monte-Carlo models for
//! path-dependent FX products (e.g. barrier / touch options). As with the
//! generic `priceInstrument` bindings, no explicit RNG-seed parameter is
//! exposed: the seed is an instrument-level concern. When an instrument's
//! `instrument_pricing_overrides.model_config.mc_seed_scenario` is `None`, the core MC pricers
//! derive a stable seed deterministically from the instrument ID, so repricing
//! the same instrument JSON is bit-reproducible. Callers needing a distinct
//! deterministic stream set that label inside the instrument JSON.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use crate::api::valuations::pricing::parse_market;
use crate::utils::input::{from_js_json, js_f64};
use crate::utils::to_js_err;
use crate::utils::wire::js_date;
use finstack_quant_valuations::instruments;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxForward.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const forward = valuations.instruments.FxForward.example().withForwardPoints(1.1, 0.002);
    /// console.log(forward.id);
    /// forward.free();
    /// ```
    JsFxForward,
    "FxForward",
    instruments::fx::fx_forward::FxForward
);
instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxOption.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const option = valuations.instruments.FxOption.example();
    /// console.log(option.id, JSON.parse(option.toJson()).instrument.type);
    /// option.free();
    /// ```
    JsFxOption,
    "FxOption",
    instruments::fx::fx_option::FxOption
);
option_greeks_bundle!(JsFxOption, "FxOption");

#[wasm_bindgen(js_class = FxForward)]
impl JsFxForward {
    /// Build an at-market forward from a trade date and a standard FX tenor.
    ///
    /// Mirrors Rust `FxForward::from_trade_date`: spot is `trade_date` plus
    /// `settlement_days` business days on the joint calendar, then the
    /// maturity is `tenor` after spot with the stated business-day convention
    /// and business-EOM choice.
    /// @param params - `FxForwardTradeDateParams` (plain object or JSON text), every key required: `id`; `base_currency` - base (foreign) ISO code, the notional currency; `quote_currency` - quote (domestic) ISO code; `trade_date` - ISO-8601 date; `tenor` - tenor object from spot (e.g. `{ count: 3, unit: "months" }`); `notional` - Money wire object `{ amount: "1000000", currency: "EUR" }` (decimal string amount) in `base_currency`; `domestic_discount_curve_id` / `foreign_discount_curve_id` - quote- and base-currency curves; `base_calendar_id` / `quote_calendar_id` - calendar ids whose joint calendar counts the spot lag and rolls the maturity (a composite such as `"target2+usny"` adds a USD gate); `settlement_days` - spot lag in business days; `business_day_convention` - serde name of the maturity roll; `preserve_business_eom` - keep a business month-end spot on the business month-end of the maturity month (month and year tenors only).
    /// @returns The validated forward without a contract rate; chain `withForwardPoints` / `withForwardPips` to fix it.
    /// @throws Error - Throws with kind `validation` if `params` is malformed, the currencies coincide, or the notional currency differs from `base_currency`; kind `not_found` if a calendar identifier is unknown; kind `invalid_type` if `params` is not an object or JSON text.
    #[wasm_bindgen(js_name = fromTradeDate)]
    pub fn from_trade_date(
        #[wasm_bindgen(
            unchecked_param_type = "generated.valuations.FxForwardTradeDateParams | JsonInput"
        )]
        params: JsValue,
    ) -> Result<JsFxForward, JsValue> {
        let params: instruments::fx::fx_forward::FxForwardTradeDateParams =
            from_js_json(&params, "params")?;
        instruments::fx::fx_forward::FxForward::from_trade_date(params)
            .map(|inner| JsFxForward { inner })
            .map_err(to_js_err)
    }

    /// Return a copy whose contract rate is `spotRate + forwardPoints`.
    ///
    /// Mirrors Rust `FxForward::with_forward_points`; forward points are in
    /// rate units (`0.0025` is 25 pips on EUR/USD). The receiver is unchanged.
    /// @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
    /// @param forwardPoints - Forward points in rate units, added to `spotRate`.
    /// @returns A new forward with the contract rate set.
    /// @throws Error - Throws with kind `validation` if `spotRate` is not positive and finite or the resulting rate is not positive.
    #[wasm_bindgen(js_name = withForwardPoints)]
    pub fn with_forward_points(
        &self,
        #[wasm_bindgen(js_name = "spotRate", unchecked_param_type = "number")] spot_rate: JsValue,
        #[wasm_bindgen(js_name = "forwardPoints", unchecked_param_type = "number")] forward_points: JsValue,
    ) -> Result<JsFxForward, JsValue> {
        self.inner
            .clone()
            .with_forward_points(
                js_f64(&spot_rate, "spotRate")?,
                js_f64(&forward_points, "forwardPoints")?,
            )
            .map(|inner| JsFxForward { inner })
            .map_err(to_js_err)
    }

    /// Return a copy whose contract rate is `spotRate + pips * pipSize`.
    ///
    /// Mirrors Rust `FxForward::with_forward_pips`; the pip size comes from
    /// the explicit `convention`, which must name this forward's ordered pair.
    /// @param spotRate - Spot rate, quote currency per unit of base currency; must be positive.
    /// @param pips - Forward points quoted in pips.
    /// @param convention - Quote metadata of this forward's base/quote order, carrying the pip size.
    /// @returns A new forward with the contract rate set.
    /// @throws Error - Throws with kind `validation` if `convention` names another pair or order, `pips` or `spotRate` is not finite, or the resulting rate is not positive.
    #[wasm_bindgen(js_name = withForwardPips)]
    pub fn with_forward_pips(
        &self,
        #[wasm_bindgen(js_name = "spotRate", unchecked_param_type = "number")] spot_rate: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] pips: JsValue,
        convention: &crate::api::core::market_fx::JsFxPairConvention,
    ) -> Result<JsFxForward, JsValue> {
        self.inner
            .clone()
            .with_forward_pips(
                js_f64(&spot_rate, "spotRate")?,
                js_f64(&pips, "pips")?,
                &convention.inner,
            )
            .map(|inner| JsFxForward { inner })
            .map_err(to_js_err)
    }
}

market_metrics!(JsFxForward, "FxForward", {
        /// Covered-interest-parity forward rate implied by the market.
        ///
        /// Mirrors Rust `FxForward::market_forward_rate`:
        /// `F = S × DF_foreign(T) / DF_domestic(T)` at the forward's maturity.
        /// @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot (or an explicit `quoted_spot` on the instrument).
        /// @param asOf - Valuation date as an ISO-8601 string.
        /// @returns Forward rate, quote currency per unit of base currency.
        /// @throws Error - Throws with kind `not_found` if a discount curve or the FX spot is missing, and kind `validation` if the market JSON or `asOf` is malformed.
        market_forward_rate as marketForwardRate,
});

factories!(JsFxForward, "FxForward", instruments::fx::fx_forward::FxForward, {
        /// Canonical example forward (mirrors Rust `FxForward::example`).
        ///
        /// A 6-month EUR/USD forward: EUR 1,000,000 at contract rate 1.12,
        /// maturity 2025-06-15, curves `USD-OIS` / `EUR-OIS`.
        /// @returns The example forward.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

#[wasm_bindgen(js_class = FxOption)]
impl JsFxOption {
    /// Build a European FX option with currency-derived OIS curves.
    ///
    /// Mirrors Rust `FxOption::european`: discount curves are `<QUOTE>-OIS`
    /// (domestic) and `<BASE>-OIS` (foreign) and the model day count is ACT/365F.
    /// @param params - `FxOptionParams` (plain object or JSON text), every key required: `id`; `base_currency` - base (foreign) ISO code, the notional currency; `quote_currency` - quote (domestic) ISO code; `strike` - quote currency per unit of base currency; `expiry` - ISO-8601 date; `notional` - Money wire object `{ amount: "1000000", currency: "EUR" }` (decimal string amount) in `base_currency`; `vol_surface_id`; `option_type` - `"call"` or `"put"` on the base currency; `spot_basis` - cash spot or a booked outright value date; `delivery` - cash at expiry or a physical exchange on a fixed date with an explicit long or short position; `delta_convention` - `{ kind, premium_currency, venue }` with `kind` one of `"spot"`, `"forward"`, `"premium_adjusted_spot"`, `"premium_adjusted_forward"`, `premium_currency` the base or quote ISO code and `venue` a non-empty quoting-source identifier.
    /// @returns The validated option.
    /// @throws Error - Throws with kind `validation` if `params` is malformed, the currencies coincide, the premium currency is neither leg, the venue is blank, or the notional is not positive; kind `invalid_type` if `params` is not an object or JSON text.
    pub fn european(
        #[wasm_bindgen(unchecked_param_type = "generated.valuations.FxOptionParams | JsonInput")]
        params: JsValue,
    ) -> Result<JsFxOption, JsValue> {
        let params: instruments::fx::fx_option::FxOptionParams = from_js_json(&params, "params")?;
        instruments::fx::fx_option::FxOption::european(params)
            .map(|inner| JsFxOption { inner })
            .map_err(to_js_err)
    }

    /// Implied volatility that reproduces `targetPrice`.
    ///
    /// Mirrors Rust `FxOption::implied_vol` (Garman–Kohlhagen inversion).
    /// @param marketJson - Serialized `MarketContext` carrying both discount curves and the FX spot.
    /// @param asOf - Valuation date as an ISO-8601 string.
    /// @param targetPrice - Observed option PV in quote currency, on the same scale as `price`.
    /// @returns Annualized lognormal volatility as a decimal (`0.10` = 10%).
    /// @throws Error - Throws with kind `not_found` if a curve or the spot is missing, kind `validation` if an input is malformed, and kind `computation` if the root search does not converge (price outside the no-arbitrage bounds).
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
        #[wasm_bindgen(js_name = "targetPrice", unchecked_param_type = "number")]
        target_price: JsValue,
    ) -> Result<f64, JsValue> {
        let market = parse_market(&market_json)?;
        let as_of = js_date(&as_of, "asOf")?;
        self.inner
            .implied_vol(&market, as_of, js_f64(&target_price, "targetPrice")?)
            .map_err(to_js_err)
    }
}

factories!(JsFxOption, "FxOption", instruments::fx::fx_option::FxOption, {
        /// Canonical example option (mirrors Rust `FxOption::example`).
        ///
        /// A EUR/USD call, strike 1.12, EUR 1,000,000, forward-delta convention
        /// with premium in USD; curves `USD-OIS` / `EUR-OIS`, surface `EURUSD-VOL`.
        /// @returns The example option.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations::metrics::MetricId;
    use finstack_quant_valuations::pricer::{InstrumentType, STANDARD_OPTION_GREEKS};

    #[test]
    fn public_json_routes_validate_instrument_before_market_json() {
        assert!(
            crate::api::valuations::pricing::tests::not_a_market_request()
                .instrument("{}")
                .is_err()
        );
        assert!(
            super::super::option_greeks(
                Box::new(instruments::fx::fx_option::FxOption::example().expect("example")),
                "not-market-json",
                "not-a-date",
                Some("not-a-model"),
                None,
            )
            .is_err()
        );
    }

    /// The Greeks the registry computes for `instrument_type`, in
    /// `STANDARD_OPTION_GREEKS` order.
    fn registry_greeks(instrument_type: InstrumentType) -> Vec<String> {
        let menu: Vec<MetricId> = STANDARD_OPTION_GREEKS
            .iter()
            .map(|metric| MetricId::parse_strict(metric).expect("standard greek"))
            .collect();
        finstack_quant_valuations::pricer::STANDARD
            .metrics()
            .applicable_subset(&menu, instrument_type)
            .iter()
            .map(|id| id.as_str().to_string())
            .collect()
    }

    #[test]
    fn option_greek_methods_match_the_metric_registry() {
        use super::super::{
            fx_barrier_option::JsFxBarrierOption, fx_digital_option::JsFxDigitalOption,
            fx_touch_option::JsFxTouchOption, quanto_option::JsQuantoOption,
        };

        let classes: [(&[&str], InstrumentType); 4] = [
            (
                JsQuantoOption::GREEK_METHODS,
                JsQuantoOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxDigitalOption::GREEK_METHODS,
                JsFxDigitalOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxTouchOption::GREEK_METHODS,
                JsFxTouchOption::INSTRUMENT_TYPE,
            ),
            (
                JsFxBarrierOption::GREEK_METHODS,
                JsFxBarrierOption::INSTRUMENT_TYPE,
            ),
        ];
        for (methods, instrument_type) in classes {
            assert_eq!(
                methods.to_vec(),
                registry_greeks(instrument_type),
                "{instrument_type:?} Greek methods drifted from the metric registry"
            );
        }
    }
}

instrument_surface!(
    JsFxForward,
    "FxForward",
    JsFxForwardBuilder,
    "generated.valuations.FxForward"
);

getters!(JsFxForward, "FxForward", |i| {
        /// Cash spot or an outright with a fixed spot value date.
        /// @returns The booked election as a tagged object.
        spot_basis as spotBasis: "generated.valuations.FxForward['spot_basis']" => json(i.spot_basis),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency: "Currency" => currency(i.base_currency),
        /// Quote (domestic) currency; the PV currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
        /// Maturity / settlement date.
        /// @returns The maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Contract forward rate (quote per base).
        /// @returns The rate, or `null` when at-market.
        contract_rate as contractRate: "NonNullable<generated.valuations.FxForward['contract_rate']> | null" => json(i.contract_rate),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId: "string" => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId: "string" => text(i.foreign_discount_curve_id),
        /// Explicit spot override (quote per base).
        /// @returns The override, or `null` to use the FX matrix.
        quoted_spot as quotedSpot: "NonNullable<generated.valuations.FxForward['quoted_spot']> | null" => json(i.quoted_spot),
        /// Base-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        base_calendar_id as baseCalendarId: "string | null" => opt_text(i.base_calendar_id),
        /// Quote-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        quote_calendar_id as quoteCalendarId: "string | null" => opt_text(i.quote_calendar_id),
        /// Expiry as seen by the pricer.
        /// @returns `null`: FX forwards carry no option expiry.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsFxForward, "FxForward");

builder_class!(
    /// Fluent builder for `FxForward`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.FxForward.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsFxForwardBuilder,
    "FxForwardBuilder",
    finstack_quant_valuations::instruments::fx::fx_forward::FxForwardBuilder,
    JsFxForward,
    |b| b.build()
);

setters!(JsFxForwardBuilder, "FxForwardBuilder", {
        /// Set the spot basis election.
        /// @param value - Cash spot or an outright with a fixed spot value date.
        /// @returns The builder for continued configuration.
        /// @throws Error - Throws on a malformed election or a consumed builder.
        spot_basis as spotBasis: "generated.valuations.FxForward['spot_basis']" => js_wire,
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX forward.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the maturity/settlement date.
        /// @param value - Maturity/settlement date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the contract forward rate (quote per base).
        /// @param value - Contract forward rate; when never set the forward is valued at-market (zero PV at inception).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        contract_rate as contractRate: "number" => js_f64,
        /// Set the domestic (quote currency) discount curve identifier.
        /// @param value - Domestic (quote currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
        /// Set the foreign (base currency) discount curve identifier.
        /// @param value - Foreign (base currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
        /// Set an explicit spot rate override (quote per base).
        /// @param value - Spot FX rate; when never set the spot is sourced from the market's FX matrix.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quoted_spot as quotedSpot: "number" => js_f64,
        /// Set the base currency calendar identifier for business day adjustment.
        /// @param value - Base currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        base_calendar_id as baseCalendarId: "string" => id,
        /// Set the quote currency calendar identifier for business day adjustment.
        /// @param value - Quote currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_calendar_id as quoteCalendarId: "string" => id,
});

common_setters!(JsFxForwardBuilder, "FxForwardBuilder");

handle_setters!(JsFxForwardBuilder, "FxForwardBuilder", {
        /// Set the base currency (foreign currency, numerator of the pair).
        /// @param value - Base currency (foreign currency, numerator of the pair), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        base_currency as baseCurrency => JsCurrency,
        /// Set the quote currency (domestic currency, denominator of the pair).
        /// @param value - Quote currency (domestic currency, denominator of the pair), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        quote_currency as quoteCurrency => JsCurrency,
        /// Set the notional amount in base currency.
        /// @param value - Notional amount in base currency, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_surface!(
    JsFxOption,
    "FxOption",
    JsFxOptionBuilder,
    "generated.valuations.FxOption"
);

getters!(JsFxOption, "FxOption", |i| {
        /// Cash payoff at expiry or a signed physical exchange on a booked date.
        /// @returns The booked election as a tagged object.
        delivery as delivery: "generated.valuations.FxOption['delivery']" => json(i.delivery),
        /// Cash spot or an outright with a fixed spot value date.
        /// @returns The booked election as a tagged object.
        spot_basis as spotBasis: "generated.valuations.FxOption['spot_basis']" => json(i.spot_basis),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency: "Currency" => currency(i.base_currency),
        /// Quote (domestic) currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
        /// Strike, quote currency per unit of base currency.
        /// @returns The strike rate.
        strike as strike: "generated.valuations.FxOption['strike']" => json(i.strike),
        /// Option type on the base currency.
        /// @returns `"call"` or `"put"`.
        option_type as optionType: "generated.valuations.FxOption['option_type']" => json(i.option_type),
        /// Delta convention.
        /// @returns `{"kind", "premium_currency", "venue"}`.
        delta_convention as deltaConvention: "generated.valuations.FxOption['delta_convention']" => json(i.delta_convention),
        /// Last date on which the option may be exercised.
        /// @returns Unadjusted calendar expiry date. Time to expiry used in pricing is measured from the valuation date to this date under the pricing day-count convention.
        expiry as expiry: "string" => date(i.expiry),
        /// Day count for the time-to-expiry year fraction.
        /// @returns Act/365F unless set otherwise.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId: "string" => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId: "string" => text(i.foreign_discount_curve_id),
        /// FX volatility surface identifier.
        /// @returns The surface id.
        vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
});

common_getters!(JsFxOption, "FxOption");

builder_class!(
    /// Fluent builder for `FxOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.FxOption.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsFxOptionBuilder,
    "FxOptionBuilder",
    finstack_quant_valuations::instruments::fx::fx_option::FxOptionBuilder,
    JsFxOption,
    |b| b.build()
);

setters!(JsFxOptionBuilder, "FxOptionBuilder", {
        /// Set the delivery election.
        /// @param value - Cash payoff at expiry or a signed physical exchange on a booked date.
        /// @returns The builder for continued configuration.
        /// @throws Error - Throws on a malformed election or a consumed builder.
        delivery as delivery: "generated.valuations.FxOption['delivery']" => js_wire,
        /// Set the spot basis election.
        /// @param value - Cash spot or an outright with a fixed spot value date.
        /// @returns The builder for continued configuration.
        /// @throws Error - Throws on a malformed election or a consumed builder.
        spot_basis as spotBasis: "generated.valuations.FxOption['spot_basis']" => js_wire,
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the strike exchange rate (quote per base).
        /// @param value - Strike exchange rate, quote currency per unit of base currency.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike: "number" => js_f64,
        /// Set the option type: `"call"` or `"put"` on base currency.
        /// @param value - Option type of the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType: "generated.valuations.FxOption['option_type']" => js_wire,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry: "string" => js_date,
        /// Set the domestic currency discount curve identifier.
        /// @param value - Domestic currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
        /// Set the foreign currency discount curve identifier.
        /// @param value - Foreign currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
        /// Set the FX volatility surface identifier.
        /// @param value - FX volatility surface identifier for option pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId: "string" => id,
});

common_setters!(JsFxOptionBuilder, "FxOptionBuilder");

handle_setters!(JsFxOptionBuilder, "FxOptionBuilder", {
        /// Set the base currency (foreign currency).
        /// @param value - Base currency (foreign currency), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        base_currency as baseCurrency => JsCurrency,
        /// Set the quote currency (domestic currency).
        /// @param value - Quote currency (domestic currency), as a `Currency` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        quote_currency as quoteCurrency => JsCurrency,
        /// Set the model day count for volatility, dividend carry and exercise times.
        /// @param value - Model day count for volatility, dividend carry and exercise times, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
        /// Set the notional amount in base currency.
        /// @param value - Notional amount in base currency, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

#[wasm_bindgen(js_class = FxOptionBuilder)]
impl JsFxOptionBuilder {
    /// Set the delta convention used to read the volatility surface.
    ///
    /// Mirrors Rust `FxOptionBuilder::delta_convention`; the convention decides
    /// which delta (spot or forward, premium-adjusted or not) a surface pillar
    /// means. The value runs `FxDeltaConvention::validate`.
    /// @param value - `FxDeltaConvention` plain object or JSON text `{ kind, premium_currency, venue }`: `kind` one of `"spot"`, `"forward"`, `"premium_adjusted_spot"`, `"premium_adjusted_forward"`; `premium_currency` the ISO-4217 code the premium is paid in (the base or quote currency, checked by `build()`); `venue` the market or broker whose quoting convention this is (for example `"generic_interbank"`), not blank. The `deltaConvention` getter returns this shape.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `validation` if `value` does not match the `FxDeltaConvention` schema, `venue` is blank, or the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = deltaConvention)]
    pub fn delta_convention(
        &self,
        #[wasm_bindgen(
            unchecked_param_type = "generated.valuations.FxDeltaConvention | JsonInput"
        )]
        value: JsValue,
    ) -> Result<JsFxOptionBuilder, JsValue> {
        let convention: finstack_quant_valuations::instruments::fx::fx_option::FxDeltaConvention =
            crate::utils::input::from_js_json(&value, "value")?;
        convention.validate().map_err(to_js_err)?;
        let inner = self.inner.apply(|b| b.delta_convention(convention))?;
        Ok(Self { inner })
    }
}
