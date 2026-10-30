"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { VmRequest } from "@/workers/finstack-contract";
export type { VmRequest } from "@/workers/finstack-contract";
/** Key the call on the complete snapshotted request. */
export function vmOptions(
  client: FinstackClient | null,
  session: number,
  request: VmRequest | null,
) {
  const snapshot = request === null ? null : structuredClone(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "vm", snapshot] as const,
    ready: snapshot !== null,
    missing: "No CSA supplied",
    queryFn: (ready) => ready.call("vm", snapshot!),
  });
}
/** Native variation margin of one exposure under a CSA, with Rust's call decision and net margin, or the actual native error; supply a Finstack/Query provider. */
export function useVm(request: VmRequest | null) {
  return useWorkerQuery((worker) => ({
    ...vmOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && request !== null,
  }));
}
