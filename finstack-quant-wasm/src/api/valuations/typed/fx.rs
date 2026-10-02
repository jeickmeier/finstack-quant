//! Member parity for `FxForward` and `FxOption`, and their fluent builders.
//!
//! The classes themselves (constructors, `fromJson`, `toJson`, `price`,
//! `metric`, Greeks) live in [`crate::api::valuations::fx`].

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use crate::api::valuations::fx::{JsFxForward, JsFxOption};
use crate::utils::to_js_err;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_builder_entry!(JsFxForward, "FxForward", JsFxForwardBuilder);

instrument_to_dict!(JsFxForward, "FxForward");

instrument_market_dependencies!(JsFxForward, "FxForward");

getters!(JsFxForward, "FxForward", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency => currency(i.base_currency),
        /// Quote (domestic) currency; the PV currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency => currency(i.quote_currency),
        /// Maturity / settlement date.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Contract forward rate (quote per base).
        /// @returns The rate, or `null` when at-market.
        contract_rate as contractRate => json(i.contract_rate),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId => text(i.foreign_discount_curve_id),
        /// Explicit spot override (quote per base).
        /// @returns The override, or `null` to use the FX matrix.
        quoted_spot as quotedSpot => json(i.quoted_spot),
        /// Base-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        base_calendar_id as baseCalendarId => opt_text(i.base_calendar_id),
        /// Quote-currency holiday calendar identifier.
        /// @returns The calendar id, or `null`.
        quote_calendar_id as quoteCalendarId => opt_text(i.quote_calendar_id),
        /// Expiry as seen by the pricer.
        /// @returns `null`: FX forwards carry no option expiry.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

builder_class!(
    /// Fluent builder for `FxForward`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsFxForwardBuilder,
    "FxForwardBuilder",
    finstack_quant_valuations::instruments::fx::fx_forward::FxForwardBuilder,
    JsFxForward,
    |b| b.build()
);

setters!(JsFxForwardBuilder, "FxForwardBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX forward.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the maturity/settlement date.
        /// @param value - Maturity/settlement date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the contract forward rate (quote per base).
        /// @param value - Contract forward rate; when never set the forward is valued at-market (zero PV at inception).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        contract_rate as contractRate => num,
        /// Set the domestic (quote currency) discount curve identifier.
        /// @param value - Domestic (quote currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId => id,
        /// Set the foreign (base currency) discount curve identifier.
        /// @param value - Foreign (base currency) discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId => id,
        /// Set an explicit spot rate override (quote per base).
        /// @param value - Spot FX rate; when never set the spot is sourced from the market's FX matrix.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quoted_spot as quotedSpot => num,
        /// Set the base currency calendar identifier for business day adjustment.
        /// @param value - Base currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        base_calendar_id as baseCalendarId => id,
        /// Set the quote currency calendar identifier for business day adjustment.
        /// @param value - Quote currency holiday calendar identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        quote_calendar_id as quoteCalendarId => id,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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

instrument_builder_entry!(JsFxOption, "FxOption", JsFxOptionBuilder);

instrument_to_dict!(JsFxOption, "FxOption");

instrument_market_dependencies!(JsFxOption, "FxOption");

getters!(JsFxOption, "FxOption", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"hazard_rate"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Base (foreign) currency; the notional currency.
        /// @returns The base currency.
        base_currency as baseCurrency => currency(i.base_currency),
        /// Quote (domestic) currency.
        /// @returns The quote currency.
        quote_currency as quoteCurrency => currency(i.quote_currency),
        /// Strike, quote currency per unit of base currency.
        /// @returns The strike rate.
        strike as strike => json(i.strike),
        /// Option type on the base currency.
        /// @returns `"call"` or `"put"`.
        option_type as optionType => json(i.option_type),
        /// Delta convention.
        /// @returns `{"kind", "premium_currency", "venue"}`.
        delta_convention as deltaConvention => json(i.delta_convention),
        /// Last date on which the option may be exercised.
        /// @returns Unadjusted calendar expiry date. Time to expiry used in pricing is measured from the valuation date to this date under the pricing day-count convention.
        expiry as expiry => date(i.expiry),
        /// Day count for the time-to-expiry year fraction.
        /// @returns Act/365F unless set otherwise.
        day_count as dayCount => day_count(i.day_count),
        /// Notional amount in the base currency.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Domestic (quote-currency) discount curve identifier.
        /// @returns The curve id.
        domestic_discount_curve_id as domesticDiscountCurveId => text(i.domestic_discount_curve_id),
        /// Foreign (base-currency) discount curve identifier.
        /// @returns The curve id.
        foreign_discount_curve_id as foreignDiscountCurveId => text(i.foreign_discount_curve_id),
        /// FX volatility surface identifier.
        /// @returns The surface id.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
});

builder_class!(
    /// Fluent builder for `FxOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    JsFxOptionBuilder,
    "FxOptionBuilder",
    finstack_quant_valuations::instruments::fx::fx_option::FxOptionBuilder,
    JsFxOption,
    |b| b.build()
);

setters!(JsFxOptionBuilder, "FxOptionBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the strike exchange rate (quote per base).
        /// @param value - Strike exchange rate, quote currency per unit of base currency.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike => num,
        /// Set the option type: `"call"` or `"put"` on base currency.
        /// @param value - Option type of the FX option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType => en,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry => date,
        /// Set the domestic currency discount curve identifier.
        /// @param value - Domestic currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        domestic_discount_curve_id as domesticDiscountCurveId => id,
        /// Set the foreign currency discount curve identifier.
        /// @param value - Foreign currency discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        foreign_discount_curve_id as foreignDiscountCurveId => id,
        /// Set the FX volatility surface identifier.
        /// @param value - FX volatility surface identifier for option pricing.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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
    /// Mirrors Rust `FxDeltaConvention::new`; the convention decides which
    /// delta (spot or forward, premium-adjusted or not) a surface pillar means.
    /// @param kind - Delta convention kind: `spot`, `forward`, `spot_premium_adjusted`, or `forward_premium_adjusted`.
    /// @param premium_currency - Currency the option premium is paid in.
    /// @param venue - Market or broker whose quoting convention this is (for example `"interbank"`); must not be blank.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `invalid_type` if `kind` or `venue` is not a string, and kind `validation` if `kind` is unknown, `venue` is blank, or the builder was already consumed by `build()`.
    #[wasm_bindgen(js_name = deltaConvention)]
    pub fn delta_convention(
        &self,
        kind: JsValue,
        premium_currency: &JsCurrency,
        venue: JsValue,
    ) -> Result<JsFxOptionBuilder, JsValue> {
        let convention =
            finstack_quant_valuations::instruments::fx::fx_option::FxDeltaConvention::new(
                super::arg::en(&kind, "kind")?,
                premium_currency.inner,
                crate::utils::input::js_string(&venue, "venue")?,
            )
            .map_err(to_js_err)?;
        let inner = self.inner.apply(|b| b.delta_convention(convention))?;
        Ok(Self { inner })
    }
}
