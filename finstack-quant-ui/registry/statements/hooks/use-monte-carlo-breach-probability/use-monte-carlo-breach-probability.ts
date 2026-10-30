"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
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
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: [
      "finstack",
      session,
      "monteCarloBreachProbability",
      key,
    ] as const,
    ready: snapshot !== null,
    missing:
      "The breach probability requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("monteCarloBreachProbability", snapshot!),
  });
}
/** Native statements.monteCarloBreachProbability: a fraction of paths, null when the results carry no path data for the metric, or a structured failure. */
export function useMonteCarloBreachProbability(
  request: MonteCarloBreachRequest | null,
) {
  return useWorkerQuery((worker) => ({
    ...monteCarloBreachProbabilityOptions(
      worker.client,
      worker.session,
      request,
    ),
    enabled: worker.status === "ready" && request !== null,
  }));
}
