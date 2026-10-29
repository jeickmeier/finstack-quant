"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PortfolioMetricSeriesRequest } from "@/workers/finstack-contract";
export type { PortfolioMetricSeriesRequest } from "@/workers/finstack-contract";
/** Key the series on the snapshotted metrics (plain numbers, never bigint) and base metric id. */
export function portfolioMetricSeriesOptions(
  client: FinstackClient | null,
  session: number,
  request: PortfolioMetricSeriesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "portfolioMetricSeries", snapshot] as const,
    ready: snapshot !== null,
    missing: "No portfolio metrics supplied",
    queryFn: (ready) => ready.call("portfolioMetricSeries", snapshot!),
  });
}
/** Native decoded series of one base metric, in the order Rust returns it, or the actual native error; supply a Finstack/Query provider. */
export function usePortfolioMetricSeries(
  request: PortfolioMetricSeriesRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...portfolioMetricSeriesOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
