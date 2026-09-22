"use client";
import { useQuery, queryOptions } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
import type { FinstackClient } from "../use-finstack/client";
import type { CashflowRequest } from "../../workers/finstack-contract";
export type { CashflowRequest } from "../../workers/finstack-contract";
/** Snapshot the complete export request, preserving original JSON and model selection. */
export function cashflowOptions(
  client: FinstackClient | null,
  session: number,
  request: CashflowRequest,
) {
  const snapshot = Object.freeze({ ...request });
  return queryOptions({
    queryKey: ["finstack", session, "cashflows", snapshot] as const,
    queryFn: () => {
      if (!client) throw new Error("Worker is not ready");
      return client.call("cashflows", snapshot);
    },
    enabled: client !== null,
    staleTime: Infinity,
    retry: false,
  });
}
/** Original native JSON text or actual native error; supply a Finstack/Query provider. */
export function useCashflows(request: CashflowRequest, enabled = true) {
  const worker = useFinstack();
  const query = useQuery({
    ...cashflowOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && enabled,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
