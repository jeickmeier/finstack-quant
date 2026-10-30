"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { SimmImRequest } from "@/workers/finstack-contract";
export type { SimmImRequest } from "@/workers/finstack-contract";
/** Key the calculation on the complete snapshotted request; omitted options stay omitted. */
export function simmImOptions(
  client: FinstackClient | null,
  session: number,
  request: SimmImRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "simmIm", snapshot] as const,
    ready: snapshot !== null,
    missing: "No SIMM sensitivities supplied",
    queryFn: (ready) => ready.call("simmIm", snapshot!),
  });
}
/** Native indicative SIMM ImResult of a sensitivity set in its own base currency, or the actual native error; supply a Finstack/Query provider. */
export function useSimmIm(request: SimmImRequest | null) {
  return useWorkerQuery((worker) => ({
    ...simmImOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
