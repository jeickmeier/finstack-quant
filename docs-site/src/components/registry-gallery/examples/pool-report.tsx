"use client";
import type { PoolReportFieldUnits, valuations } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import { PoolReport } from "@/components/finstack/valuations/components/pool-report/pool-report";
import data from "../data.json";

// Captured native poolReport and poolReportFieldUnits for the CLO mezzanine
// regression golden's collateral; this item needs no worker.
const golden = data.poolReport.cases.find((entry) => entry.name === "golden")!;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <PoolReport
      report={
        golden.report as ReturnType<typeof valuations.instruments.poolReport>
      }
      units={golden.units as PoolReportFieldUnits}
      density={density}
    />
  );
}
