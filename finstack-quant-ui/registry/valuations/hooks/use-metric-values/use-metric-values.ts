"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MetricValuesRequest } from "@/workers/finstack-contract";
export type {
  MetricValue,
  MetricValuesRequest,
} from "@/workers/finstack-contract";
/** Key the values on the complete snapshotted request; nothing is defaulted or reordered. */
export function metricValuesOptions(
  client: FinstackClient | null,
  session: number,
  request: MetricValuesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "metricValues", snapshot] as const,
    ready: snapshot !== null,
    missing: "No metric ids supplied",
    queryFn: (ready) => ready.call("metricValues", snapshot!),
  });
}
/** Native value of each supplied metric id, each with its own native error, or the error of the whole request; supply a Finstack/Query provider. */
export function useMetricValues(request: MetricValuesRequest | null) {
  return useWorkerQuery((worker) =>
    metricValuesOptions(worker.client, worker.session, request),
  );
}
