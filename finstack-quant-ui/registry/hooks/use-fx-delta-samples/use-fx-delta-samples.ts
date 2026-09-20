"use client";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
import type { FinstackClient } from "../use-finstack/client";
import type { FxDeltaSampleRequest } from "../../workers/finstack-contract";
export type { FxDeltaSampleRequest } from "../../workers/finstack-contract";
/** Snapshot every quote array and explicit coordinate/forward; null means no sampling request. */
export function fxDeltaOptions(
  client: FinstackClient | null,
  session: number,
  request: FxDeltaSampleRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  // Query's default JSON hash collapses NaN/Infinity to null; keep invalid native requests distinct.
  const key = JSON.stringify(snapshot, (_name, value) =>
    typeof value === "number" && !Number.isFinite(value)
      ? { nonFinite: String(value) }
      : value,
  );
  return queryOptions({
    queryKey: ["finstack", session, "sampleFxDelta", key] as const,
    queryFn: () => {
      if (!client || !snapshot)
        throw new Error(
          "FX sampling requires a ready worker and explicit coordinates/forwards",
        );
      return client.call("sampleFxDelta", snapshot);
    },
    enabled: client !== null && snapshot !== null,
    staleTime: Infinity,
    retry: false,
  });
}
/** Native annualized Black decimal volatilities in coordinate order; errors are unchanged. */
export function useFxDeltaSamples(request: FxDeltaSampleRequest | null) {
  const worker = useFinstack();
  const query = useQuery({
    ...fxDeltaOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
