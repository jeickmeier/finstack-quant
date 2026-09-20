"use client";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
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
    staleTime: Infinity,
    retry: false,
  });
}
/** Exact native scenario table or structured failure. */
export function useScenarioTable(request: ScenarioRequest | null) {
  const worker = useFinstack(),
    query = useQuery({
      ...scenarioOptions(worker.client, worker.session, request),
      enabled: worker.status === "ready" && request !== null,
    });
  return { ...query, error: worker.error ?? query.error };
}
