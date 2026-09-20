"use client";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
import type { FinstackClient } from "../use-finstack/client";
import type { CubeSampleRequest } from "../../workers/finstack-contract";
export type { CubeSampleRequest } from "../../workers/finstack-contract";
/** Snapshot every SABR node, forward, coordinate and output convention; null means no sampling request. */
export function cubeOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSampleRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  // Query's default JSON hash collapses NaN/Infinity to null; keep invalid native requests distinct.
  const key = JSON.stringify(snapshot, (_name, value) =>
    typeof value === "number" && !Number.isFinite(value)
      ? { nonFinite: String(value) }
      : value,
  );
  return queryOptions({
    queryKey: ["finstack", session, "sampleCube", key] as const,
    queryFn: () => {
      if (!client || !snapshot)
        throw new Error(
          "Cube sampling requires a ready worker and explicit coordinates/convention",
        );
      return client.call("sampleCube", snapshot);
    },
    enabled: client !== null && snapshot !== null,
    staleTime: Infinity,
    retry: false,
  });
}
/** Native volatilities in the selected convention and coordinate order; errors are unchanged. */
export function useCubeSamples(request: CubeSampleRequest | null) {
  const worker = useFinstack();
  const query = useQuery({
    ...cubeOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
