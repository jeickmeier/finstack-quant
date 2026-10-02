//! Typed `EquityOption` class and its fluent builder.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::utils::input::js_string;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `EquityOption` instrument (vanilla option on one equity or index).
    JsEquityOption,
    "EquityOption",
    finstack_quant_valuations::instruments::EquityOption
);

instrument_builder_entry!(JsEquityOption, "EquityOption", JsEquityOptionBuilder);

instrument_to_dict!(JsEquityOption, "EquityOption");

instrument_market_dependencies!(JsEquityOption, "EquityOption");

instrument_pricing!(JsEquityOption, "EquityOption");

getters!(JsEquityOption, "EquityOption", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Identifier of the underlying equity referenced by the option.
        /// @returns Ticker string exactly as supplied at construction; it is the key used to look up the spot price and volatility surface in the market context, so it must match the market-data identifier.
        underlying_ticker as underlyingTicker => json(i.underlying_ticker),
        /// Contractual exercise price of the option.
        /// @returns Strike expressed in the same price units and currency as the underlying spot quote, not as a percentage of spot or as moneyness.
        strike as strike => json(i.strike),
        /// Payoff direction of the option contract.
        /// @returns Serde string, either `"call"` (payoff `max(S - K, 0)`) or `"put"` (payoff `max(K - S, 0)`).
        option_type as optionType => json(i.option_type),
        /// Exercise rights attached to the option, which select the pricing engine used.
        /// @returns Serde string: `"european"` (exercise only at expiry), `"american"` (any time up to expiry) or `"bermudan"` (on a discrete set of scheduled exercise dates).
        exercise_style as exerciseStyle => json(i.exercise_style),
        /// Last date on which the option may be exercised.
        /// @returns Unadjusted calendar expiry date. Time to expiry used in pricing is measured from the valuation date to this date under the pricing day-count convention.
        expiry as expiry => date(i.expiry),
        /// Number of underlying units; PV and Greeks scale linearly with it.
        /// @returns Underlying unit count (contract size).
        quantity as quantity => json(i.quantity),
        /// Currency of the strike, premium and present value.
        /// @returns ISO-4217 currency code, e.g. `"USD"`.
        currency as currency => text(i.currency),
        /// Model day count for volatility, carry and exercise times. Discount factors use the discount curve's own date convention.
        /// @returns Act/365F unless set otherwise.
        day_count as dayCount => day_count(i.day_count),
        /// Settlement method.
        /// @returns `"physical"` or `"cash"`.
        settlement as settlement => json(i.settlement),
        /// Observed exercise state (`date`, `spot`, `settlement_date`, `exercised`).
        /// @returns The lifecycle plain object, or `null`.
        exercise as exercise => json(i.exercise),
        /// Discount curve identifier.
        /// @returns The curve id.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Equity spot price identifier.
        /// @returns The price id.
        spot_id as spotId => text(i.spot_id),
        /// Volatility surface identifier.
        /// @returns The surface id.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
        /// Continuous dividend yield identifier.
        /// @returns The id, or `null`.
        div_yield_id as divYieldId => opt_text(i.div_yield_id),
        /// Discrete dividend schedule.
        /// @returns `(ex_date, amount)` pairs in date order.
        discrete_dividends as discreteDividends => dated_nums(i.discrete_dividends),
        /// Bermudan exercise dates.
        /// @returns The dates, or `null`.
        exercise_dates as exerciseDates => opt_dates(i.exercise_dates),
});

factories!(JsEquityOption, "EquityOption", finstack_quant_valuations::instruments::EquityOption, {
        /// Canonical SPX 4500 European call expiring 2024-06-21 (mirrors Rust `EquityOption::example`): 100 units in USD, curve `USD-OIS`, spot `EQUITY-SPOT`, surface `EQUITY-VOL`, dividend yield `EQUITY-DIVYIELD`.
        /// @returns The example option.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

greek_methods!(JsEquityOption, "EquityOption", {
        /// Spot delta of the option under the selected model.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
        /// @param as_of - ISO-8601 valuation date.
        /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
        /// @returns Spot delta produced by the selected model.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed or the Greek is not produced by the selected model, and kind `computation` if pricing fails.
        delta as delta => "delta",
        /// Gamma of the option under the selected model.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
        /// @param as_of - ISO-8601 valuation date.
        /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
        /// @returns Gamma produced by the selected model.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed or the Greek is not produced by the selected model, and kind `computation` if pricing fails.
        gamma as gamma => "gamma",
        /// Vega (per 1% vol) of the option under the selected model.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
        /// @param as_of - ISO-8601 valuation date.
        /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
        /// @returns Vega (per 1% vol) produced by the selected model.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed or the Greek is not produced by the selected model, and kind `computation` if pricing fails.
        vega as vega => "vega",
        /// Theta (per day on `metric_pricing_overrides.theta_day_basis`, calendar days by default) of the option under the selected model.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
        /// @param as_of - ISO-8601 valuation date.
        /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
        /// @returns Theta (per day on `metric_pricing_overrides.theta_day_basis`, calendar days by default) produced by the selected model.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed or the Greek is not produced by the selected model, and kind `computation` if pricing fails.
        theta as theta => "theta",
        /// Rho of the option under the selected model.
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
        /// @param as_of - ISO-8601 valuation date.
        /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
        /// @returns Rho produced by the selected model.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed or the Greek is not produced by the selected model, and kind `computation` if pricing fails.
        rho as rho => "rho",
});

builder_class!(
    /// Fluent builder for `EquityOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
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
        id as id => id,
        /// Set the underlying equity ticker symbol.
        /// @param value - Underlying equity ticker symbol.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_ticker as underlyingTicker => id,
        /// Set the strike price.
        /// @param value - Strike price; must be finite and positive.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike => num,
        /// Set the option type.
        /// @param value - Option type of the equity option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType => en,
        /// Set the exercise style.
        /// @param value - Exercise style; defaults to `"european"` when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle => en,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry => date,
        /// Set the settlement method.
        /// @param value - Physical delivery or fixed cash settlement.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement => en,
        /// Set the number of underlying units the option is written on.
        /// @param value - Number of underlying units; PV and Greeks scale linearly with it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quantity as quantity => num,
        /// Set the currency of the strike, premium and present value.
        /// @param value - ISO-4217 currency code of the strike and the payoff, e.g. `"USD"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        currency as currency => en,
        /// Set the discount curve identifier for present value calculations.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the equity spot price identifier.
        /// @param value - Equity spot price identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spot_id as spotId => id,
        /// Set the equity volatility surface identifier.
        /// @param value - Equity volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set the continuous dividend yield identifier.
        /// @param value - Continuous dividend yield identifier; zero yield when never set.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        div_yield_id as divYieldId => id,
        /// Set the exercise schedule for Bermudan options.
        /// @param value - Dates on which early exercise is permitted; required when `exercise_style` is `"bermudan"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_dates as exerciseDates => dates,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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
    /// @param option_type - Option type: `call` (right to buy the underlying) or `put` (right to sell it).
    /// @returns The validated option.
    /// @throws Error - Throws with kind `validation` if `expiry` or `optionType` is malformed or the option fails validation, and kind `invalid_type` for a wrong argument type.
    #[allow(clippy::too_many_arguments)]
    pub fn european(
        id: JsValue,
        ticker: JsValue,
        strike: JsValue,
        expiry: JsValue,
        quantity: JsValue,
        currency: &JsCurrency,
        option_type: JsValue,
    ) -> Result<JsEquityOption, JsValue> {
        finstack_quant_valuations::instruments::EquityOption::european(
            js_string(&id, "id")?,
            js_string(&ticker, "ticker")?,
            super::arg::num(&strike, "strike")?,
            super::arg::date(&expiry, "expiry")?,
            super::arg::num(&quantity, "quantity")?,
            currency.inner,
            super::arg::en(&option_type, "optionType")?,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }

    /// Solve for the Black-Scholes volatility that reproduces a target price.
    ///
    /// Mirrors Rust `EquityOption::implied_vol`.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying the discount curve, spot and dividend yield.
    /// @param as_of - ISO-8601 valuation date.
    /// @param target_price - Observed option value for the whole position, on the same scale as `price`.
    /// @returns Annualized lognormal volatility as a decimal (`0.20` = 20%).
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed, and kind `computation` if the root search does not converge.
    #[wasm_bindgen(js_name = impliedVol)]
    pub fn implied_vol(
        &self,
        market_json: JsValue,
        as_of: JsValue,
        target_price: JsValue,
    ) -> Result<f64, JsValue> {
        let market = super::market(&market_json)?;
        let as_of = super::as_of(&as_of)?;
        let target_price = super::arg::num(&target_price, "targetPrice")?;
        self.inner
            .implied_vol(&market, as_of, target_price)
            .map_err(to_js_err)
    }

    /// Compute every Greek that applies to this option as a plain object.
    /// @param market_json - Canonical market-context JSON (string or plain object) supplying curves, quotes, and spot data.
    /// @param as_of - ISO-8601 valuation date.
    /// @param model - Optional pricing-model identifier; omit to use the instrument's default model.
    /// @returns Map of Greek name to value, with keys in the Rust `STANDARD_OPTION_GREEKS` order (`delta`, `gamma`, `vega`, `theta`, `rho`).
    /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if an input is malformed, and kind `computation` if pricing fails or a Greek is non-finite.
    pub fn greeks(
        &self,
        market_json: JsValue,
        as_of: JsValue,
        model: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let instrument =
            crate::api::valuations::pricing::parse_pricing_instrument_json(&self.to_json()?, None)?;
        let market = super::market(&market_json)?;
        let as_of = js_string(&as_of, "asOf")?;
        let model = crate::utils::input::js_opt_string(model.as_ref(), "model")?;
        let pairs = crate::api::valuations::pricing::standard_option_greeks_with_context(
            &instrument,
            &market,
            &as_of,
            model.as_deref().unwrap_or("default"),
        )?;
        let ordered: indexmap::IndexMap<&'static str, f64> = pairs.into_iter().collect();
        to_js_value(&ordered)
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
    /// @param settlement_date - Date the exercise payoff settles, as an ISO-8601 string.
    /// @param exercised - Whether the holder exercised.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `validation` if a date is malformed or the builder was already consumed by `build()`, and kind `invalid_type` for a wrong argument type.
    pub fn exercise(
        &self,
        date: JsValue,
        spot: JsValue,
        settlement_date: JsValue,
        exercised: JsValue,
    ) -> Result<JsEquityOptionBuilder, JsValue> {
        let exercise = finstack_quant_valuations::instruments::equity::EquityOptionExercise::new(
            super::arg::date(&date, "date")?,
            super::arg::num(&spot, "spot")?,
            super::arg::date(&settlement_date, "settlementDate")?,
            super::arg::flag(&exercised, "exercised")?,
        );
        let inner = self.inner.apply(|b| b.exercise(exercise))?;
        Ok(Self { inner })
    }

    /// Set the discrete cash dividends paid before expiry.
    /// @param value - Dividends as `[isoDate, amount]` pairs, amount in the option currency per share.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `value` is not an array of `[string, number]` pairs, and kind `validation` if a date is malformed or the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = discreteDividends)]
    pub fn discrete_dividends(&self, value: JsValue) -> Result<JsEquityOptionBuilder, JsValue> {
        let rows: Vec<(String, f64)> = super::arg::json(&value, "value")?;
        let dividends = rows
            .into_iter()
            .map(|(date, amount)| Ok((crate::utils::parse_iso_date(&date)?, amount)))
            .collect::<Result<Vec<_>, JsValue>>()?;
        let inner = self.inner.apply(|b| b.discrete_dividends(dividends))?;
        Ok(Self { inner })
    }
}
