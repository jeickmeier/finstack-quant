//! A swap on annualized realized FX variance.

use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_variance_swap::{
    FxVarianceSwap, FxVarianceSwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed WASM wrapper for the Rust FX instrument FxVarianceSwap.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations, type JsonInput } from "finstack-quant-wasm";
    /// await init();
    /// // The canonical `fx_variance_swap` envelope published with the instrument schema.
    /// const [envelope] = valuations.schema.instrumentSchema("fx_variance_swap").examples as JsonInput[];
    /// const swap = valuations.instruments.FxVarianceSwap.fromJson(envelope);
    /// console.log(swap.id);
    /// swap.free();
    /// ```
    JsFxVarianceSwap, "FxVarianceSwap", FxVarianceSwap
);
instrument_surface!(
    JsFxVarianceSwap,
    "FxVarianceSwap",
    JsFxVarianceSwapBuilder,
    "generated.valuations.FxVarianceSwap"
);
factories!(JsFxVarianceSwap, "FxVarianceSwap", FxVarianceSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxVarianceSwap, "FxVarianceSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's base currency.
    /// @returns Base currency (foreign)
    base_currency as baseCurrency: "string" => text(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (domestic)
    quote_currency as quoteCurrency: "string" => text(i.quote_currency),
    /// Read the contract's spot id.
    /// @returns Optional spot identifier used to look up historical series.
    spot_id as spotId: "string | null" => json(i.spot_id),
    /// Read the contract's notional.
    /// @returns Variance notional (in quote currency units)
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
    observation_frequency as observationFrequency: "generated.valuations.FxVarianceSwap['observation_frequency']" => json(i.observation_frequency),
    /// Read the contract's base calendar id.
    /// @returns Base-currency calendar used in the joint observation calendar.
    base_calendar_id as baseCalendarId: "string" => json(i.base_calendar_id),
    /// Read the contract's quote calendar id.
    /// @returns Quote-currency calendar used in the joint observation calendar.
    quote_calendar_id as quoteCalendarId: "string" => json(i.quote_calendar_id),
    /// Read the contract's observation business day convention.
    /// @returns Business-day convention applied to observation dates.
    observation_business_day_convention as observationBusinessDayConvention: "generated.valuations.FxVarianceSwap['observation_business_day_convention']" => json(i.observation_business_day_convention),
    /// Read the contract's observation end of month.
    /// @returns Preserve month-end rolls for month/year observation frequencies.
    observation_end_of_month as observationEndOfMonth: "boolean" => json(i.observation_end_of_month),
    /// Read the contract's realized var method.
    /// @returns Method for calculating realized variance (defaults to CloseToClose)
    realized_var_method as realizedVarMethod: "generated.valuations.FxVarianceSwap['realized_var_method']" => json(i.realized_var_method),
    /// Read the contract's trading days per year.
    /// @returns Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).
    trading_days_per_year as tradingDaysPerYear: "number" => json(i.trading_days_per_year),
    /// Read the contract's open series id.
    /// @returns Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    open_series_id as openSeriesId: "string | null" => json(i.open_series_id),
    /// Read the contract's high series id.
    /// @returns Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    high_series_id as highSeriesId: "string | null" => json(i.high_series_id),
    /// Read the contract's low series id.
    /// @returns Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    low_series_id as lowSeriesId: "string | null" => json(i.low_series_id),
    /// Read the contract's close series id.
    /// @returns Series ID for close prices. Defaults to spot_id (or currency-pair string) when absent.
    close_series_id as closeSeriesId: "string | null" => json(i.close_series_id),
    /// Read the contract's previous close date.
    /// @returns Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.
    previous_close_date as previousCloseDate: "string | null" => opt_date(i.previous_close_date),
    /// Read the contract's side.
    /// @returns Pay/receive variance
    side as side: "generated.valuations.FxVarianceSwap['side']" => json(i.side),
    /// Read the contract's domestic discount curve id.
    /// @returns Domestic currency discount curve ID
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Foreign currency discount curve ID
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's vol surface id.
    /// @returns FX volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
    /// Read the contract's day count.
    /// @returns Day count convention for time calculations (defaults to ACT/365F).
    day_count as dayCount: "DayCount" => day_count(i.day_count),
});
common_getters!(JsFxVarianceSwap, "FxVarianceSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxVarianceSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxVarianceSwapBuilder, "FxVarianceSwapBuilder", FxVarianceSwapBuilder, JsFxVarianceSwap, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsFxVarianceSwap, "FxVarianceSwap", JsFxVarianceSwapBuilder
);

setters!(JsFxVarianceSwapBuilder, "FxVarianceSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's base currency.
    /// @param value - Base currency (foreign)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_currency as baseCurrency: "string" => js_wire,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (domestic)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_currency as quoteCurrency: "string" => js_wire,
    /// Set the contract's spot id.
    /// @param value - Optional spot identifier used to look up historical series.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string | null" => optional_id via spot_id_opt,
    /// Set the contract's strike variance.
    /// @param value - Strike variance (annualized)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike_variance as strikeVariance: "number" => js_wire,
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
    settlement_date as settlementDate: "string | null" => js_wire via settlement_date_opt,
    /// Set the contract's observation frequency.
    /// @param value - Observation frequency
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_frequency as observationFrequency: "generated.valuations.FxVarianceSwap['observation_frequency']" => js_wire,
    /// Set the contract's base calendar id.
    /// @param value - Base-currency calendar used in the joint observation calendar.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    base_calendar_id as baseCalendarId: "string" => id,
    /// Set the contract's quote calendar id.
    /// @param value - Quote-currency calendar used in the joint observation calendar.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quote_calendar_id as quoteCalendarId: "string" => id,
    /// Set the contract's observation business day convention.
    /// @param value - Business-day convention applied to observation dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_business_day_convention as observationBusinessDayConvention: "generated.valuations.FxVarianceSwap['observation_business_day_convention']" => js_wire,
    /// Set the contract's observation end of month.
    /// @param value - Preserve month-end rolls for month/year observation frequencies.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observation_end_of_month as observationEndOfMonth: "boolean" => js_wire,
    /// Set the contract's realized var method.
    /// @param value - Method for calculating realized variance (defaults to CloseToClose)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    realized_var_method as realizedVarMethod: "generated.valuations.FxVarianceSwap['realized_var_method']" => js_wire,
    /// Set the contract's trading days per year.
    /// @param value - Trading days per year used to annualise daily realized variance. A contract term of the variance-swap confirmation: daily observations are annualised by trading_days_per_year / observation_frequency.count(). Must be finite and positive. Defaults to finstack_quant_valuations_core::constants::TRADING_DAYS_PER_YEAR (252).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    trading_days_per_year as tradingDaysPerYear: "number" => js_wire,
    /// Set the contract's open series id.
    /// @param value - Series ID for open prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    open_series_id as openSeriesId: "string | null" => optional_id via open_series_id_opt,
    /// Set the contract's high series id.
    /// @param value - Series ID for high prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    high_series_id as highSeriesId: "string | null" => optional_id via high_series_id_opt,
    /// Set the contract's low series id.
    /// @param value - Series ID for low prices (required for Parkinson, GarmanKlass, RogersSatchell, YangZhang). Defaults to spot_id (or currency-pair string) when absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    low_series_id as lowSeriesId: "string | null" => optional_id via low_series_id_opt,
    /// Set the contract's close series id.
    /// @param value - Series ID for close prices. Defaults to spot_id (or currency-pair string) when absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    close_series_id as closeSeriesId: "string | null" => optional_id via close_series_id_opt,
    /// Set the contract's previous close date.
    /// @param value - Contractual closing fixing immediately before the first Yang–Zhang target bar. Resolved exactly in the same close series as the target bars.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    previous_close_date as previousCloseDate: "string | null" => js_wire via previous_close_date_opt,
    /// Set the contract's side.
    /// @param value - Pay/receive variance
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.FxVarianceSwap['side']" => js_wire,
    /// Set the contract's domestic discount curve id.
    /// @param value - Domestic currency discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's foreign discount curve id.
    /// @param value - Foreign currency discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
    /// Set the contract's vol surface id.
    /// @param value - FX volatility surface ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
});
common_setters!(JsFxVarianceSwapBuilder, "FxVarianceSwapBuilder");

handle_setters!(JsFxVarianceSwapBuilder, "FxVarianceSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Variance notional (in quote currency units)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's day count.
    /// @param value - Day count convention for time calculations (defaults to ACT/365F).
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
