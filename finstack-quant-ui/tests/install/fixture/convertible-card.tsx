"use client";
import {
  ConvertibleCard,
  type ConvertibleCardProps,
} from "@/components/finstack/valuations/components/convertible-card/convertible-card";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import type { MetricMetadata } from "finstack-quant-wasm";

import data from "./data.json";
// The ladder and rows are the supplied native fixture: the item installs without a worker.
const { cases, metadata } = data.convertibleCard;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone convertible card.</p>
      <ConvertibleCard
        ladder={cases.plain.ladder as ConvertibleCardProps["ladder"]}
        rows={cases.plain.rows as unknown as MetricValueRow[]}
        metadata={metadata as unknown as MetricMetadata[]}
      />
    </>
  );
}
