//! A listed futures contract referencing an equity total-return index.

use crate::api::core::dates::JsDayCount;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::equity_total_return_future::{
    EquityTotalReturnFuture, EquityTotalReturnFutureBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A listed futures contract referencing an equity total-return index.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.EquityTotalReturnFuture.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsEquityTotalReturnFuture, "EquityTotalReturnFuture", EquityTotalReturnFuture
);
instrument_surface!(
    JsEquityTotalReturnFuture,
    "EquityTotalReturnFuture",
    JsEquityTotalReturnFutureBuilder,
    "generated.valuations.EquityTotalReturnFuture"
);
factories!(JsEquityTotalReturnFuture, "EquityTotalReturnFuture", EquityTotalReturnFuture, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsEquityTotalReturnFuture, "EquityTotalReturnFuture", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's underlying ticker.
    /// @returns Equity or equity-index ticker.
    underlying_ticker as underlyingTicker: "string" => text(i.underlying_ticker),
    /// Read the contract's terms.
    /// @returns Standard listed position and lifecycle terms.
    terms as terms: "generated.valuations.EquityTotalReturnFuture['terms']" => json(i.terms),
    /// Read the contract's spot id.
    /// @returns Current underlying close or exchange-prescribed reference level.
    spot_id as spotId: "string" => text(i.spot_id),
    /// Read the contract's accrued distributions id.
    /// @returns Cumulative distribution points published by the exchange.
    accrued_distributions_id as accruedDistributionsId: "string" => text(i.accrued_distributions_id),
    /// Read the contract's accrued funding id.
    /// @returns Cumulative funding points published by the exchange.
    accrued_funding_id as accruedFundingId: "string" => text(i.accrued_funding_id),
    /// Read the contract's spread bp id.
    /// @returns Current annualized TRF spread scalar expressed in basis points.
    spread_bp_id as spreadBpId: "string" => text(i.spread_bp_id),
    /// Read the contract's spread day count.
    /// @returns Day-count basis used to convert the annualized spread into index points.
    spread_day_count as spreadDayCount: "DayCount" => day_count(i.spread_day_count),
});
common_getters!(JsEquityTotalReturnFuture, "EquityTotalReturnFuture");
to_builder_method!(
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    JsEquityTotalReturnFuture,
    "EquityTotalReturnFuture",
    JsEquityTotalReturnFutureBuilder
);

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.EquityTotalReturnFuture.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsEquityTotalReturnFutureBuilder, "EquityTotalReturnFutureBuilder", EquityTotalReturnFutureBuilder, JsEquityTotalReturnFuture, |b| b.build()
);

setters!(JsEquityTotalReturnFutureBuilder, "EquityTotalReturnFutureBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying ticker.
    /// @param value - Equity or equity-index ticker.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying_ticker as underlyingTicker: "string" => id,
    /// Set the contract's terms.
    /// @param value - Standard listed position and lifecycle terms.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    terms as terms: "generated.valuations.EquityTotalReturnFuture['terms']" => js_wire,
    /// Set the contract's spot id.
    /// @param value - Current underlying close or exchange-prescribed reference level.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spot_id as spotId: "string" => id,
    /// Set the contract's accrued distributions id.
    /// @param value - Cumulative distribution points published by the exchange.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    accrued_distributions_id as accruedDistributionsId: "string" => id,
    /// Set the contract's accrued funding id.
    /// @param value - Cumulative funding points published by the exchange.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    accrued_funding_id as accruedFundingId: "string" => id,
    /// Set the contract's spread bp id.
    /// @param value - Current annualized TRF spread scalar expressed in basis points.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    spread_bp_id as spreadBpId: "string" => id,
});
common_setters!(
    JsEquityTotalReturnFutureBuilder,
    "EquityTotalReturnFutureBuilder"
);

handle_setters!(JsEquityTotalReturnFutureBuilder, "EquityTotalReturnFutureBuilder", {
    /// Set the contract's spread day count.
    /// @param value - Day-count basis used to convert the annualized spread into index points.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    spread_day_count as spreadDayCount => JsDayCount,
});
