"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { BenchmarkReportRequest } from "@/workers/finstack-contract";
export type {
  BenchmarkReport,
  BenchmarkReportRequest,
  BenchmarkTickerReport,
} from "@/workers/finstack-contract";
/** Key the report on the complete snapshotted panel and options; nothing is defaulted or reordered. */
export function benchmarkReportOptions(
  client: FinstackClient | null,
  session: number,
  request: BenchmarkReportRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "benchmarkReport",
    request,
    "No benchmark panel supplied",
  );
}
/** Native analytics.Performance benchmark-relative results of a host-supplied panel, or the actual native error; supply a Finstack/Query provider. */
export function useBenchmarkReport(request: BenchmarkReportRequest | null) {
  return useWorkerQuery((worker) =>
    benchmarkReportOptions(worker.client, worker.session, request),
  );
}
