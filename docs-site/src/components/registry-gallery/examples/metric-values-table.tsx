"use client";
import type { MetricMetadata } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import {
  MetricValuesTable,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import data from "../data.json";
const { metadata, cases } = data.bondCredit;
// A few supplied native rows, one with Rust's error; no worker is needed.
const shown = ["z_spread", "g_spread", "duration_mod", "dv01", "carry_total"];
const rows = cases.bullet.rows.filter((row) => shown.includes(row.metric));
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MetricValuesTable
      label="Metric values"
      rows={rows as unknown as MetricValueRow[]}
      metadata={metadata as unknown as MetricMetadata[]}
      formatValue={sixPlaces}
      density={density}
      note="Values as Rust returned them."
    />
  );
}
