"use client";
import {
  queryOptions,
  useQuery,
  type QueryKey,
  type UseQueryOptions,
} from "@tanstack/react-query";
import type { WorkerApi } from "@/workers/finstack-contract";
import { useFinstack } from "./use-finstack";
import type { FinstackClient } from "./client";
import { requestSnapshot } from "./snapshot";
/** Worker methods whose only argument is a request object. */
type SnapshotMethod = {
  [K in keyof WorkerApi]: Parameters<WorkerApi[K]> extends [object] ? K : never;
}[keyof WorkerApi];
export const workerQueryPolicy = {
  staleTime: Infinity,
  retry: false,
} as const;
/** Shared query identity, policy and missing-client throw; callers own the snapshot and call. */
export function workerQueryOptions<TData, TKey extends QueryKey>({
  client,
  queryKey,
  ready,
  missing,
  queryFn,
}: {
  client: FinstackClient | null;
  queryKey: TKey;
  ready: boolean;
  missing: string;
  queryFn: (client: FinstackClient) => Promise<TData>;
}) {
  return queryOptions({
    queryKey,
    queryFn: () => {
      if (!client || !ready) throw new Error(missing);
      return queryFn(client);
    },
    enabled: client !== null && ready,
    ...workerQueryPolicy,
  });
}
/**
 * Options for one worker method taking a single request object: the request is
 * snapshotted once and keyed on the session, method and snapshot key. Null
 * disables the call.
 */
export function snapshotQueryOptions<K extends SnapshotMethod>(
  client: FinstackClient | null,
  session: number,
  method: K,
  request: Parameters<WorkerApi[K]>[0] | null,
  missing: string,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, method, key] as const,
    ready: snapshot !== null,
    missing,
    queryFn: (ready) =>
      ready.call(method, ...([snapshot] as Parameters<WorkerApi[K]>)),
  });
}
export function useWorkerQuery<TData, TKey extends QueryKey>(
  options: (
    worker: ReturnType<typeof useFinstack>,
  ) => UseQueryOptions<TData, Error, TData, TKey>,
) {
  const worker = useFinstack();
  const supplied = options(worker);
  const enabled = supplied.enabled;
  const query = useQuery({
    ...supplied,
    enabled: (query) =>
      worker.status === "ready" &&
      worker.client !== null &&
      (typeof enabled === "function" ? enabled(query) : enabled !== false),
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
