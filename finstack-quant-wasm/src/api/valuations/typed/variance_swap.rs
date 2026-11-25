//! A swap on annualized realized equity variance.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::variance_swap::{
    VarianceSwap, VarianceSwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A swap on annualized realized equity variance.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.VarianceSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsVarianceSwap, "VarianceSwap", VarianceSwap
);
instrument_surface!(
    JsVarianceSwap,
    "VarianceSwap",
    JsVarianceSwapBuilder,
    "generated.valuations.VarianceSwap"
);
factories!(JsVarianceSwap, "VarianceSwap", VarianceSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsVarianceSwap, "VarianceSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Underlying symbol (equity/index). A label only; market data is read through spot_id, vol_surface_id and div_yield_id.
    underlying_ticker as underlyingTicker: "string" => text(i.underlying_ticker),
    /// Read the contract's spot id.
    /// @returns Market-scalar id (MarketContext::get_price) of the underlying spot level. It is also the default close-price series id.
    spot_id as spotId: "string" => text(i.spot_id),
    /// Read the contract's vol surface id.
    /// @returns Volatility surface used to replicate the unobserved variance.
    vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
    /// Read the contract's div yield id.
    /// @returns Optional unitless continuous dividend-yield scalar id (decimal, 0.02 = 2%). None means a zero dividend yield.
    div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
    /// Read the contract's notional.
    /// @returns Variance notional (in variance units)
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's strike variance.
    /// @returns Strike variance (annualized)
    strike_variance as strikeVariance: "number" => json(i.strike_variance),
    /// Read the contract's start date.
    /// @returns Start date of observation period
    start_date as startDate: "string" => date(i.start_date),
    /// Read the contract's maturity.
    /// @returns Contractual end of the observation period.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's settlement date.
    /// @returns Optional cash-settlement date. Defaults to the adjusted final observation date.
    settlement_date as settlementDate: "string | null" => opt_date(i.settlement_date),
    /// Read the contract's observation frequency.
    /// @returns Observation frequency
    observation_frequency as observationFrequency: "generated.valuations.VarianceSwap['observation_frequency']" => json(i.observation_frequency),
    /// Read the contract's observation calendar id.
    /// @returns Exchange/fixing calendar used for every realized-variance observation.
    observation_calendar_id as observationCalendarId: "string" => text(i.observation_calendar_id),
    /// Read the contract's observation business day convention.
    /// @returns Business-day convention applied to observation dates.
    observation_business_day_convention as observationBusinessDayConvention: "generated.valuations.VarianceSwap['observation_business_day_convention']" => json(i.observation_business_day_convention),
    /// Read the contract's observation end of month.
    /// @returns Preserve month-end rolls for month/year observation frequencies.
    observation_end_of_month as observationEndOfMonth: "boolean" => json(i.observation_end_of_month),
    /// Read the contract's realized var method.
    /// @returns Method for calculating realized variance (defaults to CloseToClose)
    realized_var_method as realizedVarMethod: "generated.valuations.VarianceSwap['realized_var_method']" => json(i.realized_var_method),
    /// Read the contract's trading days per year.
    /// @returns Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).
    trading_days_per_year as tradingDaysPerYear: "number" => json(i.trading_days_per_year),
    /// Read the contract's price series policy.
    /// @returns Corporate-action policy applied to all historical price series. The selected policy is declarative: input series must already conform to it. Adjusted is appropriate for single-stock variance where splits must not create artificial returns; Raw is explicit for official index levels or contracts whose calculation agent retains raw observations.
    price_series_policy as priceSeriesPolicy: "generated.valuations.VarianceSwap['price_series_policy']" => json(i.price_series_policy),
    /// Read the contract's open series id.
    /// @returns Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    open_series_id as openSeriesId: "string | null" => opt_text(i.open_series_id),
    /// Read the contract's high series id.
    /// @returns Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    high_series_id as highSeriesId: "string | null" => opt_text(i.high_series_id),
    /// Read the contract's low series id.
    /// @returns Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    low_series_id as lowSeriesId: "string | null" => opt_text(i.low_series_id),
    /// Read the contract's close series id.
    /// @returns Series ID for close prices. Defaults to spot_id when absent.
    close_series_id as closeSeriesId: "string | null" => opt_text(i.close_series_id),
    /// Read the contract's previous close date.
    /// @returns Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.
    previous_close_date as previousCloseDate: "string | null" => opt_date(i.previous_close_date),
    /// Read the contract's side.
    /// @returns Pay/receive variance
    side as side: "generated.valuations.VarianceSwap['side']" => json(i.side),
    /// Read the contract's discount curve id.
    /// @returns Discount curve identifier
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's day count.
    /// @returns Day count convention for time calculations
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsVarianceSwap, "VarianceSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.VarianceSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsVarianceSwapBuilder, "VarianceSwapBuilder", VarianceSwapBuilder, JsVarianceSwap, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsVarianceSwap, "VarianceSwap", JsVarianceSwapBuilder
);

setters!(JsVarianceSwapBuilder, "VarianceSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying ticker.
    /// @param value - Underlying symbol (equity/index). A label only; market data is read through spot_id, vol_surface_id and div_yield_id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_ticker as underlyingTicker: "string" => id,
    /// Set the contract's spot id.
    /// @param value - Market-scalar id (MarketContext::get_price) of the underlying spot level. It is also the default close-price series id.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - Volatility surface used to replicate the unobserved variance.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
    /// Set the contract's div yield id.
    /// @param value - Optional unitless continuous dividend-yield scalar id (decimal, 0.02 = 2%). None means a zero dividend yield.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    div_yield_id as divYieldId: "string | null" => optional_id via div_yield_id_opt,
    /// Set the contract's strike variance.
    /// @param value - Strike variance (annualized)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike_variance as strikeVariance: "number" => js_f64,
    /// Set the contract's start date.
    /// @param value - Start date of observation period
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    start_date as startDate: "string" => js_date,
    /// Set the contract's maturity.
    /// @param value - Contractual end of the observation period.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's settlement date.
    /// @param value - Optional cash-settlement date. Defaults to the adjusted final observation date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_date as settlementDate: "string | null" => opt_date via settlement_date_opt,
    /// Set the contract's observation frequency.
    /// @param value - Observation frequency
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_frequency as observationFrequency: "generated.valuations.VarianceSwap['observation_frequency']" => js_wire,
    /// Set the contract's observation calendar id.
    /// @param value - Exchange/fixing calendar used for every realized-variance observation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_calendar_id as observationCalendarId: "string" => id,
    /// Set the contract's observation business day convention.
    /// @param value - Business-day convention applied to observation dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_business_day_convention as observationBusinessDayConvention: "generated.valuations.VarianceSwap['observation_business_day_convention']" => js_wire,
    /// Set the contract's observation end of month.
    /// @param value - Preserve month-end rolls for month/year observation frequencies.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_end_of_month as observationEndOfMonth: "boolean" => js_bool,
    /// Set the contract's realized var method.
    /// @param value - Method for calculating realized variance (defaults to CloseToClose)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    realized_var_method as realizedVarMethod: "generated.valuations.VarianceSwap['realized_var_method']" => js_wire,
    /// Set the contract's trading days per year.
    /// @param value - Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    trading_days_per_year as tradingDaysPerYear: "number" => js_f64,
    /// Set the contract's price series policy.
    /// @param value - Corporate-action policy applied to all historical price series. The selected policy is declarative: input series must already conform to it. Adjusted is appropriate for single-stock variance where splits must not create artificial returns; Raw is explicit for official index levels or contracts whose calculation agent retains raw observations.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    price_series_policy as priceSeriesPolicy: "generated.valuations.VarianceSwap['price_series_policy']" => js_wire,
    /// Set the contract's open series id.
    /// @param value - Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    open_series_id as openSeriesId: "string | null" => optional_id via open_series_id_opt,
    /// Set the contract's high series id.
    /// @param value - Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    high_series_id as highSeriesId: "string | null" => optional_id via high_series_id_opt,
    /// Set the contract's low series id.
    /// @param value - Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    low_series_id as lowSeriesId: "string | null" => optional_id via low_series_id_opt,
    /// Set the contract's close series id.
    /// @param value - Series ID for close prices. Defaults to spot_id when absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    close_series_id as closeSeriesId: "string | null" => optional_id via close_series_id_opt,
    /// Set the contract's previous close date.
    /// @param value - Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    previous_close_date as previousCloseDate: "string | null" => opt_date via previous_close_date_opt,
    /// Set the contract's side.
    /// @param value - Pay/receive variance
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.VarianceSwap['side']" => js_wire,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve identifier
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
});
common_setters!(JsVarianceSwapBuilder, "VarianceSwapBuilder");

handle_setters!(JsVarianceSwapBuilder, "VarianceSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Variance notional (in variance units)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for time calculations
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
