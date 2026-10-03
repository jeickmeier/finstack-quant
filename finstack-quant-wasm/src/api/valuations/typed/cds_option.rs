//! Typed `CdsOption` class (option on a CDS spread or clean index price) and
//! its fluent builder.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use wasm_bindgen::prelude::*;

instrument_class!(
    /// Typed wrapper for the Rust `CdsOption` instrument: a European option on a single-name or index CDS, struck on a forward spread or a clean index price. A call is the right to buy protection (payer), a put the right to sell it (receiver); `delta`, `gamma`, `vega`, `theta` and `implied_vol` are metric ids of `price` / `metric`.
    JsCdsOption,
    "CdsOption",
    finstack_quant_valuations::instruments::CdsOption
);

instrument_builder_entry!(JsCdsOption, "CdsOption", JsCdsOptionBuilder);

instrument_to_dict!(JsCdsOption, "CdsOption");

instrument_market_dependencies!(JsCdsOption, "CdsOption");

instrument_pricing!(JsCdsOption, "CdsOption");

getters!(JsCdsOption, "CdsOption", |i| {
        /// Canonical model key used when `model="default"` is passed to `price`.
        /// @returns Registered model key of the Bloomberg CDSO quadrature pricer.
        default_model as defaultModel => text(Instrument::default_model(i)),
        /// Instrument attributes (tags and metadata) used for scenario selection.
        /// @returns The attribute bag; empty when none were set.
        attributes as attributes => json(i.attributes),
        /// Option strike in its serde form.
        /// @returns `{ spread: "<decimal rate>" }` or `{ clean_price_pct: "<price points>" }`.
        strike as strike => json(i.strike),
        /// Option type.
        /// @returns `"call"` (right to buy protection) or `"put"` (right to sell protection).
        option_type as optionType => json(i.option_type),
        /// Exercise style; only European prices.
        /// @returns `"european"`, `"american"` or `"bermudan"`.
        exercise_style as exerciseStyle => json(i.exercise_style),
        /// Legal option expiry date.
        /// @returns ISO-8601 expiry date.
        expiry as expiry => date(i.expiry),
        /// Maturity of the underlying CDS.
        /// @returns ISO-8601 maturity date.
        underlying_maturity as underlyingMaturity => date(i.underlying_maturity),
        /// Option notional.
        /// @returns Currency-tagged notional of the underlying CDS.
        notional as notional => money(i.notional),
        /// Settlement type of the exercise proceeds.
        /// @returns `"cash"` or `"physical"`.
        settlement as settlement => json(i.settlement),
        /// Explicit option premium payment date.
        /// @returns The date, or `null` for the convention settlement lag.
        premium_settlement_date as premiumSettlementDate => opt_date(i.premium_settlement_date),
        /// Explicit exercise proceeds payment date.
        /// @returns The date, or `null` for legal expiry.
        exercise_settlement_date as exerciseSettlementDate => opt_date(i.exercise_settlement_date),
        /// Explicit accrual-effective date of the underlying CDS.
        /// @returns The date, or `null` when `protectionStartConvention` selects it.
        underlying_start_date as underlyingStartDate => opt_date(i.underlying_start_date),
        /// Accrual-start convention of the synthetic underlying CDS.
        /// @returns `"spot"` or `"forward"`.
        protection_start_convention as protectionStartConvention => json(i.protection_start_convention),
        /// Whether the option knocks out on default before expiry.
        /// @returns `true` for knock-out options.
        knockout as knockout => json(i.knockout),
        /// Recovery rate assumption.
        /// @returns Recovery as a decimal (`0.4` = 40%).
        recovery_rate as recoveryRate => json(i.recovery_rate),
        /// Discount curve identifier.
        /// @returns Discount curve id in the market context.
        discount_curve_id as discountCurveId => text(i.discount_curve_id),
        /// Hazard (credit) curve identifier.
        /// @returns Hazard curve id in the market context.
        credit_curve_id as creditCurveId => text(i.credit_curve_id),
        /// Volatility surface identifier.
        /// @returns Vol surface id in the market context.
        vol_surface_id as volSurfaceId => text(i.vol_surface_id),
        /// ISDA convention of the underlying CDS (serde name).
        /// @returns `"isda_na"`, `"isda_eu"`, `"isda_as"` or `"custom"`.
        underlying_convention as underlyingConvention => json(i.underlying_convention),
        /// Whether the underlying is a CDS index.
        /// @returns `true` for an index option, `false` for a single name.
        underlying_is_index as underlyingIsIndex => json(i.underlying_is_index),
        /// Current index factor `f`.
        /// @returns Surviving fraction of original index notional, in `(0, 1]`.
        index_factor as indexFactor => json(i.index_factor),
        /// Original index factor `f0` of a clean-price strike.
        /// @returns The factor, or `null` for spread strikes.
        strike_index_factor as strikeIndexFactor => json(i.strike_index_factor),
        /// Settled cumulative index loss since option inception.
        /// @returns Decimal fraction of original index notional.
        realized_loss as realizedLoss => json(i.realized_loss),
        /// Running coupon of the underlying CDS in basis points.
        /// @returns The coupon, or `null` when the strike spread is the coupon.
        coupon_bp as couponBp => json(i.coupon_bp.and_then(|d| rust_decimal::prelude::ToPrimitive::to_f64(&d))),
});

factories!(JsCdsOption, "CdsOption", finstack_quant_valuations::instruments::CdsOption, {
        /// Canonical 100bp-strike call on a 5-year USD 10,000,000 corporate CDS (mirrors Rust `CdsOption::example`): expiry 2025-06-20, CDS maturity 2030-06-20, cash settlement, 40% recovery, curves `USD-OIS` / `CORP-HAZARD`, vol surface `CDSOPT-VOL`.
        /// @returns The example option.
        /// @throws Error - Throws if the canonical example fails validation (does not occur for a released build).
        example as example,
});

builder_class!(
    /// Fluent builder for `CdsOption`; one setter per Rust `FinancialBuilder` setter.
    ///
    /// Required: `id`, `strike`, `optionType`, `exerciseStyle`, `expiry`,
    /// `underlyingMaturity`, `notional`, `settlement`, `recoveryRate`,
    /// `discountCurveId`, `creditCurveId`, `volSurfaceId` and
    /// `underlyingIsIndex`. Every setter returns the builder, so calls chain;
    /// `build()` validates and consumes it.
    JsCdsOptionBuilder,
    "CdsOptionBuilder",
    finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOptionBuilder,
    JsCdsOption,
    |b| b.build()
);

setters!(JsCdsOptionBuilder, "CdsOptionBuilder", {
        /// Set the instrument identifier.
        /// @param value - Unique identifier for the option.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        id as id => id,
        /// Set the strike.
        /// @param value - `{ spread: "0.0325" }` (decimal forward spread) or `{ clean_price_pct: "107.0" }` (clean price points), as a plain object or JSON string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        strike as strike => json,
        /// Set the option type.
        /// @param value - `"call"` buys protection at expiry, `"put"` sells it.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        option_type as optionType => en,
        /// Set the exercise style.
        /// @param value - `"european"`, `"american"` or `"bermudan"`; pricing supports European only.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_style as exerciseStyle => en,
        /// Set the option expiry date.
        /// @param value - Legal expiry as an ISO-8601 string; must precede the underlying maturity.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        expiry as expiry => date,
        /// Set the underlying CDS maturity date.
        /// @param value - Maturity of the CDS delivered or cash-settled at exercise, as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_maturity as underlyingMaturity => date,
        /// Set the settlement type.
        /// @param value - `"cash"` or `"physical"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        settlement as settlement => en,
        /// Set the option premium payment date.
        /// @param value - Premium payment date as an ISO-8601 string; when never set the CDS convention settlement lag applies.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        premium_settlement_date as premiumSettlementDate => date,
        /// Set the exercise proceeds payment date.
        /// @param value - On or after expiry and before CDS maturity, as an ISO-8601 string; when never set legal expiry is used.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        exercise_settlement_date as exerciseSettlementDate => date,
        /// Set the underlying CDS accrual-effective date.
        /// @param value - Accrual start for the forward spread and risky annuity, as an ISO-8601 string.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_start_date as underlyingStartDate => date,
        /// Set the underlying accrual-start convention.
        /// @param value - `"spot"` (default, prior CDS roll) or `"forward"` (option expiry).
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        protection_start_convention as protectionStartConvention => en,
        /// Set whether the option knocks out on default before expiry.
        /// @param value - `true` for knock-out single-name options; default `false`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        knockout as knockout => flag,
        /// Set the recovery rate assumption.
        /// @param value - Recovery as a decimal in `[0, 1]`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        recovery_rate as recoveryRate => num,
        /// Set the discount curve identifier.
        /// @param value - Discount curve id in the market context.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        discount_curve_id as discountCurveId => id,
        /// Set the hazard (credit) curve identifier.
        /// @param value - Hazard curve id in the market context.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        credit_curve_id as creditCurveId => id,
        /// Set the volatility surface identifier.
        /// @param value - Vol surface id in the market context.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        vol_surface_id as volSurfaceId => id,
        /// Set the ISDA convention of the underlying CDS.
        /// @param value - `"isda_na"` (default), `"isda_eu"`, `"isda_as"` or `"custom"`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        underlying_convention as underlyingConvention => en,
        /// Set whether the underlying is a CDS index.
        /// @param value - `true` for an index option (no knock-out, index factor applies), `false` for a single name.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        underlying_is_index as underlyingIsIndex => flag,
        /// Set the current index factor.
        /// @param value - Surviving fraction of the original index notional, in `(0, 1]`; default `1.0`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        index_factor as indexFactor => num,
        /// Set the original index factor of a clean-price strike.
        /// @param value - `f0` the clean-price strike is quoted on.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        strike_index_factor as strikeIndexFactor => num,
        /// Set the settled cumulative index loss since option inception.
        /// @param value - Decimal fraction of original index notional in `[0, 1]`; default `0.0`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if the builder was already consumed by `build()`.
        realized_loss as realizedLoss => num,
        /// Set the running coupon of the underlying CDS.
        /// @param value - Coupon in basis points (`100` for CDX.NA.IG, `500` for CDX.NA.HY); when never set the strike spread is the coupon.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        coupon_bp as couponBp => dec,
        /// Set instrument attributes (tags and metadata).
        /// @param value - Attribute bag; a plain object populates `meta` and an optional `"tags"` entry holding a list of strings populates `tags`.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `invalid_type` if `value` has the wrong JavaScript type, and kind `validation` if it cannot be converted or the builder was already consumed by `build()`.
        attributes as attributes => json,
});

handle_setters!(JsCdsOptionBuilder, "CdsOptionBuilder", {
        /// Set the option notional.
        /// @param value - Positive notional of the underlying CDS, as a `Money` handle.
        /// @returns The builder, for chaining.
        /// @throws Error - Throws with kind `validation` if the builder was already consumed by `build()`.
        notional as notional => JsMoney,
});
