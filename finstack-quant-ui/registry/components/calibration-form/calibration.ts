import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/calibration.json";
import metadata from "@/lib/finstack/generated/meta/calibration";
/** Generated calibration envelope, including canonical step and MarketQuote variants. */
export const calibrationModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
