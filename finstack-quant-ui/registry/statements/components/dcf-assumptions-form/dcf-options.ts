import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/dcf_options.json";
import metadata from "@/lib/finstack/generated/meta/dcf_options";
/** Canonical generated DcfOptions structure and presentation metadata. Rust publishes no example, so the form starts from the caller's options (empty: every Rust default). */
export const dcfOptionsModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: undefined,
};
