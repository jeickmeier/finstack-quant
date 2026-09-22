"use client";
import type { ExampleProps } from "./props";
import { MonteCarloDiagnostics } from "@/components/finstack/models/components/monte-carlo-diagnostics/monte-carlo-diagnostics";
import type {
  ValuationResult,
  MonteCarloValuationDetails,
} from "finstack-quant-wasm";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import schema from "@/lib/finstack/generated/schemas/valuation_result.json";
import data from "../data.json";
const mc = createWireCodec(schema).parse(
  data.details.cases.at(-1)!.resultJson,
) as ValuationResult;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MonteCarloDiagnostics
      data={mc.details!.data as MonteCarloValuationDetails}
      currency={mc.value.currency}
      density={density}
    />
  );
}
