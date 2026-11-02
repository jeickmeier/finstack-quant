"use client";
import { BondCreditCard } from "@/components/finstack/valuations/components/bond-credit-card/bond-credit-card";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The rows are the supplied native fixture: the item installs without a worker.
const { metadata, cases } = data.bondCredit;
// Rows Rust answered: a per-id error is an alert, and the harness fails on any alert.
const rows = (cases.quoted.rows as unknown as MetricValueRow[]).filter(
  (row) => row.value.ok,
);
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone bond credit card.</p>
      <BondCreditCard
        rows={rows}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
