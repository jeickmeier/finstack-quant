"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { CubeSmileRequest } from "@/workers/finstack-contract";
export type { CubeSmileRequest, CubeSmile } from "@/workers/finstack-contract";
/** Snapshot every SABR node, forward, the node indices, strikes, rate and repair passes; null means no smile request. */
export function cubeSmileOptions(
  client: FinstackClient | null,
  session: number,
  request: CubeSmileRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "cubeSmile", key] as const,
    ready: snapshot !== null,
    missing:
      "A cube smile requires a ready worker, a node, strikes and an explicit rate",
    queryFn: (ready) => ready.call("cubeSmile", snapshot!),
  });
}
/** Native SABR smile, arbitrage check and optional repair of one cube node; errors are unchanged. */
export function useCubeSmile(request: CubeSmileRequest | null) {
  return useWorkerQuery((worker) =>
    cubeSmileOptions(worker.client, worker.session, request),
  );
}
