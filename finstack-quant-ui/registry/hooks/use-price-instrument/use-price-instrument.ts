"use client";
import { useQuery, queryOptions } from "@tanstack/react-query";
import { useFinstack } from "../use-finstack/use-finstack";
import type { FinstackClient } from "../use-finstack/client";
import type {
  PriceRequest,
  CashflowRequest,
} from "../../workers/finstack-contract";
/** Snapshot every input without parsing JSON, filling financial defaults or collapsing selections. */
export function priceOptions(
  client: FinstackClient | null,
  session: number,
  request: PriceRequest,
) {
  const snapshot = Object.freeze({
    ...request,
    metrics:
      request.metrics == null
        ? request.metrics
        : Object.freeze([...request.metrics]),
  });
  return queryOptions({
    queryKey: ["finstack", session, "price", snapshot] as const,
    queryFn: () => {
      if (!client) throw new Error("Worker is not ready");
      return client.call("price", snapshot);
    },
    enabled: client !== null,
    staleTime: Infinity,
    retry: false,
  });
}
/** Immutable pricing query. Supply the entire request and a Finstack/Query provider boundary. */
export function usePriceInstrument(request: PriceRequest, enabled = true) {
  const worker = useFinstack();
  const query = useQuery({
    ...priceOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && enabled,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
/** Original native cashflow JSON, with all request fields in the key. */
export function useInstrumentCashflows(
  request: CashflowRequest,
  enabled = true,
) {
  const worker = useFinstack();
  const snapshot = Object.freeze({ ...request });
  const query = useQuery({
    queryKey: ["finstack", worker.session, "cashflows", snapshot],
    queryFn: () => worker.client!.call("cashflows", snapshot),
    enabled: worker.status === "ready" && enabled,
    staleTime: Infinity,
    retry: false,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
/** Registry options are immutable within one initialized worker session. */
function useOptions<T>(
  operation: "models" | "metrics" | "calendars",
  fetch: (client: FinstackClient) => Promise<T>,
) {
  const worker = useFinstack();
  const query = useQuery({
    queryKey: ["finstack", worker.session, operation],
    queryFn: () => fetch(worker.client!),
    enabled: worker.status === "ready",
    staleTime: Infinity,
    retry: false,
  });
  return {
    ...query,
    error: worker.error ?? query.error,
    workerStatus: worker.status,
  };
}
/** Models grouped by instrument type, directly from the native pricer registry. */
export function useModels() {
  return useOptions("models", (client) => client.call("models"));
}
/** Metrics grouped by native category, without local classifications. */
export function useMetrics() {
  return useOptions("metrics", (client) => client.call("metrics"));
}
/** Canonical calendar identifiers from the native core registry. */
export function useCalendars() {
  return useOptions("calendars", (client) => client.call("calendars"));
}
