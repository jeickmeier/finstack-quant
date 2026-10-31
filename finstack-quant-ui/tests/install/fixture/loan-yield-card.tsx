"use client";
import {
  LoanYieldCard,
  type LoanYieldRow,
} from "@/components/finstack/valuations/components/loan-yield-card/loan-yield-card";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The rows are the supplied native fixture: the item installs without a worker.
const { rows, metadata } = data.loanYield.cases.quotedTree;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone loan yield card.</p>
      <LoanYieldCard
        rows={rows as unknown as LoanYieldRow[]}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
