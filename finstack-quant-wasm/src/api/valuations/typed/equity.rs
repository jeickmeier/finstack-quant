//! Typed `EquityOption` class and its fluent builder.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::valuations::pricing::parse_market;
use crate::utils::input::{from_js_json, js_bool, js_f64, js_string};
use crate::utils::to_js_err;
use crate::utils::wire::{js_date, js_wire};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `EquityOption` instrument (vanilla option on one equity or index).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const equityOption = valuations.instruments.EquityOption.example();
    /// console.log(equityOption.id, equityOption.toDict());
    /// equityOption.free();
    /// ```
    JsEquityOption,
    "EquityOption",
    finstack_quant_valuations::instruments::equity::equity_option::EquityOption
);

instrument_surface!(
    JsEquityOption,
    "EquityOption",
    JsEquityOptionBuilder,
    "generated.valuations.EquityOption"
);

getters!(JsEquityOption, "EquityOption", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Identifier of the underlying equity referenced by the option.
        /// @returns Ticker string exactly as supplied at construction; it is the key used to look up the spot price and volatility surface in the market context, so it must match the market-data identifier.
        underlying_ticker as underlyingTicker: "generated.valuations.EquityOption['underlying_ticker']" => json(i.underlying_ticker),
        /// Contractual exercise price of the option.
        /// @returns Strike expressed in the same price units and currency as the underlying spot quote, not as a percentage of spot or as moneyness.
        strike as strike: "generated.valuations.EquityOption['strike']" => json(i.strike),
        /// Payoff direction of the option contract.
        /// @returns Serde string, either `"call"` (payoff `max(S - K, 0)`) or `"put"` (payoff `max(K - S, 0)`).
        option_type as optionType: "generated.valuations.EquityOption['option_type']" => json(i.option_type),
        /// Exercise rights attached to the option, which select the pricing engine used.
        /// @returns Serde string: `"european"` (exercise only at expiry), `"american"` (any time up to expiry) or `"bermudan"` (on a discrete set of scheduled exercise dates).
        exercise_style as exerciseStyle: "NonNullable<generated.valuations.EquityOption['exercise_style']>" => json(i.exercise_style),
        /// Last date on which the option may be exercised.
        /// @returns Unadjusted calendar expiry date. Time to expiry used in pricing is measured from the valuation date to this date under the pricing day-count convention.
        expiry as expiry: "string" => date(i.expiry),
        /// Number of underlying units; PV and Greeks scale linearly with it.
        /// @returns Underlying unit count (contract size).
        quantity as quantity: "generated.valuations.EquityOption['quantity']" => json(i.quantity),
        /// Currency of the strike, premium and present value.
        /// @returns ISO-4217 currency code, e.g. `"USD"`.
        currency as currency: "string" => text(i.currency),
        /// Model day count for volatility, carry and exercise times. Discount factors use the discount curve's own date convention.
        /// @returns Act/365F unless set otherwise.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Settlement method.
        /// @returns `"physical"` or `"cash"`.
        settlement as settlement: "NonNullable<generated.valuations.EquityOption['settlement']>" => json(i.settlement),
        /// Observed exercise state (`date`, `spot`, `settlement_date`, `exercised`).
        /// @returns The lifecycle plain object, or `null`.
        exercise as exercise: "NonNullable<generated.valuations.EquityOption['exercise']> | null" => json(i.exercise),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Equity spot price identifier.
        /// @returns The price id.
        spot_id as spotId: "string" => text(i.spot_id),
        /// Volatility surface identifier.
        /// @returns The surface id.
        vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
        /// Continuous dividend yield identifier.
        /// @returns The id, or `null`.
        div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
        /// Discrete dividend schedule.
        /// @returns `(ex_date, amount)` pairs in date order.
        discrete_dividends as discreteDividends: "[string, number][]" => dated_nums(i.discrete_dividends),
        /// Bermudan exercise dates.
        /// @returns The dates, or `null`.
        exercise_dates as exerciseDates: "string[] | null" => opt_dates(i.exercise_dates),
});

common_getters!(JsEquityOption, "EquityOption");

factories!(JsEquityOption, "EquityOption", finstack_quant_valuations::instruments::equity::equity_option::EquityOption, {
        /// Canonical SPX 4500 European call expiring 2024-06-21 (mirrors Rust `EquityOption::example`): 100 units in USD, curve `USD-OIS`, spot `EQUITY-SPOT`, surface `EQUITY-VOL`, dividend yield `EQUITY-DIVYIELD`.
        /// @returns The example option.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

builder_class!(
    /// Fluent builder for `EquityOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.EquityOption.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsEquityOptionBuilder,
    "EquityOptionBuilder",
    finstack_quant_valuations::instruments::equity::equity_option::EquityOptionBuilder,
    JsEquityOption,
    |b| b.build()
);

setters!(JsEquityOptionBuilder, "EquityOptionBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the equity option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the underlying equity ticker symbol.
        /// @param value - Underlying equity ticker symbol.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_ticker as underlyingTicker: "string" => id,
        /// Set the strike price.
        /// @param value - Strike price; must be finite and positive.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike: "number" => js_f64,
        /// Set the option type.
        /// @param value - Option type of the equity option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType: "generated.valuations.EquityOption['option_type']" => js_wire,
        /// Set the exercise style.
        /// @param value - Exercise style; defaults to `"european"` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle: "NonNullable<generated.valuations.EquityOption['exercise_style']>" => js_wire,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry: "string" => js_date,
        /// Set the settlement method.
        /// @param value - Physical delivery or fixed cash settlement.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement: "NonNullable<generated.valuations.EquityOption['settlement']>" => js_wire,
        /// Set the number of underlying units the option is written on.
        /// @param value - Number of underlying units; PV and Greeks scale linearly with it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quantity as quantity: "number" => js_f64,
        /// Set the currency of the strike, premium and present value.
        /// @param value - ISO-4217 currency code of the strike and the payoff, e.g. `"USD"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        currency as currency: "generated.valuations.EquityOption['currency']" => js_wire,
        /// Set the discount curve identifier for present value calculations.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the equity spot price identifier.
        /// @param value - Equity spot price identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spot_id as spotId: "string" => id,
        /// Set the equity volatility surface identifier.
        /// @param value - Equity volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId: "string" => id,
        /// Set the continuous dividend yield identifier.
        /// @param value - Continuous dividend yield identifier; zero yield when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        div_yield_id as divYieldId: "string" => id,
        /// Set the exercise schedule for Bermudan options.
        /// @param value - Dates on which early exercise is permitted; required when `exercise_style` is `"bermudan"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_dates as exerciseDates: "string[]" => dates,
});

common_setters!(JsEquityOptionBuilder, "EquityOptionBuilder");

handle_setters!(JsEquityOptionBuilder, "EquityOptionBuilder", {
        /// Set the model day count for volatility, dividend carry and exercise times.
        /// @param value - Model day count for volatility, dividend carry and exercise times, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
});

#[wasm_bindgen(js_class = EquityOption)]
impl JsEquityOption {
    /// Create a cash-settled European option (mirrors Rust `EquityOption::european`).
    ///
    /// Market data ids take the Rust defaults: discount curve `USD-OIS`, spot
    /// `EQUITY-SPOT`, volatility surface `EQUITY-VOL` and dividend yield
    /// `EQUITY-DIVYIELD`. Use `EquityOption.builder()` for other ids.
    /// @param id - Unique instrument identifier.
    /// @param ticker - Underlying equity ticker symbol.
    /// @param strike - Strike price in `currency` units per share; must be positive.
    /// @param expiry - Expiry date as an ISO-8601 string.
    /// @param quantity - Number of shares per contract times contracts held.
    /// @param currency - Currency of the strike and the payoff.
    /// @param optionType - Option type: `call` (right to buy the underlying) or `put` (right to sell it).
    /// @returns The validated option.
    /// @throws Error - Throws with kind `validation` if `expiry` or `optionType` is malformed or the option fails validation, and kind `invalid_type` for a wrong argument type.
    pub fn european(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] ticker: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] strike: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] expiry: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] quantity: JsValue,
        currency: &JsCurrency,
        #[wasm_bindgen(js_name = "optionType", unchecked_param_type = "'call' | 'put'")]
        option_type: JsValue,
    ) -> Result<JsEquityOption, JsValue> {
        finstack_quant_valuations::instruments::equity::equity_option::EquityOption::european(
            js_string(&id, "id")?,
            js_string(&ticker, "ticker")?,
            js_f64(&strike, "strike")?,
            js_date(&expiry, "expiry")?,
            js_f64(&quantity, "quantity")?,
            currency.inner,
            js_wire(&option_type, "optionType")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Solve for the Black-Scholes volatility that reproduces a target price.
    ///
    /// Mirrors Rust `EquityOption::implied_vol`.
    /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the discount curve, spot and dividend yield.
    /// @param asOf - ISO-8601 valuation date.
    /// @param targetPrice - Observed option value for the whole position, on the same scale as `price`.
    /// @returns Annualized lognormal volatility as a decimal (`0.20` = 20%).
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed, and kind `computation` if the root search does not converge.
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
        let target_price = js_f64(&target_price, "targetPrice")?;
        self.inner
            .implied_vol(&market, as_of, target_price)
            .map_err(to_js_err)
    }

    /// Compute every Greek that applies to this option as a plain object.
    /// @param marketJson - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
    /// @param asOf - ISO-8601 valuation date.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Map of Greek name to value, with keys in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`).
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed, and kind `computation` if pricing fails or a Greek is non-finite.
    #[wasm_bindgen(unchecked_return_type = "Record<string, number>")]
    pub fn greeks(
        &self,
        #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "JsonInput")]
        market_json: JsValue,
        #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "string | null")] model: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let market_json = crate::utils::input::json_text(&market_json, "marketJson")?;
        let as_of = js_string(&as_of, "asOf")?;
        let model = crate::utils::input::js_opt_string(model.as_ref(), "model")?;
        super::option_greeks(
            Box::new(self.inner.clone()),
            &market_json,
            &as_of,
            model.as_deref(),
        )
    }
}

#[wasm_bindgen(js_class = EquityOptionBuilder)]
impl JsEquityOptionBuilder {
    /// Record an observed exercise or expiry outcome.
    ///
    /// Mirrors Rust `EquityOptionExercise::new`; an option past expiry needs
    /// this lifecycle state to be priced.
    /// @param date - Exercise or expiry observation date as an ISO-8601 string.
    /// @param spot - Underlying spot observed on `date`, in the option currency.
    /// @param settlementDate - Date the exercise payoff settles, as an ISO-8601 string.
    /// @param exercised - Whether the holder exercised.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `validation` if a date is malformed or the builder was already consumed by `build()`, and kind `invalid_type` for a wrong argument type.
    pub fn exercise(
        &self,
        #[wasm_bindgen(unchecked_param_type = "string")] date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "number")] spot: JsValue,
        #[wasm_bindgen(js_name = "settlementDate", unchecked_param_type = "string")]
        settlement_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "boolean")] exercised: JsValue,
    ) -> Result<JsEquityOptionBuilder, JsValue> {
        let exercise = finstack_quant_valuations::instruments::equity::equity_option::EquityOptionExercise::new(
            js_date(&date, "date")?,
            js_f64(&spot, "spot")?,
            js_date(&settlement_date, "settlementDate")?,
            js_bool(&exercised, "exercised")?,
        );
        let inner = self.inner.apply(|b| b.exercise(exercise))?;
        Ok(Self { inner })
    }

    /// Set the discrete cash dividends paid before expiry.
    /// @param value - Dividends as `[isoDate, amount]` pairs, amount in the option currency per share.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `value` is not an array of `[string, number]` pairs, and kind `validation` if a date is malformed or the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = discreteDividends)]
    pub fn discrete_dividends(
        &self,
        #[wasm_bindgen(unchecked_param_type = "[string, number][]")] value: JsValue,
    ) -> Result<JsEquityOptionBuilder, JsValue> {
        let rows: Vec<(String, f64)> = from_js_json(&value, "value")?;
        let dividends = rows
            .into_iter()
            .map(|(date, amount)| Ok((crate::utils::parse_iso_date(&date)?, amount)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        let inner = self.inner.apply(|b| b.discrete_dividends(dividends))?;
        Ok(Self { inner })
    }
}
