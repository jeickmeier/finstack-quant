import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/dated_lbo_request.json";
import metadata from "@/lib/finstack/generated/meta/dated_lbo_request";
/** Canonical generated DatedLboRequest structure and presentation metadata. Rust publishes no example, so the form starts from the caller's request. */
export const lboRequestModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
