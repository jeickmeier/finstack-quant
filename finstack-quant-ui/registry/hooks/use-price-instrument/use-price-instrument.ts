"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
  workerQueryPolicy,
} from "../use-finstack/query";
import type { FinstackClient } from "../use-finstack/client";
import type {
  MoneyFormatRequest,
  PriceRequest,
} from "../../workers/finstack-contract";
export type { PriceRequest } from "../../workers/finstack-contract";
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
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "price", snapshot] as const,
    ready: true,
    missing: "Worker is not ready",
    queryFn: (ready) => ready.call("price", snapshot),
  });
}
/** Immutable pricing query. Supply the entire request and a Finstack/Query provider boundary. */
export function usePriceInstrument(request: PriceRequest, enabled = true) {
  return useWorkerQuery((worker) => ({
    ...priceOptions(worker.client, worker.session, request),
    enabled: worker.status === "ready" && enabled,
  }));
}
/** Registry options are immutable within one initialized worker session. */
function useOptions<T>(
  operation: "models" | "metrics" | "calendars",
  fetch: (client: FinstackClient) => Promise<T>,
) {
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, operation],
    queryFn: () => fetch(worker.client!),
    enabled: worker.status === "ready",
    ...workerQueryPolicy,
  }));
}
/** Models grouped by instrument type, directly from the native pricer registry. */
export function useModels() {
  return useOptions("models", (client) => client.call("models"));
}
/** Metrics grouped by native category, without local classifications. */
export function useMetrics() {
  return useOptions("metrics", (client) => client.call("metrics"));
}
/** Native display text for an exact amount under its returned rounding stamp; the request is snapshotted without mutation. */
export function useMoneyFormat(request: MoneyFormatRequest | null) {
  const snapshot = request === null ? null : structuredClone(request);
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, "formatMoney", snapshot] as const,
    queryFn: () => worker.client!.call("formatMoney", snapshot!),
    enabled: worker.status === "ready" && snapshot !== null,
    ...workerQueryPolicy,
  }));
}
/** Native per-key interpretation for the supplied metric keys; keys are snapshotted without parsing. */
export function useMetricMetadata(keys: readonly string[], enabled = true) {
  const snapshot = Object.freeze([...keys]);
  return useWorkerQuery((worker) => ({
    queryKey: ["finstack", worker.session, "metricMetadata", snapshot] as const,
    queryFn: () => worker.client!.call("metricMetadata", snapshot),
    enabled: worker.status === "ready" && enabled,
    ...workerQueryPolicy,
  }));
}
/** Canonical calendar identifiers from the native core registry. */
export function useCalendars() {
  return useOptions("calendars", (client) => client.call("calendars"));
}
