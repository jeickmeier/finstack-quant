"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PortfolioMetricsSeriesRequest } from "@/workers/finstack-contract";
export type { PortfolioMetricsSeriesRequest } from "@/workers/finstack-contract";
/** Key the series on the snapshotted metrics (plain numbers, never bigint) and base metric id. */
export function portfolioMetricsSeriesOptions(
  client: FinstackClient | null,
  session: number,
  request: PortfolioMetricsSeriesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "portfolioMetricsSeries",
      snapshot,
    ] as const,
    ready: snapshot !== null,
    missing: "No portfolio metrics supplied",
    queryFn: (ready) => ready.call("portfolioMetricsSeries", snapshot!),
  });
}
/** Native decoded series of one base metric, in the order Rust returns it, or the actual native error; supply a Finstack/Query provider. */
export function usePortfolioMetricsSeries(
  request: PortfolioMetricsSeriesRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...portfolioMetricsSeriesOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
