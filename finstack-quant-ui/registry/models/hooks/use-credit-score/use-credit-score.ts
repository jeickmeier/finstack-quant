"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CreditScoreRequest } from "@/workers/finstack-contract";
export type {
  CreditScore,
  CreditScoreModel,
  CreditScoreRequest,
  ScoringResult,
} from "@/workers/finstack-contract";
/** Key the score on the complete snapshotted model and input ratios (a non-finite ratio stays distinct); nothing is defaulted or reordered. */
export function creditScoreOptions(
  client: FinstackClient | null,
  session: number,
  request: CreditScoreRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "creditScore", key] as const,
    ready: snapshot !== null,
    missing: "No scoring input supplied",
    queryFn: (ready) => ready.call("creditScore", snapshot!),
  });
}
/** A native models.credit.scoring result graded on the default PD master scale, or the actual native error; supply a Finstack/Query provider. */
export function useCreditScore(request: CreditScoreRequest | null) {
  return useWorkerQuery((worker) => ({
    ...creditScoreOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
