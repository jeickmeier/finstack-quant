"use client";
import type { ExampleProps } from "./props";
import type { TornadoEntry } from "finstack-quant-wasm";
import { TornadoChart } from "@/components/finstack/statements/components/tornado-chart/tornado-chart";
import data from "../data.json";

// Captured native statements_analytics.dcfSensitivity entries of the analyst
// model; this item needs no worker.
const entries = data.statementDcfSensitivity.entries as TornadoEntry[];
const currency =
  data.statementDcfSensitivity.baseline_enterprise_value.currency;

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <TornadoChart
      entries={entries}
      valueLabel={`Enterprise value change (${currency})`}
      caption="Enterprise value change by DCF assumption"
      density={density}
    />
  );
}
