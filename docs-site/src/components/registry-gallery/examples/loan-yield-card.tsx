"use client";
import { sixPlaces, type ExampleProps } from "./props";
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
import { loanYield } from "../fixtures";
import { WorkerPreview } from "../worker-preview";
const { type, request } = loanYield.cases.quotedTree;
// The same loan under discounting: Rust rejects the call value and keeps the rest.
const discounting = loanYield.cases.quotedDiscounting;
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
          rows={discounting.rows}
          metadata={discounting.metadata}
          formatValue={sixPlaces}
          density={density}
        />
      </div>
    </WorkerPreview>
  );
}
