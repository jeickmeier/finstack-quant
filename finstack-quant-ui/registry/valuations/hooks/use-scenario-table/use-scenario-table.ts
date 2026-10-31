"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ScenarioRequest } from "@/workers/finstack-contract";
export type { ScenarioRequest } from "@/workers/finstack-contract";
/** Snapshot all five facade inputs. Null disables computation. */
export function scenarioOptions(
  client: FinstackClient | null,
  session: number,
  request: ScenarioRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "scenarioTable", snapshot] as const,
    ready: snapshot !== null,
    missing:
      "Scenario calculation requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("scenarioTable", snapshot!),
  });
}
/** Exact native scenario table or structured failure. */
export function useScenarioTable(request: ScenarioRequest | null) {
  return useWorkerQuery((worker) =>
    scenarioOptions(worker.client, worker.session, request),
  );
}
/** Native units of a scenario table's cells for its currency. Null disables the call. */
export function useScenarioTableFieldUnits(currency: string | null) {
  return useWorkerQuery((worker) =>
    workerQueryOptions({
      client: worker.client,
      queryKey: [
        "finstack",
        worker.session,
        "scenarioTableFieldUnits",
        currency,
      ] as const,
      ready: currency !== null,
      missing: "Scenario table units require a currency and ready worker",
      queryFn: (ready) => ready.call("scenarioTableFieldUnits", currency!),
    }),
  );
}
