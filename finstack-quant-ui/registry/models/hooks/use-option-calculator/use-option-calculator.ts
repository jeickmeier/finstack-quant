"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { OptionCalculatorRequest } from "@/workers/finstack-contract";
export type {
  OptionCalculatorRequest,
  OptionCalculatorResult,
} from "@/workers/finstack-contract";
/** Key the calculation on the complete snapshotted request; nothing is defaulted or reordered. */
export function optionCalculatorOptions(
  client: FinstackClient | null,
  session: number,
  request: OptionCalculatorRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "optionCalculator", snapshot] as const,
    ready: snapshot !== null,
    missing: "No option terms supplied",
    queryFn: (ready) => ready.call("optionCalculator", snapshot!),
  });
}
/** A native Black-Scholes, Black-76 or Bachelier price or implied volatility with its Greeks, or the actual native error; supply a Finstack/Query provider. */
export function useOptionCalculator(request: OptionCalculatorRequest | null) {
  return useWorkerQuery((worker) =>
    optionCalculatorOptions(worker.client, worker.session, request),
  );
}
