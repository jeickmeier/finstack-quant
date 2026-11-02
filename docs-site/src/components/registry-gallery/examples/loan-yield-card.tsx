"use client";
import type { MetricMetadata } from "finstack-quant-wasm";
import type { ExampleProps } from "./props";
import {
  LoanYieldCard,
  loanYieldMetrics,
} from "@/components/finstack/valuations/components/loan-yield-card/loan-yield-card";
import {
  applicableMetrics,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import {
  useMetricMetadata,
  useMetrics,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useMetricValues } from "@/hooks/valuations/use-metric-values/use-metric-values";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const { type, request } = data.loanYield.cases.quotedTree;
// The same loan under discounting: Rust rejects the call value and keeps the rest.
const discounting = data.loanYield.cases.quotedDiscounting;
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
function Card({ density }: ExampleProps) {
  const available = useMetrics(type);
  const metrics = available.data
    ? applicableMetrics(loanYieldMetrics, available.data)
    : null;
  const values = useMetricValues(metrics && { ...request, metrics });
  const metadata = useMetricMetadata(metrics ?? [], metrics !== null);
  return (
    <LoanYieldCard
      rows={(values.data as MetricValueRow[] | undefined) ?? null}
      metadata={metadata.data}
      loading={available.isFetching || values.isFetching}
      error={available.error?.message ?? values.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
// The hooks sit outside the item's install closure, so the example mounts its
// own provider; the install matrix runs the item with supplied rows.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <div className="space-y-6">
        <Card density={density} />
        <LoanYieldCard
          rows={discounting.rows as unknown as MetricValueRow[]}
          metadata={discounting.metadata as unknown as MetricMetadata[]}
          formatValue={sixPlaces}
          density={density}
        />
      </div>
    </WorkerPreview>
  );
}
