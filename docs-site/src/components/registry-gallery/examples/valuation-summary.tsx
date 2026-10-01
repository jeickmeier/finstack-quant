"use client";
import type { ExampleProps } from "./props";
import { ValuationSummary } from "@/components/finstack/valuations/components/valuation-summary/valuation-summary";
import type { ValuationResult } from "finstack-quant-wasm";
import data from "../data.json";

const result = data.bond.result as unknown as ValuationResult;

export function Example({ density = "compact" }: ExampleProps) {
  return <ValuationSummary result={result} density={density} />;
}
