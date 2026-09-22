"use client";
import { requestSnapshot } from "../use-finstack/snapshot";
import { useWorkerQuery, workerQueryOptions } from "../use-finstack/query";
import type { FinstackClient } from "../use-finstack/client";
import type { FxDeltaSampleRequest } from "../../workers/finstack-contract";
export type { FxDeltaSampleRequest } from "../../workers/finstack-contract";
/** Snapshot every quote array and explicit coordinate/forward; null means no sampling request. */
export function fxDeltaOptions(
  client: FinstackClient | null,
  session: number,
  request: FxDeltaSampleRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "sampleFxDelta", key] as const,
    ready: snapshot !== null,
    missing:
      "FX sampling requires a ready worker and explicit coordinates/forwards",
    queryFn: (ready) => ready.call("sampleFxDelta", snapshot!),
  });
}
/** Native annualized Black decimal volatilities in coordinate order; errors are unchanged. */
export function useFxDeltaSamples(request: FxDeltaSampleRequest | null) {
  return useWorkerQuery((worker) => ({
    ...fxDeltaOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
