"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { EvaluateLboRequest } from "@/workers/finstack-contract";
export type { EvaluateLboRequest } from "@/workers/finstack-contract";
export type { LboResult } from "finstack-quant-wasm";
/** Snapshot the model and LboConfig JSON. Null disables the evaluation. */
export function evaluateLboOptions(
  client: FinstackClient | null,
  session: number,
  request: EvaluateLboRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "evaluateLbo", snapshot] as const,
    ready: snapshot !== null,
    missing: "The LBO requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("evaluateLbo", snapshot!),
  });
}
/** Native statements_analytics.evaluateLbo LboResult or structured failure. */
export function useEvaluateLbo(request: EvaluateLboRequest | null) {
  return useWorkerQuery((worker) =>
    evaluateLboOptions(worker.client, worker.session, request),
  );
}
