"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { EvaluateMonteCarloRequest } from "@/workers/finstack-contract";
export type { EvaluateMonteCarloRequest } from "@/workers/finstack-contract";
export type { MonteCarloResults } from "finstack-quant-wasm";
/** Snapshot the model and MonteCarloConfig JSON. Null disables the simulation. */
export function evaluateMonteCarloOptions(
  client: FinstackClient | null,
  session: number,
  request: EvaluateMonteCarloRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "evaluateMonteCarlo", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "The Monte Carlo evaluation requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("evaluateMonteCarlo", snapshot!),
  });
}
/** Native statements.Evaluator.evaluateMonteCarlo MonteCarloResults or structured failure. */
export function useEvaluateMonteCarlo(
  request: EvaluateMonteCarloRequest | null,
) {
  return useWorkerQuery((worker) =>
    evaluateMonteCarloOptions(worker.client, worker.session, request),
  );
}
