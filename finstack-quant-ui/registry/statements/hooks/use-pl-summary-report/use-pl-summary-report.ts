"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { PlSummaryReportRequest } from "@/workers/finstack-contract";
export type { PlSummaryReportRequest } from "@/workers/finstack-contract";
/** Snapshot the evaluated result, line items and periods. Null disables the report. */
export function plSummaryReportOptions(
  client: FinstackClient | null,
  session: number,
  request: PlSummaryReportRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "plSummaryReport",
    request,
    "P&L summary requires an explicit request and ready worker",
  );
}
/** Native P&L summary TableEnvelope (one row per line item and period) or structured failure. */
export function usePlSummaryReport(request: PlSummaryReportRequest | null) {
  return useWorkerQuery((worker) =>
    plSummaryReportOptions(worker.client, worker.session, request),
  );
}
