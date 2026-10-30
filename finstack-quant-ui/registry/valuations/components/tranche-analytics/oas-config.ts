import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/oas_config.json";
import metadata from "@/lib/finstack/generated/meta/oas_config";
/** Generated structured-credit OasConfig contract; its Rust example is `OasConfig::default()`. */
export const oasConfigModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
