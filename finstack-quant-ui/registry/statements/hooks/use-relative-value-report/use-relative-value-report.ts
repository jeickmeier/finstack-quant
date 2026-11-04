"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "relativeValueReport",
    request,
    "The relative value report requires an explicit request and ready worker",
  );
}
/** Native statements_analytics.relativeValueReport: one subject against its filtered peers, or the native error. */
export function useRelativeValueReport(
  request: RelativeValueReportRequest | null,
) {
  return useWorkerQuery((worker) =>
    relativeValueReportOptions(worker.client, worker.session, request),
  );
}
