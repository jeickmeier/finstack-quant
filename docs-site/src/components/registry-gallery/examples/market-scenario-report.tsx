"use client";
import type { ApplicationReport } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { MarketScenarioReport } from "@/components/finstack/scenarios/components/market-scenario-report/market-scenario-report";
import data from "../data.json";
// The native report of a one-month roll plus a 25bp USD-OIS shift on the example bond.
const report = data.marketScenario.cases.find(
  (entry) => entry.name === "roll-1m-25bp",
)!.report as unknown as ApplicationReport;
export function Example({ density = "compact" }: ExampleProps) {
  return <MarketScenarioReport report={report} density={density} />;
}
