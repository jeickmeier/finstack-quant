"use client";
import { useState } from "react";
import type { ExampleProps } from "./props";
import {
  ConvertibleCard,
  convertibleMetrics,
  ladderMetrics,
  type ConvertibleCardProps,
} from "@/components/finstack/valuations/components/convertible-card/convertible-card";
import {
  applicableMetrics,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import { useConvertibleSpotLadder } from "@/hooks/valuations/use-convertible-spot-ladder/use-convertible-spot-ladder";
import {
  useMetricMetadata,
  useMetrics,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useMetricValues } from "@/hooks/valuations/use-metric-values/use-metric-values";
import data from "../data.json";
import { WorkerPreview } from "../worker-preview";
const { type, cases } = data.convertibleCard;
const { quoted } = cases;
// Host display choice: six decimal places of the returned values.
const sixPlaces = (value: number) => value.toFixed(6);
function Card({ density }: ExampleProps) {
  const [spots, setSpots] = useState<readonly number[] | null>(null);
  const ladder = useConvertibleSpotLadder({ ...quoted.request, spots });
  const ladderMetadata = useMetricMetadata(ladderMetrics);
  const available = useMetrics(type);
  const metrics = available.data
    ? applicableMetrics(convertibleMetrics, available.data)
    : null;
  const values = useMetricValues(metrics && { ...quoted.metrics, metrics });
  const metadata = useMetricMetadata(metrics ?? [], metrics !== null);
  return (
    <ConvertibleCard
      ladder={(ladder.data as ConvertibleCardProps["ladder"]) ?? null}
      ladderMetadata={ladderMetadata.data}
      ladderLoading={ladder.isFetching}
      ladderError={ladder.error?.message}
      onSpotsChange={setSpots}
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
// own provider; the install matrix runs the item with supplied values.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <Card density={density} />
    </WorkerPreview>
  );
}
