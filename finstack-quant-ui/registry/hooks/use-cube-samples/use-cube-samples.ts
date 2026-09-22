"use client";
import { requestSnapshot } from "../use-finstack/snapshot";
import { useWorkerQuery, workerQueryOptions } from "../use-finstack/query";
import type { FinstackClient } from "../use-finstack/client";
import type { CubeSampleRequest } from "../../workers/finstack-contract";
export type { CubeSampleRequest } from "../../workers/finstack-contract";
/** Snapshot every SABR node, forward, coordinate and output convention; null means no sampling request. */
export function cubeOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSampleRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "sampleCube", key] as const,
    ready: snapshot !== null,
    missing:
      "Cube sampling requires a ready worker and explicit coordinates/convention",
    queryFn: (ready) => ready.call("sampleCube", snapshot!),
  });
}
/** Native volatilities in the selected convention and coordinate order; errors are unchanged. */
export function useCubeSamples(request: CubeSampleRequest | null) {
  return useWorkerQuery((worker) => ({
    ...cubeOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
