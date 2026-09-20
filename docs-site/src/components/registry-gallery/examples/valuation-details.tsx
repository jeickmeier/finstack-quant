"use client";
import type { ExampleProps } from "./props";
import { ValuationDetails } from "@/components/finstack/components/valuation-details/valuation-details";
import { valuationCodec, type ValuationResult } from "@/lib/finstack/host";
import data from "../data.json";
const mc = valuationCodec.parse(
  data.details.cases.at(-1)!.resultJson,
) as ValuationResult;
export function Example({ density = "compact" }: ExampleProps) {
  return <ValuationDetails result={mc} density={density} />;
}
