/**
 * The `extras` argument of a direct native price call for a UI `PriceRequest`.
 *
 * The request carries its metric-pricing override patch and market history as
 * JSON text; the native call reads them as the `metric_pricing_overrides` and
 * `market_history` keys of one `PriceExtras` object. An omitted or `null`
 * field stays omitted.
 *
 * @param {{ metricPricingOverrides?: string | null, marketHistory?: string | null }} request
 * @returns {{ metric_pricing_overrides?: unknown, market_history?: unknown }}
 */
export function priceExtras(request) {
  return {
    metric_pricing_overrides: parsed(request.metricPricingOverrides),
    market_history: parsed(request.marketHistory),
  };
}

/** JSON text parsed, or `undefined` for an omitted or `null` field. */
const parsed = (text) => (text == null ? undefined : JSON.parse(text));
