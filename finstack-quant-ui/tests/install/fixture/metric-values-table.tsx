"use client";
import {
  MetricValuesTable,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The rows are the supplied native fixture: the item installs without a worker.
const { metadata, cases } = data.bondCredit;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone metric values table.</p>
      <MetricValuesTable
        label="Metric values"
        rows={cases.quoted.rows as unknown as MetricValueRow[]}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
