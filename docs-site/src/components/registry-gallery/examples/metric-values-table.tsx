"use client";
import { sixPlaces, type ExampleProps } from "./props";
import { MetricValuesTable } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import { bondCredit } from "../fixtures";
const { metadata, cases } = bondCredit;
// A few supplied native rows, one with Rust's error; no worker is needed.
const shown = ["z_spread", "g_spread", "duration_mod", "dv01", "carry_total"];
const rows = cases.bullet.rows.filter((row) => shown.includes(row.metric));
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <MetricValuesTable
      label="Metric values"
      rows={rows}
      metadata={metadata}
      formatValue={sixPlaces}
      density={density}
      note="Values as Rust returned them."
    />
  );
}
