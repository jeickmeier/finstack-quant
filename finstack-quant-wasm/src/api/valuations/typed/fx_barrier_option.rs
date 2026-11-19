//! An FX option whose activation depends on its contractual barrier observations.

use crate::api::core::currency::JsCurrency;
use crate::api::core::dates::JsDayCount;
use crate::api::core::money::JsMoney;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fx::fx_barrier_option::{
    FxBarrierOption, FxBarrierOptionBuilder,
};
use wasm_bindgen::prelude::*;

use super::fx::JsFxBarrierOption;

instrument_surface!(
    JsFxBarrierOption,
    "FxBarrierOption",
    JsFxBarrierOptionBuilder,
    "generated.valuations.FxBarrierOption",
    unpriced
);
factories!(JsFxBarrierOption, "FxBarrierOption", FxBarrierOption, {
    /// Construct the native example for this instrument.
    /// @returns Validated contract with its lifecycle and market dependencies.
    /// @throws FinstackError if the example terms are invalid.
    example as example,
});
getters!(JsFxBarrierOption, "FxBarrierOption", |i| {
    /// Registered pricing model for the contract.
    /// @returns The registered default pricing model name.
    default_model as defaultModel: "string" => text(Instrument::default_model(i)),
    /// Read the contract's strike.
    /// @returns Strike exchange rate (quote per base, dimensionless)
    strike as strike: "number" => json(i.strike),
    /// Read the contract's barrier.
    /// @returns Barrier level (exchange rate that triggers knock-in/out, dimensionless)
    barrier as barrier: "number" => json(i.barrier),
    /// Read the contract's rebate.
    /// @returns Total contractual trade rebate, paid in the quote (settlement) currency and independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.
    rebate as rebate: "NonNullable<generated.valuations.FxBarrierOption['rebate']> | null" => json(i.rebate),
    /// Read the contract's rebate timing.
    /// @returns Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.
    rebate_timing as rebateTiming: "NonNullable<generated.valuations.FxBarrierOption['rebate_timing']> | null" => json(i.rebate_timing),
    /// Read the contract's option type.
    /// @returns Option type (call or put on foreign currency)
    option_type as optionType: "generated.valuations.FxBarrierOption['option_type']" => json(i.option_type),
    /// Read the contract's barrier type.
    /// @returns Barrier type (up/down, in/out)
    barrier_type as barrierType: "generated.valuations.FxBarrierOption['barrier_type']" => json(i.barrier_type),
    /// Read the contract's expiry.
    /// @returns Option expiry date
    expiry as expiry: "string" => date(i.expiry),
    /// Read the contract's monitoring start date.
    /// @returns First date on which barrier monitoring is active. When set, a live valuation after this date requires observed_barrier_breached.
    monitoring_start_date as monitoringStartDate: "string | null" => opt_date(i.monitoring_start_date),
    /// Read the contract's observed barrier breached.
    /// @returns Processed barrier-monitoring state through the valuation date. Some(true) records a breach whose at-hit rebate has already settled, including a hit processed on the valuation date. That rebate is not a remaining claim at expiry. An at-expiry rebate remains due on expiry. Some(false) records no breach in the processed monitoring history. Historical monitoring must be supplied explicitly for seasoned and expired contracts. Same-day unpaid at-hit claims require a separate cash receivable; this boolean does not represent pending settlement.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => json(i.observed_barrier_breached),
    /// Read the contract's notional.
    /// @returns Notional amount in foreign currency
    notional as notional: "Money" => money(i.notional),
    /// Read the contract's base currency.
    /// @returns Base currency (the currency being priced, formerly foreign_currency)
    base_currency as baseCurrency: "Currency" => currency(i.base_currency),
    /// Read the contract's quote currency.
    /// @returns Quote currency (the pricing/settlement currency, formerly domestic_currency)
    quote_currency as quoteCurrency: "Currency" => currency(i.quote_currency),
    /// Read the contract's day count.
    /// @returns Day count convention (defaults to ACT/365F, consistent with FxOption)
    day_count as dayCount: "DayCount" => day_count(i.day_count),
    /// Read the contract's monitoring.
    /// @returns Contractual barrier-monitoring convention. Continuous monitoring uses the analytical Reiner-Rubinstein pricer by default. Discrete monitoring requires explicit observation dates and uses Monte Carlo without interpolating barrier hits between those dates.
    monitoring as monitoring: "generated.valuations.FxBarrierOption['monitoring']" => json(i.monitoring),
    /// Read the contract's domestic discount curve id.
    /// @returns Domestic discount curve ID
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => json(i.domestic_discount_curve_id),
    /// Read the contract's foreign discount curve id.
    /// @returns Foreign discount curve ID
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => json(i.foreign_discount_curve_id),
    /// Read the contract's fx spot id.
    /// @returns Optional FX spot scalar identifier. If omitted, pricing falls back to FxMatrix(base_currency, quote_currency).
    fx_spot_id as fxSpotId: "string | null" => json(i.fx_spot_id),
    /// Read the contract's vol surface id.
    /// @returns FX volatility surface ID
    vol_surface_id as volSurfaceId: "string" => json(i.vol_surface_id),
});
common_getters!(JsFxBarrierOption, "FxBarrierOption");

builder_class!(
    /// Builder for an instrument.
    ///
    /// @example
    /// ```typescript
    /// import init, { valuations } from "finstack-quant-wasm";
    /// await init();
    /// const initial = valuations.instruments.FxBarrierOption.example();
    /// const builder = initial.toBuilder();
    /// const updated = builder.id("CONTRACT-REVIEW").build();
    /// console.log(updated.id);
    /// updated.free();
    /// initial.free();
    /// ```
    JsFxBarrierOptionBuilder, "FxBarrierOptionBuilder", FxBarrierOptionBuilder, JsFxBarrierOption, |b| b.build()
);

#[wasm_bindgen(js_class = FxBarrierOption)]
impl JsFxBarrierOption {
    /// Seed an independent builder from the complete contract.
    /// @returns Builder carrying contractual terms, settlement state and market identifiers.
    #[wasm_bindgen(js_name = toBuilder)]
    pub fn to_builder(&self) -> JsFxBarrierOptionBuilder {
        JsFxBarrierOptionBuilder {
            inner: super::Staged::new(self.inner.to_builder()),
        }
    }
}

setters!(JsFxBarrierOptionBuilder, "FxBarrierOptionBuilder", {
    /// Set the stable contract identifier.
    /// @param value - Identifier used for scenario selection and metrics.
    /// @returns The active builder.
    /// @throws FinstackError for invalid inputs or an inactive builder.
    id as id: "string" => id,
    /// Set the contract's strike.
    /// @param value - Strike exchange rate (quote per base, dimensionless)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    strike as strike: "number" => js_wire,
    /// Set the contract's barrier.
    /// @param value - Barrier level (exchange rate that triggers knock-in/out, dimensionless)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier as barrier: "number" => js_wire,
    /// Set the contract's rebate.
    /// @param value - Total contractual trade rebate, paid in the quote (settlement) currency and independent of notional. Knock-outs pay on a hit according to rebate_timing; knock-ins pay at expiry only if no hit occurred.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    rebate as rebate: "NonNullable<generated.valuations.FxBarrierOption['rebate']> | null" => js_wire via rebate_opt,
    /// Set the contract's rebate timing.
    /// @param value - Payment timing of the rebate. Required exactly when rebate is set. For a knock-out, at_hit pays the rebate at the first qualifying hit and at_expiry defers it to expiry. A knock-in (no-hit) rebate is only determined at expiry, so it must be at_expiry; at_hit is rejected. The analytical pricer values at-hit knock-out rebates via the discounted first-passage closed form; Monte Carlo applies at-hit timing via with_rebate_at_hit.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    rebate_timing as rebateTiming: "NonNullable<generated.valuations.FxBarrierOption['rebate_timing']> | null" => js_wire via rebate_timing_opt,
    /// Set the contract's option type.
    /// @param value - Option type (call or put on foreign currency)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    option_type as optionType: "generated.valuations.FxBarrierOption['option_type']" => js_wire,
    /// Set the contract's barrier type.
    /// @param value - Barrier type (up/down, in/out)
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    barrier_type as barrierType: "generated.valuations.FxBarrierOption['barrier_type']" => js_wire,
    /// Set the contract's expiry.
    /// @param value - Option expiry date
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    expiry as expiry: "string" => js_date,
    /// Set the contract's monitoring start date.
    /// @param value - First date on which barrier monitoring is active. When set, a live valuation after this date requires observed_barrier_breached.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    monitoring_start_date as monitoringStartDate: "string | null" => js_wire via monitoring_start_date_opt,
    /// Set the contract's observed barrier breached.
    /// @param value - Processed barrier-monitoring state through the valuation date. Some(true) records a breach whose at-hit rebate has already settled, including a hit processed on the valuation date. That rebate is not a remaining claim at expiry. An at-expiry rebate remains due on expiry. Some(false) records no breach in the processed monitoring history. Historical monitoring must be supplied explicitly for seasoned and expired contracts. Same-day unpaid at-hit claims require a separate cash receivable; this boolean does not represent pending settlement.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    observed_barrier_breached as observedBarrierBreached: "boolean | null" => js_wire via observed_barrier_breached_opt,
    /// Set the contract's monitoring.
    /// @param value - Contractual barrier-monitoring convention. Continuous monitoring uses the analytical Reiner-Rubinstein pricer by default. Discrete monitoring requires explicit observation dates and uses Monte Carlo without interpolating barrier hits between those dates.
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    monitoring as monitoring: "generated.valuations.FxBarrierOption['monitoring']" => js_wire,
    /// Set the contract's domestic discount curve id.
    /// @param value - Domestic discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    domestic_discount_curve_id as domesticDiscountCurveId: "string" => id,
    /// Set the contract's foreign discount curve id.
    /// @param value - Foreign discount curve ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    foreign_discount_curve_id as foreignDiscountCurveId: "string" => id,
    /// Set the contract's fx spot id.
    /// @param value - Optional FX spot scalar identifier. If omitted, pricing falls back to FxMatrix(base_currency, quote_currency).
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    fx_spot_id as fxSpotId: "string | null" => optional_id via fx_spot_id_opt,
    /// Set the contract's vol surface id.
    /// @param value - FX volatility surface ID
    /// @returns The active builder.
    /// @throws FinstackError when the value cannot be converted or the builder has already been consumed.
    vol_surface_id as volSurfaceId: "string" => id,
});
common_setters!(JsFxBarrierOptionBuilder, "FxBarrierOptionBuilder");

handle_setters!(JsFxBarrierOptionBuilder, "FxBarrierOptionBuilder", {
    /// Set the contract's notional.
    /// @param value - Notional amount in foreign currency
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    notional as notional => JsMoney,
    /// Set the contract's base currency.
    /// @param value - Base currency (the currency being priced, formerly foreign_currency)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    base_currency as baseCurrency => JsCurrency,
    /// Set the contract's quote currency.
    /// @param value - Quote currency (the pricing/settlement currency, formerly domestic_currency)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    quote_currency as quoteCurrency => JsCurrency,
    /// Set the contract's day count.
    /// @param value - Day count convention (defaults to ACT/365F, consistent with FxOption)
    /// @returns The active builder.
    /// @throws FinstackError if the builder has already been consumed.
    day_count as dayCount => JsDayCount,
});
