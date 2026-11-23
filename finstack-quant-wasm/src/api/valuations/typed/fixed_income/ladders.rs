//! Bond breakeven ladders, workout ladders, and quote-set WASM entry points.

use super::bond::JsBond;

use crate::api::valuations::pricing::parse_market;
use crate::utils::wire::js_date;
use crate::utils::{to_js_err, to_js_value};
use wasm_bindgen::prelude::*;

/// Measure carry and signed basis-point breakeven across the supplied horizons.
/// @param bond - Typed bond whose native terms and price settings govern every row.
/// @param market - Market context supplying the required prices, curves and risk inputs.
/// @param asOf - ISO valuation date from which calendar horizons are measured.
/// @param horizons - Nonempty list of positive calendar tenors in the desired row order, such as ["1M", "1Y"].
/// @param config - Yield or spread target and solve mode. Iterative OAS is unsupported.
/// @returns Ordered rows with native currency amounts and signed breakeven shifts in basis points.
/// @throws TypeError for invalid input types or FinstackError for missing pricing or risk inputs, invalid horizons, or unsupported targets or modes.
#[wasm_bindgen(js_name = bondBreakevenLadder, unchecked_return_type = "generated.valuations.BondBreakevenLadder")]
pub fn bond_breakeven_ladder(
    bond: &JsBond,
    market: &crate::api::core::market_context::JsMarketContext,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(unchecked_param_type = "readonly string[]")] horizons: JsValue,
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.BreakevenConfig")] config: JsValue,
) -> Result<JsValue, JsValue> {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions;
    let as_of = js_date(&as_of, "asOf")?;
    let horizons = crate::utils::input::js_string_seq(&horizons, "horizons")?
        .iter()
        .map(|tenor| tenor.parse().map_err(to_js_err))
        .collect::<Result<Vec<_>, _>>()?;
    let config = crate::utils::input::from_js_json(&config, "config")?;
    let result = quote_conversions::bond_breakeven_ladder(
        &bond.inner,
        market.inner(),
        as_of,
        &horizons,
        config,
        finstack_quant_calibration::recalibration::pricing_options(),
    )
    .map_err(to_js_err)?;
    to_js_value(&result)
}

/// Export a breakeven ladder to ordered columns.
/// @param ladder - Native ladder returned by bondBreakevenLadder.
/// @returns Horizon, carry, signed sensitivity, signed basis-point breakeven and currency columns.
/// @throws FinstackError for invalid ladder data.
#[wasm_bindgen(js_name = bondBreakevenLadderTable, unchecked_return_type = "TableEnvelope")]
pub fn bond_breakeven_ladder_table(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.BondBreakevenLadder")]
    ladder: JsValue,
) -> Result<JsValue, JsValue> {
    use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::BondBreakevenLadder;
    let ladder: BondBreakevenLadder = crate::utils::input::from_js_json(&ladder, "ladder")?;
    to_js_value(&ladder.to_table())
}

/// Yield and I-spread to every call, put and maturity exit of a bond at one market price, with the worst marked (mirrors Rust `bond_workout_ladder`).
///
/// The exits are the ones the `ytw` metric walks, a return floor lowered into calls first, and the row marked worst holds that metric's yield and the `i_spread` metric's spread.
/// @param bond - Borrowed `Bond` handle; it is not consumed. A bond with exercise rights must carry `quoted_clean_price_pct`; one with none falls back to its model price.
/// @param marketJson - Canonical market-context JSON holding the bond's discount curve, the source of the swap par rates, and any floating index and make-whole reference curves.
/// @param asOf - ISO-8601 valuation date; the quote date and coupon entitlement follow the bond's settlement convention.
/// @returns `BondWorkoutLadder` plain object: `clean_price_pct` and `dirty_price`, the one price every exit is solved at, and `rows` in date order, each with `date`, `kind` (`call`, `put` or `maturity`), `redemption_price_pct` in percent of the principal redeemed, `redemption_amount`, `make_whole`, `yield_rate` and `i_spread` as decimals (the par rate runs to `date` when the bond has an exit other than maturity, and to contractual maturity when it has none) or `null` when the path has no finite yield (an exit on the quote date, or same-day cash at or above the price; a worst row can be `null`, where the `ytw` metric is negative infinity), and `is_worst`.
/// @throws Error - Throws with kind `not_found` if a bond with exercise rights has no quoted clean price or a curve is missing, kind `validation` if the market JSON or `asOf` is malformed, and kind `computation` if a yield cannot be solved.
#[wasm_bindgen(js_name = bondWorkoutLadder, unchecked_return_type = "generated.valuations.BondWorkoutLadder")]
pub fn bond_workout_ladder(
    bond: &JsBond,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "string")] market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
) -> Result<JsValue, JsValue> {
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    to_js_value(
        &finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::bond_workout_ladder(
            &bond.inner,
            &market,
            as_of,
        )
        .map_err(to_js_err)?,
    )
}

/// One row per exit of a bond workout ladder (mirrors Rust `BondWorkoutLadder::to_table`).
///
/// Python reaches the same table through `BondWorkoutLadder.to_dataframe`.
/// @param ladder - `BondWorkoutLadder` plain object or JSON string, as `bondWorkoutLadder` returns it.
/// @returns `TableEnvelope` in date order with columns `date` (ISO string), `kind` (`call`, `put` or `maturity`), `redemption_price_pct` (percent of the principal redeemed), `redemption_amount` (float), `currency` (ISO code of the amount), `make_whole` (boolean), `yield_rate` and `i_spread` (nullable decimals) and `is_worst` (boolean). The columns remain when there are no rows.
/// @throws Error - Throws with kind `validation` if `ladder` does not match the `BondWorkoutLadder` schema.
#[wasm_bindgen(js_name = bondWorkoutLadderTable, unchecked_return_type = "TableEnvelope")]
pub fn bond_workout_ladder_table(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.BondWorkoutLadder | string")]
    ladder: JsValue,
) -> Result<JsValue, JsValue> {
    let ladder: finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::BondWorkoutLadder =
        crate::utils::input::from_js_json(&ladder, "ladder")?;
    crate::utils::to_js_table_numeric(&ladder.to_table())
}

/// The quote set of a bond as a one-row table (mirrors Rust `BondQuoteSet::to_table`).
///
/// Python reaches the same table through `BondQuoteSet.to_dataframe`.
/// @param quotes - `BondQuoteSet` plain object or JSON string, as `computeQuotes` returns it.
/// @returns `TableEnvelope` with one row and one column per field in declaration order: float `clean_price_currency`, `clean_price_pct` and `dirty_price_currency`, then nullable float `ytm`, `ytw`, `z_spread`, `discount_margin`, `oas`, `asw_par`, `asw_market`, `i_spread`, `japanese_simple_yield` and `moosmuller_ytm`, null where the quote does not apply.
/// @throws Error - Throws with kind `validation` if `quotes` does not match the `BondQuoteSet` schema.
#[wasm_bindgen(js_name = bondQuoteSetTable, unchecked_return_type = "TableEnvelope")]
pub fn bond_quote_set_table(
    #[wasm_bindgen(unchecked_param_type = "generated.valuations.BondQuoteSet | string")]
    quotes: JsValue,
) -> Result<JsValue, JsValue> {
    let quotes: finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::BondQuoteSet =
        crate::utils::input::from_js_json(&quotes, "quotes")?;
    crate::utils::to_js_table_numeric(&quotes.to_table())
}

/// Every price, yield and spread quote of a bond, solved from any one of them (mirrors Rust `compute_quotes`).
///
/// The quote is normalized to a clean price and every other field is the metric of the same name at that price, in one call. The bond's default pricing model runs (tree when it has exercise rights) on the bond family's registries; any quote the bond carries is replaced.
/// @param bond - Borrowed `Bond` handle; it is not consumed. A bond with exercise rights needs HW1F parameters on the bond or in the market, because its `oas` is part of the set.
/// @param marketJson - Canonical market-context JSON holding the bond's discount curve, the source of the swap par rates, and any floating index curve.
/// @param asOf - ISO-8601 valuation date; the quote date and accrued interest follow the bond's settlement convention.
/// @param quoteInput - One `BondQuoteInput`, a plain object or JSON string with exactly one key: `clean_price_pct` (percent of par), `dirty_price_currency` (currency, the whole notional), `ytm`, `ytw`, `z_spread`, `discount_margin`, `oas`, `asw_market`, `i_spread` or `japanese_simple_yield` (decimals, `0.01` = 100bp).
/// @returns `BondQuoteSet` plain object: `clean_price_currency`, `clean_price_pct`, `dirty_price_currency`, and `ytm`, `ytw`, `z_spread`, `discount_margin`, `oas`, `asw_par`, `asw_market`, `i_spread`, `japanese_simple_yield` and `moosmuller_ytm` as decimals, each `null` when it does not apply to the bond or has no finite value.
/// @throws Error - Throws with kind `validation` if `quoteInput` has an unknown or second key or a non-finite value, a dirty price is not positive, the market JSON or `asOf` is malformed, the quote kind does not apply to the bond (`discount_margin` with no floating coupons, `asw_market` or `japanese_simple_yield` on a floating coupon, `z_spread` or `asw_market` with exercise rights, whose workout path depends on the price), or a metric of the set cannot be solved, which fails the whole set (`z_spread` with a call or put on the quote date below the price, `oas` with exercise rights and no HW1F parameters); and with kind `not_found` if a curve the bond needs to normalize the quote is missing.
#[wasm_bindgen(js_name = computeQuotes, unchecked_return_type = "generated.valuations.BondQuoteSet")]
pub fn compute_quotes(
    bond: &JsBond,
    #[wasm_bindgen(js_name = "marketJson", unchecked_param_type = "string")] market_json: JsValue,
    #[wasm_bindgen(js_name = "asOf", unchecked_param_type = "string")] as_of: JsValue,
    #[wasm_bindgen(
        js_name = "quoteInput",
        unchecked_param_type = "generated.valuations.BondQuoteInput | string"
    )]
    quote_input: JsValue,
) -> Result<JsValue, JsValue> {
    let market = parse_market(&market_json)?;
    let as_of = js_date(&as_of, "asOf")?;
    let quote_input = crate::utils::input::from_js_json(&quote_input, "quoteInput")?;
    to_js_value(
        &finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::compute_quotes(
            &bond.inner,
            &market,
            as_of,
            quote_input,
            finstack_quant_calibration::recalibration::pricing_options(),
        )
        .map_err(to_js_err)?,
    )
}
