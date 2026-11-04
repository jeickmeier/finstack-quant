"use client";
import { useState } from "react";
import { sixPlaces, type ExampleProps } from "./props";
import {
  BondCreditCard,
  bondCreditMetrics,
} from "@/components/finstack/valuations/components/bond-credit-card/bond-credit-card";
import {
  applicableMetrics,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import {
  useMetricMetadata,
  useMetrics,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useMetricValues } from "@/hooks/valuations/use-metric-values/use-metric-values";
import { bondCredit } from "../fixtures";
import { WorkerPreview } from "../worker-preview";
const { type, metadata: fixtureMetadata, cases } = bondCredit;
const { request } = cases.quoted;
// A bond with no quote, curve or config: those rows keep Rust's error.
const bullet = cases.bullet;
function Card({ density }: ExampleProps) {
  const [overrides, setOverrides] = useState(request.metricPricingOverrides);
  const available = useMetrics(type);
  const metrics = available.data
    ? applicableMetrics(bondCreditMetrics, available.data)
    : null;
  const values = useMetricValues(
    metrics && { ...request, metrics, metricPricingOverrides: overrides },
  );
  const metadata = useMetricMetadata(metrics ?? [], metrics !== null);
  return (
    <BondCreditCard
      rows={(values.data as MetricValueRow[] | undefined) ?? null}
      metadata={metadata.data}
      loading={available.isFetching || values.isFetching}
      error={available.error?.message ?? values.error?.message}
      formatValue={sixPlaces}
      density={density}
      overrides={overrides}
      onOverridesChange={setOverrides}
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
        <BondCreditCard
          rows={bullet.rows}
          metadata={fixtureMetadata}
          formatValue={sixPlaces}
          density={density}
        />
      </div>
    </WorkerPreview>
  );
}
