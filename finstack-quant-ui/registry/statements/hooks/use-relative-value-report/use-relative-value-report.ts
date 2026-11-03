"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { RelativeValueReportRequest } from "@/workers/finstack-contract";
export type {
  RelativeValueReport,
  RelativeValueReportRequest,
} from "@/workers/finstack-contract";
/** Snapshot the subject, universe and config JSON. Null disables the query. */
export function relativeValueReportOptions(
  client: FinstackClient | null,
  session: number,
  request: RelativeValueReportRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "relativeValueReport", key] as const,
    ready: snapshot !== null,
    missing:
      "The relative value report requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("relativeValueReport", snapshot!),
  });
}
/** Native statements_analytics.relativeValueReport: one subject against its filtered peers, or the native error. */
export function useRelativeValueReport(
  request: RelativeValueReportRequest | null,
) {
  return useWorkerQuery((worker) =>
    relativeValueReportOptions(worker.client, worker.session, request),
  );
}
