"use client";
import { requestSnapshot } from "@/hooks/shared/use-finstack/snapshot";
import {
  useWorkerQuery,
  workerQueryOptions,
} from "@/hooks/shared/use-finstack/query";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { BacktestForecastRequest } from "@/workers/finstack-contract";
export type { BacktestForecastRequest } from "@/workers/finstack-contract";
export type { ForecastMetrics } from "finstack-quant-wasm";
/** Snapshot both series. Null disables the backtest. A non-finite value stays distinct in the key and reaches Rust, which rejects it. */
export function backtestForecastOptions(
  client: FinstackClient | null,
  session: number,
  request: BacktestForecastRequest | null,
) {
  const { snapshot, key } = requestSnapshot(request);
  return workerQueryOptions({
    client,
    queryKey: ["finstack", session, "backtestForecast", key] as const,
    ready: snapshot !== null,
    missing:
      "The forecast backtest requires an explicit request and ready worker",
    queryFn: (ready) => ready.call("backtestForecast", snapshot!),
  });
}
/** Native statements_analytics.backtestForecast ForecastMetrics or structured failure. */
export function useBacktestForecast(request: BacktestForecastRequest | null) {
  return useWorkerQuery((worker) =>
    backtestForecastOptions(worker.client, worker.session, request),
  );
}
