"use client";
import type { ExampleProps } from "./props";
import { ValuationSummary } from "@/components/finstack/valuations/components/valuation-summary/valuation-summary";
import { bondResult } from "../fixtures";

const result = bondResult;

export function Example({ density = "compact" }: ExampleProps) {
  return <ValuationSummary result={result} density={density} />;
}
