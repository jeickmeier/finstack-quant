//! A commodity forward with contractual delivery and settlement.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::commodity::commodity_forward::{
    CommodityForward, CommodityForwardBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A commodity forward with contractual delivery and settlement.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.CommodityForward.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsCommodityForward, "CommodityForward", CommodityForward
);
instrument_surface!(
    JsCommodityForward,
    "CommodityForward",
    JsCommodityForwardBuilder,
    "generated.valuations.CommodityForward"
);
factories!(JsCommodityForward, "CommodityForward", CommodityForward, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsCommodityForward, "CommodityForward", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying.
    /// @returns Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => json(i.underlying),
    /// Read the contract's quantity.
    /// @returns Contract quantity in units.
    quantity as quantity: "number" => json(i.quantity),
    /// Read the contract's multiplier.
    /// @returns Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).
    multiplier as multiplier: "number" => json(i.multiplier),
    /// Read the contract's maturity.
    /// @returns Settlement/delivery date.
    maturity as maturity: "string" => date(i.maturity),
    /// Read the contract's settlement.
    /// @returns Settlement type (physical or cash). Defaults to cash settlement when omitted in serialized payloads.
    settlement as settlement: "generated.valuations.CommodityForward['settlement']" => json(i.settlement),
    /// Read the contract's position.
    /// @returns Position direction (long or short). - Long: buyer of the commodity at settlement - Short: seller of the commodity at settlement
    position as position: "generated.valuations.CommodityForward['position']" => json(i.position),
    /// Read the contract's contract price.
    /// @returns Contract price (entry/trade price K). If None, the forward is treated as **at-market** (K = F), meaning NPV ≈ 0 at inception (like a newly opened futures position). If Some(K), the forward is **off-market** and NPV reflects the mark-to-market difference: sign × (F - K) × Q × M × DF.
    contract_price as contractPrice: "number | null" => json(i.contract_price),
    /// Read the contract's quoted forward.
    /// @returns Optional quoted forward price (overrides curve lookup for F). This is a market price override, not the contract entry price. Use contract_price for the trade entry price K.
    quoted_forward as quotedForward: "number | null" => json(i.quoted_forward),
    /// Read the contract's forward curve id.
    /// @returns Forward/futures price curve ID for price interpolation. Should reference a PriceCurve in the MarketContext.
    forward_curve_id as forwardCurveId: "string" => text(i.forward_curve_id),
    /// Read the contract's spot id.
    /// @returns Optional observed spot identifier. At maturity the scalar is the delivery observation. After maturity use the exact delivery observation in the corresponding FIXING:{spot_id} series. Before maturity an explicitly identified spot can supply the cost-of-carry input if the forward curve is absent.
    spot_id as spotId: "string | null" => opt_text(i.spot_id),
    /// Read the contract's discount curve id.
    /// @returns Discount curve ID.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
    /// Read the contract's exchange.
    /// @returns Optional exchange identifier (e.g., "NYMEX", "ICE").
    exchange as exchange: "string | null" => opt_text(i.exchange),
    /// Read the contract's contract month.
    /// @returns Optional contract month (e.g., "2025M03" for March 2025).
    contract_month as contractMonth: "string | null" => opt_text(i.contract_month),
    /// Read the contract's convention.
    /// @returns Optional market convention for this commodity. When set, provides default settlement days and calendar if not explicitly specified. See CommodityConvention for available options.
    convention as convention: "NonNullable<generated.valuations.CommodityForward['convention']> | null" => json(i.convention),
    /// Read the contract's settlement days.
    /// @returns Settlement lag in business days (T+N). Defaults to 2 for most commodity markets (T+2). If convention is set, uses the convention's default unless explicitly overridden here. # Market Standards | Market | Settlement | |--------|------------| | Energy (WTI, Brent, NG) | T+2 | | Precious metals | T+2 | | Base metals (LME) | T+2 | | Power | T+1 |
    settlement_days as settlementDays: "number | null" => json(i.settlement_days),
    /// Read the contract's settlement calendar id.
    /// @returns The contract's settlement calendar ID. Required whenever the settlement (payment) date is calculated: no calendar is inferred from convention or defaulted to weekends only. nerc is a power-market label calendar, not a settlement calendar, and is rejected.
    settlement_calendar_id as settlementCalendarId: "string | null" => opt_text(i.settlement_calendar_id),
    /// Read the contract's settlement business day convention.
    /// @returns Business day convention for settlement date adjustment. Defaults to Following for energy commodities, ModifiedFollowing for precious metals.
    settlement_business_day_convention as settlementBusinessDayConvention: "NonNullable<generated.valuations.CommodityForward['settlement_business_day_convention']> | null" => json(i.settlement_business_day_convention),
});
common_getters!(JsCommodityForward, "CommodityForward");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.CommodityForward.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsCommodityForwardBuilder, "CommodityForwardBuilder", CommodityForwardBuilder, JsCommodityForward, |b| b.build()
);

to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsCommodityForward, "CommodityForward", JsCommodityForwardBuilder
);

setters!(JsCommodityForwardBuilder, "CommodityForwardBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Commodity underlying parameters (commodity_type, underlying_ticker, unit, currency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying as underlying: "Pick<generated.valuations.CommodityForward, 'commodity_type' | 'underlying_ticker' | 'unit' | 'currency'>" => js_wire,
    /// Set the contract's quantity.
    /// @param value - Contract quantity in units.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quantity as quantity: "number" => js_f64,
    /// Set the contract's multiplier.
    /// @param value - Contract multiplier (typically 1.0 for OTC forwards, defaults to 1.0).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    multiplier as multiplier: "number" => js_f64,
    /// Set the contract's maturity.
    /// @param value - Settlement/delivery date.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    maturity as maturity: "string" => js_date,
    /// Set the contract's settlement.
    /// @param value - Settlement type (physical or cash). Defaults to cash settlement when omitted in serialized payloads.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement as settlement: "generated.valuations.CommodityForward['settlement']" => js_wire,
    /// Set the contract's position.
    /// @param value - Position direction (long or short). - Long: buyer of the commodity at settlement - Short: seller of the commodity at settlement
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    position as position: "generated.valuations.CommodityForward['position']" => js_wire,
    /// Set the contract's contract price.
    /// @param value - Contract price (entry/trade price K). If None, the forward is treated as **at-market** (K = F), meaning NPV ≈ 0 at inception (like a newly opened futures position). If Some(K), the forward is **off-market** and NPV reflects the mark-to-market difference: sign × (F - K) × Q × M × DF.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    contract_price as contractPrice: "number | null" => opt_f64 via contract_price_opt,
    /// Set the contract's quoted forward.
    /// @param value - Optional quoted forward price (overrides curve lookup for F). This is a market price override, not the contract entry price. Use contract_price for the trade entry price K.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    quoted_forward as quotedForward: "number | null" => opt_f64 via quoted_forward_opt,
    /// Set the contract's forward curve id.
    /// @param value - Forward/futures price curve ID for price interpolation. Should reference a PriceCurve in the MarketContext.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    forward_curve_id as forwardCurveId: "string" => id,
    /// Set the contract's spot id.
    /// @param value - Optional observed spot identifier. At maturity the scalar is the delivery observation. After maturity use the exact delivery observation in the corresponding FIXING:{spot_id} series. Before maturity an explicitly identified spot can supply the cost-of-carry input if the forward curve is absent.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string | null" => optional_id via spot_id_opt,
    /// Set the contract's discount curve id.
    /// @param value - Discount curve ID.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discount_curve_id as discountCurveId: "string" => id,
    /// Set the contract's exchange.
    /// @param value - Optional exchange identifier (e.g., "NYMEX", "ICE").
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    exchange as exchange: "string | null" => optional_id via exchange_opt,
    /// Set the contract's contract month.
    /// @param value - Optional contract month (e.g., "2025M03" for March 2025).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    contract_month as contractMonth: "string | null" => optional_id via contract_month_opt,
    /// Set the contract's convention.
    /// @param value - Optional market convention for this commodity. When set, provides default settlement days and calendar if not explicitly specified. See CommodityConvention for available options.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    convention as convention: "NonNullable<generated.valuations.CommodityForward['convention']> | null" => opt_wire via convention_opt,
    /// Set the contract's settlement days.
    /// @param value - Settlement lag in business days (T+N). Defaults to 2 for most commodity markets (T+2). If convention is set, uses the convention's default unless explicitly overridden here. # Market Standards | Market | Settlement | |--------|------------| | Energy (WTI, Brent, NG) | T+2 | | Precious metals | T+2 | | Base metals (LME) | T+2 | | Power | T+1 |
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_days as settlementDays: "number | null" => opt_uint via settlement_days_opt,
    /// Set the contract's settlement calendar id.
    /// @param value - The contract's settlement calendar ID. Required whenever the settlement (payment) date is calculated: no calendar is inferred from convention or defaulted to weekends only. nerc is a power-market label calendar, not a settlement calendar, and is rejected.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_calendar_id as settlementCalendarId: "string | null" => optional_id via settlement_calendar_id_opt,
    /// Set the contract's settlement business day convention.
    /// @param value - Business day convention for settlement date adjustment. Defaults to Following for energy commodities, ModifiedFollowing for precious metals.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    settlement_business_day_convention as settlementBusinessDayConvention: "NonNullable<generated.valuations.CommodityForward['settlement_business_day_convention']> | null" => opt_wire via settlement_business_day_convention_opt,
});
common_setters!(JsCommodityForwardBuilder, "CommodityForwardBuilder");
