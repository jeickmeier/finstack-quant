//! Typed `InterestRateSwap`, `Swaption` and `CapFloor` classes and their
//! fluent builders.

use crate::api::core::dates::{JsDayCount, JsTenor};
use crate::api::core::money::JsMoney;
use crate::utils::to_js_err;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `InterestRateSwap` instrument (fixed-for-floating swap).
    JsInterestRateSwap,
    "InterestRateSwap",
    finstack_quant_valuations::instruments::InterestRateSwap
);

instrument_builder_entry!(
    JsInterestRateSwap,
    "InterestRateSwap",
    JsInterestRateSwapBuilder
);

instrument_to_dict!(JsInterestRateSwap, "InterestRateSwap");

instrument_market_dependencies!(JsInterestRateSwap, "InterestRateSwap");

instrument_pricing!(JsInterestRateSwap, "InterestRateSwap");

getters!(JsInterestRateSwap, "InterestRateSwap", |i| {
        /// Notional shared by both legs.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Swap direction for the fixed leg.
        /// @returns `"pay"` or `"receive"`.
        side as side => json(i.side),
        /// Fixed leg specification.
        /// @returns The fixed leg.
        fixed_leg as fixedLeg => json(i.fixed_leg),
        /// Floating leg specification.
        /// @returns The floating leg.
        float_leg as floatLeg => json(i.float_leg),
        /// OTC margin (CSA / initial-margin) specification in serde form.
        /// @returns The spec plain object, or `null`.
        margin_spec as marginSpec => json(i.margin_spec),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsInterestRateSwap, "InterestRateSwap", finstack_quant_valuations::instruments::InterestRateSwap, {
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
        id as id => id,
        /// Set the swap direction for the fixed leg.
        /// @param value - Swap direction for the fixed leg (serde string). `"pay"` pays fixed / receives floating.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        side as side => en,
        /// Set the fixed leg specification.
        /// @param value - Fixed leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        fixed_leg as fixedLeg => json,
        /// Set the floating leg specification.
        /// @param value - Floating leg specification.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        float_leg as floatLeg => json,
        /// Set the OTC margin (CSA / initial-margin) specification.
        /// @param value - Rust `OtcMarginSpec` in serde form (plain object or JSON string), e.g. the `margin_spec` value of a margined swap's `toDict()`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        margin_spec as marginSpec => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsInterestRateSwapBuilder, "InterestRateSwapBuilder", {
        /// Set the notional shared by both legs.
        /// @param value - Notional shared by both legs, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `Swaption` instrument (European, American or Bermudan option on a swap).
    JsSwaption,
    "Swaption",
    finstack_quant_valuations::instruments::Swaption
);

instrument_builder_entry!(JsSwaption, "Swaption", JsSwaptionBuilder);

instrument_to_dict!(JsSwaption, "Swaption");

instrument_market_dependencies!(JsSwaption, "Swaption");

instrument_pricing!(JsSwaption, "Swaption");

getters!(JsSwaption, "Swaption", |i| {
        /// Option type of the swaption.
        /// @returns `"call"` (payer) or `"put"` (receiver).
        option_type as optionType => json(i.option_type),
        /// Notional of the underlying swap.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Option expiry date.
        /// @returns The expiry date.
        expiry as expiry => date(i.expiry),
        /// Exercise style of the swaption.
        /// @returns `"european"`, `"bermudan"` or `"american"`.
        exercise_style as exerciseStyle => json(i.exercise_style),
        /// Settlement method.
        /// @returns `"physical"` or `"cash"`.
        settlement as settlement => json(i.settlement),
        /// Cash settlement annuity method (serde string).
        /// @returns `"collateralized_cash_price"`, `"par_yield"`, `"isda_par_par"` or `"zero_coupon"`.
        cash_settlement_method as cashSettlementMethod => json(i.cash_settlement_method),
        /// Volatility model.
        /// @returns `"black"` or `"normal"`.
        vol_model as volModel => json(i.vol_model),
        /// Volatility surface identifier.
        /// @returns Surface id looked up in the market context.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
        /// Fixed leg of the underlying swap.
        /// @returns The fixed leg.
        underlying_fixed_leg as underlyingFixedLeg => json(i.underlying_fixed_leg),
        /// Floating leg of the underlying swap.
        /// @returns The floating leg.
        underlying_float_leg as underlyingFloatLeg => json(i.underlying_float_leg),
        /// SABR parameters (`alpha`, `beta`, `nu`, `rho`, `shift`).
        /// @returns The parameter plain object, or `null`.
        sabr_params as sabrParams => json(i.sabr_params),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
});

accessors!(JsSwaption, "Swaption", |i| {
        /// Fixed strike of the underlying swap (mirrors Rust `get_strike`).
        /// @returns Strike as a decimal rate.
        get_strike as getStrike => dec(i.get_strike()),
        /// Effective date of the underlying swap (mirrors Rust `get_underlying_start_date`).
        /// @returns The underlying start date.
        get_underlying_start_date as getUnderlyingStartDate => date(i.get_underlying_start_date()),
        /// Maturity of the underlying swap (mirrors Rust `get_underlying_maturity`).
        /// @returns The underlying end date.
        get_underlying_maturity as getUnderlyingMaturity => date(i.get_underlying_maturity()),
});

factories!(JsSwaption, "Swaption", finstack_quant_valuations::instruments::Swaption, {
        /// Canonical European 1Yx5Y USD payer swaption (mirrors Rust `Swaption::example`): cash-settled, Black vol, 3% strike on a 5-year swap, vol surface `USD-SWPNVOL`.
        /// @returns The example swaption.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
        /// Bermudan-exercise variant of the example (mirrors Rust `Swaption::example_bermudan`).
        /// @returns The example swaption with `exercise_style == "bermudan"`.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example_bermudan as exampleBermudan,
});

market_metrics!(JsSwaption, "Swaption", {
        /// Forward swap rate of the underlying (mirrors Rust `Swaption::forward_swap_rate`).
        /// @param market_json - Canonical market-context JSON (string or plain object) supplying the curves and quotes this instrument needs.
        /// @param as_of - ISO-8601 valuation date.
        /// @returns Par swap rate of the underlying as a decimal.
        /// @throws Error - Throws with kind `not_found` if required market data is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if the calculation fails.
        forward_swap_rate as forwardSwapRate,
});

builder_class!(
    /// Fluent builder for `Swaption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Every setter returns the builder, so calls chain; `build()` validates
    /// and consumes it.
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
        id as id => id,
        /// Set the option type.
        /// @param value - Option type (serde string). `"call"` is a payer, `"put"` a receiver swaption.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType => en,
        /// Set the option expiry date.
        /// @param value - Option expiry date as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry => date,
        /// Set the exercise style.
        /// @param value - Exercise style (serde string). Default `"european"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle => en,
        /// Set the settlement method.
        /// @param value - Settlement method (serde string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement => en,
        /// Set the cash settlement annuity method (only used when `settlement` is `"cash"`).
        /// @param value - Cash settlement annuity method (only used when `settlement` is `"cash"`) (serde string). `"collateralized_cash_price"` discounts the physical fixed-leg annuity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        cash_settlement_method as cashSettlementMethod => en,
        /// Set the volatility model used for pricing.
        /// @param value - Volatility model used for pricing (serde string).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_model as volModel => en,
        /// Set the volatility surface identifier.
        /// @param value - Volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set the complete fixed leg of the underlying swap.
        /// @param value - Fixed leg of the underlying swap.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_fixed_leg as underlyingFixedLeg => json,
        /// Set the complete floating leg of the underlying swap.
        /// @param value - Floating leg of the underlying swap.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_float_leg as underlyingFloatLeg => json,
        /// Set the SABR volatility model parameters.
        /// @param value - Rust `SabrParameters` in serde form (plain object or JSON string), e.g. `{"alpha": 0.025, "beta": 0.5, "nu": 0.4, "rho": -0.3, "shift": `null`}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        sabr_params as sabrParams => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsSwaptionBuilder, "SwaptionBuilder", {
        /// Set the notional of the underlying swap.
        /// @param value - Notional of the underlying swap, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});

instrument_class!(
    /// Typed wrapper for the Rust `CapFloor` instrument (interest-rate cap, floor, caplet or floorlet).
    JsCapFloor,
    "CapFloor",
    finstack_quant_valuations::instruments::CapFloor
);

instrument_builder_entry!(JsCapFloor, "CapFloor", JsCapFloorBuilder);

instrument_to_dict!(JsCapFloor, "CapFloor");

instrument_market_dependencies!(JsCapFloor, "CapFloor");

instrument_pricing!(JsCapFloor, "CapFloor");

getters!(JsCapFloor, "CapFloor", |i| {
        /// Option type of the cap/floor.
        /// @returns `"cap"`, `"floor"`, `"caplet"` or `"floorlet"`.
        rate_option_type as rateOptionType => json(i.rate_option_type),
        /// Notional amount.
        /// @returns Currency-tagged notional.
        notional as notional => money(i.notional),
        /// Strike as a decimal rate.
        /// @returns `0.03` for 3%.
        strike as strike => dec(i.strike),
        /// Contractual margin added to the index, in basis points.
        /// @returns `0.0` when unset.
        spread_bp as spreadBp => dec(i.spread_bp),
        /// Start date of the underlying period.
        /// @returns The start date.
        start_date as startDate => date(i.start_date),
        /// End date of the underlying period.
        /// @returns The maturity date.
        maturity as maturity => date(i.maturity),
        /// Payment frequency.
        /// @returns The payment tenor.
        frequency as frequency => tenor(i.frequency),
        /// Accrual day-count convention.
        /// @returns The day count.
        day_count as dayCount => day_count(i.day_count),
        /// Stub-period handling rule for the schedule.
        /// @returns Stub rule serde name (`"none"`, `"short_front"`, ...).
        stub as stub => json(i.stub),
        /// Business day convention (serde string).
        /// @returns `"modified_following"` unless overridden.
        business_day_convention as businessDayConvention => json(i.business_day_convention),
        /// Holiday calendar identifier.
        /// @returns Calendar id, or `null`.
        calendar_id as calendarId => opt_text(i.calendar_id),
        /// Exercise style (serde string).
        /// @returns `"european"` unless overridden.
        exercise_style as exerciseStyle => json(i.exercise_style),
        /// Settlement type (serde string).
        /// @returns `"cash"` unless overridden.
        settlement as settlement => json(i.settlement),
        /// Discount curve identifier.
        /// @returns Curve id used for discounting.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Forward curve identifier.
        /// @returns Curve id used to project the index.
        forward_curve_id as forwardCurveId => text(i.forward_curve_id),
        /// Volatility surface identifier.
        /// @returns Surface id looked up in the market context.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
        /// Volatility convention.
        /// @returns `"lognormal"`, `"shifted_lognormal"`, `"normal"` or `"auto"`.
        vol_type as volType => json(i.vol_type),
        /// Displacement shift for shifted-lognormal pricing.
        /// @returns Non-negative shift; `0.0` when unset.
        vol_shift as volShift => json(i.vol_shift),
        /// Overnight (RFR) coupon convention in serde form.
        /// @returns The convention plain object, or `null`.
        overnight_coupon as overnightCoupon => json(i.overnight_coupon),
        /// Dated premium paid by the holder.
        /// @returns `(payment_date, amount)` or `null`.
        premium as premium => dated_money(i.premium),
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key such as `"discounting"` or `"black76"`.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Expiry date exposed by the Rust `Instrument` trait.
        /// @returns The expiry/maturity date, or `null` when the instrument type reports none.
        expiry as expiry => opt_date(Instrument::expiry(i)),
});

factories!(JsCapFloor, "CapFloor", finstack_quant_valuations::instruments::CapFloor, {
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
        id as id => id,
        /// Set the option type.
        /// @param value - Option type (serde string). `"cap"`/`"floor"` price a series of caplets/floorlets, `"caplet"`/`"floorlet"` a single period.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        rate_option_type as rateOptionType => en,
        /// Set the strike rate of every caplet/floorlet.
        /// @param value - Strike rate as a decimal (`0.05` = 5%).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike => dec,
        /// Set the contractual margin added to the referenced rate.
        /// @param value - Margin in basis points (`10` = 10bp), added after projecting the index.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        spread_bp as spreadBp => dec,
        /// Set the start date of the underlying period.
        /// @param value - Start date of the underlying period as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        start_date as startDate => date,
        /// Set the end date of the underlying period.
        /// @param value - End date of the underlying period as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        maturity as maturity => date,
        /// Set the stub rule.
        /// @param value - Stub rule (serde string). Default `"short_front"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        stub as stub => en,
        /// Set the business day convention.
        /// @param value - Business day convention (serde string). Default `"modified_following"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        business_day_convention as businessDayConvention => en,
        /// Set the holiday calendar identifier for schedule and roll conventions.
        /// @param value - Holiday calendar identifier for schedule and roll conventions.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        calendar_id as calendarId => id,
        /// Set the exercise style.
        /// @param value - Exercise style (serde string). Default `"european"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle => en,
        /// Set the settlement type.
        /// @param value - Settlement type (serde string). Default `"cash"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement => en,
        /// Set the discount curve identifier.
        /// @param value - Discount curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the forward curve identifier.
        /// @param value - Forward curve identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        forward_curve_id as forwardCurveId => id,
        /// Set the volatility surface identifier.
        /// @param value - Volatility surface identifier.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set the volatility type convention.
        /// @param value - Volatility type convention (serde string). Must match the configured surface; `"auto"` (the default when unset) follows source convention and displacement metadata. Normal quotes use decimal rate units; Black quotes use dimensionless annual volatility. Incompatible model/source conventions raise `ValueError`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_type as volType => en,
        /// Set the displacement shift used for shifted-lognormal pricing.
        /// @param value - Displacement added to forward and strike; must be non-negative.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        vol_shift as volShift => num,
        /// Set the overnight (RFR) coupon convention for compounded caplets.
        /// @param value - Rust `OvernightCouponConvention` in serde form (plain object or JSON string), e.g. `{"compounding": {"compounded_in_arrears": {"lookback_days": 0}}, "payment_lag_days": 2}`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        overnight_coupon as overnightCoupon => json,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

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
    /// @param fixed_rate - Fixed coupon as a decimal (`0.035` = 3.5%).
    /// @param start_date - Effective date as an ISO-8601 string.
    /// @param maturity - Maturity date as an ISO-8601 string.
    /// @param index_id - Rate index whose conventions apply, e.g. `"USD-SOFR"`.
    /// @param discount_curve_id - Discount curve identifier used for both legs.
    /// @param forward_curve_id - Forward curve identifier projecting the floating leg.
    /// @returns The validated swap.
    /// @throws Error - Throws with kind `not_found` if `indexId` has no registered conventions, kind `validation` if `side` or a date is malformed or the swap fails validation, and kind `invalid_type` for a wrong argument type.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = fromConventions)]
    pub fn from_conventions(
        id: JsValue,
        notional: &JsMoney,
        side: JsValue,
        fixed_rate: JsValue,
        start_date: JsValue,
        maturity: JsValue,
        index_id: JsValue,
        discount_curve_id: JsValue,
        forward_curve_id: JsValue,
    ) -> Result<JsInterestRateSwap, JsValue> {
        let index_id = crate::utils::input::js_string(&index_id, "indexId")?;
        let discount_curve_id =
            crate::utils::input::js_string(&discount_curve_id, "discountCurveId")?;
        let forward_curve_id = crate::utils::input::js_string(&forward_curve_id, "forwardCurveId")?;
        let params = finstack_quant_valuations::instruments::rates::irs::ConventionSwapParams {
            id: super::arg::id(&id, "id")?,
            notional: notional.inner,
            side: super::arg::en(&side, "side")?,
            fixed_rate: super::arg::num(&fixed_rate, "fixedRate")?,
            start_date: super::arg::date(&start_date, "startDate")?,
            maturity: super::arg::date(&maturity, "maturity")?,
            index_id: &index_id,
            discount_curve_id: &discount_curve_id,
            forward_curve_id: &forward_curve_id,
        };
        finstack_quant_valuations::instruments::InterestRateSwap::from_conventions(params)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }
}

#[wasm_bindgen(js_class = CapFloorBuilder)]
impl JsCapFloorBuilder {
    /// Set the option premium paid on a given date.
    /// @param payment_date - Premium payment date as an ISO-8601 string.
    /// @param amount - Premium amount; its currency must match the notional currency.
    /// @returns The builder, for chaining.
    /// @throws Error - Throws with kind `validation` if `paymentDate` is malformed or the builder was already consumed by `build()`, and kind `invalid_type` if `paymentDate` is not a string.
    pub fn premium(
        &self,
        payment_date: JsValue,
        amount: &JsMoney,
    ) -> Result<JsCapFloorBuilder, JsValue> {
        let premium = (
            super::arg::date(&payment_date, "paymentDate")?,
            amount.inner,
        );
        let inner = self.inner.apply(|b| b.premium(premium))?;
        Ok(Self { inner })
    }
}
