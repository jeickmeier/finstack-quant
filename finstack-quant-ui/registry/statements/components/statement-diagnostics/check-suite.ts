import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/check_suite_spec.json";
import metadata from "@/lib/finstack/generated/meta/check_suite_spec";
/** Canonical Rust CheckSuiteSpec contract. */
export const checkSuiteModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example: schema.examples[0],
};
