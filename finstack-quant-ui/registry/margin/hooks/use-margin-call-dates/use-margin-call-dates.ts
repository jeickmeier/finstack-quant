"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MarginCallDatesRequest } from "@/workers/finstack-contract";
export type { MarginCallDatesRequest } from "@/workers/finstack-contract";
/** Key the call on the complete snapshotted request. */
export function marginCallDatesOptions(
  client: FinstackClient | null,
  session: number,
  request: MarginCallDatesRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "marginCallDates", snapshot] as const,
    ready: snapshot !== null,
    missing: "No CSA supplied",
    queryFn: (ready) => ready.call("marginCallDates", snapshot!),
  });
}
/** Native contractual call dates of a CSA between two dates, or the actual native error; supply a Finstack/Query provider. */
export function useMarginCallDates(request: MarginCallDatesRequest | null) {
  return useWorkerQuery((worker) => ({
    ...marginCallDatesOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
