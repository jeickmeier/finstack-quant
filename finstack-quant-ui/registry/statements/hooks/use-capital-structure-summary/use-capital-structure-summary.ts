"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CapitalStructureSummaryRequest } from "@/workers/finstack-contract";
export type {
  CapitalStructureSummary,
  CapitalStructureSummaryRequest,
} from "@/workers/finstack-contract";
/** Snapshot the result, model and config JSON. Null disables the query. */
export function capitalStructureSummaryOptions(
  client: FinstackClient | null,
  session: number,
  request: CapitalStructureSummaryRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "capitalStructureSummary",
    request,
    "The capital structure summary requires an explicit request and ready worker",
  );
}
/** Native statements_analytics.capitalStructureSummary: the debt stack at one period, or the native error. */
export function useCapitalStructureSummary(
  request: CapitalStructureSummaryRequest | null,
) {
  return useWorkerQuery((worker) =>
    capitalStructureSummaryOptions(worker.client, worker.session, request),
  );
}
