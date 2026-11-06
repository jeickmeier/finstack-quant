"use client";
import {
  useMetricMetadata,
  useMetrics,
  type PriceRequest,
} from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { useMetricValues } from "@/hooks/valuations/use-metric-values/use-metric-values";
import {
  applicableMetrics,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
/**
 * The props of a metric-id card over the completed request: the supplied ids Rust
 * lists for the type, each priced on its own call under the request's model.
 */
export function useInstrumentMetrics(
  instrumentType: string,
  ids: readonly string[],
  request: PriceRequest,
) {
  const available = useMetrics(instrumentType);
  const metrics = available.data
    ? applicableMetrics(ids, available.data)
    : null;
  const values = useMetricValues(metrics && { ...request, metrics });
  const metadata = useMetricMetadata(metrics ?? [], metrics !== null);
  return {
    rows: (values.data as MetricValueRow[] | undefined) ?? null,
    metadata: metadata.data,
    loading: available.isFetching || values.isFetching,
    error: available.error?.message ?? values.error?.message,
  };
}
