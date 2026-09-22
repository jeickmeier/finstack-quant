"use client";
import { queryOptions } from "@tanstack/react-query";
import { useWorkerQuery, workerQueryPolicy } from "../use-finstack/query";
import type { FinstackClient } from "../use-finstack/client";
import type { ScenarioRequest } from "../../workers/finstack-contract";
export type { ScenarioRequest } from "../../workers/finstack-contract";
/** Snapshot all five facade inputs. Null disables computation. */
export function scenarioOptions(
  client: FinstackClient | null,
  session: number,
  request: ScenarioRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return queryOptions({
    queryKey: ["finstack", session, "scenarioTable", snapshot] as const,
    queryFn: () => {
      if (!client || !snapshot)
        throw new Error(
          "Scenario calculation requires an explicit request and ready worker",
        );
      return client.call("scenarioTable", snapshot);
    },
    enabled: client !== null && snapshot !== null,
    ...workerQueryPolicy,
  });
}
/** Exact native scenario table or structured failure. */
export function useScenarioTable(request: ScenarioRequest | null) {
  return useWorkerQuery((worker) => ({
    ...scenarioOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
