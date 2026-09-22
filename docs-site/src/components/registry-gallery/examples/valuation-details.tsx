"use client";
import type { ExampleProps } from "./props";
import { ValuationDetails } from "@/components/finstack/valuations/components/valuation-details/valuation-details";
import type { ValuationResult } from "finstack-quant-wasm";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/valuation_result.json";
import data from "../data.json";
const mc = createWireCodec(schema).parse(
  data.details.cases.at(-1)!.resultJson,
) as ValuationResult;
export function Example({ density = "compact" }: ExampleProps) {
  return <ValuationDetails result={mc} density={density} />;
}
