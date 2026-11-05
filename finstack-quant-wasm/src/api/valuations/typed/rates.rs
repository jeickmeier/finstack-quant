//! Typed `InterestRateSwap`, `Swaption` and `CapFloor` classes and their
//! fluent builders.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::input::js_f64;
use crate::utils::to_js_err;
use crate::utils::wire::{js_date, js_wire};
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `InterestRateSwap` instrument (fixed-for-floating swap).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const interestRateSwap = valuations.instruments.InterestRateSwap.example();
    /// console.log(interestRateSwap.id, interestRateSwap.toDict());
    /// interestRateSwap.free();
    /// ```
    JsInterestRateSwap,
    "InterestRateSwap",
    finstack_quant_valuations::instruments::rates::irs::InterestRateSwap
);

instrument_surface!(
    JsInterestRateSwap,
    "InterestRateSwap",
    JsInterestRateSwapBuilder,
    "generated.valuations.InterestRateSwap"
);

getters!(JsInterestRateSwap, "InterestRateSwap", |i| {
        /// Notional shared by both legs.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Swap direction for the fixed leg.
        /// @returns `"pay"` or `"receive"`.
        side as side: "generated.valuations.InterestRateSwap['side']" => json(i.side),
        /// Fixed leg specification.
        /// @returns The fixed leg.
        fixed_leg as fixedLeg: "generated.valuations.InterestRateSwap['fixed_leg']" => json(i.fixed_leg),
        /// Floating leg specification.
        /// @returns The floating leg.
        float_leg as floatLeg: "generated.valuations.InterestRateSwap['float_leg']" => json(i.float_leg),
        /// Whether fixed accrual boundaries use the leg's calendar and business-day convention.
        /// @returns True for adjusted accrual dates; false for contractual unadjusted dates (default).
        adjust_fixed_accrual_dates as adjustFixedAccrualDates: "boolean" => json(i.adjust_fixed_accrual_dates),
        /// Whether floating accrual boundaries use the leg's calendar and business-day convention.
        /// @returns True for adjusted accrual dates; false for contractual unadjusted dates (default).
        adjust_float_accrual_dates as adjustFloatAccrualDates: "boolean" => json(i.adjust_float_accrual_dates),
        /// OTC margin (CSA / initial-margin) specification in serde form.
        /// @returns The spec plain object, or `null`.
        margin_spec as marginSpec: "NonNullable<generated.valuations.InterestRateSwap['margin_spec']> | null" => json(i.margin_spec),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsInterestRateSwap, "InterestRateSwap");

factories!(JsInterestRateSwap, "InterestRateSwap", finstack_quant_valuations::instruments::rates::irs::InterestRateSwap, {
        /// Canonical 5-year USD pay-fixed swap (mirrors Rust `InterestRateSwap::example`): semi-annual 30/360 fixed vs quarterly ACT/360 `USD-SOFR-3M`, T-2 reset lag, `usny` calendar.
        /// @returns The example swap.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

builder_class!(
    /// Fluent builder for `InterestRateSwap`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.InterestRateSwap.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsInterestRateSwapBuilder,
    "InterestRateSwapBuilder",
    finstack_quant_valuations::instruments::rates::irs::InterestRateSwapBuilder,
    JsInterestRateSwap,
    |b| b.build()
);

setters!(JsInterestRateSwapBuilder, "InterestRateSwapBuilder", {
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the swap direction for the fixed leg.
        /// @param value - Swap direction for the fixed leg (serde string). `"pay"` pays fixed / receives floating.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side: "generated.valuations.InterestRateSwap['side']" => js_wire,
        /// Set the fixed leg specification.
        /// @param value - Fixed leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fixed_leg as fixedLeg: "generated.valuations.InterestRateSwap['fixed_leg']" => from_js_json,
        /// Set the floating leg specification.
        /// @param value - Floating leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        float_leg as floatLeg: "generated.valuations.InterestRateSwap['float_leg']" => from_js_json,
        /// Select fixed accrual-date adjustment independently of payment dates.
        /// @param value - True applies the fixed leg's calendar and business-day convention; false (default) retains contractual accrual boundaries.
        /// @returns The builder for chaining.
        /// @throws Error - Throws for a non-boolean value or an already consumed builder.
        adjust_fixed_accrual_dates as adjustFixedAccrualDates: "boolean" => js_bool,
        /// Select floating accrual-date adjustment independently of payment dates.
        /// @param value - True applies the floating leg's calendar and business-day convention; false (default) retains contractual accrual boundaries.
        /// @returns The builder for chaining.
        /// @throws Error - Throws for a non-boolean value or an already consumed builder.
        adjust_float_accrual_dates as adjustFloatAccrualDates: "boolean" => js_bool,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string), e.g. the `margin_spec` value of a margined swap's `toDict()`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec: "NonNullable<generated.valuations.InterestRateSwap['margin_spec']>" => from_js_json,
});

common_setters!(JsInterestRateSwapBuilder, "InterestRateSwapBuilder");

handle_setters!(JsInterestRateSwapBuilder, "InterestRateSwapBuilder", {
        /// Set the notional shared by both legs.
        /// @param value - Notional shared by both legs, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `Swaption` instrument (European option on a swap).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const swaption = valuations.instruments.Swaption.example();
    /// console.log(swaption.id, swaption.toDict());
    /// swaption.free();
    /// ```
    JsSwaption,
    "Swaption",
    finstack_quant_valuations::instruments::rates::swaption::Swaption
);

instrument_surface!(
    JsSwaption,
    "Swaption",
    JsSwaptionBuilder,
    "generated.valuations.Swaption"
);

getters!(JsSwaption, "Swaption", |i| {
        /// Option type of the swaption.
        /// @returns `"call"` (payer) or `"put"` (receiver).
        option_type as optionType: "generated.valuations.Swaption['option_type']" => json(i.option_type),
        /// Notional of the underlying swap.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Option expiry date.
        /// @returns The expiry date.
        expiry as expiry: "string" => date(i.expiry),
        /// Settlement method.
        /// @returns `"physical"` or `"cash"`.
        settlement as settlement: "generated.valuations.Swaption['settlement']" => json(i.settlement),
        /// Cash settlement annuity method (serde string).
        /// @returns `"collateralized_cash_price"`, `"par_yield"`, `"isda_par_par"` or `"zero_coupon"`.
        cash_settlement_method as cashSettlementMethod: "generated.valuations.Swaption['cash_settlement_method']" => json(i.cash_settlement_method),
        /// Volatility model.
        /// @returns `"black"` or `"normal"`.
        vol_model as volModel: "generated.valuations.Swaption['vol_model']" => json(i.vol_model),
        /// Volatility surface identifier.
        /// @returns Surface id looked up in the market context.
        vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
        /// Fixed leg of the underlying swap.
        /// @returns The fixed leg.
        underlying_fixed_leg as underlyingFixedLeg: "generated.valuations.Swaption['underlying_fixed_leg']" => json(i.underlying_fixed_leg),
        /// Floating leg of the underlying swap.
        /// @returns The floating leg.
        underlying_float_leg as underlyingFloatLeg: "generated.valuations.Swaption['underlying_float_leg']" => json(i.underlying_float_leg),
        /// SABR parameters (`alpha`, `beta`, `nu`, `rho`, `shift`).
        /// @returns The parameter plain object, or `null`.
        sabr_params as sabrParams: "NonNullable<generated.valuations.Swaption['sabr_params']> | null" => json(i.sabr_params),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
});

common_getters!(JsSwaption, "Swaption");

accessors!(JsSwaption, "Swaption", |i| {
        /// Fixed strike of the underlying swap (mirrors Rust `get_strike`).
        /// @returns Strike as a decimal rate.
        get_strike as getStrike: "number" => dec(i.get_strike()),
        /// Effective date of the underlying swap (mirrors Rust `get_underlying_start_date`).
        /// @returns The underlying start date.
        get_underlying_start_date as getUnderlyingStartDate: "string" => date(i.get_underlying_start_date()),
        /// Maturity of the underlying swap (mirrors Rust `get_underlying_maturity`).
        /// @returns The underlying end date.
        get_underlying_maturity as getUnderlyingMaturity: "string" => date(i.get_underlying_maturity()),
});

factories!(JsSwaption, "Swaption", finstack_quant_valuations::instruments::rates::swaption::Swaption, {
        /// Canonical European 1Yx5Y USD payer swaption (mirrors Rust `Swaption::example`): cash-settled, Black vol, 3% strike on a 5-year swap, vol surface `USD-SWPNVOL`.
        /// @returns The example swaption.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

market_metrics!(JsSwaption, "Swaption", {
        /// Forward swap rate of the underlying (mirrors Rust `Swaption::forward_swap_rate`).
        /// @param marketJson - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param asOf - ISO-8601 valuation date.
        /// @returns Par swap rate of the underlying as a decimal.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        forward_swap_rate as forwardSwapRate,
});

builder_class!(
    /// Fluent builder for `Swaption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.Swaption.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsSwaptionBuilder,
    "SwaptionBuilder",
    finstack_quant_valuations::instruments::rates::swaption::SwaptionBuilder,
    JsSwaption,
    |b| b.build()
);

setters!(JsSwaptionBuilder, "SwaptionBuilder", {
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the option type.
        /// @param value - Option type (serde string). `"call"` is a payer, `"put"` a receiver swaption.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType: "generated.valuations.Swaption['option_type']" => js_wire,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry: "string" => js_date,
        /// Set the settlement method.
        /// @param value - Settlement method (serde string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement: "generated.valuations.Swaption['settlement']" => js_wire,
        /// Set the cash settlement annuity method (only used when `settlement` is `"cash"`).
        /// @param value - Cash settlement annuity method (only used when `settlement` is `"cash"`) (serde string). `"collateralized_cash_price"` discounts the physical fixed-leg annuity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cash_settlement_method as cashSettlementMethod: "generated.valuations.Swaption['cash_settlement_method']" => js_wire,
        /// Set the volatility model used for pricing.
        /// @param value - Volatility model used for pricing (serde string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_model as volModel: "generated.valuations.Swaption['vol_model']" => js_wire,
        /// Set the volatility surface identifier.
        /// @param value - Volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId: "string" => id,
        /// Set the complete fixed leg of the underlying swap.
        /// @param value - Fixed leg of the underlying swap.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_fixed_leg as underlyingFixedLeg: "generated.valuations.Swaption['underlying_fixed_leg']" => from_js_json,
        /// Set the complete floating leg of the underlying swap.
        /// @param value - Floating leg of the underlying swap.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_float_leg as underlyingFloatLeg: "generated.valuations.Swaption['underlying_float_leg']" => from_js_json,
        /// Set the SABR volatility model parameters.
        /// @param value - Rust `SabrParameters` in serde form (plain object or JSON string), e.g. `{"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": `null`}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        sabr_params as sabrParams: "NonNullable<generated.valuations.Swaption['sabr_params']>" => from_js_json,
});

common_setters!(JsSwaptionBuilder, "SwaptionBuilder");

handle_setters!(JsSwaptionBuilder, "SwaptionBuilder", {
        /// Set the notional of the underlying swap.
        /// @param value - Notional of the underlying swap, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `CapFloor` instrument (interest-rate cap, floor, caplet or floorlet).
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const capFloor = valuations.instruments.CapFloor.example();
    /// console.log(capFloor.id, capFloor.toDict());
    /// capFloor.free();
    /// ```
    JsCapFloor,
    "CapFloor",
    finstack_quant_valuations::instruments::rates::cap_floor::CapFloor
);

instrument_surface!(
    JsCapFloor,
    "CapFloor",
    JsCapFloorBuilder,
    "generated.valuations.CapFloor"
);

getters!(JsCapFloor, "CapFloor", |i| {
        /// Option type of the cap/floor.
        /// @returns `"cap"`, `"floor"`, `"caplet"` or `"floorlet"`.
        rate_option_type as rateOptionType: "generated.valuations.CapFloor['rate_option_type']" => json(i.rate_option_type),
        /// Notional amount.
        /// @returns Currency-tagged notional.
        notional as notional: "Money" => money(i.notional),
        /// Strike as a decimal rate.
        /// @returns `0.03` for 3%.
        strike as strike: "number" => dec(i.strike),
        /// Contractual margin added to the index, in basis points.
        /// @returns `0.0` when unset.
        spread_bp as spreadBp: "number" => dec(i.spread_bp),
        /// Start date of the underlying period.
        /// @returns The start date.
        start_date as startDate: "string" => date(i.start_date),
        /// End date of the underlying period.
        /// @returns The maturity date.
        maturity as maturity: "string" => date(i.maturity),
        /// Payment frequency.
        /// @returns The payment tenor.
        frequency as frequency: "Tenor" => tenor(i.frequency),
        /// Accrual day-count convention.
        /// @returns The day count.
        day_count as dayCount: "DayCount" => day_count(i.day_count),
        /// Stub-period handling rule for the schedule.
        /// @returns Stub rule serde name (`"none"`, `"short_front"`, ...).
        stub as stub: "NonNullable<generated.valuations.CapFloor['stub']>" => json(i.stub),
        /// Business day convention (serde string).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention: "NonNullable< generated.valuations.CapFloor['business_day_convention'] >" => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns Calendar id, or `null`.
        calendar_id as calendarId: "string | null" => opt_text(i.calendar_id),
        /// Exercise style (serde string).
        /// @returns `"european"` unless overridden.
        exercise_style as exerciseStyle: "NonNullable<generated.valuations.CapFloor['exercise_style']>" => json(i.exercise_style),
        /// Settlement type (serde string).
        /// @returns `"cash"` unless overridden.
        settlement as settlement: "NonNullable<generated.valuations.CapFloor['settlement']>" => json(i.settlement),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
        /// Forward curve identifier.
        /// @returns Curve id used to project the index.
        forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
        /// Volatility surface identifier.
        /// @returns Surface id looked up in the market context.
        vol_surface_id as volSurfaceId: "string" => text(i.vol_surface_id),
        /// Volatility convention.
        /// @returns `"lognormal"`, `"shifted_lognormal"`, `"normal"` or `"auto"`.
        vol_type as volType: "NonNullable<generated.valuations.CapFloor['vol_type']>" => json(i.vol_type),
        /// Displacement shift for shifted-lognormal pricing.
        /// @returns Non-negative shift; `0.0` when unset.
        vol_shift as volShift: "NonNullable<generated.valuations.CapFloor['vol_shift']>" => json(i.vol_shift),
        /// Overnight (RFR) coupon convention in serde form.
        /// @returns The convention plain object, or `null`.
        overnight_coupon as overnightCoupon: "NonNullable<generated.valuations.CapFloor['overnight_coupon']> | null" => json(i.overnight_coupon),
        /// Dated premium paid by the holder.
        /// @returns `(payment_date, amount)` or `null`.
        premium as premium: "[string, Money] | null" => dated_money(i.premium),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel: "string" => text(Instrument::default_model(i)),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
});

common_getters!(JsCapFloor, "CapFloor");

factories!(JsCapFloor, "CapFloor", finstack_quant_valuations::instruments::rates::cap_floor::CapFloor, {
        /// Canonical 5-year USD 3% cap (mirrors Rust `CapFloor::example`): quarterly ACT/360 on `USD-SOFR-3M` discounted on `USD-OIS` with vol surface `USD-CAPFLOOR-VOL`.
        /// @returns The example cap.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

builder_class!(
    /// Fluent builder for `CapFloor`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const builder = valuations.instruments.CapFloor.builder();
    /// console.log(typeof builder.build);
    /// builder.free();
    /// ```
    JsCapFloorBuilder,
    "CapFloorBuilder",
    finstack_quant_valuations::instruments::rates::cap_floor::CapFloorBuilder,
    JsCapFloor,
    |b| b.build()
);

setters!(JsCapFloorBuilder, "CapFloorBuilder", {
        /// Set the instrument identifier.
        /// @param value - Instrument identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id: "string" => id,
        /// Set the option type.
        /// @param value - Option type (serde string). `"cap"`/`"floor"` price a series of caplets/floorlets, `"caplet"`/`"floorlet"` a single period.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate_option_type as rateOptionType: "generated.valuations.CapFloor['rate_option_type']" => js_wire,
        /// Set the strike rate of every caplet/floorlet.
        /// @param value - Strike rate as a decimal (`0.05` = 5%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike: "number" => dec,
        /// Set the contractual margin added to the referenced rate.
        /// @param value - Margin in basis points (`10` = 10bp), added after projecting the index.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spread_bp as spreadBp: "number" => dec,
        /// Set the start date of the underlying period.
        /// @param value - Start date of the underlying period as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        start_date as startDate: "string" => js_date,
        /// Set the end date of the underlying period.
        /// @param value - End date of the underlying period as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity: "string" => js_date,
        /// Set the stub rule.
        /// @param value - Stub rule (serde string). Default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub: "NonNullable<generated.valuations.CapFloor['stub']>" => js_wire,
        /// Set the business day convention.
        /// @param value - Business day convention (serde string). Default `"modified_following"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention: "NonNullable<generated.valuations.CapFloor['business_day_convention']>" => js_wire,
        /// Set the holiday calendar identifier for schedule and roll conventions.
        /// @param value - Holiday calendar identifier for schedule and roll conventions.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId: "string" => id,
        /// Set the exercise style.
        /// @param value - Exercise style (serde string). Default `"european"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle: "NonNullable<generated.valuations.CapFloor['exercise_style']>" => js_wire,
        /// Set the settlement type.
        /// @param value - Settlement type (serde string). Default `"cash"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement: "NonNullable<generated.valuations.CapFloor['settlement']>" => js_wire,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId: "string" => id,
        /// Set the forward curve identifier.
        /// @param value - Forward curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        forward_curve_id as forwardCurveId: "string" => id,
        /// Set the volatility surface identifier.
        /// @param value - Volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId: "string" => id,
        /// Set the volatility type convention.
        /// @param value - Volatility type convention (serde string). Must match the configured surface; `"auto"` (the default when unset) follows source convention and displacement metadata. Normal quotes use decimal rate units; Black quotes use dimensionless annual volatility. Incompatible model/source conventions raise `ValueError`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_type as volType: "NonNullable<generated.valuations.CapFloor['vol_type']>" => js_wire,
        /// Set the displacement shift used for shifted-lognormal pricing.
        /// @param value - Displacement added to forward and strike; must be non-negative.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_shift as volShift: "number" => js_f64,
        /// Set the overnight (RFR) coupon convention for compounded caplets.
        /// @param value - Rust `OvernightCouponConvention` in serde form (plain object or JSON string), e.g. `{"compounding": {"compounded_in_arrears": {"lookback_days": 0}}, "payment_lag_days": 2}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        overnight_coupon as overnightCoupon: "NonNullable<generated.valuations.CapFloor['overnight_coupon']>" => from_js_json,
});

common_setters!(JsCapFloorBuilder, "CapFloorBuilder");

handle_setters!(JsCapFloorBuilder, "CapFloorBuilder", {
        /// Set the notional amount.
        /// @param value - Notional amount, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
        /// Set the payment frequency.
        /// @param value - Payment frequency, as a `Tenor` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        frequency as frequency => JsTenor,
        /// Set the day count convention.
        /// @param value - Day count convention, as a `DayCount` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        day_count as dayCount => JsDayCount,
});

#[wasm_bindgen(js_class = InterestRateSwap)]
impl JsInterestRateSwap {
    /// Create a swap from the conventions registered for a rate index.
    ///
    /// Mirrors Rust `InterestRateSwap::from_conventions`: payment frequency,
    /// day counts, calendar, business-day convention, reset and payment lags
    /// all come from the rate-index convention registry.
    /// @param id - Unique instrument identifier.
    /// @param notional - Notional shared by both legs.
    /// @param side - Fixed-leg direction: `pay` (pay fixed, receive floating) or `receive`.
    /// @param fixedRate - Fixed coupon as a decimal (`0.035` = 3.5%).
    /// @param startDate - Effective date as an ISO-8601 string.
    /// @param maturity - Maturity date as an ISO-8601 string.
    /// @param indexId - Rate index whose conventions apply, e.g. `"USD-SOFR"`.
    /// @param discountCurveId - Discount curve identifier used for both legs.
    /// @param forwardCurveId - Forward curve identifier projecting the floating leg.
    /// @returns The validated swap.
    /// @throws Error - Throws with kind `not_found` if `indexId` has no registered conventions, kind `validation` if `side` or a date is malformed or the swap fails validation, and kind `invalid_type` for a wrong argument type.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = fromConventions)]
    pub fn from_conventions(
        #[wasm_bindgen(unchecked_param_type = "string")] id: JsValue,
        notional: &JsMoney,
        #[wasm_bindgen(unchecked_param_type = "'pay' | 'receive'")] side: JsValue,
        #[wasm_bindgen(js_name = "fixedRate", unchecked_param_type = "number")] fixed_rate: JsValue,
        #[wasm_bindgen(js_name = "startDate", unchecked_param_type = "string")] start_date: JsValue,
        #[wasm_bindgen(unchecked_param_type = "string")] maturity: JsValue,
        #[wasm_bindgen(js_name = "indexId", unchecked_param_type = "string")] index_id: JsValue,
        #[wasm_bindgen(js_name = "discountCurveId", unchecked_param_type = "string")]
        discount_curve_id: JsValue,
        #[wasm_bindgen(js_name = "forwardCurveId", unchecked_param_type = "string")]
        forward_curve_id: JsValue,
    ) -> Result<JsInterestRateSwap, JsValue> {
        let index_id = crate::utils::input::js_string(&index_id, "indexId")?;
        let discount_curve_id =
            crate::utils::input::js_string(&discount_curve_id, "discountCurveId")?;
        let forward_curve_id = crate::utils::input::js_string(&forward_curve_id, "forwardCurveId")?;
        let params = finstack_quant_valuations::instruments::rates::irs::ConventionSwapParams {
            id: super::arg::id(&id, "id")?,
            notional: notional.inner,
            side: js_wire(&side, "side")?,
            fixed_rate: js_f64(&fixed_rate, "fixedRate")?,
            start_date: js_date(&start_date, "startDate")?,
            maturity: js_date(&maturity, "maturity")?,
            index_id: &index_id,
            discount_curve_id: &discount_curve_id,
            forward_curve_id: &forward_curve_id,
        };
        finstack_quant_valuations::instruments::rates::irs::InterestRateSwap::from_conventions(
            params,
        )
        .map(|inner| Self { inner })
        .map_err(to_js_err)
    }
}

dated_money_setter!(
    JsCapFloorBuilder,
    "CapFloorBuilder",
    /// Set the option premium paid on a given date.
    /// @param paymentDate - Premium payment date as an ISO-8601 string.
    /// @param amount - Premium amount; its currency must match the notional currency.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `validation` if `paymentDate` is malformed or the builder was already consumed by `build()`, and kind `invalid_type` if `paymentDate` is not a string.
    premium
);
