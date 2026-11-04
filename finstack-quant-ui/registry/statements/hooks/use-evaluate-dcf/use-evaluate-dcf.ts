"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { EvaluateDcfRequest } from "@/workers/finstack-contract";
export type {
  EvaluateDcfRequest,
  CorporateValuationResult,
} from "@/workers/finstack-contract";
/** Snapshot the model and every DCF assumption, market and valuation date. Null disables the DCF. A non-finite input stays distinct in the key and reaches Rust as entered. */
export function evaluateDcfOptions(
  client: FinstackClient | null,
  session: number,
  request: EvaluateDcfRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "evaluateDcf",
    request,
    "The DCF requires an explicit request and ready worker",
  );
}
/** Native statements_analytics.evaluateDcf CorporateValuationResult or structured failure. */
export function useEvaluateDcf(request: EvaluateDcfRequest | null) {
  return useWorkerQuery((worker) =>
    evaluateDcfOptions(worker.client, worker.session, request),
  );
}
