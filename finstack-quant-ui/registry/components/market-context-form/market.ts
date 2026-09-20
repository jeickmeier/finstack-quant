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
/** Extract only the published final_market field, preserving all JSON number tokens. */
export function importMarket(text: string): Record<string, unknown> {
  const value = parse(text) as { result?: { final_market?: unknown } } | null;
  const market = value?.result?.final_market;
  return marketModule.codec.parse(
    market === undefined ? text : stringify(market)!,
  ) as Record<string, unknown>;
}
