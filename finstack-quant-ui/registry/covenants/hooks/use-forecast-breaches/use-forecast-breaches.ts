"use client";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { ForecastBreachesRequest } from "@/workers/finstack-contract";
export type {
  ForecastBreachesRequest,
  FutureBreach,
} from "@/workers/finstack-contract";
/** Snapshot the engine, model, result and config JSON. Null disables the forecast. */
export function forecastBreachesOptions(
  client: FinstackClient | null,
  session: number,
  request: ForecastBreachesRequest | null,
) {
  const snapshot = request === null ? null : { ...request };
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "forecastBreaches", snapshot] as const,
    ready: snapshot !== null,
    missing: "Breach forecasting requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("forecastBreaches", snapshot!),
  });
}
/** Native statements_analytics.forecastBreaches FutureBreach rows or structured failure. */
export function useForecastBreaches(request: ForecastBreachesRequest | null) {
  return useWorkerQuery((worker) =>
    forecastBreachesOptions(worker.client, worker.session, request),
  );
}
