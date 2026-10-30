"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MertonReportRequest } from "@/workers/finstack-contract";
export type {
  MertonReport,
  MertonReportRequest,
} from "@/workers/finstack-contract";
/** Key the report on the complete snapshotted source, horizons and CDS terms; nothing is defaulted or reordered. */
export function mertonReportOptions(
  client: FinstackClient | null,
  session: number,
  request: MertonReportRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "mertonReport", snapshot] as const,
    ready: snapshot !== null,
    missing: "No Merton model supplied",
    queryFn: (ready) => ready.call("mertonReport", snapshot!),
  });
}
/** Native models.credit.MertonModel results graded on the default PD master scale, or the actual native error; supply a Finstack/Query provider. */
export function useMertonReport(request: MertonReportRequest | null) {
  return useWorkerQuery((worker) =>
    mertonReportOptions(worker.client, worker.session, request),
  );
}
