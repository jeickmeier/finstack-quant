"use client";
import type { PnlAttribution } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import {
  PnlAttributionReport,
  type PnlAttributionReportProps,
} from "@/components/finstack/attribution/components/pnl-attribution-report/pnl-attribution-report";
import data from "../data.json";
// A convertible's one-day credit widening under Rust's default waterfall
// order, with Rust's explain text, residual check and share of total.
const entry = data.pnlAttribution.cases.find(
  (entry) => entry.name === "waterfall",
)!;
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PnlAttributionReport
      attribution={entry.attribution as unknown as PnlAttribution}
      explain={entry.explain}
      residualWithinTolerance={entry.residualWithinTolerance}
      pctOfTotal={entry.pctOfTotal as PnlAttributionReportProps["pctOfTotal"]}
      density={density}
    />
  );
}
