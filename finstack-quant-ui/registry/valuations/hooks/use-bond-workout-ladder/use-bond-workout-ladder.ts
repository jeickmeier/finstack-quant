"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { BondWorkoutLadderRequest } from "@/workers/finstack-contract";
export type {
  BondWorkoutLadder,
  BondWorkoutLadderRequest,
} from "@/workers/finstack-contract";
/** Key the ladder on the complete snapshotted request; nothing is defaulted or reordered. */
export function bondWorkoutLadderOptions(
  client: FinstackClient | null,
  session: number,
  request: BondWorkoutLadderRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "bondWorkoutLadder", snapshot] as const,
    ready: snapshot !== null,
    missing: "No bond supplied",
    queryFn: (ready) => ready.call("bondWorkoutLadder", snapshot!),
  });
}
/** Native yield and I-spread to every exit of a bond, worst marked by Rust; supply a Finstack/Query provider. */
export function useBondWorkoutLadder(request: BondWorkoutLadderRequest | null) {
  return useWorkerQuery((worker) =>
    bondWorkoutLadderOptions(worker.client, worker.session, request),
  );
}
