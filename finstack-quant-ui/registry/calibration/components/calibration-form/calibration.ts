import { createWireCodec } from "@/lib/finstack/codec.mjs";
import source from "@/lib/finstack/generated/schemas/calibration.json";
import shared from "@/lib/finstack/generated/defs/shared.json";
import localMetadata from "@/lib/finstack/generated/meta/calibration";
import sharedMetadata from "@/lib/finstack/generated/meta/shared";
import { linkSchema } from "@/lib/finstack/schema.mjs";

const schema = linkSchema(source, shared);
/** Generated calibration envelope, including canonical step and MarketQuote variants. */
export const calibrationModule = {
  schema,
  metadata: [
    ...localMetadata,
    ...sharedMetadata.filter((entry) => entry.path.startsWith("#/$defs/")),
  ],
  codec: createWireCodec(schema),
  example: source.examples[0],
};
