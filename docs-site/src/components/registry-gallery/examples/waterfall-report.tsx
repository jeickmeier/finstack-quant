"use client";
import type { StructuredCredit } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { WaterfallReport } from "@/components/finstack/valuations/components/waterfall-report/waterfall-report";
import data from "../data.json";

// Captured native runSimulation of the example CLO cut to one year with an OC test
// it fails, so cash diverts each period; this item needs no worker.
const diverting = data.waterfallReport.cases.find(
  (entry) => entry.name === "diverting",
)!;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WaterfallReport
      run={
        diverting.run as unknown as ReturnType<
          StructuredCredit["runSimulation"]
        >
      }
      density={density}
    />
  );
}
