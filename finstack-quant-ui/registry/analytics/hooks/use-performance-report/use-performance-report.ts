"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PerformanceReportRequest } from "@/workers/finstack-contract";
export type {
  PerformanceReport,
  PerformanceReportRequest,
  PerformanceSeries,
  PerformanceTickerReport,
} from "@/workers/finstack-contract";
/** Key the report on the complete snapshotted panel and options; nothing is defaulted or reordered. */
export function performanceReportOptions(
  client: FinstackClient | null,
  session: number,
  request: PerformanceReportRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "performanceReport", snapshot] as const,
    ready: snapshot !== null,
    missing: "No performance panel supplied",
    queryFn: (ready) => ready.call("performanceReport", snapshot!),
  });
}
/** Native analytics.Performance results of a host-supplied panel, or the actual native error; supply a Finstack/Query provider. */
export function usePerformanceReport(request: PerformanceReportRequest | null) {
  return useWorkerQuery((worker) => ({
    ...performanceReportOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
