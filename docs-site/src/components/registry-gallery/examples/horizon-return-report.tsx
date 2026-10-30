"use client";
import type { HorizonReport } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { HorizonReturnReport } from "@/components/finstack/scenarios/components/horizon-return-report/horizon-return-report";
import data from "../data.json";
// The example bond over a three-month roll plus a 50bp USD-OIS shift.
const entry = data.horizonReturn.cases.find(
  (entry) => entry.name === "roll-3m-50bp",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <HorizonReturnReport
      report={entry.report as unknown as HorizonReport}
      explain={entry.explain}
      density={density}
    />
  );
}
