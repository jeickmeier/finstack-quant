"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MarketScenarioApplicationRequest } from "@/workers/finstack-contract";
export type {
  MarketScenarioApplication,
  MarketScenarioApplicationRequest,
} from "@/workers/finstack-contract";
/** Key the application on the complete request; the JSON texts are never parsed here. */
export function applyMarketScenarioOptions(
  client: FinstackClient | null,
  session: number,
  request: MarketScenarioApplicationRequest | null,
) {
  const snapshot = request === null ? null : Object.freeze({ ...request });
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "applyMarketScenario", snapshot] as const,
    ready: snapshot !== null,
    missing: "No market scenario application supplied",
    queryFn: (ready) => ready.call("applyMarketScenario", snapshot!),
  });
}
/** Native scenarios.applyScenario report plus the shocked market, valuation date and instrument to price, or the actual native error; supply a Finstack/Query provider. */
export function useApplyMarketScenario(
  request: MarketScenarioApplicationRequest | null,
) {
  return useWorkerQuery((worker) =>
    applyMarketScenarioOptions(worker.client, worker.session, request),
  );
}
