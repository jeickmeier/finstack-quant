import { parse, stringify } from "lossless-json";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/market_context_state.json";
import metadata from "@/lib/finstack/generated/meta/market_context_state";
/** Canonical generated market structure and presentation metadata, sharing the wire codec. */
export const marketModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
/** Parse MarketContextState JSON through the generated contract, preserving all JSON number tokens. */
export function parseMarket(text: string): Record<string, unknown> {
  return marketModule.codec.parse(text) as Record<string, unknown>;
}
/** Parse the `result.final_market` of a calibration result envelope, preserving all JSON number tokens. */
export function calibratedMarket(text: string): Record<string, unknown> {
  const value = parse(text) as { result?: { final_market?: unknown } } | null;
  const market = value?.result?.final_market;
  if (market === undefined)
    throw new TypeError(
      "Not a calibration result: result.final_market is missing",
    );
  return parseMarket(stringify(market)!);
}
