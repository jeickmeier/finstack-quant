import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/financial_model_spec.json";
import metadata from "@/lib/finstack/generated/meta/financial_model_spec";
import example from "@/lib/finstack/generated/examples/financial_model_spec.json";

/** Canonical Rust model contract and checked-in Rust fixture. */
export const financialModelModule = {
  schema,
  metadata,
  codec: createWireCodec(schema),
  example,
};
