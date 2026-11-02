"use client";
import { LoanYieldCard } from "@/components/finstack/valuations/components/loan-yield-card/loan-yield-card";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The rows are the supplied native fixture: the item installs without a worker.
const { rows, metadata } = data.loanYield.cases.quotedTree;
// Rows Rust answered: a per-id error is an alert, and the harness fails on any alert.
const answered = (rows as unknown as MetricValueRow[]).filter(
  (row) => row.value.ok,
);
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone loan yield card.</p>
      <LoanYieldCard
        rows={answered}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
