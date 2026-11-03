"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { WaccRequest } from "@/workers/finstack-contract";
export type { WaccRequest } from "@/workers/finstack-contract";
/** Snapshot the five inputs, with a NaN kept distinct from null in the key. Null disables the query. */
export function waccOptions(
  client: FinstackClient | null,
  session: number,
  request: WaccRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "wacc", key] as const,
    ready: snapshot !== null,
    missing: "WACC requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("wacc", snapshot!),
  });
}
/** Native WACC of the five inputs, a decimal, or the native rejection as the hook error. */
export function useWacc(request: WaccRequest | null) {
  return useWorkerQuery((worker) =>
    waccOptions(worker.client, worker.session, request),
  );
}
