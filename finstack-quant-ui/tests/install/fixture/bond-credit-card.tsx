"use client";
import { BondCreditCard } from "@/components/finstack/valuations/components/bond-credit-card/bond-credit-card";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The rows are the supplied native fixture: the item installs without a worker.
const { metadata, cases } = data.bondCredit;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone bond credit card.</p>
      <BondCreditCard
        rows={cases.quoted.rows as unknown as MetricValueRow[]}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
