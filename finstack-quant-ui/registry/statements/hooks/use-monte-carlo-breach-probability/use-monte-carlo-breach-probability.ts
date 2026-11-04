"use client";
import {
  snapshotQueryOptions,
  useWorkerQuery,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MonteCarloBreachRequest } from "@/workers/finstack-contract";
export type { MonteCarloBreachRequest } from "@/workers/finstack-contract";
/** Snapshot the results, metric and threshold. Null disables the test. A non-finite threshold stays distinct in the key and reaches Rust as entered. */
export function monteCarloBreachProbabilityOptions(
  client: FinstackClient | null,
  session: number,
  request: MonteCarloBreachRequest | null,
) {
  return snapshotQueryOptions(
    client,
    session,
    "monteCarloBreachProbability",
    request,
    "The breach probability requires an explicit request and ready worker",
  );
}
/** Native statements.monteCarloBreachProbability: a fraction of paths, null when the results carry no path data for the metric, or a structured failure. */
export function useMonteCarloBreachProbability(
  request: MonteCarloBreachRequest | null,
) {
  return useWorkerQuery((worker) =>
    monteCarloBreachProbabilityOptions(worker.client, worker.session, request),
  );
}
