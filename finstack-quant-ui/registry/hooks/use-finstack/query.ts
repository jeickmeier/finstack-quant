"use client";
import {
  useQuery,
  type QueryKey,
  type UseQueryOptions,
} from "@tanstack/react-query";
import { useFinstack } from "./use-finstack";
export const workerQueryPolicy = {
  staleTime: Infinity,
  retry: false,
} as const;
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
