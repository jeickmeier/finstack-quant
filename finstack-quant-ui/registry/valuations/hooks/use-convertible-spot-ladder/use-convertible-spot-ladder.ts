"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ConvertibleSpotLadderRequest } from "@/workers/finstack-contract";
export type {
  ConvertibleSpotLadder,
  ConvertibleSpotLadderRequest,
} from "@/workers/finstack-contract";
/** Key the ladder on the complete snapshotted request; nothing is defaulted or reordered. */
export function convertibleSpotLadderOptions(
  client: FinstackClient | null,
  session: number,
  request: ConvertibleSpotLadderRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "convertibleSpotLadder", snapshot] as const,
    ready: snapshot !== null,
    missing: "No convertible supplied",
    queryFn: (ready) => ready.call("convertibleSpotLadder", snapshot!),
  });
}
/** Native price, delta, parity, premium and regime of a convertible by share price; supply a Finstack/Query provider. */
export function useConvertibleSpotLadder(
  request: ConvertibleSpotLadderRequest | null,
) {
  return useWorkerQuery((worker) =>
    convertibleSpotLadderOptions(worker.client, worker.session, request),
  );
}
