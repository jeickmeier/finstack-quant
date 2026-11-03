"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { SectorPeriod } from "@/workers/finstack-contract";
export type {
  CarinoLinkedAttribution,
  SectorPeriod,
} from "@/workers/finstack-contract";
/** Key the attribution on the snapshotted periods, in the order supplied; nothing is defaulted. */
export function brinsonLinkedOptions(
  client: FinstackClient | null,
  session: number,
  periods: readonly (readonly SectorPeriod[])[] | null,
) {
  const snapshot = periods === null ? null : structuredClone(periods);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "brinsonLinked", snapshot] as const,
    ready: snapshot !== null,
    missing: "No attribution periods supplied",
    queryFn: (ready) => ready.call("brinsonLinked", { periods: snapshot! }),
  });
}
/** Native portfolio.brinsonLinked of host-supplied periods of SectorPeriod rows, or its native error; null disables the call. Supply a Finstack/Query provider. */
export function useBrinsonLinked(
  periods: readonly (readonly SectorPeriod[])[] | null,
) {
  return useWorkerQuery((worker) =>
    brinsonLinkedOptions(worker.client, worker.session, periods),
  );
}
