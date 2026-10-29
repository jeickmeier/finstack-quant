"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { HorizonReturnRequest } from "@/workers/finstack-contract";
export type {
  HorizonReturn,
  HorizonReturnRequest,
} from "@/workers/finstack-contract";
/** Key the horizon return on the complete snapshotted request; the JSON texts are never parsed and nothing is defaulted. */
export function horizonReturnOptions(
  client: FinstackClient | null,
  session: number,
  request: HorizonReturnRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "horizonReturn", snapshot] as const,
    ready: snapshot !== null,
    missing: "No horizon return request supplied",
    queryFn: (ready) => ready.call("horizonReturn", snapshot!),
  });
}
/** Native scenarios.computeHorizonReturn report and its explain text, or the actual native error; supply a Finstack/Query provider. */
export function useHorizonReturn(request: HorizonReturnRequest | null) {
  return useWorkerQuery((worker) => ({
    ...horizonReturnOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
