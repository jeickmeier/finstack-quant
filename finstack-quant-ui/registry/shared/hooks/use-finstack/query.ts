"use client";
import {
  queryOptions,
  useQuery,
  type QueryKey,
  type UseQueryOptions,
} from "@tanstack/react-query";
import { useFinstack } from "./use-finstack";
import type { FinstackClient } from "./client";
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
export function useWorkerQuery<TData, TKey extends QueryKey>(
  options: (
    worker: ReturnType<typeof useFinstack>,
  ) => UseQueryOptions<TData, Error, TData, TKey>,
) {
  const worker = useFinstack();
  const query = useQuery(options(worker));
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
