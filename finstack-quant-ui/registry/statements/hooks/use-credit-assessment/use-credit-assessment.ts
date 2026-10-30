"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CreditAssessmentRequest } from "@/workers/finstack-contract";
export type { CreditAssessmentRequest } from "@/workers/finstack-contract";
/** Snapshot the evaluated result and period. Null disables the assessment. */
export function creditAssessmentOptions(
  client: FinstackClient | null,
  session: number,
  request: CreditAssessmentRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "creditAssessment", snapshot] as const,
    ready: snapshot !== null,
    missing: "Credit assessment requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("creditAssessment", snapshot!),
  });
}
/** Native statements_analytics CreditAssessment or structured failure. */
export function useCreditAssessment(request: CreditAssessmentRequest | null) {
  return useWorkerQuery((worker) =>
    creditAssessmentOptions(worker.client, worker.session, request),
  );
}
