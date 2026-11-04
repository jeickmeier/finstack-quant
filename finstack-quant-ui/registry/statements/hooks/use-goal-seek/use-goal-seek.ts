"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
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
  return snapshotQueryOptions(
    client,
    session,
    "goalSeek",
    request,
    "The goal seek requires an explicit request and ready worker",
  );
}
/** Native statements_analytics.goalSeek GoalSeekResult or structured failure; the caller sets the request on an explicit Solve. */
export function useGoalSeek(request: GoalSeekRequest | null) {
  return useWorkerQuery((worker) =>
    goalSeekOptions(worker.client, worker.session, request),
  );
}
