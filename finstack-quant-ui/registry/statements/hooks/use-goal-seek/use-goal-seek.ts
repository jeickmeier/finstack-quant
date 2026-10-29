"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { GoalSeekRequest } from "@/workers/finstack-contract";
export type { GoalSeekRequest } from "@/workers/finstack-contract";
export type { GoalSeekResult } from "finstack-quant-wasm";
/** Snapshot the model, target, driver, update flag and optional bounds. Null disables the solve. A non-finite input stays distinct in the key and reaches Rust as entered. */
export function goalSeekOptions(
  client: FinstackClient | null,
  session: number,
  request: GoalSeekRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "goalSeek", key] as const,
    ready: snapshot !== null,
    missing: "The goal seek requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("goalSeek", snapshot!),
  });
}
/** Native statements_analytics.goalSeek GoalSeekResult or structured failure; the caller sets the request on an explicit Solve. */
export function useGoalSeek(request: GoalSeekRequest | null) {
  return useWorkerQuery((worker) => ({
    ...goalSeekOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
