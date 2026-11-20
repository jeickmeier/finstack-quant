//! A swap exchanging equity total return for contractual financing.

use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::equity::equity_trs::{
    EquityTotalReturnSwap, EquityTotalReturnSwapBuilder,
};
use wasm_bindgen::prelude::*;

instrument_class!(
    /// A swap exchanging equity total return for contractual financing.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const future = valuations.instruments.EquityTotalReturnSwap.example();
    /// console.log(future.id);
    /// future.free();
    /// ```
    JsEquityTotalReturnSwap, "EquityTotalReturnSwap", EquityTotalReturnSwap
);
instrument_surface!(
    JsEquityTotalReturnSwap,
    "EquityTotalReturnSwap",
    JsEquityTotalReturnSwapBuilder,
    "generated.valuations.EquityTotalReturnSwap"
);
factories!(JsEquityTotalReturnSwap, "EquityTotalReturnSwap", EquityTotalReturnSwap, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsEquityTotalReturnSwap, "EquityTotalReturnSwap", |i| {
    /// Effective expiry of the contract.
    /// @returns The ISO expiry, or null if the instrument defines none.
    expiry as expiry: "string | null" => opt_date(Instrument::expiry(i)),
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's notional.
    /// @returns Notional amount for the swap.
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's underlying.
    /// @returns Underlying equity parameters (spot ID, dividend yield, contract size).
    underlying as underlying: "generated.valuations.EquityTotalReturnSwap['underlying']" => json(i.underlying),
    /// Read the contract's financing leg.
    /// @returns Financing leg specification (curves, spread, day count).
    financing_leg as financingLeg: "generated.valuations.EquityTotalReturnSwap['financing_leg']" => json(i.financing_leg),
    /// Read the contract's schedule.
    /// @returns Schedule specification (payment dates and frequency).
    schedule as schedule: "generated.valuations.EquityTotalReturnSwap['schedule']" => json(i.schedule),
    /// Read the contract's side.
    /// @returns Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.
    side as side: "generated.valuations.EquityTotalReturnSwap['side']" => json(i.side),
    /// Read the contract's initial level.
    /// @returns Optional first-period start level. Pricing uses this only when the first period is in progress and no matching entry exists in past_fixings. Future periods use live spot.
    initial_level as initialLevel: "number | null" => json(i.initial_level),
    /// Read the contract's past fixings.
    /// @returns Observed underlying levels at contractual reset boundaries. Provide (reset_date, level) for each boundary on or before the valuation date that is needed by an unpaid return period. This includes the final period end when payment lag leaves its cashflow outstanding. The first boundary may use initial_level; a boundary equal to as_of may use live spot. Pricing errors when any other required fixing is unavailable.
    past_fixings as pastFixings: "generated.valuations.EquityTotalReturnSwap['past_fixings']" => dated_nums(i.past_fixings),
    /// Read the contract's margin spec.
    /// @returns Optional OTC margin specification for VM/IM. Equity TRS use SIMM equity bucket for margin calculation.
    margin_spec as marginSpec: "NonNullable<generated.valuations.EquityTotalReturnSwap['margin_spec']> | null" => json(i.margin_spec),
    /// Read the contract's dividend tax rate.
    /// @returns Dividend withholding tax rate for net return calculation. Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding). When set to 0.0 (default), the TRS passes through 100% of dividends (gross return). When set to a positive value, the dividend return component is reduced: text net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate) # Market Context Withholding tax varies by jurisdiction and investor domicile: - US qualified dividends: typically 0% for domestic investors - US non-qualified: up to 30% for foreign investors (varies by treaty) - European: varies by country (15-30% typical)
    dividend_tax_rate as dividendTaxRate: "number" => json(i.dividend_tax_rate),
    /// Read the contract's dividend settlement.
    /// @returns Settlement timing for explicit manufactured dividends. This field is mandatory because dividend-date and period-end settlement differ by the funding carry between the two dates.
    dividend_settlement as dividendSettlement: "generated.valuations.EquityTotalReturnSwap['dividend_settlement']" => json(i.dividend_settlement),
    /// Read the contract's discrete dividends.
    /// @returns Optional discrete cash dividends (ex_date, amount) for the underlying. When non-empty, pricing uses explicit period dividend pass-through and does not add continuous-yield dividend return to avoid double counting.
    discrete_dividends as discreteDividends: "generated.valuations.EquityTotalReturnSwap['discrete_dividends']" => dated_nums(i.discrete_dividends),
});
common_getters!(JsEquityTotalReturnSwap, "EquityTotalReturnSwap");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.EquityTotalReturnSwap.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsEquityTotalReturnSwapBuilder, "EquityTotalReturnSwapBuilder", EquityTotalReturnSwapBuilder, JsEquityTotalReturnSwap, |b| b.build()
);

#[wasm_bindgen(js_class = EquityTotalReturnSwap)]
impl JsEquityTotalReturnSwap {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsEquityTotalReturnSwapBuilder {
        JsEquityTotalReturnSwapBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsEquityTotalReturnSwapBuilder, "EquityTotalReturnSwapBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's underlying.
    /// @param value - Underlying equity parameters (spot ID, dividend yield, contract size).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    underlying as underlying: "generated.valuations.EquityTotalReturnSwap['underlying']" => js_wire,
    /// Set the contract's financing leg.
    /// @param value - Financing leg specification (curves, spread, day count).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    financing_leg as financingLeg: "generated.valuations.EquityTotalReturnSwap['financing_leg']" => js_wire,
    /// Set the contract's schedule.
    /// @param value - Schedule specification (payment dates and frequency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    schedule as schedule: "generated.valuations.EquityTotalReturnSwap['schedule']" => js_wire,
    /// Set the contract's side.
    /// @param value - Trade side: receive receives the total-return leg and pays financing; pay pays the total return and receives financing.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    side as side: "generated.valuations.EquityTotalReturnSwap['side']" => js_wire,
    /// Set the contract's initial level.
    /// @param value - Optional first-period start level. Pricing uses this only when the first period is in progress and no matching entry exists in past_fixings. Future periods use live spot.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    initial_level as initialLevel: "number | null" => js_wire via initial_level_opt,
    /// Set the contract's past fixings.
    /// @param value - Observed underlying levels at contractual reset boundaries. Provide (reset_date, level) for each boundary on or before the valuation date that is needed by an unpaid return period. This includes the final period end when payment lag leaves its cashflow outstanding. The first boundary may use initial_level; a boundary equal to as_of may use live spot. Pricing errors when any other required fixing is unavailable.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    past_fixings as pastFixings: "generated.valuations.EquityTotalReturnSwap['past_fixings']" => js_wire,
    /// Set the contract's margin spec.
    /// @param value - Optional OTC margin specification for VM/IM. Equity TRS use SIMM equity bucket for margin calculation.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    margin_spec as marginSpec: "NonNullable<generated.valuations.EquityTotalReturnSwap['margin_spec']> | null" => js_wire via margin_spec_opt,
    /// Set the contract's dividend tax rate.
    /// @param value - Dividend withholding tax rate for net return calculation. Specifies the fraction of dividends withheld for tax (e.g., 0.15 for 15% withholding). When set to 0.0 (default), the TRS passes through 100% of dividends (gross return). When set to a positive value, the dividend return component is reduced: text net_dividend_return = gross_dividend_return × (1 - dividend_tax_rate) # Market Context Withholding tax varies by jurisdiction and investor domicile: - US qualified dividends: typically 0% for domestic investors - US non-qualified: up to 30% for foreign investors (varies by treaty) - European: varies by country (15-30% typical)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    dividend_tax_rate as dividendTaxRate: "number" => js_wire,
    /// Set the contract's dividend settlement.
    /// @param value - Settlement timing for explicit manufactured dividends. This field is mandatory because dividend-date and period-end settlement differ by the funding carry between the two dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    dividend_settlement as dividendSettlement: "generated.valuations.EquityTotalReturnSwap['dividend_settlement']" => js_wire,
    /// Set the contract's discrete dividends.
    /// @param value - Optional discrete cash dividends (ex_date, amount) for the underlying. When non-empty, pricing uses explicit period dividend pass-through and does not add continuous-yield dividend return to avoid double counting.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    discrete_dividends as discreteDividends: "generated.valuations.EquityTotalReturnSwap['discrete_dividends']" => js_wire,
});
common_setters!(
    JsEquityTotalReturnSwapBuilder,
    "EquityTotalReturnSwapBuilder"
);

handle_setters!(JsEquityTotalReturnSwapBuilder, "EquityTotalReturnSwapBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount for the swap.
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
});
