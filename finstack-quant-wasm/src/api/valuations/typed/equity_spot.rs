//! An equity holding with a quoted or market-observed share price.

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::spot::{Equity, EquityBuilder};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// An equity holding with a quoted or market-observed share price.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.Equity.example();
    /// console.log(future.currency);
    /// future.free();
    /// ```
    JsEquity, "Equity", Equity
);
instrument_surface!(
    JsEquity,
    "Equity",
    JsEquityBuilder,
    "generated.valuations.Equity"
);
factories!(JsEquity, "Equity", Equity, {
    /// Construct the native example for this equity holding.
    /// @returns Validated equity position with its market quote identifiers.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsEquity, "Equity", |i| {
    /// Effective expiry of the equity holding.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Market or holding input for ticker.
    /// @returns Ticker identifying the underlying issuer or index.
    ticker as ticker: "string" => text(i.ticker),
    /// Market or holding input for currency.
    /// @returns ISO currency of the quoted price per share.
    currency as currency: "string" => text(i.currency),
    /// Market or holding input for quantity.
    /// @returns Signed number of shares held. Omit for one share.
    quantity as quantity: "number | null" => json(i.quantity),
    /// Market or holding input for quoted spot.
    /// @returns Nonnegative price per share in the quote currency. This price takes precedence over the spot quote identifier.
    quoted_spot as quotedSpot: "number | null" => json(i.quoted_spot),
    /// Market or holding input for spot id.
    /// @returns Market quote identifier for the price per share.
    spot_id as spotId: "string | null" => opt_text(i.spot_id),
    /// Market or holding input for div yield id.
    /// @returns Market quote identifier for the continuously compounded decimal dividend yield.
    div_yield_id as divYieldId: "string | null" => opt_text(i.div_yield_id),
    /// Market or holding input for discrete dividends.
    /// @returns Cash dividend ex-date and amount per share in the quote currency.
    discrete_dividends as discreteDividends: "Array<[string, number]>" => dated_nums(i.discrete_dividends),
    /// Market or holding input for discount curve id.
    /// @returns Identifier of the curve used to discount equity cash flows in the quote currency.
    discount_curve_id as discountCurveId: "string" => text(i.discount_curve_id),
});
common_getters!(JsEquity, "Equity");

builder_class!(
    /// Builder for an equity holding.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.Equity.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("EQUITY-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsEquityBuilder, "EquityBuilder", EquityBuilder, JsEquity, |b| b.build()
);

#[wasm_bindgen(js_class = Equity)]
impl JsEquity {
    /// Seed an independent builder from the complete equity holding.
    /// @returns Builder carrying the share count, pricing inputs and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsEquityBuilder {
        JsEquityBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsEquityBuilder, "EquityBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the equity holding's ticker.
    /// @param value - Ticker identifying the underlying issuer or index.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    ticker as ticker: "string" => js_wire,
    /// Set the equity holding's currency.
    /// @param value - ISO currency of the quoted price per share.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    currency as currency: "string" => js_wire,
    /// Set the equity holding's quantity.
    /// @param value - Signed number of shares held. Omit for one share.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    quantity as quantity: "number | null" => js_wire via quantity_opt,
    /// Set the equity holding's quoted spot.
    /// @param value - Nonnegative price per share in the quote currency. This price takes precedence over the spot quote identifier.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    quoted_spot as quotedSpot: "number | null" => js_wire via quoted_spot_opt,
    /// Set the equity holding's spot id.
    /// @param value - Market quote identifier for the price per share.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    spot_id as spotId: "string | null" => js_wire via spot_id_opt,
    /// Set the equity holding's div yield id.
    /// @param value - Market quote identifier for the continuously compounded decimal dividend yield.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    div_yield_id as divYieldId: "string | null" => js_wire via div_yield_id_opt,
    /// Set the equity holding's discrete dividends.
    /// @param value - Cash dividend ex-date and amount per share in the quote currency.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    discrete_dividends as discreteDividends: "Array<[string, number]>" => js_wire,
    /// Set the equity holding's discount curve id.
    /// @param value - Identifier of the curve used to discount equity cash flows in the quote currency.
    /// @returns The active builder.
    /// @throws FinstackError if the input cannot be converted or the builder has been consumed.
    discount_curve_id as discountCurveId: "string" => js_wire,
});
common_setters!(JsEquityBuilder, "EquityBuilder");
